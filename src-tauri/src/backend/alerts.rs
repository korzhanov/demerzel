//! Движок правил алертов (#20) и политика доставки (#21).
//!
//! Оценивает события → `AlertCandidate`, затем `DeliveryGate` решает
//! emit / speak / notify (дедуп, троттлинг голоса, тихие часы).
//! Системные уведомления (#22) и голос `say` (#23) — отдельные задачи.

use chrono::{DateTime, NaiveTime, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

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

/// Тихие часы: голос выкл., notify только для `critical` (#21).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuietHoursConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Локальное время `HH:MM` в `AlertsConfig.timezone` (по умолчанию 23:00).
    #[serde(default = "default_quiet_start")]
    pub start: String,
    /// Локальное время `HH:MM` (по умолчанию 08:00). Окно через полночь допустимо.
    #[serde(default = "default_quiet_end")]
    pub end: String,
}

impl Default for QuietHoursConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            start: default_quiet_start(),
            end: default_quiet_end(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_quiet_start() -> String {
    "23:00".into()
}

fn default_quiet_end() -> String {
    "08:00".into()
}

fn default_dedupe_window_secs() -> u64 {
    600
}

fn default_voice_min_interval_secs() -> u64 {
    60
}

fn default_timezone() -> String {
    "Europe/Berlin".into()
}

/// Секция алертов в конфиге приложения.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertsConfig {
    #[serde(default = "default_rules")]
    pub rules: Vec<AlertRule>,
    /// Окно схлопывания одинаковых событий, секунды (по умолчанию 10 мин).
    #[serde(default = "default_dedupe_window_secs")]
    pub dedupe_window_secs: u64,
    /// Минимальный интервал между голосовыми алертами, секунды (по умолчанию 1 мин).
    #[serde(default = "default_voice_min_interval_secs")]
    pub voice_min_interval_secs: u64,
    #[serde(default)]
    pub quiet_hours: QuietHoursConfig,
    /// IANA TZ для тихих часов (по умолчанию Europe/Berlin).
    #[serde(default = "default_timezone")]
    pub timezone: String,
}

impl Default for AlertsConfig {
    fn default() -> Self {
        Self {
            rules: default_rules(),
            dedupe_window_secs: default_dedupe_window_secs(),
            voice_min_interval_secs: default_voice_min_interval_secs(),
            quiet_hours: QuietHoursConfig::default(),
            timezone: default_timezone(),
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

/// Ключ дедупа: одно и то же логическое событие схлопывается в окне.
pub fn dedupe_key(event: &AlertEvent) -> String {
    match event {
        AlertEvent::ProcessExited { project, .. } => format!("process_exited:{project}"),
        AlertEvent::PortConflict { port } => format!("port_conflict:{port}"),
        AlertEvent::AgentFailed { run_id } => format!("agent_failed:{run_id}"),
        AlertEvent::TestsFailed { run_id } => format!("tests_failed:{run_id}"),
    }
}

/// Решение политики доставки (#21). `emit` — показать в UI / передать дальше.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeliveryDecision {
    pub emit: bool,
    pub speak: bool,
    pub notify: bool,
}

impl DeliveryDecision {
    pub const SUPPRESSED: Self = Self {
        emit: false,
        speak: false,
        notify: false,
    };
}

fn parse_hhmm(s: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(s.trim(), "%H:%M").ok()
}

/// Тихие часы в локальных минутах от полуночи. Окно через полночь: start > end.
pub fn is_quiet_hours_local(now: NaiveTime, start: NaiveTime, end: NaiveTime) -> bool {
    if start == end {
        return false;
    }
    if start < end {
        now >= start && now < end
    } else {
        now >= start || now < end
    }
}

/// Тихие часы для `now` в заданной IANA TZ и конфиге `HH:MM`.
pub fn is_quiet_hours(now: DateTime<Utc>, timezone: &str, quiet: &QuietHoursConfig) -> bool {
    if !quiet.enabled {
        return false;
    }
    let Some(start) = parse_hhmm(&quiet.start) else {
        return false;
    };
    let Some(end) = parse_hhmm(&quiet.end) else {
        return false;
    };
    let Ok(tz) = Tz::from_str(timezone) else {
        return false;
    };
    let local = now.with_timezone(&tz).time();
    is_quiet_hours_local(local, start, end)
}

/// Stateful-ворота: дедуп 10 мин, голос ≤1/мин, тихие часы Europe/Berlin.
#[derive(Debug, Clone)]
pub struct DeliveryGate {
    dedupe_window: Duration,
    voice_min_interval: Duration,
    quiet_hours: QuietHoursConfig,
    timezone: String,
    last_emitted: HashMap<String, DateTime<Utc>>,
    last_voice_at: Option<DateTime<Utc>>,
}

impl DeliveryGate {
    pub fn from_config(cfg: &AlertsConfig) -> Self {
        Self {
            dedupe_window: Duration::from_secs(cfg.dedupe_window_secs),
            voice_min_interval: Duration::from_secs(cfg.voice_min_interval_secs),
            quiet_hours: cfg.quiet_hours.clone(),
            timezone: cfg.timezone.clone(),
            last_emitted: HashMap::new(),
            last_voice_at: None,
        }
    }

    pub fn new_default() -> Self {
        Self::from_config(&AlertsConfig::default())
    }

    /// Применяет дедуп / тихие часы / троттлинг голоса к кандидату.
    pub fn decide(&mut self, candidate: &AlertCandidate, now: DateTime<Utc>) -> DeliveryDecision {
        let key = dedupe_key(&candidate.event);
        if let Some(prev) = self.last_emitted.get(&key) {
            let elapsed = now
                .signed_duration_since(*prev)
                .to_std()
                .unwrap_or(Duration::ZERO);
            if elapsed < self.dedupe_window {
                return DeliveryDecision::SUPPRESSED;
            }
        }
        self.last_emitted.insert(key, now);

        let quiet = is_quiet_hours(now, &self.timezone, &self.quiet_hours);
        let mut speak = should_speak(candidate.severity);
        let mut notify = should_notify(candidate.severity);

        if quiet {
            speak = false;
            if candidate.severity != Severity::Critical {
                notify = false;
            }
        }

        if speak
            && let Some(last) = self.last_voice_at
        {
            let elapsed = now
                .signed_duration_since(last)
                .to_std()
                .unwrap_or(Duration::ZERO);
            if elapsed < self.voice_min_interval {
                speak = false;
            }
        }
        if speak {
            self.last_voice_at = Some(now);
        }

        DeliveryDecision {
            emit: true,
            speak,
            notify,
        }
    }
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
        assert_eq!(back.dedupe_window_secs, 600);
        assert_eq!(back.voice_min_interval_secs, 60);
        assert_eq!(back.timezone, "Europe/Berlin");
        assert!(back.quiet_hours.enabled);
        assert_eq!(back.quiet_hours.start, "23:00");
        assert_eq!(back.quiet_hours.end, "08:00");
    }

    #[test]
    fn missing_delivery_fields_use_defaults() {
        let cfg: AlertsConfig = serde_json::from_str(r#"{"rules":[]}"#).unwrap();
        assert_eq!(cfg.dedupe_window_secs, 600);
        assert_eq!(cfg.timezone, "Europe/Berlin");
        assert_eq!(cfg.quiet_hours.start, "23:00");
    }

    fn candidate_port(port: u16) -> AlertCandidate {
        let e = AlertEvent::PortConflict { port };
        evaluate(&default_rules(), &e).expect("candidate")
    }

    fn candidate_agent(run_id: &str) -> AlertCandidate {
        let e = AlertEvent::AgentFailed {
            run_id: run_id.into(),
        };
        evaluate(&default_rules(), &e).expect("candidate")
    }

    fn berlin_utc(ymd: (i32, u32, u32), hhmm: (u32, u32)) -> DateTime<Utc> {
        use chrono::TimeZone;
        let tz: chrono_tz::Tz = "Europe/Berlin".parse().unwrap();
        tz.with_ymd_and_hms(ymd.0, ymd.1, ymd.2, hhmm.0, hhmm.1, 0)
            .single()
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn quiet_hours_cross_midnight_berlin() {
        let start = NaiveTime::from_hms_opt(23, 0, 0).unwrap();
        let end = NaiveTime::from_hms_opt(8, 0, 0).unwrap();
        assert!(is_quiet_hours_local(
            NaiveTime::from_hms_opt(23, 30, 0).unwrap(),
            start,
            end
        ));
        assert!(is_quiet_hours_local(
            NaiveTime::from_hms_opt(3, 0, 0).unwrap(),
            start,
            end
        ));
        assert!(!is_quiet_hours_local(
            NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
            start,
            end
        ));
        assert!(!is_quiet_hours_local(
            NaiveTime::from_hms_opt(12, 0, 0).unwrap(),
            start,
            end
        ));

        let cfg = QuietHoursConfig::default();
        assert!(is_quiet_hours(
            berlin_utc((2026, 10, 30), (23, 15)),
            "Europe/Berlin",
            &cfg
        ));
        assert!(!is_quiet_hours(
            berlin_utc((2026, 10, 30), (12, 0)),
            "Europe/Berlin",
            &cfg
        ));
    }

    #[test]
    fn dedupe_collapses_same_event_within_10_min() {
        let mut gate = DeliveryGate::new_default();
        let c = candidate_port(5173);
        let t0 = berlin_utc((2026, 10, 30), (12, 0));
        let d1 = gate.decide(&c, t0);
        assert!(d1.emit);
        assert!(d1.speak);
        assert!(d1.notify);

        let d2 = gate.decide(&c, t0 + chrono::Duration::minutes(5));
        assert_eq!(d2, DeliveryDecision::SUPPRESSED);

        let d3 = gate.decide(&c, t0 + chrono::Duration::minutes(10));
        assert!(d3.emit);
    }

    #[test]
    fn different_dedupe_keys_are_independent() {
        let mut gate = DeliveryGate::new_default();
        let t0 = berlin_utc((2026, 10, 30), (12, 0));
        assert!(gate.decide(&candidate_port(5173), t0).emit);
        assert!(gate.decide(&candidate_port(3000), t0).emit);
    }

    #[test]
    fn voice_throttled_to_once_per_minute() {
        let mut gate = DeliveryGate::new_default();
        let t0 = berlin_utc((2026, 10, 30), (12, 0));
        let a = gate.decide(&candidate_port(5173), t0);
        assert!(a.speak);
        let b = gate.decide(&candidate_port(3000), t0 + chrono::Duration::seconds(30));
        assert!(b.emit);
        assert!(!b.speak);
        assert!(b.notify);
        let c = gate.decide(&candidate_port(8080), t0 + chrono::Duration::seconds(60));
        assert!(c.speak);
    }

    #[test]
    fn quiet_hours_mute_voice_notify_only_critical() {
        let mut gate = DeliveryGate::new_default();
        let night = berlin_utc((2026, 10, 30), (23, 30));

        let critical = gate.decide(&candidate_port(5173), night);
        assert!(critical.emit);
        assert!(!critical.speak);
        assert!(critical.notify);

        let important = gate.decide(&candidate_agent("r1"), night);
        assert!(important.emit);
        assert!(!important.speak);
        assert!(!important.notify);
    }

    #[test]
    fn quiet_hours_disabled_keeps_daytime_flags() {
        let mut cfg = AlertsConfig::default();
        cfg.quiet_hours.enabled = false;
        let mut gate = DeliveryGate::from_config(&cfg);
        let night = berlin_utc((2026, 10, 30), (23, 30));
        let d = gate.decide(&candidate_port(5173), night);
        assert!(d.speak);
        assert!(d.notify);
    }

    #[test]
    fn dedupe_key_stable_for_process() {
        let e = AlertEvent::ProcessExited {
            project: "api".into(),
            code: 1,
        };
        assert_eq!(dedupe_key(&e), "process_exited:api");
        let e2 = AlertEvent::ProcessExited {
            project: "api".into(),
            code: 2,
        };
        assert_eq!(dedupe_key(&e), dedupe_key(&e2));
    }
}
