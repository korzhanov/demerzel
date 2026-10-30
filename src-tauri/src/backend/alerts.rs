//! Правила алертов и уровни важности (#20).

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Important,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    ProcessExited { project: String, code: i32 },
    PortConflict { port: u16 },
    AgentFailed { run_id: String },
    TestsFailed { run_id: String },
}

/// Правила по умолчанию. Голос (#23) и уведомления (#22) строятся поверх этой функции.
pub fn severity(event: &Event) -> Severity {
    match event {
        Event::ProcessExited { code, .. } if *code != 0 => Severity::Critical,
        Event::ProcessExited { .. } => Severity::Info,
        Event::PortConflict { .. } => Severity::Critical,
        Event::AgentFailed { .. } => Severity::Important,
        Event::TestsFailed { .. } => Severity::Important,
    }
}

/// Голос включается только для critical.
pub fn should_speak(sev: Severity) -> bool {
    sev == Severity::Critical
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonzero_exit_is_critical_and_speaks() {
        let e = Event::ProcessExited { project: "api".into(), code: 1 };
        assert_eq!(severity(&e), Severity::Critical);
        assert!(should_speak(severity(&e)));
    }

    #[test]
    fn clean_exit_is_info() {
        let e = Event::ProcessExited { project: "api".into(), code: 0 };
        assert_eq!(severity(&e), Severity::Info);
    }

    #[test]
    fn failed_tests_do_not_speak() {
        let e = Event::TestsFailed { run_id: "r1".into() };
        assert!(!should_speak(severity(&e)));
    }
}
