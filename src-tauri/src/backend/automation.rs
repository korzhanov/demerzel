//! Обнаружение автозадач проекта (#153, этап A — только чтение).
//!
//! Ничего не запускает. Смотрит: `package.json` → `scripts`, `pyproject.toml` →
//! `[project.scripts]` / `[tool.poetry.scripts]`, `Makefile` → цели, `auto-run.json`
//! (факт наличия и верхний уровень ключей). Имена, похожие на автозапуски, помечаются.

use crate::backend::config::AutomationSelection;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Найденная автозадача-кандидат.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredAutomation {
    /// Источник: `package.json`, `pyproject.toml`, `Makefile`, `auto-run.json`.
    pub source: String,
    /// Имя скрипта или цели.
    pub name: String,
    /// Команда, если известна (для скриптов и Makefile — текст команды).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Похоже на автозапуск (pipeline, sync, watch, cron, job, worker, parse…).
    pub likely_automation: bool,
}

/// Слова в имени, по которым задача считается похожей на автозапуск.
const AUTOMATION_HINTS: &[&str] = &[
    "pipeline", "parse", "sync", "watch", "cron", "job", "worker", "auto", "schedule", "daemon",
];

pub fn is_likely_automation(name: &str) -> bool {
    let n = name.to_lowercase();
    AUTOMATION_HINTS.iter().any(|h| n.contains(h))
}

/// Все найденные кандидаты в каталоге проекта. Отсутствующие файлы — не ошибка.
pub fn discover(project: &Path) -> Vec<DiscoveredAutomation> {
    let mut out = Vec::new();
    out.extend(from_package_json(project));
    out.extend(from_pyproject(project));
    out.extend(from_makefile(project));
    if project.join("auto-run.json").is_file() {
        out.push(DiscoveredAutomation {
            source: "auto-run.json".into(),
            name: "auto-run".into(),
            command: None,
            likely_automation: true,
        });
    }
    out
}

fn from_package_json(project: &Path) -> Vec<DiscoveredAutomation> {
    let Ok(text) = fs::read_to_string(project.join("package.json")) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    let Some(scripts) = value.get("scripts").and_then(|s| s.as_object()) else {
        return Vec::new();
    };
    let mut names: Vec<(&String, &serde_json::Value)> = scripts.iter().collect();
    names.sort_by(|a, b| a.0.cmp(b.0));
    names
        .into_iter()
        .map(|(name, cmd)| DiscoveredAutomation {
            source: "package.json".into(),
            name: name.clone(),
            command: cmd.as_str().map(str::to_string),
            likely_automation: is_likely_automation(name),
        })
        .collect()
}

fn from_pyproject(project: &Path) -> Vec<DiscoveredAutomation> {
    let Ok(text) = fs::read_to_string(project.join("pyproject.toml")) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut in_scripts = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            in_scripts = line == "[project.scripts]" || line == "[tool.poetry.scripts]";
            continue;
        }
        if !in_scripts || line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            let name = key.trim().trim_matches('"').to_string();
            let command = val.trim().trim_matches('"').to_string();
            out.push(DiscoveredAutomation {
                source: "pyproject.toml".into(),
                likely_automation: is_likely_automation(&name),
                name,
                command: Some(command),
            });
        }
    }
    out
}

fn from_makefile(project: &Path) -> Vec<DiscoveredAutomation> {
    let Ok(text) = fs::read_to_string(project.join("Makefile")) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in text.lines() {
        if line.starts_with('\t') || line.starts_with(' ') || line.starts_with('#') {
            continue;
        }
        if let Some((target, _)) = line.split_once(':') {
            let name = target.trim();
            let valid = !name.is_empty()
                && !name.starts_with('.')
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
            if valid && !out.iter().any(|d: &DiscoveredAutomation| d.name == name) {
                out.push(DiscoveredAutomation {
                    source: "Makefile".into(),
                    name: name.to_string(),
                    command: None,
                    likely_automation: is_likely_automation(name),
                });
            }
        }
    }
    out
}

/// Записывает выбор в `orchestrator.json` → `automation.selected`; остальные поля не трогает (#153).
/// Если файла нет — создаёт минимальный с именем папки.
pub fn save_selection(project: &Path, selected: &[AutomationSelection]) -> Result<(), String> {
    let file = project.join("orchestrator.json");
    let mut doc: serde_json::Value = if file.is_file() {
        let text = fs::read_to_string(&file).map_err(|e| e.to_string())?;
        serde_json::from_str(&text).map_err(|e| format!("orchestrator.json: {e}"))?
    } else {
        let name = project
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "project".into());
        serde_json::json!({ "name": name })
    };
    let obj = doc
        .as_object_mut()
        .ok_or_else(|| "orchestrator.json: корень должен быть объектом".to_string())?;
    // Сохраняем соседние поля секции automation (например, restart).
    let section = obj
        .entry("automation")
        .or_insert_with(|| serde_json::json!({}));
    let section = section
        .as_object_mut()
        .ok_or_else(|| "orchestrator.json: automation должен быть объектом".to_string())?;
    section.insert("selected".into(), serde_json::json!(selected));
    let text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    fs::write(&file, text + "\n").map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("demerzel-auto-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn package_json_scripts_are_listed_and_flagged() {
        let d = tmp("pkg");
        fs::write(
            d.join("package.json"),
            r#"{"scripts":{"dev":"vite dev","pipeline":"bun parser/tools/dashboard/server.ts","test:pipeline":"bun test"}}"#,
        )
        .unwrap();
        let found = discover(&d);
        let pipe = found.iter().find(|x| x.name == "pipeline").unwrap();
        assert!(pipe.likely_automation);
        assert_eq!(pipe.source, "package.json");
        assert!(pipe.command.as_deref().unwrap().contains("server.ts"));
        assert!(!found.iter().find(|x| x.name == "dev").unwrap().likely_automation);
    }

    #[test]
    fn pyproject_scripts_section_only() {
        let d = tmp("py");
        fs::write(
            d.join("pyproject.toml"),
            "[project]\nname = \"x\"\n\n[project.scripts]\nparse-words = \"app.cli:main\"\nhello = \"app:hi\"\n\n[tool.other]\nskip = \"1\"\n",
        )
        .unwrap();
        let found = discover(&d);
        let names: Vec<&str> = found.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, vec!["parse-words", "hello"]);
        assert!(found[0].likely_automation);
    }

    #[test]
    fn makefile_targets_skip_phony_and_recipes() {
        let d = tmp("mk");
        fs::write(d.join("Makefile"), ".PHONY: all\nall: build\n\techo hi\nsync-data:\n\tcp a b\n").unwrap();
        let found = discover(&d);
        let names: Vec<&str> = found.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, vec!["all", "sync-data"]);
        assert!(found.iter().find(|x| x.name == "sync-data").unwrap().likely_automation);
    }

    #[test]
    fn auto_run_json_is_detected_without_parsing() {
        let d = tmp("auto");
        fs::write(d.join("auto-run.json"), "{}").unwrap();
        let found = discover(&d);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].source, "auto-run.json");
    }

    #[test]
    fn missing_files_give_empty_result() {
        let d = tmp("empty");
        assert!(discover(&d).is_empty());
    }

    #[test]
    fn save_keeps_other_fields_and_writes_selection() {
        let d = tmp("save");
        fs::write(d.join("orchestrator.json"), r#"{"name":"parser","commands":{"dev":"x"}}"#).unwrap();
        save_selection(&d, &[AutomationSelection { source: "package.json".into(), name: "pipeline".into() }])
            .unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(d.join("orchestrator.json")).unwrap()).unwrap();
        assert_eq!(v["name"], "parser");
        assert_eq!(v["commands"]["dev"], "x");
        assert_eq!(v["automation"]["selected"][0]["name"], "pipeline");
    }

    #[test]
    fn save_creates_file_when_missing() {
        let d = tmp("create");
        save_selection(&d, &[]).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(d.join("orchestrator.json")).unwrap()).unwrap();
        assert!(v["automation"]["selected"].as_array().unwrap().is_empty());
    }
}

/// Состояние выбранной автозадачи по данным реестра процессов (#153, этап B).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AutomationStatus {
    pub source: String,
    pub name: String,
    pub command: Option<String>,
    /// Последний запуск этой команды в папке проекта; `None` — ещё не запускали.
    pub run_id: Option<u64>,
    pub status: Option<crate::backend::process::Status>,
}

/// Сопоставляет выбранные автозадачи с запусками. `runs` — (id, путь проекта, команда, статус).
/// Берётся самый свежий (наибольший id) запуск той же команды в той же папке.
pub fn match_statuses(
    project: &Path,
    selected: &[AutomationSelection],
    discovered: &[DiscoveredAutomation],
    runs: &[(u64, std::path::PathBuf, String, crate::backend::process::Status)],
) -> Vec<AutomationStatus> {
    selected
        .iter()
        .map(|sel| {
            let command = discovered
                .iter()
                .find(|d| d.source == sel.source && d.name == sel.name)
                .and_then(|d| d.command.clone());
            let latest = command.as_ref().and_then(|cmd| {
                runs.iter()
                    .filter(|(_, p, c, _)| p == project && c == cmd)
                    .max_by_key(|(id, ..)| *id)
            });
            AutomationStatus {
                source: sel.source.clone(),
                name: sel.name.clone(),
                command,
                run_id: latest.map(|r| r.0),
                status: latest.map(|r| r.3.clone()),
            }
        })
        .collect()
}

#[cfg(test)]
mod status_tests {
    use super::*;
    use crate::backend::process::Status;

    #[test]
    fn matches_latest_run_of_same_command_in_project() {
        let p = Path::new("/x/parser");
        let sel = vec![AutomationSelection { source: "package.json".into(), name: "pipeline".into() }];
        let disc = vec![DiscoveredAutomation {
            source: "package.json".into(),
            name: "pipeline".into(),
            command: Some("bun run pipeline".into()),
            likely_automation: true,
        }];
        let runs = vec![
            (1, p.to_path_buf(), "bun run pipeline".into(), Status::Exited(1)),
            (5, p.to_path_buf(), "bun run pipeline".into(), Status::Running),
            (6, Path::new("/x/other").to_path_buf(), "bun run pipeline".into(), Status::Running),
        ];
        let out = match_statuses(p, &sel, &disc, &runs);
        assert_eq!(out[0].run_id, Some(5));
        assert_eq!(out[0].status, Some(Status::Running));
    }

    #[test]
    fn unknown_command_has_no_run() {
        let sel = vec![AutomationSelection { source: "s".into(), name: "gone".into() }];
        let out = match_statuses(Path::new("/x"), &sel, &[], &[]);
        assert_eq!(out[0].run_id, None);
        assert_eq!(out[0].command, None);
    }
}

/// Лимит одновременно работающих автозадач на проект (#153, этап C). Значение по умолчанию.
pub const MAX_RUNNING_PER_PROJECT: usize = 2;

/// Проверка перед запуском автозадачи: сколько выбранных автозадач уже работают в папке проекта.
/// Возвращает `Err` с понятным текстом, если лимит достигнут.
pub fn check_start_allowed(
    project: &Path,
    statuses: &[AutomationStatus],
    limit: usize,
) -> Result<(), String> {
    let running = statuses
        .iter()
        .filter(|s| s.status.as_ref().is_some_and(|st| *st == crate::backend::process::Status::Running || *st == crate::backend::process::Status::Starting))
        .count();
    if running >= limit {
        return Err(format!(
            "в {} уже работают {running} автозадач(и) из лимита {limit}; остановите одну или поднимите лимит",
            project.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod limit_tests {
    use super::*;
    use crate::backend::process::Status;

    fn st(status: Option<Status>) -> AutomationStatus {
        AutomationStatus { source: "s".into(), name: "n".into(), command: Some("c".into()), run_id: Some(1), status }
    }

    #[test]
    fn allows_below_limit_and_blocks_at_limit() {
        let p = Path::new("/x");
        let two_running = vec![st(Some(Status::Running)), st(Some(Status::Exited(0)))];
        assert!(check_start_allowed(p, &two_running, 2).is_ok());
        let full = vec![st(Some(Status::Running)), st(Some(Status::Starting))];
        assert!(check_start_allowed(p, &full, 2).is_err());
    }
}

/// Записывает правило перезапуска в `automation.restart`, выбор и прочие поля не трогает.
pub fn save_restart_rule(project: &Path, rule: &crate::backend::config::RestartRule) -> Result<(), String> {
    let file = project.join("orchestrator.json");
    let mut doc: serde_json::Value = if file.is_file() {
        serde_json::from_str(&fs::read_to_string(&file).map_err(|e| e.to_string())?)
            .map_err(|e| format!("orchestrator.json: {e}"))?
    } else {
        serde_json::json!({ "name": project.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default() })
    };
    let obj = doc
        .as_object_mut()
        .ok_or_else(|| "orchestrator.json: корень должен быть объектом".to_string())?;
    let section = obj.entry("automation").or_insert_with(|| serde_json::json!({}));
    let section = section
        .as_object_mut()
        .ok_or_else(|| "orchestrator.json: automation должен быть объектом".to_string())?;
    section.insert("restart".into(), serde_json::to_value(rule).map_err(|e| e.to_string())?);
    let text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    fs::write(&file, text + "\n").map_err(|e| e.to_string())
}

/// Нужно ли перезапускать автозадачу после её статуса (#153, этап D).
/// Перезапуск — только при сбое (ненулевой выход или Crashed) и если попыток меньше лимита.
pub fn should_restart(
    status: &crate::backend::process::Status,
    attempts: u32,
    rule: &crate::backend::config::RestartRule,
) -> bool {
    use crate::backend::process::Status;
    if !rule.enabled || attempts >= rule.max_retries {
        return false;
    }
    match status {
        Status::Crashed => true,
        Status::Exited(code) => *code != 0,
        _ => false,
    }
}

#[cfg(test)]
mod restart_tests {
    use super::*;
    use crate::backend::config::RestartRule;
    use crate::backend::process::Status;

    fn on(max: u32) -> RestartRule {
        RestartRule { enabled: true, max_retries: max }
    }

    #[test]
    fn off_by_default() {
        assert!(!should_restart(&Status::Exited(1), 0, &RestartRule::default()));
    }

    #[test]
    fn restarts_only_on_failure_within_limit() {
        assert!(should_restart(&Status::Exited(1), 0, &on(3)));
        assert!(should_restart(&Status::Crashed, 2, &on(3)));
        assert!(!should_restart(&Status::Exited(0), 0, &on(3)));
        assert!(!should_restart(&Status::Running, 0, &on(3)));
        assert!(!should_restart(&Status::Exited(1), 3, &on(3)));
    }
}

/// Общий лимит работающих процессов на все проекты (#153, этап F). Решение по умолчанию.
pub const MAX_RUNNING_TOTAL: usize = 4;

/// Проверка общего лимита по всем запускам реестра (любые процессы, включая не автозадачи).
pub fn check_total_allowed(runs: &[(u64, std::path::PathBuf, String, crate::backend::process::Status)], limit: usize) -> Result<(), String> {
    let running = runs
        .iter()
        .filter(|(_, _, _, s)| *s == crate::backend::process::Status::Running || *s == crate::backend::process::Status::Starting)
        .count();
    if running >= limit {
        return Err(format!("общий лимит процессов {limit} достигнут ({running} работают); остановите что-нибудь или поднимите лимит"));
    }
    Ok(())
}

#[cfg(test)]
mod total_limit_tests {
    use super::*;
    use crate::backend::process::Status;

    #[test]
    fn total_cap_counts_only_running() {
        let p = std::path::PathBuf::from("/x");
        let mk = |s| (1u64, p.clone(), "c".to_string(), s);
        assert!(check_total_allowed(&[mk(Status::Running), mk(Status::Exited(0))], 2).is_ok());
        assert!(check_total_allowed(&[mk(Status::Running), mk(Status::Starting)], 2).is_err());
    }
}
