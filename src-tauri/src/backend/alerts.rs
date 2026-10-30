//! Движок правил алертов (#20).
//!
//! Оценивает события → `AlertCandidate`. Доставка: дедуп/тихие часы (#21),
//! системные уведомления (#22), голос `say` (#23) — отдельные задачи.

use serde::{Deserialize, Serialize};

/// Уровни: critical (голос + уведомление), important (уведомление), info (только UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Important,
    Critical,
}

/// Виды событий, на которые могут ссылаться правила.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    ProcessExited,
    PortConflict,
    AgentFailed,
    TestsFailed,
}

/// Условие срабатывания правила после совпадения `kind`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum RuleWhen {
    /// Любое событие данного вида.
    #[default]
    Always,
    /// Только ненулевой код выхода (`process_exited`).
    ExitCodeNonZero,
}

/// Правило из конфига приложения (`settings.json` → `alerts.rules`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertRule {
    pub id: String,
    pub kind: EventKind,
    #[serde(default)]
    pub when: RuleWhen,
    pub severity: Severity,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

/// Секция алертов в конфиге приложения.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertsConfig {
    #[serde(default = "default_rules")]
    pub rules: Vec<AlertRule>,
}

impl Default for AlertsConfig {
    fn default() -> Self {
        Self {
            rules: default_rules(),
        }
    }
}

/// Входящее событие рантайма (ещё не алерт).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlertEvent {
    ProcessExited { project: String, code: i32 },
    PortConflict { port: u16 },
    AgentFailed { run_id: String },
    TestsFailed { run_id: String },
}

impl AlertEvent {
    pub fn kind(&self) -> EventKind {
        match self {
            Self::ProcessExited { .. } => EventKind::ProcessExited,
            Self::PortConflict { .. } => EventKind::PortConflict,
            Self::AgentFailed { .. } => EventKind::AgentFailed,
            Self::TestsFailed { .. } => EventKind::TestsFailed,
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::ProcessExited { project, code } => {
                format!("process {project} exited with code {code}")
            }
            Self::PortConflict { port } => format!("port {port} conflict"),
            Self::AgentFailed { run_id } => format!("agent run {run_id} failed"),
            Self::TestsFailed { run_id } => format!("tests failed after agent run {run_id}"),
        }
    }
}

/// Кандидат алерта после матчинга правила. Без доставки (#21–#23).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertCandidate {
    pub rule_id: String,
    pub severity: Severity,
    pub event: AlertEvent,
    pub message: String,
}

/// Правила по умолчанию из #20.
pub fn default_rules() -> Vec<AlertRule> {
    vec![
        AlertRule {
            id: "process_exited_nonzero".into(),
            kind: EventKind::ProcessExited,
            when: RuleWhen::ExitCodeNonZero,
            severity: Severity::Critical,
            enabled: true,
        },
        AlertRule {
            id: "port_conflict".into(),
            kind: EventKind::PortConflict,
            when: RuleWhen::Always,
            severity: Severity::Critical,
            enabled: true,
        },
        AlertRule {
            id: "agent_failed".into(),
            kind: EventKind::AgentFailed,
            when: RuleWhen::Always,
            severity: Severity::Important,
            enabled: true,
        },
        AlertRule {
            id: "tests_failed".into(),
            kind: EventKind::TestsFailed,
            when: RuleWhen::Always,
            severity: Severity::Important,
            enabled: true,
        },
    ]
}

fn when_matches(when: &RuleWhen, event: &AlertEvent) -> bool {
    match when {
        RuleWhen::Always => true,
        RuleWhen::ExitCodeNonZero => match event {
            AlertEvent::ProcessExited { code, .. } => *code != 0,
            _ => false,
        },
    }
}

/// Первое подходящее включённое правило → кандидат; иначе `None`.
pub fn evaluate(rules: &[AlertRule], event: &AlertEvent) -> Option<AlertCandidate> {
    let kind = event.kind();
    for rule in rules {
        if !rule.enabled || rule.kind != kind {
            continue;
        }
        if !when_matches(&rule.when, event) {
            continue;
        }
        return Some(AlertCandidate {
            rule_id: rule.id.clone(),
            severity: rule.severity,
            event: event.clone(),
            message: event.message(),
        });
    }
    None
}

/// Голос включается только для critical (#23 поверх этого флага).
pub fn should_speak(sev: Severity) -> bool {
    sev == Severity::Critical
}

/// Системное уведомление для important и critical; info — только UI (#22).
pub fn should_notify(sev: Severity) -> bool {
    matches!(sev, Severity::Important | Severity::Critical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_rules_cover_four_kinds() {
        let rules = default_rules();
        assert_eq!(rules.len(), 4);
        assert!(rules.iter().any(|r| r.id == "process_exited_nonzero"));
        assert!(rules.iter().any(|r| r.id == "port_conflict"));
        assert!(rules.iter().any(|r| r.id == "agent_failed"));
        assert!(rules.iter().any(|r| r.id == "tests_failed"));
    }

    #[test]
    fn nonzero_exit_is_critical_and_speaks() {
        let e = AlertEvent::ProcessExited {
            project: "api".into(),
            code: 1,
        };
        let c = evaluate(&default_rules(), &e).expect("candidate");
        assert_eq!(c.rule_id, "process_exited_nonzero");
        assert_eq!(c.severity, Severity::Critical);
        assert!(should_speak(c.severity));
        assert!(should_notify(c.severity));
        assert!(c.message.contains("api"));
        assert!(c.message.contains('1'));
    }

    #[test]
    fn clean_exit_produces_no_candidate() {
        let e = AlertEvent::ProcessExited {
            project: "api".into(),
            code: 0,
        };
        assert!(evaluate(&default_rules(), &e).is_none());
    }

    #[test]
    fn port_conflict_is_critical() {
        let e = AlertEvent::PortConflict { port: 5173 };
        let c = evaluate(&default_rules(), &e).expect("candidate");
        assert_eq!(c.rule_id, "port_conflict");
        assert_eq!(c.severity, Severity::Critical);
        assert!(should_speak(c.severity));
    }

    #[test]
    fn agent_failed_is_important_no_speak() {
        let e = AlertEvent::AgentFailed {
            run_id: "run-1".into(),
        };
        let c = evaluate(&default_rules(), &e).expect("candidate");
        assert_eq!(c.rule_id, "agent_failed");
        assert_eq!(c.severity, Severity::Important);
        assert!(!should_speak(c.severity));
        assert!(should_notify(c.severity));
    }

    #[test]
    fn tests_failed_is_important_no_speak() {
        let e = AlertEvent::TestsFailed {
            run_id: "r1".into(),
        };
        let c = evaluate(&default_rules(), &e).expect("candidate");
        assert_eq!(c.rule_id, "tests_failed");
        assert_eq!(c.severity, Severity::Important);
        assert!(!should_speak(c.severity));
        assert!(should_notify(c.severity));
    }

    #[test]
    fn disabled_rule_is_skipped() {
        let mut rules = default_rules();
        for r in &mut rules {
            if r.id == "port_conflict" {
                r.enabled = false;
            }
        }
        let e = AlertEvent::PortConflict { port: 80 };
        assert!(evaluate(&rules, &e).is_none());
    }

    #[test]
    fn custom_rule_overrides_severity() {
        let rules = vec![AlertRule {
            id: "port_info".into(),
            kind: EventKind::PortConflict,
            when: RuleWhen::Always,
            severity: Severity::Info,
            enabled: true,
        }];
        let e = AlertEvent::PortConflict { port: 3000 };
        let c = evaluate(&rules, &e).expect("candidate");
        assert_eq!(c.severity, Severity::Info);
        assert!(!should_speak(c.severity));
        assert!(!should_notify(c.severity));
    }

    #[test]
    fn first_matching_rule_wins() {
        let rules = vec![
            AlertRule {
                id: "first".into(),
                kind: EventKind::AgentFailed,
                when: RuleWhen::Always,
                severity: Severity::Critical,
                enabled: true,
            },
            AlertRule {
                id: "second".into(),
                kind: EventKind::AgentFailed,
                when: RuleWhen::Always,
                severity: Severity::Info,
                enabled: true,
            },
        ];
        let e = AlertEvent::AgentFailed {
            run_id: "x".into(),
        };
        let c = evaluate(&rules, &e).expect("candidate");
        assert_eq!(c.rule_id, "first");
        assert_eq!(c.severity, Severity::Critical);
    }

    #[test]
    fn severity_serde_wire_names() {
        assert_eq!(
            serde_json::to_string(&Severity::Critical).unwrap(),
            "\"critical\""
        );
        assert_eq!(
            serde_json::to_string(&EventKind::ProcessExited).unwrap(),
            "\"process_exited\""
        );
        let rule: AlertRule = serde_json::from_str(
            r#"{"id":"r","kind":"port_conflict","severity":"critical"}"#,
        )
        .unwrap();
        assert_eq!(rule.when, RuleWhen::Always);
        assert!(rule.enabled);
    }

    #[test]
    fn alerts_config_roundtrip() {
        let cfg = AlertsConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let back: AlertsConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.rules.len(), 4);
    }
}
