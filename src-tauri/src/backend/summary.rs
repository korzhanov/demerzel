//! Экспорт summary.json для виджета (#24).
//!
//! Формат версионирован. Запись атомарная: сначала временный файл, потом rename.
//! Если содержимое не изменилось, файл не переписывается.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

pub const SUMMARY_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ProjectStatus {
    pub name: String,
    pub running: usize,
    pub failing: usize,
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

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("summary is always serializable")
    }
}

/// Пишет файл атомарно. Возвращает `true`, если файл был переписан.
pub fn write_if_changed(path: &Path, content: &str) -> io::Result<bool> {
    if let Ok(existing) = fs::read_to_string(path) {
        if existing == content {
            return Ok(false);
        }
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
        s.projects.push(ProjectStatus { name: "api".into(), running: 1, failing: 1 });
        s.projects.push(ProjectStatus { name: "web".into(), running: 2, failing: 0 });
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
