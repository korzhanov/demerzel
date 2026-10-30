//! Запуски агентов: статусы run (#27–#31).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Queued,
    Running,
    NeedsReview,
    Failed,
    Discarded,
    Accepted,
}

/// Допустимые переходы. Accepted только после NeedsReview: слияние всегда вручную.
pub fn can_transition(from: RunStatus, to: RunStatus) -> bool {
    use RunStatus::*;
    matches!(
        (from, to),
        (Queued, Running)
            | (Running, NeedsReview)
            | (Running, Failed)
            | (NeedsReview, Accepted)
            | (NeedsReview, Discarded)
            | (Failed, Discarded)
            | (Queued, Discarded)
    )
}

/// Лимит параллельных агентов по умолчанию.
pub const DEFAULT_CONCURRENCY: usize = 1;

#[cfg(test)]
mod tests {
    use super::*;
    use RunStatus::*;

    #[test]
    fn cannot_accept_without_review() {
        assert!(!can_transition(Running, Accepted));
        assert!(can_transition(NeedsReview, Accepted));
    }

    #[test]
    fn default_concurrency_is_one() {
        assert_eq!(DEFAULT_CONCURRENCY, 1);
    }
}
