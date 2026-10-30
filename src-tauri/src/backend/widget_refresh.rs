//! Решение, когда обновлять виджет (#26).
//!
//! Обновляем только при изменении содержимого и не чаще `min_interval_secs`.
//! Бюджет WidgetKit ограничен (~40–70 обновлений в сутки), поэтому таймер не используем.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub const DEFAULT_MIN_INTERVAL_SECS: u64 = 30;

#[derive(Debug, Default)]
pub struct Throttle {
    last_refresh: Option<u64>,
    last_hash: Option<u64>,
    min_interval_secs: u64,
}

impl Throttle {
    pub fn new(min_interval_secs: u64) -> Self {
        Self { min_interval_secs, ..Default::default() }
    }

    /// Нужно ли обновить виджет сейчас с таким содержимым.
    pub fn should_refresh(&self, now: u64, content: &str) -> bool {
        let hash = hash_of(content);
        if self.last_hash == Some(hash) {
            return false;
        }
        match self.last_refresh {
            None => true,
            Some(t) => now.saturating_sub(t) >= self.min_interval_secs,
        }
    }

    /// Отмечает, что обновление выполнено.
    pub fn mark(&mut self, now: u64, content: &str) {
        self.last_refresh = Some(now);
        self.last_hash = Some(hash_of(content));
    }
}

fn hash_of(content: &str) -> u64 {
    let mut h = DefaultHasher::new();
    content.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_update_always_allowed() {
        let t = Throttle::new(30);
        assert!(t.should_refresh(100, "a"));
    }

    #[test]
    fn unchanged_content_is_skipped() {
        let mut t = Throttle::new(30);
        t.mark(100, "a");
        assert!(!t.should_refresh(500, "a"));
    }

    #[test]
    fn changed_content_waits_for_interval() {
        let mut t = Throttle::new(30);
        t.mark(100, "a");
        assert!(!t.should_refresh(110, "b"));
        assert!(t.should_refresh(130, "b"));
    }
}
