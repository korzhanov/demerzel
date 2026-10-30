//! Настройки приложения: корневые папки сканирования (#11).
//!
//! Хранятся в JSON-файле в папке данных приложения. Чтение отсутствующего
//! файла даёт настройки по умолчанию, а не ошибку.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

use super::agents::AgentsConfig;
use super::alerts::AlertsConfig;
use super::icloud_sync::ICloudSyncConfig;

pub const SETTINGS_FILE: &str = "settings.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub scan_roots: Vec<String>,
    /// Правила алертов (#20). Отсутствующее поле → `AlertsConfig::default()`.
    #[serde(default)]
    pub alerts: AlertsConfig,
    /// Лимит параллельных агентов и порог памяти (#29).
    #[serde(default)]
    pub agents: AgentsConfig,
    /// Синхронизация `summary.json` в iCloud Drive для iPhone (#37).
    #[serde(default)]
    pub icloud: ICloudSyncConfig,
}

/// Убирает пустые строки и дубликаты, сохраняя порядок.
pub fn normalize_roots(roots: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for r in roots {
        let t = r.trim();
        if !t.is_empty() && !out.iter().any(|x| x == t) {
            out.push(t.to_string());
        }
    }
    out
}

pub fn load(path: &Path) -> io::Result<Settings> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(e) => Err(e),
    }
}

pub fn save(path: &Path, settings: &Settings) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut normalized = settings.clone();
    normalized.scan_roots = normalize_roots(&settings.scan_roots);
    let json = serde_json::to_string_pretty(&normalized)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::alerts::AlertsConfig;

    fn tmp_file(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("demerzel-settings-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join(SETTINGS_FILE)
    }

    #[test]
    fn missing_file_gives_defaults() {
        let path = tmp_file("missing");
        assert_eq!(load(&path).unwrap(), Settings::default());
    }

    #[test]
    fn roundtrip_normalizes_roots() {
        let path = tmp_file("roundtrip");
        let s = Settings {
            scan_roots: vec!["  /a ".into(), "".into(), "/a".into(), "/b".into()],
            ..Settings::default()
        };
        save(&path, &s).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.scan_roots, vec!["/a".to_string(), "/b".to_string()]);
        assert_eq!(loaded.alerts.rules.len(), 4);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn missing_alerts_section_uses_default_rules() {
        let path = tmp_file("alerts-default");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, r#"{"scan_roots":["/proj"]}"#).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.scan_roots, vec!["/proj".to_string()]);
        assert_eq!(loaded.alerts, AlertsConfig::default());
        assert_eq!(loaded.agents, crate::backend::agents::AgentsConfig::default());
        assert_eq!(loaded.icloud, crate::backend::icloud_sync::ICloudSyncConfig::default());
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn icloud_section_roundtrip() {
        use crate::backend::icloud_sync::ICloudSyncConfig;
        let path = tmp_file("icloud");
        let s = Settings {
            scan_roots: vec!["/p".into()],
            icloud: ICloudSyncConfig {
                enabled: true,
                folder: "Demerzel/Status".into(),
            },
            ..Settings::default()
        };
        save(&path, &s).unwrap();
        let loaded = load(&path).unwrap();
        assert!(loaded.icloud.enabled);
        assert_eq!(loaded.icloud.folder, "Demerzel/Status");
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn agents_section_roundtrip() {
        use crate::backend::agents::AgentsConfig;
        let path = tmp_file("agents");
        let s = Settings {
            scan_roots: vec!["/p".into()],
            agents: AgentsConfig {
                max_concurrent: 2,
                min_free_memory_mb: 1024,
                ollama_reserve_mb: 2048,
                runner: crate::backend::agents::AgentRunner::Ollama,
                ollama_model: "qwen2.5-coder:7b".into(),
            },
            ..Settings::default()
        };
        save(&path, &s).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.agents.max_concurrent, 2);
        assert_eq!(loaded.agents.min_free_memory_mb, 1024);
        assert_eq!(loaded.agents.ollama_reserve_mb, 2048);
        assert_eq!(loaded.agents.runner, crate::backend::agents::AgentRunner::Ollama);
        assert_eq!(loaded.agents.ollama_model, "qwen2.5-coder:7b");
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn corrupt_file_is_an_error() {
        let path = tmp_file("corrupt");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{not json").unwrap();
        assert!(load(&path).is_err());
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
