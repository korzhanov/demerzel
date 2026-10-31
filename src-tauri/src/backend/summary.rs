//! Экспорт summary.json для виджета (#24), iCloud (#37) и native WidgetKit (#38).
//!
//! Формат версионирован. Запись атомарная: сначала временный файл, потом rename.
//! Если содержимое не изменилось, файл не переписывается.
//! Копия в iCloud Drive — см. `icloud_sync`; App Group — `widgetkit_fallback`.
//! Опциональный пульс проектов (#98); источник команды/файла — #97.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

pub const SUMMARY_VERSION: u32 = 1;

/// Снимок пульса для виджета / summary.json (#98).
/// Полный контракт опроса (команда / файл, metrics) — #97.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ProjectPulse {
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eta_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    /// Числовые метрики прогресса (например parsed / checked); виджет их не рисует.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics: Option<BTreeMap<String, i64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ProjectStatus {
    pub name: String,
    pub running: usize,
    pub failing: usize,
    /// Последний известный пульс; отсутствует → строка пульса в виджете не рисуется.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pulse: Option<ProjectPulse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Summary {
    pub version: u32,
    pub projects: Vec<ProjectStatus>,
    pub failing_total: usize,
    pub port_conflicts: usize,
    pub tasks_todo: usize,
    pub tasks_in_progress: usize,
    pub agents_running: usize,
    pub updated_at: String,
}

impl Summary {
    pub fn new(updated_at: impl Into<String>) -> Self {
        Self {
            version: SUMMARY_VERSION,
            updated_at: updated_at.into(),
            ..Default::default()
        }
    }

    /// Пересчитывает суммы по проектам.
    pub fn recompute(&mut self) {
        self.failing_total = self.projects.iter().map(|p| p.failing).sum();
    }

    /// Проекты с непустым текстом пульса (для виджета #98).
    pub fn projects_with_pulse(&self) -> impl Iterator<Item = (&str, &ProjectPulse)> {
        self.projects.iter().filter_map(|p| {
            let pulse = p.pulse.as_ref()?;
            let summary = pulse.summary.trim();
            if summary.is_empty() {
                return None;
            }
            Some((p.name.as_str(), pulse))
        })
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("summary is always serializable")
    }
}

/// Человекочитаемый ETA для строки пульса (RU).
pub fn format_eta_ru(secs: u64) -> String {
    if secs < 60 {
        format!("ещё {secs} с")
    } else if secs < 3600 {
        let mins = secs.div_ceil(60);
        format!("ещё {mins} мин")
    } else {
        let hours = secs.div_ceil(3600);
        format!("ещё {hours} ч")
    }
}

/// `name: summary` или `name: summary, ещё N мин`.
pub fn format_pulse_line(name: &str, pulse: &ProjectPulse) -> String {
    let summary = pulse.summary.trim();
    match pulse.eta_seconds {
        Some(secs) => format!("{name}: {summary}, {}", format_eta_ru(secs)),
        None => format!("{name}: {summary}"),
    }
}

/// Пишет файл атомарно. Возвращает `true`, если файл был переписан.
pub fn write_if_changed(path: &Path, content: &str) -> io::Result<bool> {
    if let Ok(existing) = fs::read_to_string(path)
        && existing == content {
            return Ok(false);
        }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, content)?;
    fs::rename(&tmp, path)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Summary {
        let mut s = Summary::new("2026-10-30T18:00:00+01:00");
        s.projects.push(ProjectStatus {
            name: "api".into(),
            running: 1,
            failing: 1,
            pulse: Some(ProjectPulse {
                summary: "1500 новых лемм, 450 проверено".into(),
                eta_seconds: Some(600),
                state: Some("running".into()),
                metrics: Some(BTreeMap::from([
                    ("parsed".into(), 1500),
                    ("checked".into(), 450),
                ])),
            }),
        });
        s.projects.push(ProjectStatus {
            name: "web".into(),
            running: 2,
            failing: 0,
            pulse: None,
        });
        s.recompute();
        s
    }

    #[test]
    fn recompute_sums_failing() {
        assert_eq!(sample().failing_total, 1);
    }

    #[test]
    fn json_has_version_and_fields() {
        let json = sample().to_json();
        assert!(json.contains("\"version\": 1"));
        assert!(json.contains("\"failing_total\": 1"));
        assert!(json.contains("\"updated_at\""));
        assert!(json.contains("\"pulse\""));
        assert!(json.contains("\"eta_seconds\": 600"));
    }

    #[test]
    fn pulse_line_includes_eta() {
        let pulse = ProjectPulse {
            summary: "1500 новых лемм, 450 проверено".into(),
            eta_seconds: Some(600),
            state: None,
            metrics: None,
        };
        assert_eq!(
            format_pulse_line("parser", &pulse),
            "parser: 1500 новых лемм, 450 проверено, ещё 10 мин"
        );
        assert_eq!(format_eta_ru(45), "ещё 45 с");
        assert_eq!(format_eta_ru(3600), "ещё 1 ч");
    }

    #[test]
    fn projects_with_pulse_skips_empty() {
        let mut s = sample();
        s.projects.push(ProjectStatus {
            name: "empty".into(),
            running: 0,
            failing: 0,
            pulse: Some(ProjectPulse {
                summary: "  ".into(),
                eta_seconds: None,
                state: None,
                metrics: None,
            }),
        });
        let names: Vec<_> = s.projects_with_pulse().map(|(n, _)| n).collect();
        assert_eq!(names, vec!["api"]);
    }

    #[test]
    fn write_is_atomic_and_skips_unchanged() {
        let dir = std::env::temp_dir().join(format!("demerzel-summary-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("summary.json");
        let content = sample().to_json();

        assert!(write_if_changed(&path, &content).unwrap());
        assert!(!write_if_changed(&path, &content).unwrap());
        assert!(!dir.join("summary.json.tmp").exists());
        assert_eq!(fs::read_to_string(&path).unwrap(), content);

        fs::remove_dir_all(&dir).unwrap();
    }
}
