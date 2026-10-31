//! Таблица установки CLI-агентов и открытие Terminal.app (#140).
//!
//! Данные: `config/agent-install.json` (обновлять без релиза логики детекта).

use serde::Deserialize;
use std::path::PathBuf;
use std::process::Command;

use super::agent_catalog::{detect_agent, AgentCatalogEntry};
use super::agents::{AgentCatalogPref, AgentRunner};

const INSTALL_JSON: &str = include_str!("../../../config/agent-install.json");

#[derive(Debug, Clone, Deserialize)]
pub struct AgentInstallTable {
    pub terminal_default: String,
    pub agents: Vec<AgentInstallRow>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentInstallRow {
    pub runner: String,
    pub binary: String,
    #[serde(default)]
    pub binaries: Vec<String>,
    pub install: Option<String>,
    #[serde(default)]
    pub install_alt: Option<String>,
    pub update: Option<String>,
    pub install_hint: String,
    #[serde(default = "default_true")]
    pub verified: bool,
    #[serde(default)]
    pub headless_supported: bool,
    pub notes: Option<String>,
}

fn default_true() -> bool {
    true
}

pub fn load_install_table() -> Result<AgentInstallTable, String> {
    serde_json::from_str(INSTALL_JSON).map_err(|e| format!("agent-install.json: {e}"))
}

pub fn row_for(runner: AgentRunner) -> Option<AgentInstallRow> {
    let table = load_install_table().ok()?;
    table
        .agents
        .into_iter()
        .find(|a| AgentRunner::parse(&a.runner).ok() == Some(runner))
}

pub fn install_command(runner: AgentRunner) -> Option<String> {
    row_for(runner)?.install
}

/// Порядок prefs: заданный + недостающие runner'ы (enabled=true).
pub fn merge_catalog_prefs(prefs: &[AgentCatalogPref]) -> Vec<AgentCatalogPref> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for p in prefs {
        if seen.insert(p.runner) {
            out.push(p.clone());
        }
    }
    for &r in AgentRunner::ALL {
        if seen.insert(r) {
            out.push(AgentCatalogPref {
                runner: r,
                enabled: true,
            });
        }
    }
    out
}

/// Первый включённый и установленный на PATH.
pub fn pick_preferred_runner(
    prefs: &[AgentCatalogPref],
    path_env: Option<&std::ffi::OsStr>,
) -> Option<AgentRunner> {
    let prefs = merge_catalog_prefs(prefs);
    for p in prefs {
        if !p.enabled {
            continue;
        }
        let entry = detect_agent(p.runner, path_env);
        if entry.available {
            return Some(p.runner);
        }
    }
    None
}

/// Список для UI: детект + install_hint из таблицы + verified.
pub fn enrich_catalog(path_env: Option<&std::ffi::OsStr>) -> Vec<AgentCatalogEntry> {
    let table = load_install_table().unwrap_or_else(|_| AgentInstallTable {
        terminal_default: "Terminal.app".into(),
        agents: vec![],
    });
    AgentRunner::ALL
        .iter()
        .copied()
        .map(|runner| {
            let mut entry = detect_agent(runner, path_env);
            if let Some(row) = table
                .agents
                .iter()
                .find(|a| AgentRunner::parse(&a.runner).ok() == Some(runner))
            {
                entry.install_hint = row.install_hint.clone();
                entry.binary = row.binary.clone();
                entry.headless_supported = row.headless_supported;
            }
            entry
        })
        .collect()
}

/// Экранирование для AppleScript double-quoted string.
pub fn applescript_escape(cmd: &str) -> String {
    cmd.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Открывает Terminal.app и предлагает выполнить команду (после confirm в UI).
#[cfg(target_os = "macos")]
pub fn open_install_in_terminal(command: &str) -> Result<(), String> {
    let escaped = applescript_escape(command);
    let script = format!(
        r#"tell application "Terminal"
  activate
  do script "{escaped}"
end tell"#
    );
    let status = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .status()
        .map_err(|e| format!("osascript: {e}"))?;
    if !status.success() {
        return Err("Terminal.app: osascript завершился с ошибкой".into());
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn open_install_in_terminal(_command: &str) -> Result<(), String> {
    Err("Установка через Terminal.app доступна только на macOS".into())
}

/// Путь к JSON на диске (для docs/тестов).
pub fn install_json_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../config/agent-install.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_table_and_commands() {
        let t = load_install_table().unwrap();
        assert_eq!(t.terminal_default, "Terminal.app");
        assert!(install_command(AgentRunner::Codex).unwrap().contains("brew"));
        assert!(install_command(AgentRunner::Hermes).unwrap().contains("hermes"));
        assert!(install_command(AgentRunner::Claude).is_none());
        let grok = row_for(AgentRunner::Grok).unwrap();
        assert!(!grok.verified);
        let gemini = row_for(AgentRunner::Gemini).unwrap();
        assert!(gemini.install_hint.contains("Antigravity"));
    }

    #[test]
    fn pick_preferred_respects_enabled_and_order() {
        let prefs = vec![
            AgentCatalogPref {
                runner: AgentRunner::Codex,
                enabled: false,
            },
            AgentCatalogPref {
                runner: AgentRunner::Claude,
                enabled: true,
            },
        ];
        // Без PATH claude — None; с пустым path всё unavailable.
        let empty = std::ffi::OsStr::new("");
        assert!(pick_preferred_runner(&prefs, Some(empty)).is_none());
    }

    #[test]
    fn applescript_escape_quotes() {
        assert_eq!(applescript_escape(r#"echo "hi""#), r#"echo \"hi\""#);
    }

    #[test]
    fn merge_catalog_appends_missing() {
        let prefs = vec![AgentCatalogPref {
            runner: AgentRunner::Codex,
            enabled: false,
        }];
        let m = merge_catalog_prefs(&prefs);
        assert_eq!(m[0].runner, AgentRunner::Codex);
        assert!(!m[0].enabled);
        assert!(m.iter().any(|p| p.runner == AgentRunner::Claude && p.enabled));
        assert_eq!(m.len(), AgentRunner::ALL.len());
    }
}
