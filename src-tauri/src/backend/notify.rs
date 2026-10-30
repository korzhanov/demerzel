//! Системные уведомления (#22).
//!
//! Чистая сборка payload из `AlertCandidate` + `DeliveryDecision.notify`.
//! Показ через `tauri-plugin-notification` и клик → проект — слой Tauri (`lib.rs`).
//! Голос `say` (#23) сюда не входит.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use super::alerts::{AlertCandidate, AlertEvent, DeliveryDecision, Severity};

/// Ключ `extra` в уведомлении: имя проекта для deep-link при клике.
pub const EXTRA_PROJECT: &str = "project";

/// Событие фронтенду после клика по уведомлению.
pub const OPEN_PROJECT_EVENT: &str = "alerts:open-project";

/// Payload клика / deep-link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenProjectPayload {
    pub project: String,
}

/// Готовый текст и метаданные для OS-уведомления.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotifyPayload {
    pub title: String,
    pub body: String,
    /// Имя проекта, если событие привязано к проекту (`process_exited`).
    pub project: Option<String>,
    pub severity: Severity,
    pub rule_id: String,
}

/// Короткий заголовок по severity.
pub fn notify_title(severity: Severity) -> String {
    match severity {
        Severity::Critical => "Demerzel · Критично".into(),
        Severity::Important => "Demerzel · Важно".into(),
        Severity::Info => "Demerzel".into(),
    }
}

/// Проект для deep-link; у port/agent/tests — `None`.
pub fn project_hint(event: &AlertEvent) -> Option<String> {
    match event {
        AlertEvent::ProcessExited { project, .. } => Some(project.clone()),
        AlertEvent::PortConflict { .. }
        | AlertEvent::AgentFailed { .. }
        | AlertEvent::TestsFailed { .. } => None,
    }
}

/// Собирает payload из кандидата (без проверки `DeliveryDecision`).
pub fn build_payload(candidate: &AlertCandidate) -> NotifyPayload {
    NotifyPayload {
        title: notify_title(candidate.severity),
        body: candidate.message.clone(),
        project: project_hint(&candidate.event),
        severity: candidate.severity,
        rule_id: candidate.rule_id.clone(),
    }
}

/// Учитывает флаг `notify` из `DeliveryGate` (тихие часы / severity).
pub fn maybe_notify(
    candidate: &AlertCandidate,
    decision: &DeliveryDecision,
) -> Option<NotifyPayload> {
    if !decision.notify {
        return None;
    }
    Some(build_payload(candidate))
}

/// Достаёт имя проекта из `extra` уведомления (клик).
pub fn project_from_extra(extra: &HashMap<String, Value>) -> Option<String> {
    extra.get(EXTRA_PROJECT).and_then(|v| {
        v.as_str()
            .map(str::to_owned)
            .filter(|s| !s.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::alerts::{
        default_rules, evaluate, AlertRule, DeliveryGate, EventKind, RuleWhen,
    };
    use chrono::{TimeZone, Utc};

    #[test]
    fn process_exit_payload_has_project_and_short_body() {
        let e = AlertEvent::ProcessExited {
            project: "api".into(),
            code: 1,
        };
        let c = evaluate(&default_rules(), &e).expect("candidate");
        let p = build_payload(&c);
        assert_eq!(p.project.as_deref(), Some("api"));
        assert_eq!(p.title, "Demerzel · Критично");
        assert!(p.body.contains("api"));
        assert!(p.body.contains('1'));
        assert_eq!(p.rule_id, "process_exited_nonzero");
    }

    #[test]
    fn port_conflict_has_no_project() {
        let e = AlertEvent::PortConflict { port: 5173 };
        let c = evaluate(&default_rules(), &e).expect("candidate");
        let p = build_payload(&c);
        assert!(p.project.is_none());
        assert_eq!(p.title, "Demerzel · Критично");
        assert!(p.body.contains("5173"));
    }

    #[test]
    fn important_title() {
        let e = AlertEvent::AgentFailed {
            run_id: "r1".into(),
        };
        let c = evaluate(&default_rules(), &e).expect("candidate");
        assert_eq!(build_payload(&c).title, "Demerzel · Важно");
    }

    #[test]
    fn maybe_notify_respects_decision_flag() {
        let e = AlertEvent::PortConflict { port: 3000 };
        let c = evaluate(&default_rules(), &e).expect("candidate");
        assert!(maybe_notify(&c, &DeliveryDecision {
            emit: true,
            speak: false,
            notify: true,
        })
        .is_some());
        assert!(maybe_notify(&c, &DeliveryDecision::SUPPRESSED).is_none());
        assert!(maybe_notify(
            &c,
            &DeliveryDecision {
                emit: true,
                speak: false,
                notify: false,
            }
        )
        .is_none());
    }

    fn berlin_night() -> chrono::DateTime<Utc> {
        let tz: chrono_tz::Tz = "Europe/Berlin".parse().unwrap();
        tz.with_ymd_and_hms(2026, 10, 30, 23, 30, 0)
            .single()
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn quiet_hours_gate_skips_important_notify() {
        let mut gate = DeliveryGate::new_default();
        let night = berlin_night();
        let important = evaluate(
            &default_rules(),
            &AlertEvent::AgentFailed {
                run_id: "r-quiet".into(),
            },
        )
        .expect("candidate");
        let d = gate.decide(&important, night);
        assert!(!d.notify);
        assert!(maybe_notify(&important, &d).is_none());

        let critical = evaluate(
            &default_rules(),
            &AlertEvent::PortConflict { port: 8080 },
        )
        .expect("candidate");
        let d2 = gate.decide(&critical, night);
        assert!(d2.notify);
        assert!(maybe_notify(&critical, &d2).is_some());
    }

    #[test]
    fn project_from_extra_reads_string() {
        let mut extra = HashMap::new();
        assert!(project_from_extra(&extra).is_none());
        extra.insert(EXTRA_PROJECT.into(), Value::String("web".into()));
        assert_eq!(project_from_extra(&extra).as_deref(), Some("web"));
        extra.insert(EXTRA_PROJECT.into(), Value::String("".into()));
        assert!(project_from_extra(&extra).is_none());
    }

    #[test]
    fn info_severity_never_notifies_via_gate_defaults() {
        let rules = vec![AlertRule {
            id: "info_port".into(),
            kind: EventKind::PortConflict,
            when: RuleWhen::Always,
            severity: Severity::Info,
            enabled: true,
        }];
        let c = evaluate(&rules, &AlertEvent::PortConflict { port: 1 }).expect("c");
        let mut gate = DeliveryGate::new_default();
        let d = gate.decide(&c, Utc::now());
        assert!(d.emit);
        assert!(!d.notify);
        assert!(maybe_notify(&c, &d).is_none());
    }
}
