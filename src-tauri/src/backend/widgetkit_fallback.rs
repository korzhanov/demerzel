//! Запасной путь: нативный Swift WidgetKit читает `summary.json` из App Group (#38).
//!
//! Основной путь — `tauri-plugin-widgets` (ADR-001). Этот модуль пишет ту же сводку
//! в контейнер App Group, когда включён `settings.widgetkit` или плагин недоступен.
//! Не App Store release: scaffold + атомарная запись + доки.

use super::summary::{write_if_changed, Summary};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// App Group, совпадает с `tauri.conf.json` / ADR-001.
pub const APP_GROUP_ID: &str = "group.app.demerzel.orchestrator";

/// Имя файла сводки в контейнере App Group.
pub const SUMMARY_FILE: &str = "summary.json";

/// Kind виджета в Swift (`DemerzelNativeWidget`).
pub const WIDGET_KIND: &str = "DemerzelNativeWidget";

/// Настройки запасного WidgetKit (#38).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub struct WidgetKitFallbackConfig {
    /// Писать `summary.json` в App Group для нативного extension (выкл. по умолчанию).
    #[serde(default)]
    pub enabled: bool,
}


/// Результат попытки записи в App Group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncOutcome {
    /// `enabled == false` и не forced — ничего не писали.
    SkippedDisabled,
    /// Файл уже совпадал.
    Unchanged,
    /// Файл записан.
    Written,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncResult {
    pub outcome: SyncOutcome,
    /// Абсолютный путь к `summary.json` (если не skipped).
    pub path: Option<String>,
}

/// Корень App Group: `<home>/Library/Group Containers/<appGroup>/`.
pub fn app_group_container(home: &Path, app_group: &str) -> PathBuf {
    home.join("Library")
        .join("Group Containers")
        .join(app_group)
}

/// Полный путь к `summary.json` в App Group.
pub fn summary_path(home: &Path, app_group: &str) -> PathBuf {
    app_group_container(home, app_group).join(SUMMARY_FILE)
}

/// HOME пользователя (для путей App Group).
pub fn user_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Пишет summary в App Group атомарно.
///
/// `force`: писать даже если `config.enabled == false` (fallback после ошибки плагина).
pub fn sync_summary(
    home: &Path,
    config: &WidgetKitFallbackConfig,
    summary: &Summary,
    force: bool,
) -> io::Result<SyncResult> {
    if !config.enabled && !force {
        return Ok(SyncResult {
            outcome: SyncOutcome::SkippedDisabled,
            path: None,
        });
    }

    let path = summary_path(home, APP_GROUP_ID);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let content = summary.to_json();
    let written = write_if_changed(&path, &content)?;
    Ok(SyncResult {
        outcome: if written {
            SyncOutcome::Written
        } else {
            SyncOutcome::Unchanged
        },
        path: Some(path.to_string_lossy().into_owned()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::summary::ProjectStatus;

    fn sample() -> Summary {
        let mut s = Summary::new("2026-10-30T23:00:00+01:00");
        s.projects.push(ProjectStatus {
            name: "api".into(),
            running: 1,
            failing: 2,
            ..Default::default()
        });
        s.recompute();
        s.port_conflicts = 1;
        s.tasks_todo = 3;
        s.tasks_in_progress = 1;
        s.agents_running = 0;
        s
    }

    fn tmp_home(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "demerzel-widgetkit-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn summary_path_uses_app_group() {
        let home = Path::new("/Users/demo");
        let p = summary_path(home, APP_GROUP_ID);
        assert_eq!(
            p,
            PathBuf::from(
                "/Users/demo/Library/Group Containers/group.app.demerzel.orchestrator/summary.json"
            )
        );
    }

    #[test]
    fn skipped_when_disabled() {
        let home = tmp_home("skip");
        let r = sync_summary(&home, &WidgetKitFallbackConfig::default(), &sample(), false).unwrap();
        assert_eq!(r.outcome, SyncOutcome::SkippedDisabled);
        assert!(r.path.is_none());
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn writes_when_enabled() {
        let home = tmp_home("write");
        let cfg = WidgetKitFallbackConfig { enabled: true };
        let r = sync_summary(&home, &cfg, &sample(), false).unwrap();
        assert_eq!(r.outcome, SyncOutcome::Written);
        let path = PathBuf::from(r.path.unwrap());
        assert!(path.exists());
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"failing_total\": 2"));
        assert!(text.contains("\"version\": 1"));

        let r2 = sync_summary(&home, &cfg, &sample(), false).unwrap();
        assert_eq!(r2.outcome, SyncOutcome::Unchanged);
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn force_writes_even_if_disabled() {
        let home = tmp_home("force");
        let cfg = WidgetKitFallbackConfig { enabled: false };
        let r = sync_summary(&home, &cfg, &sample(), true).unwrap();
        assert_eq!(r.outcome, SyncOutcome::Written);
        assert!(PathBuf::from(r.path.unwrap()).exists());
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn default_config_disabled() {
        assert!(!WidgetKitFallbackConfig::default().enabled);
    }
}
