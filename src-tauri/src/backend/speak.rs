//! Голосовые алерты через macOS `say` (#23).
//!
//! Формирование фразы и решение `DeliveryDecision.speak` — чистая логика.
//! Вызов `say` вынесен за трейт `VoiceSpeaker`, чтобы мокать в тестах.
//! Троттлинг ≤1/мин и тихие часы (speak off) — в `DeliveryGate` (#21).

use serde::{Deserialize, Serialize};
use std::io;
use std::process::{Command, Stdio};

use super::alerts::{AlertCandidate, AlertEvent, DeliveryDecision};

/// Имя голоса и шаблоны фраз в `settings.json` → `alerts.voice`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceConfig {
    /// Голос `say -v` (по умолчанию Milena).
    #[serde(default = "default_voice_name")]
    pub name: String,
    /// Имя пользователя для `{user}` в фразах (#99).
    #[serde(default = "default_user_name")]
    pub user_name: String,
    /// Как произносить имя в `say` (если пусто — `user_name`).
    #[serde(default)]
    pub user_pronunciation: String,
    #[serde(default)]
    pub phrases: VoicePhrases,
}

impl VoiceConfig {
    /// Имя, которое пойдёт в `say`.
    pub fn spoken_name(&self) -> &str {
        let p = self.user_pronunciation.trim();
        if p.is_empty() {
            self.user_name.trim()
        } else {
            p
        }
    }
}

fn default_user_name() -> String {
    "Олег".into()
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            name: default_voice_name(),
            user_name: default_user_name(),
            user_pronunciation: String::new(),
            phrases: VoicePhrases::default(),
        }
    }
}

fn default_voice_name() -> String {
    "Milena".into()
}

/// Короткие шаблоны без повторов; плейсхолдеры `{project}`, `{code}`, `{port}`, `{run_id}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoicePhrases {
    #[serde(default = "default_phrase_process_exited")]
    pub process_exited: String,
    #[serde(default = "default_phrase_port_conflict")]
    pub port_conflict: String,
    #[serde(default = "default_phrase_agent_failed")]
    pub agent_failed: String,
    #[serde(default = "default_phrase_tests_failed")]
    pub tests_failed: String,
}

impl Default for VoicePhrases {
    fn default() -> Self {
        Self {
            process_exited: default_phrase_process_exited(),
            port_conflict: default_phrase_port_conflict(),
            agent_failed: default_phrase_agent_failed(),
            tests_failed: default_phrase_tests_failed(),
        }
    }
}

fn default_phrase_process_exited() -> String {
    "{user}, внимание. {project} упал, код {code}.".into()
}

fn default_phrase_port_conflict() -> String {
    "{user}, внимание. Порт {port} занят.".into()
}

fn default_phrase_agent_failed() -> String {
    "{user}, внимание. Агент {run_id} не справился.".into()
}

fn default_phrase_tests_failed() -> String {
    "{user}, внимание. Тесты {run_id} упали.".into()
}

/// Готовый вызов для `say`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeakPayload {
    pub voice: String,
    pub phrase: String,
}

/// Подставляет плейсхолдеры события в шаблон (один проход, без повторов фразы).
pub fn render_phrase(template: &str, event: &AlertEvent) -> String {
    match event {
        AlertEvent::ProcessExited { project, code } => template
            .replace("{project}", project)
            .replace("{code}", &code.to_string()),
        AlertEvent::PortConflict { port } => template.replace("{port}", &port.to_string()),
        AlertEvent::AgentFailed { run_id } => template.replace("{run_id}", run_id),
        AlertEvent::TestsFailed { run_id } => template.replace("{run_id}", run_id),
    }
}

/// Выбирает шаблон по виду события.
pub fn phrase_template<'a>(phrases: &'a VoicePhrases, event: &AlertEvent) -> &'a str {
    match event {
        AlertEvent::ProcessExited { .. } => phrases.process_exited.as_str(),
        AlertEvent::PortConflict { .. } => phrases.port_conflict.as_str(),
        AlertEvent::AgentFailed { .. } => phrases.agent_failed.as_str(),
        AlertEvent::TestsFailed { .. } => phrases.tests_failed.as_str(),
    }
}

/// Собирает payload из кандидата и конфига голоса (без проверки `speak`).
pub fn build_payload(candidate: &AlertCandidate, voice: &VoiceConfig) -> SpeakPayload {
    let template = phrase_template(&voice.phrases, &candidate.event);
    SpeakPayload {
        voice: voice.name.clone(),
        phrase: render_phrase(&template.replace("{user}", voice.spoken_name()), &candidate.event),
    }
}

/// Учитывает флаг `speak` из `DeliveryGate` (severity / троттлинг / тихие часы).
pub fn maybe_speak(
    candidate: &AlertCandidate,
    decision: &DeliveryDecision,
    voice: &VoiceConfig,
) -> Option<SpeakPayload> {
    if !decision.speak {
        return None;
    }
    Some(build_payload(candidate, voice))
}

/// Вызов голосового движка (мок в тестах).
pub trait VoiceSpeaker {
    fn speak(&self, voice: &str, phrase: &str) -> io::Result<()>;
}

/// Реальный `say -v <voice> <phrase>` (неблокирующий spawn).
#[derive(Debug, Default, Clone, Copy)]
pub struct SaySpeaker;

impl VoiceSpeaker for SaySpeaker {
    fn speak(&self, voice: &str, phrase: &str) -> io::Result<()> {
        Command::new("say")
            .arg("-v")
            .arg(voice)
            .arg(phrase)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| ())
    }
}

/// Доставляет голос через speaker; `Ok(true)` если был вызов.
pub fn deliver_speak<S: VoiceSpeaker>(
    candidate: &AlertCandidate,
    decision: &DeliveryDecision,
    voice: &VoiceConfig,
    speaker: &S,
) -> io::Result<bool> {
    let Some(payload) = maybe_speak(candidate, decision, voice) else {
        return Ok(false);
    };
    speaker.speak(&payload.voice, &payload.phrase)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::alerts::{
        default_rules, evaluate, DeliveryGate, Severity,
    };
    use chrono::{TimeZone, Utc};
    use std::sync::Mutex;

    #[test]
    fn process_exit_phrase_fills_project_and_code() {
        let e = AlertEvent::ProcessExited {
            project: "api".into(),
            code: 1,
        };
        let c = evaluate(&default_rules(), &e).expect("candidate");
        let p = build_payload(&c, &VoiceConfig::default());
        assert_eq!(p.voice, "Milena");
        assert_eq!(p.phrase, "Олег, внимание. api упал, код 1.");
        assert!(!p.phrase.contains("{project}"));
        assert!(!p.phrase.contains("{code}"));
    }

    #[test]
    fn port_conflict_phrase() {
        let e = AlertEvent::PortConflict { port: 5173 };
        let c = evaluate(&default_rules(), &e).expect("candidate");
        let p = build_payload(&c, &VoiceConfig::default());
        assert_eq!(p.phrase, "Олег, внимание. Порт 5173 занят.");
    }

    #[test]
    fn user_name_and_pronunciation_are_substituted() {
        let e = AlertEvent::PortConflict { port: 5173 };
        let c = evaluate(&default_rules(), &e).expect("candidate");
        let mut v = VoiceConfig::default();
        v.user_name = "Anna".into();
        assert_eq!(build_payload(&c, &v).phrase, "Anna, внимание. Порт 5173 занят.");
        v.user_pronunciation = "Анна".into();
        assert_eq!(build_payload(&c, &v).phrase, "Анна, внимание. Порт 5173 занят.");
    }

    #[test]
    fn custom_template_and_voice() {
        let voice = VoiceConfig {
            name: "Yuri".into(),
            user_name: "Олег".into(),
            user_pronunciation: String::new(),
            phrases: VoicePhrases {
                process_exited: "{project} код {code}".into(),
                ..VoicePhrases::default()
            },
        };
        let e = AlertEvent::ProcessExited {
            project: "web".into(),
            code: 137,
        };
        let c = evaluate(&default_rules(), &e).expect("candidate");
        let p = build_payload(&c, &voice);
        assert_eq!(p.voice, "Yuri");
        assert_eq!(p.phrase, "web код 137");
    }

    #[test]
    fn maybe_speak_respects_decision_flag() {
        let e = AlertEvent::PortConflict { port: 3000 };
        let c = evaluate(&default_rules(), &e).expect("candidate");
        let voice = VoiceConfig::default();
        assert!(maybe_speak(
            &c,
            &DeliveryDecision {
                emit: true,
                speak: true,
                notify: true,
            },
            &voice,
        )
        .is_some());
        assert!(maybe_speak(&c, &DeliveryDecision::SUPPRESSED, &voice).is_none());
        assert!(maybe_speak(
            &c,
            &DeliveryDecision {
                emit: true,
                speak: false,
                notify: true,
            },
            &voice,
        )
        .is_none());
    }

    #[test]
    fn important_never_speaks_via_gate_defaults() {
        let e = AlertEvent::AgentFailed {
            run_id: "r1".into(),
        };
        let c = evaluate(&default_rules(), &e).expect("candidate");
        assert_eq!(c.severity, Severity::Important);
        let mut gate = DeliveryGate::new_default();
        let d = gate.decide(&c, Utc::now());
        assert!(!d.speak);
        assert!(maybe_speak(&c, &d, &VoiceConfig::default()).is_none());
    }

    fn berlin_night() -> chrono::DateTime<Utc> {
        let tz: chrono_tz::Tz = "Europe/Berlin".parse().unwrap();
        tz.with_ymd_and_hms(2026, 10, 30, 23, 30, 0)
            .single()
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn quiet_hours_gate_skips_speak_even_for_critical() {
        let mut gate = DeliveryGate::new_default();
        let night = berlin_night();
        let critical = evaluate(
            &default_rules(),
            &AlertEvent::PortConflict { port: 8080 },
        )
        .expect("candidate");
        let d = gate.decide(&critical, night);
        assert!(!d.speak);
        assert!(d.notify);
        assert!(maybe_speak(&critical, &d, &VoiceConfig::default()).is_none());
    }

    #[test]
    fn voice_throttle_second_speak_skipped() {
        let mut gate = DeliveryGate::new_default();
        let day = {
            let tz: chrono_tz::Tz = "Europe/Berlin".parse().unwrap();
            tz.with_ymd_and_hms(2026, 10, 30, 12, 0, 0)
                .single()
                .unwrap()
                .with_timezone(&Utc)
        };
        let a = evaluate(
            &default_rules(),
            &AlertEvent::PortConflict { port: 1 },
        )
        .expect("a");
        let b = evaluate(
            &default_rules(),
            &AlertEvent::PortConflict { port: 2 },
        )
        .expect("b");
        let d1 = gate.decide(&a, day);
        let d2 = gate.decide(&b, day);
        assert!(d1.speak);
        assert!(!d2.speak);
        assert!(maybe_speak(&a, &d1, &VoiceConfig::default()).is_some());
        assert!(maybe_speak(&b, &d2, &VoiceConfig::default()).is_none());
    }

    struct RecordingSpeaker {
        calls: Mutex<Vec<(String, String)>>,
    }

    impl RecordingSpeaker {
        fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<(String, String)> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl VoiceSpeaker for RecordingSpeaker {
        fn speak(&self, voice: &str, phrase: &str) -> io::Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push((voice.to_string(), phrase.to_string()));
            Ok(())
        }
    }

    #[test]
    fn deliver_speak_uses_trait_not_real_say() {
        let e = AlertEvent::ProcessExited {
            project: "demo".into(),
            code: 2,
        };
        let c = evaluate(&default_rules(), &e).expect("candidate");
        let decision = DeliveryDecision {
            emit: true,
            speak: true,
            notify: true,
        };
        let speaker = RecordingSpeaker::new();
        assert!(deliver_speak(&c, &decision, &VoiceConfig::default(), &speaker).unwrap());
        let calls = speaker.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "Milena");
        assert_eq!(calls[0].1, "Олег, внимание. demo упал, код 2.");
    }

    #[test]
    fn deliver_speak_skips_when_gate_says_no() {
        let e = AlertEvent::PortConflict { port: 9 };
        let c = evaluate(&default_rules(), &e).expect("candidate");
        let speaker = RecordingSpeaker::new();
        assert!(!deliver_speak(
            &c,
            &DeliveryDecision {
                emit: true,
                speak: false,
                notify: true,
            },
            &VoiceConfig::default(),
            &speaker,
        )
        .unwrap());
        assert!(speaker.calls().is_empty());
    }

    #[test]
    fn voice_config_missing_fields_default() {
        let v: VoiceConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(v.name, "Milena");
        assert_eq!(
            v.phrases.process_exited,
            default_phrase_process_exited()
        );
    }
}
