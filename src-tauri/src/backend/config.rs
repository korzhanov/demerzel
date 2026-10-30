//! Конфиг проекта `orchestrator.json` (#5).
//!
//! Лежит в корне каждого проекта и подпроекта. Валидация выполняется при загрузке,
//! ошибка содержит путь к файлу и причину.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::ports::PortRange;

pub const CONFIG_FILE: &str = "orchestrator.json";

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
    fn rejects_unknown_project_type() {
        let json = r#"{ "name": "a", "type": "game" }"#;
        assert!(matches!(parse(json), Err(ConfigError::Parse(_))));
    }

    #[test]
    fn rejects_empty_name() {
        assert_eq!(parse(r#"{ "name": " ", "type": "app" }"#), Err(ConfigError::EmptyName));
    }
}
