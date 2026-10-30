//! Синхронизация `summary.json` в iCloud Drive для чтения с iPhone (#37).
//!
//! Путь по умолчанию:
//! `~/Library/Mobile Documents/com~apple~CloudDocs/Demerzel/summary.json`
//! (Files → iCloud Drive → Demerzel на iOS). Не путать со Swift WidgetKit (#38).

use super::summary::{write_if_changed, Summary};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

/// Имя контейнера iCloud Drive в `~/Library/Mobile Documents/`.
pub const ICLOUD_DRIVE_CONTAINER: &str = "com~apple~CloudDocs";

/// Папка внутри iCloud Drive по умолчанию.
pub const DEFAULT_ICLOUD_FOLDER: &str = "Demerzel";

/// Имя файла сводки.
pub const SUMMARY_FILE: &str = "summary.json";

/// Настройки синхронизации summary → iCloud (#37).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ICloudSyncConfig {
    /// Писать `summary.json` в iCloud Drive (выкл. по умолчанию).
    #[serde(default)]
    pub enabled: bool,
    /// Относительный путь папки внутри CloudDocs (по умолчанию `Demerzel`).
    #[serde(default = "default_folder")]
    pub folder: String,
}

fn default_folder() -> String {
    DEFAULT_ICLOUD_FOLDER.into()
}

impl Default for ICloudSyncConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            folder: default_folder(),
        }
    }
}

/// Результат попытки синхронизации.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncOutcome {
    /// `enabled == false` — ничего не писали.
    SkippedDisabled,
    /// Файл уже совпадал — rename не делали.
    Unchanged,
    /// Файл записан (или перезаписан).
    Written,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncResult {
    pub outcome: SyncOutcome,
    /// Абсолютный путь к `summary.json` (если sync не disabled).
    pub path: Option<String>,
}

/// Корень iCloud Drive: `<home>/Library/Mobile Documents/com~apple~CloudDocs`.
pub fn icloud_drive_root(home: &Path) -> PathBuf {
    home.join("Library")
        .join("Mobile Documents")
        .join(ICLOUD_DRIVE_CONTAINER)
}

/// Нормализует относительную папку: без `..`, абсолютных и пустых сегментов.
/// Пустая / невалидная → `Demerzel`.
pub fn sanitize_folder(folder: &str) -> String {
    let raw = folder.trim();
    if raw.is_empty() {
        return DEFAULT_ICLOUD_FOLDER.into();
    }
    // Абсолютный путь до strip `/` — иначе `/abs` превратится в `abs`.
    if Path::new(raw).is_absolute() {
        return DEFAULT_ICLOUD_FOLDER.into();
    }
    let trimmed = raw.trim_matches('/');
    if trimmed.is_empty() {
        return DEFAULT_ICLOUD_FOLDER.into();
    }
    let mut parts: Vec<&str> = Vec::new();
    for c in Path::new(trimmed).components() {
        match c {
            Component::Normal(s) => {
                if let Some(t) = s.to_str() {
                    if !t.is_empty() && t != "." {
                        parts.push(t);
                    }
                }
            }
            Component::CurDir => {}
            // ParentDir / RootDir / Prefix — отклоняем весь путь.
            _ => return DEFAULT_ICLOUD_FOLDER.into(),
        }
    }
    if parts.is_empty() {
        DEFAULT_ICLOUD_FOLDER.into()
    } else {
        parts.join("/")
    }
}

/// Полный путь к `summary.json` внутри iCloud Drive.
pub fn summary_path(home: &Path, folder: &str) -> PathBuf {
    let safe = sanitize_folder(folder);
    icloud_drive_root(home).join(safe).join(SUMMARY_FILE)
}

/// Пишет summary в iCloud Drive атомарно (`write_if_changed`).
///
/// Если iCloud Drive отсутствует — `Err` с `NotFound` (на CI / без аккаунта Apple).
pub fn sync_summary(
    home: &Path,
    config: &ICloudSyncConfig,
    summary: &Summary,
) -> io::Result<SyncResult> {
    if !config.enabled {
        return Ok(SyncResult {
            outcome: SyncOutcome::SkippedDisabled,
            path: None,
        });
    }

    let path = summary_path(home, &config.folder);
    let root = icloud_drive_root(home);
    if !root.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "iCloud Drive not found at {}; sign in to iCloud or grant Files access",
                root.display()
            ),
        ));
    }

    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }

    let content = summary.to_json();
    let written = write_if_changed(&path, &content)?;
    Ok(SyncResult {
        outcome: if written {
            SyncOutcome::Written
        } else {
            SyncOutcome::Unchanged
        },
        path: Some(path.display().to_string()),
    })
}

/// Домашний каталог пользователя (`HOME` / `USERPROFILE`).
pub fn user_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Summary {
        let mut s = Summary::new("2026-10-30T23:00:00+01:00");
        s.projects.push(super::super::summary::ProjectStatus {
            name: "api".into(),
            running: 1,
            failing: 0,
        });
        s.recompute();
        s
    }

    #[test]
    fn default_config_is_disabled() {
        let c = ICloudSyncConfig::default();
        assert!(!c.enabled);
        assert_eq!(c.folder, DEFAULT_ICLOUD_FOLDER);
    }

    #[test]
    fn sanitize_rejects_parent_and_absolute() {
        assert_eq!(sanitize_folder(""), DEFAULT_ICLOUD_FOLDER);
        assert_eq!(sanitize_folder("  "), DEFAULT_ICLOUD_FOLDER);
        assert_eq!(sanitize_folder("../evil"), DEFAULT_ICLOUD_FOLDER);
        assert_eq!(sanitize_folder("/abs"), DEFAULT_ICLOUD_FOLDER);
        assert_eq!(sanitize_folder("Demerzel/Status"), "Demerzel/Status");
        assert_eq!(sanitize_folder("Demerzel/./x"), "Demerzel/x");
    }

    #[test]
    fn summary_path_joins_cloud_docs() {
        let home = Path::new("/Users/demo");
        let p = summary_path(home, "Demerzel");
        assert_eq!(
            p,
            PathBuf::from(
                "/Users/demo/Library/Mobile Documents/com~apple~CloudDocs/Demerzel/summary.json"
            )
        );
    }

    #[test]
    fn sync_skips_when_disabled() {
        let home = std::env::temp_dir().join(format!("demerzel-icloud-skip-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(&home).unwrap();
        let cfg = ICloudSyncConfig::default();
        let r = sync_summary(&home, &cfg, &sample()).unwrap();
        assert_eq!(r.outcome, SyncOutcome::SkippedDisabled);
        assert!(r.path.is_none());
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn sync_writes_atomically_under_fake_icloud() {
        let home = std::env::temp_dir().join(format!("demerzel-icloud-write-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        let cloud = icloud_drive_root(&home);
        fs::create_dir_all(&cloud).unwrap();

        let cfg = ICloudSyncConfig {
            enabled: true,
            folder: "Demerzel".into(),
        };
        let summary = sample();
        let r1 = sync_summary(&home, &cfg, &summary).unwrap();
        assert_eq!(r1.outcome, SyncOutcome::Written);
        let path = PathBuf::from(r1.path.as_ref().unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), summary.to_json());
        assert!(!path.with_extension("json.tmp").exists());

        let r2 = sync_summary(&home, &cfg, &summary).unwrap();
        assert_eq!(r2.outcome, SyncOutcome::Unchanged);

        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn sync_errors_if_icloud_missing() {
        let home = std::env::temp_dir().join(format!("demerzel-icloud-miss-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(&home).unwrap();
        let cfg = ICloudSyncConfig {
            enabled: true,
            folder: "Demerzel".into(),
        };
        let err = sync_summary(&home, &cfg, &sample()).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
        fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn serde_defaults_folder_when_omitted() {
        let c: ICloudSyncConfig = serde_json::from_str(r#"{"enabled":true}"#).unwrap();
        assert!(c.enabled);
        assert_eq!(c.folder, DEFAULT_ICLOUD_FOLDER);
    }
}
