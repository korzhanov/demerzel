//! Клиент Agent Client Protocol (ACP, https://zed.dev/acp): JSON-RPC 2.0 поверх stdio.
//!
//! Допущения: одно сообщение — одна строка JSON (построчный фрейминг); методы
//! `initialize`, `session/new`, `session/prompt`, `session/update`, `session/request_permission`.
//! Клиент не объявляет возможностей `fs` и `terminal`, поэтому запросы агента к ним
//! получают ответ «method not found». Разрешения: выбирается вариант `allow_once`,
//! иначе ход отменяется.

use std::io::{BufRead, Write};
#[cfg(unix)]
use std::io::{BufReader, Read};
#[cfg(unix)]
use std::path::Path;
#[cfg(unix)]
use std::process::{Command, Stdio};
#[cfg(unix)]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(unix)]
use std::sync::{mpsc, Arc, Mutex};
#[cfg(unix)]
use std::thread;
#[cfg(unix)]
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::agents::ClaudeRunResult;

/// Версия протокола, которую поддерживает клиент.
pub const PROTOCOL_VERSION: u64 = 1;

const ID_INITIALIZE: u64 = 1;
const ID_NEW_SESSION: u64 = 2;
const ID_PROMPT: u64 = 3;

/// Результат хода: собранный текст ответа и причина остановки (`end_turn`, `cancelled`, ...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptOutcome {
    pub text: String,
    pub stop_reason: String,
}

/// Входящее сообщение агента.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    /// Ответ на наш запрос.
    Response { id: u64, result: Option<Value>, error: Option<Value> },
    /// Уведомление без ответа (например, `session/update`).
    Notification { method: String, params: Value },
    /// Запрос агента к клиенту (например, `session/request_permission`); на него нужен ответ с тем же id.
    Request { id: Value, method: String, params: Value },
}

pub fn initialize_request() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": ID_INITIALIZE,
        "method": "initialize",
        "params": {
            "protocolVersion": PROTOCOL_VERSION,
            "clientCapabilities": {
                "fs": { "readTextFile": false, "writeTextFile": false },
                "terminal": false
            },
            "clientInfo": { "name": "demerzel", "title": "Demerzel", "version": env!("CARGO_PKG_VERSION") }
        }
    })
}

pub fn new_session_request(cwd: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": ID_NEW_SESSION,
        "method": "session/new",
        "params": { "cwd": cwd, "mcpServers": [] }
    })
}

pub fn prompt_request(session_id: &str, text: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": ID_PROMPT,
        "method": "session/prompt",
        "params": {
            "sessionId": session_id,
            "prompt": [{ "type": "text", "text": text }]
        }
    })
}

/// Выбирает вариант разрешения: только `allow_once`. Остальные варианты (в том числе
/// постоянные) не принимаем автоматически.
pub fn pick_allow_option(options: &[Value]) -> Option<String> {
    options
        .iter()
        .find(|o| o.get("kind").and_then(Value::as_str) == Some("allow_once"))
        .and_then(|o| o.get("optionId").and_then(Value::as_str))
        .map(str::to_string)
}

/// Ответ на `session/request_permission`: выбранный вариант или отмена.
pub fn permission_reply(id: &Value, option_id: Option<&str>) -> Value {
    let outcome = match option_id {
        Some(opt) => json!({ "outcome": "selected", "optionId": opt }),
        None => json!({ "outcome": "cancelled" }),
    };
    json!({ "jsonrpc": "2.0", "id": id, "result": { "outcome": outcome } })
}

/// Ответ на запрос, который клиент не поддерживает.
pub fn method_not_found_reply(id: &Value, method: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": -32601, "message": format!("method not supported by client: {method}") }
    })
}

/// Разбирает строку протокола. Пустые и непонятные строки пропускаются (None).
pub fn parse_line(line: &str) -> Option<Incoming> {
    let v: Value = serde_json::from_str(line.trim()).ok()?;
    let method = v.get("method").and_then(Value::as_str).map(str::to_string);
    match (method, v.get("id")) {
        (Some(method), Some(id)) => Some(Incoming::Request {
            id: id.clone(),
            method,
            params: v.get("params").cloned().unwrap_or(Value::Null),
        }),
        (Some(method), None) => Some(Incoming::Notification {
            method,
            params: v.get("params").cloned().unwrap_or(Value::Null),
        }),
        (None, Some(id)) => Some(Incoming::Response {
            id: id.as_u64()?,
            result: v.get("result").cloned(),
            error: v.get("error").cloned(),
        }),
        (None, None) => None,
    }
}

fn send<W: Write>(writer: &mut W, msg: &Value) -> Result<(), String> {
    let mut line = msg.to_string();
    line.push('\n');
    writer.write_all(line.as_bytes()).map_err(|e| format!("ACP: запись: {e}"))?;
    writer.flush().map_err(|e| format!("ACP: запись: {e}"))
}

/// Читает строки до ответа с нужным id. Уведомления и запросы пока игнорируем:
/// на этапе инициализации агент их не присылает.
fn await_response<R: BufRead>(reader: &mut R, id: u64) -> Result<Value, String> {
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).map_err(|e| format!("ACP: чтение: {e}"))?;
        if n == 0 {
            return Err("ACP: агент завершился до ответа".into());
        }
        if let Some(Incoming::Response { id: got, result, error }) = parse_line(&line) {
            if got != id {
                continue;
            }
            if let Some(err) = error {
                return Err(format!("ACP: ошибка агента: {err}"));
            }
            return Ok(result.unwrap_or(Value::Null));
        }
    }
}

/// Один ход: initialize → session/new → session/prompt. Текст собирается из `agent_message_chunk`.
/// Вызывается на потоке, который читает stdout агента; прерывание — закрытием процесса (EOF).
pub fn run_turn<R: BufRead, W: Write>(
    mut reader: R,
    writer: &mut W,
    cwd: &str,
    prompt: &str,
) -> Result<PromptOutcome, String> {
    send(writer, &initialize_request())?;
    await_response(&mut reader, ID_INITIALIZE)?;

    send(writer, &new_session_request(cwd))?;
    let session = await_response(&mut reader, ID_NEW_SESSION)?;
    let session_id = session
        .get("sessionId")
        .and_then(Value::as_str)
        .ok_or("ACP: в ответе session/new нет sessionId")?
        .to_string();

    send(writer, &prompt_request(&session_id, prompt))?;

    let mut text = String::new();
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).map_err(|e| format!("ACP: чтение: {e}"))?;
        if n == 0 {
            return Err("ACP: агент завершился во время хода".into());
        }
        match parse_line(&line) {
            Some(Incoming::Notification { method, params }) if method == "session/update" => {
                collect_text(&params, &mut text);
            }
            Some(Incoming::Request { id, method, params }) if method == "session/request_permission" => {
                let options = params.get("options").and_then(Value::as_array).cloned().unwrap_or_default();
                let choice = pick_allow_option(&options);
                send(writer, &permission_reply(&id, choice.as_deref()))?;
            }
            Some(Incoming::Request { id, method, .. }) => {
                send(writer, &method_not_found_reply(&id, &method))?;
            }
            Some(Incoming::Response { id, result, error }) if id == ID_PROMPT => {
                if let Some(err) = error {
                    return Err(format!("ACP: ошибка хода: {err}"));
                }
                let stop_reason = result
                    .as_ref()
                    .and_then(|r| r.get("stopReason"))
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
                return Ok(PromptOutcome { text, stop_reason });
            }
            _ => {}
        }
    }
}

/// Добавляет текст из `session/update`, если это фрагмент ответа агента.
fn collect_text(params: &Value, out: &mut String) {
    let Some(update) = params.get("update") else {
        return;
    };
    if update.get("sessionUpdate").and_then(Value::as_str) != Some("agent_message_chunk") {
        return;
    }
    if let Some(t) = update.get("content").and_then(|c| {
        (c.get("type").and_then(Value::as_str) == Some("text"))
            .then(|| c.get("text").and_then(Value::as_str))
            .flatten()
    }) {
        out.push_str(t);
    }
}

/// Запускает ACP-агента по команде из `settings.agents.acp_command` (через `sh -c`) в `worktree`
/// и выполняет один ход с `prompt`. Таймаут и отмена снимают всю process group агента.
/// Результат — в той же форме, что у остальных headless-раннеров: текст ответа в `stdout`.
#[cfg(unix)]
pub fn run_acp(
    command: &str,
    worktree: &Path,
    prompt: &str,
    timeout: Duration,
    cancel: Option<&AtomicBool>,
) -> Result<ClaudeRunResult, String> {
    use std::os::unix::process::CommandExt;

    let command = command.trim();
    if command.is_empty() {
        return Err("не задана команда ACP-агента: settings.json → agents.acp_command".into());
    }
    if !worktree.is_absolute() || !worktree.is_dir() {
        return Err(format!("ACP: worktree должен быть абсолютным каталогом: {}", worktree.display()));
    }
    if prompt.trim().is_empty() {
        return Err("prompt must not be empty".into());
    }

    let started = Instant::now();
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(worktree)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(|e| format!("ACP: запуск `{command}`: {e}"))?;
    let pgid = child.id();

    let stderr_buf = Arc::new(Mutex::new(String::new()));
    if let Some(mut err) = child.stderr.take() {
        let buf = stderr_buf.clone();
        thread::spawn(move || {
            let mut text = String::new();
            let _ = err.read_to_string(&mut text);
            if let Ok(mut b) = buf.lock() {
                b.push_str(&text);
            }
        });
    }
    let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        kill_group(pgid);
        let _ = child.wait();
        return Err("ACP: у процесса нет stdin/stdout".into());
    };

    let (tx, rx) = mpsc::channel();
    let cwd = worktree.display().to_string();
    let task = prompt.to_string();
    thread::spawn(move || {
        let mut stdin = stdin;
        let result = run_turn(BufReader::new(stdout), &mut stdin, &cwd, &task);
        let _ = tx.send(result);
    });

    let deadline = Instant::now() + timeout;
    let mut timed_out = false;
    let mut cancelled = false;
    let turn = loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(result) => break Some(result),
            Err(mpsc::RecvTimeoutError::Disconnected) => break None,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if cancel.is_some_and(|c| c.load(Ordering::SeqCst)) {
            cancelled = true;
            break None;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            break None;
        }
    };

    // Ход завершён или прерван: агент больше не нужен, снимаем его целиком.
    kill_group(pgid);
    let _ = child.wait();

    let stderr = stderr_buf.lock().map(|s| s.clone()).unwrap_or_default();
    let duration_ms = started.elapsed().as_millis() as u64;
    let finish = |stdout: String, exit_code: Option<i32>| ClaudeRunResult {
        stdout,
        stderr: stderr.clone(),
        exit_code,
        duration_ms,
        timed_out,
        cancelled,
    };

    if timed_out || cancelled {
        return Ok(finish(String::new(), None));
    }
    match turn {
        Some(Ok(outcome)) => Ok(finish(outcome.text, Some(0))),
        Some(Err(e)) => Err(if stderr.trim().is_empty() {
            e
        } else {
            format!("{e}\\nstderr: {}", stderr.trim())
        }),
        None => Err("ACP: агент не ответил".into()),
    }
}

/// SIGTERM всей process group, затем SIGKILL через короткую паузу.
#[cfg(unix)]
fn kill_group(pgid: u32) {
    let target = format!("-{pgid}");
    let _ = Command::new("kill").args(["-s", "TERM", "--", &target]).status();
    thread::sleep(Duration::from_millis(200));
    let _ = Command::new("kill").args(["-s", "KILL", "--", &target]).status();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn script(lines: &[&str]) -> Cursor<Vec<u8>> {
        let mut data = String::new();
        for l in lines {
            data.push_str(l);
            data.push('\n');
        }
        Cursor::new(data.into_bytes())
    }

    fn written_lines(buf: &[u8]) -> Vec<Value> {
        String::from_utf8_lossy(buf)
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    const INIT: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"agentCapabilities":{}}}"#;
    const NEW: &str = r#"{"jsonrpc":"2.0","id":2,"result":{"sessionId":"s1"}}"#;

    #[test]
    fn collects_text_and_allows_once_on_permission() {
        let input = script(&[
            INIT,
            NEW,
            r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s1","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Hel"}}}}"#,
            r#"{"jsonrpc":"2.0","id":100,"method":"session/request_permission","params":{"sessionId":"s1","toolCall":{},"options":[{"optionId":"always","kind":"allow_always"},{"optionId":"once","kind":"allow_once"}]}}"#,
            r#"{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s1","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"lo"}}}}"#,
            r#"{"jsonrpc":"2.0","id":3,"result":{"stopReason":"end_turn"}}"#,
        ]);
        let mut out = Vec::new();
        let got = run_turn(input, &mut out, "/work/wt", "fix it").unwrap();
        assert_eq!(got, PromptOutcome { text: "Hello".into(), stop_reason: "end_turn".into() });

        let sent = written_lines(&out);
        assert_eq!(sent[0]["method"], "initialize");
        assert_eq!(sent[1]["params"]["cwd"], "/work/wt");
        assert_eq!(sent[2]["params"]["prompt"][0]["text"], "fix it");
        assert_eq!(sent[2]["params"]["sessionId"], "s1");
        assert_eq!(sent[3]["id"], 100);
        assert_eq!(sent[3]["result"]["outcome"]["outcome"], "selected");
        assert_eq!(sent[3]["result"]["outcome"]["optionId"], "once");
    }

    #[test]
    fn cancels_permission_without_allow_once() {
        let input = script(&[
            INIT,
            NEW,
            r#"{"jsonrpc":"2.0","id":7,"method":"session/request_permission","params":{"sessionId":"s1","toolCall":{},"options":[{"optionId":"no","kind":"reject_once"}]}}"#,
            r#"{"jsonrpc":"2.0","id":3,"result":{"stopReason":"end_turn"}}"#,
        ]);
        let mut out = Vec::new();
        run_turn(input, &mut out, "/w", "x").unwrap();
        let sent = written_lines(&out);
        assert_eq!(sent[3]["result"]["outcome"]["outcome"], "cancelled");
    }

    #[test]
    fn refuses_unsupported_client_methods() {
        let input = script(&[
            INIT,
            NEW,
            r#"{"jsonrpc":"2.0","id":9,"method":"fs/read_text_file","params":{"sessionId":"s1","path":"/x"}}"#,
            r#"{"jsonrpc":"2.0","id":3,"result":{"stopReason":"end_turn"}}"#,
        ]);
        let mut out = Vec::new();
        run_turn(input, &mut out, "/w", "x").unwrap();
        let sent = written_lines(&out);
        assert_eq!(sent[3]["error"]["code"], -32601);
    }

    #[test]
    fn eof_before_answer_is_an_error() {
        let input = script(&[INIT]);
        let mut out = Vec::new();
        let err = run_turn(input, &mut out, "/w", "x").unwrap_err();
        assert!(err.contains("завершился"));
    }

    #[test]
    fn error_response_to_prompt_is_an_error() {
        let input = script(&[
            INIT,
            NEW,
            r#"{"jsonrpc":"2.0","id":3,"error":{"code":-32000,"message":"quota"}}"#,
        ]);
        let mut out = Vec::new();
        let err = run_turn(input, &mut out, "/w", "x").unwrap_err();
        assert!(err.contains("quota"));
    }

    #[test]
    fn parse_line_classifies_messages() {
        assert!(matches!(parse_line(INIT), Some(Incoming::Response { id: 1, .. })));
        assert!(matches!(
            parse_line(r#"{"jsonrpc":"2.0","method":"session/update","params":{}}"#),
            Some(Incoming::Notification { .. })
        ));
        assert!(matches!(
            parse_line(r#"{"jsonrpc":"2.0","id":"abc","method":"x"}"#),
            Some(Incoming::Request { .. })
        ));
        assert_eq!(parse_line("not json"), None);
    }

    #[test]
    fn requests_use_protocol_names_and_version() {
        assert_eq!(initialize_request()["params"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(new_session_request("/w")["method"], "session/new");
        assert_eq!(prompt_request("s", "t")["method"], "session/prompt");
    }

    /// Фейковый ACP-агент на Python: отвечает на initialize/session/new, на prompt шлёт чанк и запрос разрешения.
    #[cfg(unix)]
    const FAKE_AGENT: &str = r#"
import json, sys
def out(m):
    sys.stdout.write(json.dumps(m) + "\n"); sys.stdout.flush()
for line in sys.stdin:
    m = json.loads(line)
    meth = m.get("method"); mid = m.get("id")
    if meth == "initialize":
        out({"jsonrpc": "2.0", "id": mid, "result": {"protocolVersion": 1, "agentCapabilities": {}}})
    elif meth == "session/new":
        out({"jsonrpc": "2.0", "id": mid, "result": {"sessionId": "s1"}})
    elif meth == "session/prompt":
        text = m["params"]["prompt"][0]["text"]
        out({"jsonrpc": "2.0", "method": "session/update", "params": {"sessionId": "s1", "update": {"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": "echo: " + text}}}})
        out({"jsonrpc": "2.0", "id": 900, "method": "session/request_permission", "params": {"sessionId": "s1", "toolCall": {}, "options": [{"optionId": "once", "kind": "allow_once"}]}})
        sys.stdin.readline()
        out({"jsonrpc": "2.0", "id": mid, "result": {"stopReason": "end_turn"}})
"#;

    #[cfg(unix)]
    fn python_available() -> bool {
        std::process::Command::new("python3")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[cfg(unix)]
    #[test]
    fn run_acp_talks_to_a_real_process() {
        if !python_available() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("demerzel-acp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("agent.py");
        std::fs::write(&script, FAKE_AGENT).unwrap();

        let got = run_acp(
            &format!("python3 {}", script.display()),
            &dir,
            "fix it",
            Duration::from_secs(20),
            None,
        )
        .unwrap();
        assert_eq!(got.stdout, "echo: fix it");
        assert!(!got.timed_out && !got.cancelled);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn run_acp_times_out_on_silent_agent() {
        let dir = std::env::temp_dir();
        let got = run_acp("sleep 30", &dir, "x", Duration::from_millis(300), None).unwrap();
        assert!(got.timed_out);
        assert!(got.stdout.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn run_acp_rejects_empty_command() {
        assert!(run_acp("   ", &std::env::temp_dir(), "x", Duration::from_secs(1), None).is_err());
    }
}
