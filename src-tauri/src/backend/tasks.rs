//! Модель задачи и трейт источника (#16); файловый адаптер TODO.md / tasks.json (#17).
//! GitHub Issues REST + ETag — `github_tasks::GithubSource` (#18).
//! Notion database REST — `notion_tasks::NotionSource` (#36).
//! Обновление статуса при старте агента (#33): `mark_task_in_progress`.

use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Todo,
    InProgress,
    Done,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Task {
    pub id: String,
    pub project: String,
    pub title: String,
    pub status: TaskStatus,
    pub source: String,
    pub url: Option<String>,
}

/// Источник задач: TODO.md, tasks.json, GitHub Issues, Notion (#17, #18, #36).
pub trait TaskSource {
    fn name(&self) -> &str;
    fn fetch(&self, project: &str) -> Result<Vec<Task>, String>;
}

/// Сводка по статусам для дашборда и виджета.
pub fn count_by_status(tasks: &[Task]) -> (usize, usize, usize, usize) {
    tasks.iter().fold((0, 0, 0, 0), |(t, p, d, b), task| match task.status {
        TaskStatus::Todo => (t + 1, p, d, b),
        TaskStatus::InProgress => (t, p + 1, d, b),
        TaskStatus::Done => (t, p, d + 1, b),
        TaskStatus::Blocked => (t, p, d, b + 1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(status: TaskStatus) -> Task {
        Task {
            id: "1".into(),
            project: "demo".into(),
            title: "t".into(),
            status,
            source: "test".into(),
            url: None,
        }
    }

    struct StubSource {
        tasks: Vec<Task>,
    }

    impl TaskSource for StubSource {
        fn name(&self) -> &str {
            "stub"
        }

        fn fetch(&self, project: &str) -> Result<Vec<Task>, String> {
            Ok(self
                .tasks
                .iter()
                .filter(|t| t.project == project)
                .cloned()
                .collect())
        }
    }

    #[test]
    fn counts_each_status() {
        let tasks = vec![
            task(TaskStatus::Todo),
            task(TaskStatus::InProgress),
            task(TaskStatus::Done),
            task(TaskStatus::Done),
            task(TaskStatus::Blocked),
        ];
        assert_eq!(count_by_status(&tasks), (1, 1, 2, 1));
    }

    #[test]
    fn model_serializes_wire_statuses() {
        assert_eq!(serde_json::to_string(&TaskStatus::Todo).unwrap(), "\"todo\"");
        assert_eq!(
            serde_json::to_string(&TaskStatus::InProgress).unwrap(),
            "\"in_progress\""
        );
        assert_eq!(serde_json::to_string(&TaskStatus::Done).unwrap(), "\"done\"");
        assert_eq!(
            serde_json::to_string(&TaskStatus::Blocked).unwrap(),
            "\"blocked\""
        );
        assert!(serde_json::from_str::<TaskStatus>("\"unknown\"").is_err());
    }

    #[test]
    fn task_roundtrips_optional_url() {
        let with_url = Task {
            id: "a/b#1".into(),
            project: "/proj".into(),
            title: "Fix".into(),
            status: TaskStatus::InProgress,
            source: "github".into(),
            url: Some("https://example.test/1".into()),
        };
        let json = serde_json::to_string(&with_url).unwrap();
        assert_eq!(serde_json::from_str::<Task>(&json).unwrap(), with_url);

        let bare = task(TaskStatus::Todo);
        let bare_json = serde_json::to_string(&bare).unwrap();
        assert!(bare_json.contains("\"url\":null") || !bare_json.contains("\"url\""));
        assert_eq!(serde_json::from_str::<Task>(&bare_json).unwrap().url, None);
    }

    #[test]
    fn task_source_trait_filters_by_project() {
        let source = StubSource {
            tasks: vec![
                Task {
                    id: "1".into(),
                    project: "alpha".into(),
                    title: "one".into(),
                    status: TaskStatus::Todo,
                    source: "stub".into(),
                    url: None,
                },
                Task {
                    id: "2".into(),
                    project: "beta".into(),
                    title: "two".into(),
                    status: TaskStatus::Done,
                    source: "stub".into(),
                    url: None,
                },
            ],
        };
        assert_eq!(source.name(), "stub");
        let alpha = source.fetch("alpha").unwrap();
        assert_eq!(alpha.len(), 1);
        assert_eq!(alpha[0].title, "one");
        assert!(source.fetch("missing").unwrap().is_empty());
    }
}

/// Файловый источник: `TODO.md` или `tasks.json` (#17). Id квалифицируются проектом.
pub struct FileSource {
    pub root: std::path::PathBuf,
    pub json: bool,
}

impl TaskSource for FileSource {
    fn name(&self) -> &str {
        if self.json {
            "tasks-json"
        } else {
            "todo"
        }
    }

    fn fetch(&self, project: &str) -> Result<Vec<Task>, String> {
        let path = self
            .root
            .join(if self.json { "tasks.json" } else { "TODO.md" });
        // Missing tasks file = no tasks for this project (not an error).
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        if self.json {
            let mut tasks: Vec<Task> = serde_json::from_str(&text)
                .map_err(|e| format!("{}: {e}", path.display()))?;
            let mut ids = std::collections::HashSet::new();
            for task in &mut tasks {
                if task.id.trim().is_empty()
                    || task.title.trim().is_empty()
                    || !ids.insert(task.id.clone())
                {
                    return Err(format!("{}: empty title/id or duplicate id", path.display()));
                }
                task.project = project.into();
                task.source = self.name().into();
                task.id = format!("{project}:json:{}", task.id);
                task.url = Some(path.to_string_lossy().into_owned());
            }
            Ok(tasks)
        } else {
            Ok(parse_markdown(&text, project, &path.to_string_lossy()))
        }
    }
}

/// Переводит задачу в `in_progress` при старте агента (#33).
///
/// - `todo` / `tasks-json` — пишет в файл источника.
/// - `github` / `notion` — remote не трогает (закрытие только вручную); возвращает копию со статусом.
pub fn mark_task_in_progress(task: &Task) -> Result<Task, String> {
    if task.status == TaskStatus::Done {
        return Err("cannot start agent for a done task".into());
    }
    let mut updated = task.clone();
    updated.status = TaskStatus::InProgress;

    match task.source.as_str() {
        "todo" => {
            let path = task
                .url
                .as_deref()
                .filter(|u| !u.is_empty())
                .ok_or("todo task has no file url")?;
            let line = parse_todo_line_number(&task.id)?;
            set_todo_checkbox(Path::new(path), line, '~')?;
        }
        "tasks-json" => {
            let path = task
                .url
                .as_deref()
                .filter(|u| !u.is_empty())
                .ok_or("tasks-json task has no file url")?;
            let raw_id = parse_json_task_id(&task.id)?;
            set_tasks_json_status(Path::new(path), &raw_id, TaskStatus::InProgress)?;
        }
        "github" | "notion" => {
            // Статус только для UI/ответа; remote page/issue не меняем.
        }
        other => {
            return Err(format!("cannot update status for task source '{other}'"));
        }
    }
    Ok(updated)
}

/// Номер строки из `{project}:todo:{line}` (1-based).
pub fn parse_todo_line_number(task_id: &str) -> Result<usize, String> {
    let (_, line) = task_id
        .rsplit_once(":todo:")
        .ok_or_else(|| format!("not a todo task id: {task_id}"))?;
    line.parse::<usize>()
        .map_err(|_| format!("invalid todo line in task id: {task_id}"))
        .and_then(|n| {
            if n == 0 {
                Err(format!("todo line must be 1-based: {task_id}"))
            } else {
                Ok(n)
            }
        })
}

/// Исходный id из `{project}:json:{id}`.
pub fn parse_json_task_id(task_id: &str) -> Result<String, String> {
    let (_, raw) = task_id
        .rsplit_once(":json:")
        .ok_or_else(|| format!("not a tasks-json task id: {task_id}"))?;
    if raw.trim().is_empty() {
        return Err(format!("empty json id in task id: {task_id}"));
    }
    Ok(raw.to_string())
}

fn replace_checkbox_mark(line: &str, mark: char) -> Option<String> {
    let trimmed_start = line.len() - line.trim_start().len();
    let rest = &line[trimmed_start..];
    let item = rest.strip_prefix("- [")?;
    let (_old_mark, after_bracket) = item.split_once(']')?;
    Some(format!(
        "{}- [{mark}]{after_bracket}",
        &line[..trimmed_start]
    ))
}

/// Меняет маркер чекбокса на указанной 1-based строке TODO.md.
pub fn set_todo_checkbox(path: &Path, line_1based: usize, mark: char) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let idx = line_1based
        .checked_sub(1)
        .ok_or_else(|| format!("invalid todo line {line_1based}"))?;
    let line = lines
        .get_mut(idx)
        .ok_or_else(|| format!("{}: line {line_1based} out of range", path.display()))?;
    let replaced = replace_checkbox_mark(line, mark)
        .ok_or_else(|| format!("{}: line {line_1based} is not a checkbox task", path.display()))?;
    *line = replaced;
    let mut out = lines.join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    fs::write(path, out).map_err(|e| format!("{}: {e}", path.display()))
}

/// Обновляет `status` записи с исходным `id` в tasks.json.
pub fn set_tasks_json_status(
    path: &Path,
    raw_id: &str,
    status: TaskStatus,
) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut rows: Vec<serde_json::Value> = serde_json::from_str(&text)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let status_json = serde_json::to_value(status).map_err(|e| e.to_string())?;
    let mut found = false;
    for row in &mut rows {
        if row.get("id").and_then(|v| v.as_str()) == Some(raw_id) {
            row.as_object_mut()
                .ok_or_else(|| format!("{}: task '{raw_id}' is not an object", path.display()))?
                .insert("status".into(), status_json.clone());
            found = true;
            break;
        }
    }
    if !found {
        return Err(format!("{}: task id '{raw_id}' not found", path.display()));
    }
    let out = serde_json::to_string_pretty(&rows).map_err(|e| e.to_string())?;
    fs::write(path, format!("{out}\n")).map_err(|e| format!("{}: {e}", path.display()))
}

/// Парсер чекбоксов Markdown: `- [ ]` / `- [~]` / `- [x]` / `- [!]`. См. `docs/tasks-format.md`.
pub fn parse_markdown(text: &str, project: &str, path: &str) -> Vec<Task> {
    let mut fenced = false;
    text.lines()
        .enumerate()
        .filter_map(|(line, raw)| {
            let raw = raw.trim_start();
            if raw.starts_with("```") || raw.starts_with("~~~") {
                fenced = !fenced;
                return None;
            }
            if fenced {
                return None;
            }
            let item = raw.strip_prefix("- [")?;
            let (mark, title) = item.split_once("] ")?;
            let status = match mark {
                " " => TaskStatus::Todo,
                "x" | "X" => TaskStatus::Done,
                "~" => TaskStatus::InProgress,
                "!" => TaskStatus::Blocked,
                _ => return None,
            };
            if title.trim().is_empty() {
                return None;
            }
            Some(Task {
                id: format!("{project}:todo:{}", line + 1),
                project: project.into(),
                title: title.trim().into(),
                status,
                source: "todo".into(),
                url: Some(path.into()),
            })
        })
        .collect()
}

#[cfg(test)]
mod file_tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    const FIXTURE_TODO: &str = include_str!("fixtures/tasks/TODO.md");
    const FIXTURE_JSON: &str = include_str!("fixtures/tasks/tasks.json");

    fn temp_root(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("demerzel-tasks-{label}-{nanos}"));
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn markdown_fixture_handles_statuses_unicode_and_code_blocks() {
        let tasks = parse_markdown(FIXTURE_TODO, "a", "/a/TODO.md");
        assert_eq!(count_by_status(&tasks), (1, 1, 1, 1));
        assert_eq!(tasks[0].title, "Привет");
        assert_eq!(tasks[0].id, "a:todo:3");
        assert_ne!(
            tasks[0].id,
            parse_markdown("- [ ] Привет", "b", "/b/TODO.md")[0].id
        );
    }

    #[test]
    fn file_source_reads_todo_fixture() {
        let root = temp_root("todo");
        fs::write(root.join("TODO.md"), FIXTURE_TODO).unwrap();
        let source = FileSource {
            root: root.clone(),
            json: false,
        };
        assert_eq!(source.name(), "todo");
        let tasks = source.fetch("/proj").unwrap();
        assert_eq!(count_by_status(&tasks), (1, 1, 1, 1));
        assert!(tasks.iter().all(|t| t.project == "/proj" && t.source == "todo"));
        assert!(tasks[0]
            .url
            .as_deref()
            .unwrap()
            .ends_with("TODO.md"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_source_reads_tasks_json_fixture() {
        let root = temp_root("json");
        fs::write(root.join("tasks.json"), FIXTURE_JSON).unwrap();
        let source = FileSource {
            root: root.clone(),
            json: true,
        };
        assert_eq!(source.name(), "tasks-json");
        let tasks = source.fetch("demo").unwrap();
        assert_eq!(count_by_status(&tasks), (1, 1, 1, 1));
        assert_eq!(tasks[0].id, "demo:json:setup");
        assert_eq!(tasks[1].title, "Парсер чекбоксов");
        assert!(tasks.iter().all(|t| t.source == "tasks-json"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn tasks_json_rejects_duplicate_or_empty_ids() {
        let root = temp_root("bad-json");
        fs::write(
            root.join("tasks.json"),
            r#"[{"id":"a","project":"","title":"one","status":"todo","source":"","url":null},{"id":"a","project":"","title":"two","status":"done","source":"","url":null}]"#,
        )
        .unwrap();
        let err = FileSource {
            root: root.clone(),
            json: true,
        }
        .fetch("p")
        .unwrap_err();
        assert!(err.contains("duplicate") || err.contains("empty"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_source_missing_todo_md_returns_empty() {
        let root = temp_root("missing-todo");
        let source = FileSource {
            root: root.clone(),
            json: false,
        };
        let tasks = source.fetch("/proj").expect("missing TODO.md is Ok([])");
        assert!(tasks.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_source_missing_tasks_json_returns_empty() {
        let root = temp_root("missing-json");
        let source = FileSource {
            root: root.clone(),
            json: true,
        };
        let tasks = source.fetch("demo").expect("missing tasks.json is Ok([])");
        assert!(tasks.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_source_io_errors_still_surface() {
        let root = temp_root("io-err");
        // Point at a path that exists as a directory named TODO.md so read_to_string fails
        // with an error other than NotFound.
        let todo_as_dir = root.join("TODO.md");
        fs::create_dir_all(&todo_as_dir).unwrap();
        let err = FileSource {
            root: root.clone(),
            json: false,
        }
        .fetch("/proj")
        .unwrap_err();
        assert!(err.contains("TODO.md"), "{err}");
        assert!(!err.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn mark_todo_task_in_progress_rewrites_checkbox() {
        let root = temp_root("mark-todo");
        let path = root.join("TODO.md");
        fs::write(&path, "# T\n\n- [ ] Fix me\n- [x] Done\n").unwrap();
        let task = Task {
            id: format!("{}:todo:3", root.display()),
            project: root.display().to_string(),
            title: "Fix me".into(),
            status: TaskStatus::Todo,
            source: "todo".into(),
            url: Some(path.display().to_string()),
        };
        let updated = mark_task_in_progress(&task).unwrap();
        assert_eq!(updated.status, TaskStatus::InProgress);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("- [~] Fix me"), "{text}");
        assert!(text.contains("- [x] Done"), "{text}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn mark_json_task_in_progress_updates_status() {
        let root = temp_root("mark-json");
        let path = root.join("tasks.json");
        fs::write(
            &path,
            r#"[{"id":"setup","project":"","title":"Setup","status":"todo","source":"","url":null}]
"#,
        )
        .unwrap();
        let task = Task {
            id: format!("{}:json:setup", root.display()),
            project: root.display().to_string(),
            title: "Setup".into(),
            status: TaskStatus::Todo,
            source: "tasks-json".into(),
            url: Some(path.display().to_string()),
        };
        let updated = mark_task_in_progress(&task).unwrap();
        assert_eq!(updated.status, TaskStatus::InProgress);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"in_progress\""), "{text}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn mark_github_task_in_progress_is_local_only() {
        let task = Task {
            id: "github:a/b#1".into(),
            project: "/proj".into(),
            title: "Bug".into(),
            status: TaskStatus::Todo,
            source: "github".into(),
            url: Some("https://github.com/a/b/issues/1".into()),
        };
        let updated = mark_task_in_progress(&task).unwrap();
        assert_eq!(updated.status, TaskStatus::InProgress);
        assert_eq!(updated.id, task.id);
    }

    #[test]
    fn mark_notion_task_in_progress_is_local_only() {
        let task = Task {
            id: "notion:0123456789abcdef0123456789abcdef:aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"
                .into(),
            project: "/proj".into(),
            title: "Notion bug".into(),
            status: TaskStatus::Todo,
            source: "notion".into(),
            url: Some("https://www.notion.so/aaa".into()),
        };
        let updated = mark_task_in_progress(&task).unwrap();
        assert_eq!(updated.status, TaskStatus::InProgress);
        assert_eq!(updated.id, task.id);
    }

    #[test]
    fn mark_done_task_rejected() {
        let task = Task {
            id: "github:a/b#2".into(),
            project: "/p".into(),
            title: "Closed".into(),
            status: TaskStatus::Done,
            source: "github".into(),
            url: None,
        };
        assert!(mark_task_in_progress(&task).is_err());
    }
}
