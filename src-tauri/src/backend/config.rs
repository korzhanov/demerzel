//! Конфиг проекта `orchestrator.json` (#5).
//!
//! Лежит в корне каждого проекта и подпроекта. Валидация выполняется при загрузке,
//! ошибка содержит путь к файлу и причину.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::ports::PortRange;

pub const CONFIG_FILE: &str = "orchestrator.json";
const PACKAGE_JSON: &str = "package.json";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectType {
    App,
    Parser,
    Dashboard,
    Service,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Commands {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dev: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migrate: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PortsConfig {
    pub range: [u16; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TasksConfig {
    pub source: String,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: ProjectType,
    #[serde(default)]
    pub commands: Commands,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ports: Option<PortsConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tasks: Option<TasksConfig>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ConfigError {
    Parse(String),
    EmptyName,
    InvalidPortRange { start: u16, end: u16 },
    DefaultOutsideRange { port: u16 },
    UnknownTaskSource(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Parse(e) => write!(f, "invalid JSON: {e}"),
            ConfigError::EmptyName => write!(f, "name must not be empty"),
            ConfigError::InvalidPortRange { start, end } => {
                write!(f, "port range {start}..{end} is inverted")
            }
            ConfigError::DefaultOutsideRange { port } => {
                write!(f, "default port {port} is outside the range")
            }
            ConfigError::UnknownTaskSource(s) => write!(f, "unknown task source '{s}'"),
        }
    }
}

const TASK_SOURCES: &[&str] = &["todo", "tasks-json", "github"];

/// Разбирает и валидирует конфиг. Путь к файлу добавляется вызывающим кодом.
pub fn parse(json: &str) -> Result<ProjectConfig, ConfigError> {
    let cfg: ProjectConfig =
        serde_json::from_str(json).map_err(|e| ConfigError::Parse(e.to_string()))?;
    validate(&cfg)?;
    Ok(cfg)
}

/// Читает `orchestrator.json` из папки проекта, если файл есть и валиден.
pub fn load_from_dir(dir: &Path) -> Option<ProjectConfig> {
    let path = dir.join(CONFIG_FILE);
    let raw = fs::read_to_string(path).ok()?;
    parse(&raw).ok()
}

/// Missing configuration is allowed; malformed or unreadable configuration is not.
pub fn load_checked(dir: &Path) -> Result<Option<ProjectConfig>, String> {
    let path = dir.join(CONFIG_FILE);
    match fs::read_to_string(&path) {
        Ok(text) => parse(&text)
            .map(Some)
            .map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Сообщение `dem sync`, когда `commands.migrate` нет или пусто (#43).
pub const MIGRATE_SKIPPED: &str = "no commands.migrate; skipped";

/// ORM-агностичная команда миграций из `orchestrator.json` (#43).
///
/// Пустая строка и пробелы считаются отсутствием поля — `dem sync` пропускает миграции.
pub fn resolve_migrate_command(commands: &Commands) -> Option<&str> {
    commands
        .migrate
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// Команда запуска для UI / менеджера процессов.
///
/// Порядок:
/// 1. `commands.dev` из `orchestrator.json`
/// 2. если в `package.json` есть `scripts.dev` — `bun|pnpm|npm run dev` по lockfile
///
/// Если ни того ни другого нет (как у корня 100words только с `pipeline`) — `None`:
/// UI должен спросить команду у пользователя.
pub fn resolve_dev_command(dir: &Path) -> Option<String> {
    if let Some(cfg) = load_from_dir(dir) {
        if let Some(dev) = cfg.commands.dev.filter(|s| !s.trim().is_empty()) {
            return Some(dev);
        }
    }
    package_json_dev_command(dir)
}

fn package_json_dev_command(dir: &Path) -> Option<String> {
    let raw = fs::read_to_string(dir.join(PACKAGE_JSON)).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let has_dev = value
        .get("scripts")
        .and_then(|s| s.get("dev"))
        .and_then(|d| d.as_str())
        .is_some_and(|s| !s.is_empty());
    if !has_dev {
        return None;
    }
    let runner = if dir.join("bun.lock").exists() || dir.join("bun.lockb").exists() {
        "bun"
    } else if dir.join("pnpm-lock.yaml").exists() {
        "pnpm"
    } else if dir.join("yarn.lock").exists() {
        "yarn"
    } else {
        "npm"
    };
    Some(format!("{runner} run dev"))
}

pub fn validate(cfg: &ProjectConfig) -> Result<(), ConfigError> {
    if cfg.name.trim().is_empty() {
        return Err(ConfigError::EmptyName);
    }
    if let Some(ports) = &cfg.ports {
        let range = PortRange::new(ports.range[0], ports.range[1]).ok_or(
            ConfigError::InvalidPortRange { start: ports.range[0], end: ports.range[1] },
        )?;
        if let Some(d) = ports.default {
            if !range.contains(d) {
                return Err(ConfigError::DefaultOutsideRange { port: d });
            }
        }
    }
    if let Some(tasks) = &cfg.tasks {
        if !TASK_SOURCES.contains(&tasks.source.as_str()) {
            return Err(ConfigError::UnknownTaskSource(tasks.source.clone()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"{
        "name": "api",
        "type": "service",
        "commands": { "dev": "npm run dev", "migrate": "npx prisma migrate deploy" },
        "ports": { "range": [5100, 5199], "default": 5100 },
        "tasks": { "source": "github" },
        "env": { "LOG_LEVEL": "debug" }
    }"#;

    #[test]
    fn parses_valid_config() {
        let cfg = parse(VALID).unwrap();
        assert_eq!(cfg.name, "api");
        assert_eq!(cfg.kind, ProjectType::Service);
        assert_eq!(cfg.commands.migrate.as_deref(), Some("npx prisma migrate deploy"));
    }

    #[test]
    fn minimal_config_is_enough() {
        let cfg = parse(r#"{ "name": "parser", "type": "parser" }"#).unwrap();
        assert!(cfg.ports.is_none());
        assert!(cfg.commands.dev.is_none());
        assert!(resolve_migrate_command(&cfg.commands).is_none());
    }

    #[test]
    fn resolve_migrate_reads_nonempty_command() {
        let cfg = parse(VALID).unwrap();
        assert_eq!(
            resolve_migrate_command(&cfg.commands),
            Some("npx prisma migrate deploy")
        );
    }

    #[test]
    fn resolve_migrate_skips_blank_and_missing() {
        let blank = parse(
            r#"{ "name": "api", "type": "service", "commands": { "migrate": "  " } }"#,
        )
        .unwrap();
        assert!(resolve_migrate_command(&blank.commands).is_none());
        assert_eq!(MIGRATE_SKIPPED, "no commands.migrate; skipped");

        let missing = parse(r#"{ "name": "api", "type": "service" }"#).unwrap();
        assert!(resolve_migrate_command(&missing.commands).is_none());
    }

    #[test]
    fn rejects_inverted_range() {
        let json = r#"{ "name": "a", "type": "app", "ports": { "range": [5200, 5100] } }"#;
        assert!(matches!(parse(json), Err(ConfigError::InvalidPortRange { .. })));
    }

    #[test]
    fn rejects_default_outside_range() {
        let json = r#"{ "name": "a", "type": "app", "ports": { "range": [5100, 5110], "default": 5200 } }"#;
        assert!(matches!(parse(json), Err(ConfigError::DefaultOutsideRange { port: 5200 })));
    }

    #[test]
    fn rejects_unknown_task_source() {
        let json = r#"{ "name": "a", "type": "app", "tasks": { "source": "jira" } }"#;
        assert!(matches!(parse(json), Err(ConfigError::UnknownTaskSource(_))));
    }

    #[test]
    fn parses_github_task_source_with_repository_and_label() {
        let json = r#"{
            "name": "a",
            "type": "app",
            "tasks": { "source": "github", "repository": "owner/name", "label": "phase-3" }
        }"#;
        let cfg = parse(json).unwrap();
        let tasks = cfg.tasks.unwrap();
        assert_eq!(tasks.source, "github");
        assert_eq!(tasks.repository.as_deref(), Some("owner/name"));
        assert_eq!(tasks.label.as_deref(), Some("phase-3"));
    }

    #[test]
    fn rejects_unknown_project_type() {
        let json = r#"{ "name": "a", "type": "game" }"#;
        assert!(matches!(parse(json), Err(ConfigError::Parse(_))));
    }

    #[test]
    fn rejects_empty_name() {
        assert_eq!(parse(r#"{ "name": " ", "type": "app" }"#), Err(ConfigError::EmptyName));
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("demerzel-cfg-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn resolve_prefers_orchestrator_dev() {
        let dir = tmp("orch");
        fs::write(
            dir.join(CONFIG_FILE),
            r#"{ "name": "root", "type": "app", "commands": { "dev": "bun run pipeline" } }"#,
        )
        .unwrap();
        fs::write(
            dir.join(PACKAGE_JSON),
            r#"{ "scripts": { "dev": "vite", "pipeline": "echo hi" } }"#,
        )
        .unwrap();
        assert_eq!(resolve_dev_command(&dir).as_deref(), Some("bun run pipeline"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn resolve_falls_back_to_package_scripts_dev() {
        let dir = tmp("pkg");
        fs::write(dir.join(PACKAGE_JSON), r#"{ "scripts": { "dev": "vite dev" } }"#).unwrap();
        fs::write(dir.join("bun.lock"), "").unwrap();
        assert_eq!(resolve_dev_command(&dir).as_deref(), Some("bun run dev"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn resolve_none_without_dev_script() {
        let dir = tmp("nodev");
        fs::write(
            dir.join(PACKAGE_JSON),
            r#"{ "scripts": { "pipeline": "bun server.ts" } }"#,
        )
        .unwrap();
        assert_eq!(resolve_dev_command(&dir), None);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn load_checked_missing_is_none() {
        let dir = tmp("nocheck");
        assert_eq!(load_checked(&dir).unwrap(), None);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn load_checked_rejects_bad_json() {
        let dir = tmp("badcheck");
        fs::write(dir.join(CONFIG_FILE), "{").unwrap();
        assert!(load_checked(&dir).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }
}
