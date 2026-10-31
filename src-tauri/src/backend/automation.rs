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
    obj.insert("automation".into(), serde_json::json!({ "selected": selected }));
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
