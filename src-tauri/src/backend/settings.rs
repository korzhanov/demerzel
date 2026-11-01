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
use super::widgetkit_fallback::WidgetKitFallbackConfig;

pub const SETTINGS_FILE: &str = "settings.json";

/// Правила сканирования для конкретного корня / вложенной папки.
/// Более длинный префикс пути имеет приоритет над предком.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanRootRules {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_depth: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_nesting: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignore: Vec<String>,
}

/// Корень (или вложенный узел) в дереве сканирования.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanRootEntry {
    pub path: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules: Option<ScanRootRules>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// Включённые корни (зеркало `scan_entries` для совместимости / Siri).
    #[serde(default)]
    pub scan_roots: Vec<String>,
    /// Полное дерево выбора: enabled + per-folder rules.
    #[serde(default)]
    pub scan_entries: Vec<ScanRootEntry>,
    /// Правила алертов (#20). Отсутствующее поле → `AlertsConfig::default()`.
    #[serde(default)]
    pub alerts: AlertsConfig,
    /// Лимит параллельных агентов и порог памяти (#29).
    #[serde(default)]
    pub agents: AgentsConfig,
    /// Синхронизация `summary.json` в iCloud Drive для iPhone (#37).
    #[serde(default)]
    pub icloud: ICloudSyncConfig,
    /// Запасной нативный WidgetKit: запись `summary.json` в App Group (#38).
    #[serde(default)]
    pub widgetkit: WidgetKitFallbackConfig,
}

/// Убирает пустые строки и дубликаты, сохраняя порядок.
pub fn normalize_roots(roots: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for r in roots {
        let t = normalize_path(r);
        if !t.is_empty() && !out.iter().any(|x| x == &t) {
            out.push(t);
        }
    }
    out
}

pub fn normalize_path(path: &str) -> String {
    let t = path.trim().trim_end_matches('/');
    if t.is_empty() {
        String::new()
    } else {
        t.to_string()
    }
}

pub fn normalize_entries(entries: &[ScanRootEntry]) -> Vec<ScanRootEntry> {
    let mut out: Vec<ScanRootEntry> = Vec::new();
    for e in entries {
        let path = normalize_path(&e.path);
        if path.is_empty() {
            continue;
        }
        if let Some(existing) = out.iter_mut().find(|x| x.path == path) {
            *existing = ScanRootEntry {
                path,
                enabled: e.enabled,
                rules: e.rules.clone(),
            };
        } else {
            out.push(ScanRootEntry {
                path,
                enabled: e.enabled,
                rules: e.rules.clone(),
            });
        }
    }
    out
}

/// Синхронизирует legacy `scan_roots` ↔ `scan_entries`.
pub fn sync_scan_fields(settings: &mut Settings) {
    settings.scan_entries = normalize_entries(&settings.scan_entries);
    if settings.scan_entries.is_empty() && !settings.scan_roots.is_empty() {
        settings.scan_entries = normalize_roots(&settings.scan_roots)
            .into_iter()
            .map(|path| ScanRootEntry {
                path,
                enabled: true,
                rules: None,
            })
            .collect();
    }
    settings.scan_roots = settings
        .scan_entries
        .iter()
        .filter(|e| e.enabled)
        .map(|e| e.path.clone())
        .collect();
}

/// Самый специфичный entry (длинный префикс), покрывающий `path`.
pub fn resolve_entry_for_path<'a>(
    path: &str,
    entries: &'a [ScanRootEntry],
) -> Option<&'a ScanRootEntry> {
    let path = normalize_path(path);
    let mut best: Option<&ScanRootEntry> = None;
    let mut best_len = -1isize;
    for e in entries {
        let p = normalize_path(&e.path);
        if path == p || path.starts_with(&(p.clone() + "/")) {
            let len = p.len() as isize;
            if len > best_len {
                best = Some(e);
                best_len = len;
            }
        }
    }
    best
}

pub fn resolve_rules_for_path<'a>(
    path: &str,
    entries: &'a [ScanRootEntry],
) -> Option<&'a ScanRootRules> {
    let path = normalize_path(path);
    let mut covering: Vec<&ScanRootEntry> = entries
        .iter()
        .filter(|e| {
            let p = normalize_path(&e.path);
            e.rules.is_some() && (path == p || path.starts_with(&(p + "/")))
        })
        .collect();
    covering.sort_by_key(|e| std::cmp::Reverse(normalize_path(&e.path).len()));
    covering.first().and_then(|e| e.rules.as_ref())
}

pub fn load(path: &Path) -> io::Result<Settings> {
    match fs::read_to_string(path) {
        Ok(text) => {
            let mut s: Settings = serde_json::from_str(&text)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            sync_scan_fields(&mut s);
            Ok(s)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(e) => Err(e),
    }
}

pub fn save(path: &Path, settings: &Settings) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut normalized = settings.clone();
    sync_scan_fields(&mut normalized);
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
        assert_eq!(loaded.scan_entries.len(), 2);
        assert!(loaded.scan_entries.iter().all(|e| e.enabled));
        assert_eq!(loaded.alerts.rules.len(), 4);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn nested_rules_priority() {
        let entries = vec![
            ScanRootEntry {
                path: "/code".into(),
                enabled: true,
                rules: Some(ScanRootRules {
                    max_depth: Some(2),
                    ..Default::default()
                }),
            },
            ScanRootEntry {
                path: "/code/app".into(),
                enabled: true,
                rules: Some(ScanRootRules {
                    max_depth: Some(5),
                    ignore: vec!["tmp".into()],
                    ..Default::default()
                }),
            },
        ];
        let r = resolve_rules_for_path("/code/app/src", &entries).unwrap();
        assert_eq!(r.max_depth, Some(5));
        assert_eq!(r.ignore, vec!["tmp".to_string()]);
        assert_eq!(
            resolve_rules_for_path("/code/other", &entries)
                .unwrap()
                .max_depth,
            Some(2)
        );
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
        assert_eq!(
            loaded.widgetkit,
            crate::backend::widgetkit_fallback::WidgetKitFallbackConfig::default()
        );
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn widgetkit_section_roundtrip() {
        use crate::backend::widgetkit_fallback::WidgetKitFallbackConfig;
        let path = tmp_file("widgetkit");
        let s = Settings {
            scan_roots: vec!["/p".into()],
            widgetkit: WidgetKitFallbackConfig { enabled: true },
            ..Settings::default()
        };
        save(&path, &s).unwrap();
        let loaded = load(&path).unwrap();
        assert!(loaded.widgetkit.enabled);
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
                autofix: crate::backend::agents::AutofixMode::Suggest,
                catalog: vec![],
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
        assert_eq!(
            loaded.agents.autofix,
            crate::backend::agents::AutofixMode::Suggest
        );
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

/// Формат файла по расширению: yaml/yml → YAML, иначе JSON (#settings-io).
pub fn is_yaml_path(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    p.ends_with(".yaml") || p.ends_with(".yml")
}

/// Текст настроек для экспорта. Порядок полей — как в структуре.
pub fn to_text(settings: &Settings, yaml: bool) -> Result<String, String> {
    let mut normalized = settings.clone();
    sync_scan_fields(&mut normalized);
    if yaml {
        serde_yaml::to_string(&normalized).map_err(|e| e.to_string())
    } else {
        serde_json::to_string_pretty(&normalized).map(|t| t + "\n").map_err(|e| e.to_string())
    }
}

/// Разбор импортируемого файла с проверкой структуры; ошибка — с указанием места.
pub fn from_text(text: &str, yaml: bool) -> Result<Settings, String> {
    let mut s: Settings = if yaml {
        serde_yaml::from_str(text).map_err(|e| format!("YAML: {e}"))?
    } else {
        serde_json::from_str(text).map_err(|e| format!("JSON: {e}"))?
    };
    sync_scan_fields(&mut s);
    Ok(s)
}

#[cfg(test)]
mod io_tests {
    use super::*;

    #[test]
    fn json_and_yaml_roundtrip_keep_roots() {
        let mut s = Settings::default();
        s.scan_roots = vec!["/Users/demo/code".into()];
        sync_scan_fields(&mut s);
        for yaml in [false, true] {
            let text = to_text(&s, yaml).unwrap();
            let back = from_text(&text, yaml).unwrap();
            assert_eq!(back.scan_roots, s.scan_roots, "yaml={yaml}");
        }
    }

    #[test]
    fn detects_yaml_extension_and_reports_bad_input() {
        assert!(is_yaml_path("/a/b.YML"));
        assert!(!is_yaml_path("/a/b.json"));
        assert!(from_text("{ broken", false).unwrap_err().starts_with("JSON:"));
    }
}
