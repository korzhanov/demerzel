//! Автоисправление по алерту (#96).
//!
//! Режимы: `off` / `suggest` / `auto` (settings → `agents.autofix`, override в
//! `orchestrator.json` → `agents.autofix`). Из алерта (crash / port conflict /
//! tests failed) — агент в worktree с контекстом (хвост логов + команда + diff).
//! Лимитер и ревью как у `start_agent_from_task` (#27–#33): результат →
//! `needs_review` / `failed`, не в main.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use super::agents::{
    capture_diff, complete_agent_run, create_worktree, new_run_id, now_rfc3339, project_slug,
    queue_reason_message, resolve_runner_for_project, validate_segment, AgentLimiter,
    AgentRunRecord, AgentRunRegistry, AgentRunner, AutofixMode, MemoryProbe, OllamaProbe,
    RunStatus, StartDecision,
};
use super::alerts::{AlertCandidate, AlertEvent, EventKind};
use super::config;
use super::logs::{LogLine, Stream};

/// Сколько последних строк логов класть в промпт.
pub const DEFAULT_LOG_TAIL_LINES: usize = 80;

/// Контекст для промпта автофикса.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertAutofixContext {
    pub project_path: String,
    pub project: String,
    pub message: String,
    pub kind: EventKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_tail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
}

/// Предложение «исправить» в режиме suggest (#96).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutofixOffer {
    pub mode: AutofixMode,
    pub project: String,
    pub project_path: String,
    pub message: String,
    pub kind: EventKind,
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

/// Результат запуска агента из алерта (#96).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartAgentFromAlertResult {
    pub run: AgentRunRecord,
    pub queued: bool,
    pub queue_reason: Option<String>,
    pub mode: AutofixMode,
}

/// Событие UI при предложении автофикса.
pub const AUTOFIX_OFFER_EVENT: &str = "alerts:autofix-offer";

/// Краш / конфликт порта / упавшие тесты — да; `agent_failed` — нет (петля).
pub fn autofix_eligible(event: &AlertEvent) -> bool {
    match event {
        AlertEvent::ProcessExited { code, .. } => *code != 0,
        AlertEvent::PortConflict { .. } => true,
        AlertEvent::TestsFailed { .. } => true,
        AlertEvent::AgentFailed { .. } => false,
    }
}

/// Проектный `agents.autofix` перекрывает settings; иначе settings; иначе Off.
pub fn resolve_autofix_mode(project_path: Option<&Path>, settings_mode: AutofixMode) -> AutofixMode {
    if let Some(path) = project_path
        && let Some(raw) = config::load_from_dir(path)
            .and_then(|c| c.agents)
            .and_then(|a| a.autofix)
            && let Ok(mode) = AutofixMode::parse(&raw) {
                return mode;
            }
    settings_mode
}

/// Хвост логов: последние `max_lines` с префиксом потока.
pub fn format_log_tail(lines: &[LogLine], max_lines: usize) -> String {
    if lines.is_empty() || max_lines == 0 {
        return String::new();
    }
    let start = lines.len().saturating_sub(max_lines);
    lines[start..]
        .iter()
        .map(|l| {
            let tag = match l.stream {
                Stream::Stdout => "out",
                Stream::Stderr => "err",
            };
            format!("{tag}| {}", l.text)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Промпт агента: задача + команда + хвост логов + diff.
pub fn build_alert_autofix_prompt(ctx: &AlertAutofixContext) -> String {
    let mut out = String::new();
    out.push_str("# Task\n");
    out.push_str("Auto-fix from Demerzel alert. Investigate and fix the failure.\n");
    out.push_str("Do not merge to main — leave changes in this worktree for human review.\n\n");
    out.push_str(&format!("Alert: {}\n", ctx.message.trim()));
    out.push_str(&format!("Kind: {}\n", event_kind_str(ctx.kind)));
    out.push_str(&format!("Project: {}\n", ctx.project));

    if let Some(cmd) = ctx.command.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        out.push_str("\n## Command\n```\n");
        out.push_str(cmd);
        out.push_str("\n```\n");
    }
    if let Some(logs) = ctx.log_tail.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        out.push_str("\n## Log tail\n```\n");
        out.push_str(logs);
        out.push_str("\n```\n");
    }
    if let Some(diff) = ctx.diff.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        out.push_str("\n## Diff\n```diff\n");
        out.push_str(diff);
        out.push_str("\n```\n");
    }
    out
}

fn event_kind_str(kind: EventKind) -> &'static str {
    match kind {
        EventKind::ProcessExited => "process_exited",
        EventKind::PortConflict => "port_conflict",
        EventKind::AgentFailed => "agent_failed",
        EventKind::TestsFailed => "tests_failed",
    }
}

/// Решение после матча алерта: предложить / автозапуск / ничего.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutofixDecision {
    Skip,
    Suggest(AutofixOffer),
    AutoStart(AlertAutofixContext),
}

/// Учитывает режим и eligibility; не стартует агента.
pub fn decide_autofix(
    candidate: &AlertCandidate,
    ctx: Option<AlertAutofixContext>,
    mode: AutofixMode,
) -> AutofixDecision {
    if mode == AutofixMode::Off || !autofix_eligible(&candidate.event) {
        return AutofixDecision::Skip;
    }
    let Some(ctx) = ctx else {
        return AutofixDecision::Skip;
    };
    match mode {
        AutofixMode::Off => AutofixDecision::Skip,
        AutofixMode::Suggest => AutofixDecision::Suggest(AutofixOffer {
            mode: AutofixMode::Suggest,
            project: ctx.project.clone(),
            project_path: ctx.project_path.clone(),
            message: ctx.message.clone(),
            kind: ctx.kind,
            prompt: build_alert_autofix_prompt(&ctx),
            command: ctx.command.clone(),
        }),
        AutofixMode::Auto => AutofixDecision::AutoStart(ctx),
    }
}

/// Параметры `prepare_agent_from_alert` / `start_agent_from_alert`.
pub struct StartAlertAgentParams<'a, M: MemoryProbe, O: OllamaProbe> {
    pub project_path: &'a Path,
    pub prompt: &'a str,
    pub registry: &'a mut AgentRunRegistry,
    pub limiter: &'a mut AgentLimiter<M, O>,
    pub timeout: Duration,
    pub base_ref: Option<&'a str>,
    pub cancel: Option<&'a AtomicBool>,
    pub run_id: Option<String>,
    pub runner: AgentRunner,
    pub ollama_model: Option<&'a str>,
    /// Команда ACP-агента, если `runner == Acp`.
    pub acp_command: Option<&'a str>,
    pub mode: AutofixMode,
}

/// Готовит run из алерта: limiter + worktree; без смены статуса задачи.
pub fn prepare_agent_from_alert<M: MemoryProbe, O: OllamaProbe>(
    project_path: &Path,
    registry: &mut AgentRunRegistry,
    limiter: &mut AgentLimiter<M, O>,
    base_ref: Option<&str>,
    run_id: Option<String>,
    runner: AgentRunner,
    mode: AutofixMode,
) -> Result<StartAgentFromAlertResult, String> {
    if !project_path.is_dir() {
        return Err(format!(
            "project_path is not a directory: {}",
            project_path.display()
        ));
    }

    let project = project_slug(project_path)?;
    let id = run_id.unwrap_or_else(new_run_id);
    validate_segment("run_id", &id)?;
    let base = base_ref.unwrap_or("HEAD").trim().to_string();
    if base.is_empty() {
        return Err("base_ref must not be empty".into());
    }

    let mut record = AgentRunRecord {
        id: id.clone(),
        project: project.clone(),
        project_path: project_path
            .to_str()
            .ok_or("project_path is not UTF-8")?
            .to_string(),
        task_id: None,
        status: RunStatus::Queued,
        worktree: None,
        base_ref: base.clone(),
        runner,
        tests: None,
        diff: None,
        started_at: Some(now_rfc3339()),
        finished_at: None,
        token_usage: None,
        review: None,
    };
    registry.upsert(record.clone())?;

    match limiter.try_acquire(&id)? {
        StartDecision::Queue(reason) => {
            return Ok(StartAgentFromAlertResult {
                run: record,
                queued: true,
                queue_reason: Some(queue_reason_message(&reason)),
                mode,
            });
        }
        StartDecision::Start => {}
    }

    let handle = match create_worktree(project_path, &project, &id, Some(&base)) {
        Ok(h) => h,
        Err(e) => {
            limiter.release(&id);
            record.status = RunStatus::Failed;
            record.finished_at = Some(now_rfc3339());
            let _ = registry.upsert(record);
            return Err(e);
        }
    };
    record.status = RunStatus::Running;
    record.worktree = Some(handle.path.display().to_string());
    registry.upsert(record.clone())?;

    Ok(StartAgentFromAlertResult {
        run: record,
        queued: false,
        queue_reason: None,
        mode,
    })
}

/// Полный sync-путь: prepare + complete. Accept в main не делает.
pub fn start_agent_from_alert<M: MemoryProbe, O: OllamaProbe>(
    params: StartAlertAgentParams<'_, M, O>,
) -> Result<StartAgentFromAlertResult, String> {
    let StartAlertAgentParams {
        project_path,
        prompt,
        registry,
        limiter,
        timeout,
        base_ref,
        cancel,
        run_id,
        runner,
        ollama_model,
        acp_command,
        mode,
    } = params;
    if prompt.trim().is_empty() {
        return Err("autofix prompt must not be empty".into());
    }
    let prepared = prepare_agent_from_alert(
        project_path,
        registry,
        limiter,
        base_ref,
        run_id,
        runner,
        mode,
    )?;
    if prepared.queued {
        return Ok(prepared);
    }
    let run = complete_agent_run(
        &prepared.run.id,
        prompt,
        registry,
        limiter,
        timeout,
        cancel,
        ollama_model,
        acp_command,
    )?;
    Ok(StartAgentFromAlertResult {
        run,
        queued: false,
        queue_reason: None,
        mode,
    })
}

/// Diff рабочего дерева проекта (для контекста); пустая строка при ошибке git.
pub fn optional_project_diff(project_path: &Path) -> Option<String> {
    capture_diff(project_path, "HEAD")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Собирает контекст из пути + опциональных логов/команды/diff.
pub fn context_from_parts(
    project_path: &Path,
    event: &AlertEvent,
    command: Option<String>,
    logs: Option<&[LogLine]>,
    diff: Option<String>,
) -> Result<AlertAutofixContext, String> {
    let project = project_slug(project_path)?;
    let path_s = project_path
        .to_str()
        .ok_or("project_path is not UTF-8")?
        .to_string();
    let log_tail = logs
        .map(|l| format_log_tail(l, DEFAULT_LOG_TAIL_LINES))
        .filter(|s| !s.is_empty());
    let diff = diff.or_else(|| optional_project_diff(project_path));
    Ok(AlertAutofixContext {
        project_path: path_s,
        project,
        message: event.message(),
        kind: event.kind(),
        command,
        log_tail,
        diff,
    })
}

/// Резолв runner с учётом проекта (зеркало agents).
pub fn runner_for_autofix(project_path: &Path, settings_default: AgentRunner) -> AgentRunner {
    resolve_runner_for_project(project_path, settings_default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::agents::AutofixMode;
    use crate::backend::alerts::{default_rules, evaluate, AlertEvent};
    use crate::backend::logs::{LogLine, Stream};
    use std::fs;

    #[test]
    fn eligible_kinds() {
        assert!(autofix_eligible(&AlertEvent::ProcessExited {
            project: "a".into(),
            code: 1,
        }));
        assert!(!autofix_eligible(&AlertEvent::ProcessExited {
            project: "a".into(),
            code: 0,
        }));
        assert!(autofix_eligible(&AlertEvent::PortConflict { port: 5173 }));
        assert!(autofix_eligible(&AlertEvent::TestsFailed {
            run_id: "r1".into()
        }));
        assert!(!autofix_eligible(&AlertEvent::AgentFailed {
            run_id: "r1".into()
        }));
    }

    #[test]
    fn mode_parse_and_default_off() {
        assert_eq!(AutofixMode::default(), AutofixMode::Off);
        assert_eq!(AutofixMode::parse("suggest").unwrap(), AutofixMode::Suggest);
        assert_eq!(AutofixMode::parse("auto").unwrap(), AutofixMode::Auto);
        assert_eq!(AutofixMode::parse("off").unwrap(), AutofixMode::Off);
        assert!(AutofixMode::parse("maybe").is_err());
    }

    #[test]
    fn resolve_mode_project_overrides_settings() {
        let dir = std::env::temp_dir().join(format!(
            "demerzel-autofix-mode-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("orchestrator.json"),
            r#"{ "name": "demo", "type": "app", "agents": { "autofix": "auto" } }"#,
        )
        .unwrap();
        assert_eq!(
            resolve_autofix_mode(Some(&dir), AutofixMode::Suggest),
            AutofixMode::Auto
        );
        assert_eq!(
            resolve_autofix_mode(None, AutofixMode::Suggest),
            AutofixMode::Suggest
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn log_tail_keeps_last_n() {
        let lines: Vec<_> = (0..5)
            .map(|i| LogLine {
                stream: if i % 2 == 0 {
                    Stream::Stdout
                } else {
                    Stream::Stderr
                },
                text: format!("L{i}"),
            })
            .collect();
        let tail = format_log_tail(&lines, 2);
        assert_eq!(tail, "err| L3\nout| L4");
    }

    #[test]
    fn prompt_includes_command_logs_diff() {
        let ctx = AlertAutofixContext {
            project_path: "/tmp/demo".into(),
            project: "demo".into(),
            message: "process demo exited with code 1".into(),
            kind: EventKind::ProcessExited,
            command: Some("bun run dev".into()),
            log_tail: Some("err| boom".into()),
            diff: Some("-a\n+b".into()),
        };
        let p = build_alert_autofix_prompt(&ctx);
        assert!(p.contains("Auto-fix"));
        assert!(p.contains("bun run dev"));
        assert!(p.contains("err| boom"));
        assert!(p.contains("-a\n+b"));
        assert!(p.contains("human review"));
    }

    #[test]
    fn decide_suggest_and_skip() {
        let e = AlertEvent::PortConflict { port: 3000 };
        let c = evaluate(&default_rules(), &e).unwrap();
        let ctx = AlertAutofixContext {
            project_path: "/p".into(),
            project: "p".into(),
            message: e.message(),
            kind: e.kind(),
            command: None,
            log_tail: None,
            diff: None,
        };
        assert_eq!(
            decide_autofix(&c, Some(ctx.clone()), AutofixMode::Off),
            AutofixDecision::Skip
        );
        match decide_autofix(&c, Some(ctx.clone()), AutofixMode::Suggest) {
            AutofixDecision::Suggest(o) => {
                assert_eq!(o.mode, AutofixMode::Suggest);
                assert!(o.prompt.contains("port 3000"));
            }
            other => panic!("expected suggest, got {other:?}"),
        }
        match decide_autofix(&c, Some(ctx), AutofixMode::Auto) {
            AutofixDecision::AutoStart(_) => {}
            other => panic!("expected auto, got {other:?}"),
        }
        assert_eq!(
            decide_autofix(&c, None, AutofixMode::Auto),
            AutofixDecision::Skip
        );
    }

}
