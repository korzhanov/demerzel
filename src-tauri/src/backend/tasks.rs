//! Модель задачи и трейт источника (#16); файловый адаптер TODO.md / tasks.json (#17).

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

/// Источник задач: TODO.md, tasks.json, GitHub Issues (#17, #18).
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
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
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
}
