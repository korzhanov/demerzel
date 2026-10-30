//! Проброс корня экосистемы в App Group для App Intents (#101).
//!
//! Swift читает `siri-config.json` из `group.app.demerzel.orchestrator`
//! (см. `macos-siri/Sources/EcosystemRoot.swift`).

use super::widgetkit_fallback::{app_group_container, APP_GROUP_ID};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const SIRI_CONFIG_FILE: &str = "siri-config.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SiriConfig {
    /// Корень для `dem pulse` / `dem restart` (первый `scan_roots` или явный путь).
    #[serde(
        default,
        rename = "ecosystemRoot",
        skip_serializing_if = "Option::is_none"
    )]
    pub ecosystem_root: Option<String>,
}

/// Путь к `siri-config.json` в App Group.
pub fn config_path(home: &Path, app_group: &str) -> PathBuf {
    app_group_container(home, app_group).join(SIRI_CONFIG_FILE)
}

/// Пишет корень экосистемы в App Group (атомарно).
/// Пустой `ecosystem_root` → файл с `null`/без поля (Swift уйдёт в fallback).
pub fn sync_ecosystem_root(home: &Path, ecosystem_root: Option<&str>) -> io::Result<PathBuf> {
    let path = config_path(home, APP_GROUP_ID);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let cfg = SiriConfig {
        ecosystem_root: ecosystem_root
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
    };
    let json = serde_json::to_string_pretty(&cfg)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(&tmp, &path)?;
    Ok(path)
}

/// Берёт первый непустой scan_root как ecosystem root.
pub fn root_from_scan_roots(scan_roots: &[String]) -> Option<&str> {
    scan_roots
        .iter()
        .map(|s| s.trim())
        .find(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_home(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "demerzel-siri-config-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn config_path_uses_app_group() {
        let p = config_path(Path::new("/Users/demo"), APP_GROUP_ID);
        assert_eq!(
            p,
            PathBuf::from(
                "/Users/demo/Library/Group Containers/group.app.demerzel.orchestrator/siri-config.json"
            )
        );
    }

    #[test]
    fn sync_writes_ecosystem_root() {
        let home = tmp_home("write");
        let path = sync_ecosystem_root(&home, Some(" /eco/parser ")).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        let cfg: SiriConfig = serde_json::from_str(&text).unwrap();
        assert_eq!(cfg.ecosystem_root.as_deref(), Some("/eco/parser"));
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn root_from_scan_roots_picks_first() {
        assert_eq!(
            root_from_scan_roots(&["  ".into(), "/a".into(), "/b".into()]),
            Some("/a")
        );
        assert_eq!(root_from_scan_roots(&[]), None);
    }
}
