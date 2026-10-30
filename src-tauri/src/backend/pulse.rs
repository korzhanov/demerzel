//! Контракт «пульс проекта»: опрос команды / файла (#97).
//!
//! Конфиг — `orchestrator.json` → `pulse`. Снимок совместим с `summary::ProjectPulse` (#98).
//! CLI `dem pulse` / `dem restart` — #102 (`backend/cli`).

use super::config::{resolve_pulse_command, resolve_pulse_file, PulseConfig};
use super::summary::ProjectPulse;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Сырой JSON пульса (stdout команды или содержимое файла).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PulsePayload {
    pub summary: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metrics: BTreeMap<String, i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eta_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
}

impl PulsePayload {
    /// Снимок для `summary.json` / виджета (#98).
    pub fn to_project_pulse(&self) -> ProjectPulse {
        ProjectPulse {
            summary: self.summary.trim().to_string(),
            eta_seconds: self.eta_seconds,
            state: self
                .state
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            metrics: if self.metrics.is_empty() {
                None
            } else {
                Some(self.metrics.clone())
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PulseError {
    EmptySummary,
    InvalidJson(String),
    Io(String),
    CommandFailed { code: Option<i32>, stderr: String },
    Timeout { secs: u64 },
    NoSource,
}

impl std::fmt::Display for PulseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PulseError::EmptySummary => write!(f, "pulse.summary must not be empty"),
            PulseError::InvalidJson(e) => write!(f, "invalid pulse JSON: {e}"),
            PulseError::Io(e) => write!(f, "pulse IO: {e}"),
            PulseError::CommandFailed { code, stderr } => {
                let code = code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "?".into());
                if stderr.trim().is_empty() {
                    write!(f, "pulse command exited {code}")
                } else {
                    write!(f, "pulse command exited {code}: {}", stderr.trim())
                }
            }
            PulseError::Timeout { secs } => {
                write!(f, "pulse command timed out after {secs}s")
            }
            PulseError::NoSource => write!(f, "pulse has neither command nor file"),
        }
    }
}

impl From<io::Error> for PulseError {
    fn from(e: io::Error) -> Self {
        PulseError::Io(e.to_string())
    }
}

/// Разбирает и валидирует JSON пульса.
pub fn parse_payload(json: &str) -> Result<PulsePayload, PulseError> {
    let payload: PulsePayload =
        serde_json::from_str(json).map_err(|e| PulseError::InvalidJson(e.to_string()))?;
    validate_payload(&payload)?;
    Ok(payload)
}

pub fn validate_payload(payload: &PulsePayload) -> Result<(), PulseError> {
    if payload.summary.trim().is_empty() {
        return Err(PulseError::EmptySummary);
    }
    Ok(())
}

/// Читает пульс из файла относительно `project_dir`.
pub fn fetch_from_file(project_dir: &Path, relative: &str) -> Result<PulsePayload, PulseError> {
    let path = resolve_pulse_path(project_dir, relative)?;
    let raw = fs::read_to_string(&path).map_err(|e| {
        PulseError::Io(format!("{}: {e}", path.display()))
    })?;
    parse_payload(&raw)
}

fn resolve_pulse_path(project_dir: &Path, relative: &str) -> Result<PathBuf, PulseError> {
    let rel = relative.trim();
    if rel.is_empty() {
        return Err(PulseError::NoSource);
    }
    let path = Path::new(rel);
    if path.is_absolute() {
        return Err(PulseError::Io(
            "pulse.file must be a relative path".into(),
        ));
    }
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(PulseError::Io(
            "pulse.file must not contain '..'".into(),
        ));
    }
    Ok(project_dir.join(path))
}

/// Запускает `sh -c` команду с таймаутом; stdout = JSON пульса.
#[cfg(unix)]
pub fn fetch_from_command(
    project_dir: &Path,
    command: &str,
    timeout: Duration,
    env: &BTreeMap<String, String>,
) -> Result<PulsePayload, PulseError> {
    use std::os::unix::process::CommandExt;

    let cmd = command.trim();
    if cmd.is_empty() {
        return Err(PulseError::NoSource);
    }
    if timeout.is_zero() {
        return Err(PulseError::Timeout { secs: 0 });
    }

    let mut child_cmd = Command::new("sh");
    child_cmd
        .arg("-c")
        .arg(cmd)
        .current_dir(project_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    for (k, v) in env {
        child_cmd.env(k, v);
    }

    let mut child = child_cmd.spawn()?;
    let deadline = Instant::now() + timeout;
    let timeout_secs = timeout.as_secs().max(1);

    loop {
        match child.try_wait()? {
            Some(status) => {
                let stdout = {
                    let mut buf = String::new();
                    if let Some(mut out) = child.stdout.take() {
                        use std::io::Read;
                        let _ = out.read_to_string(&mut buf);
                    }
                    buf
                };
                let stderr = {
                    let mut buf = String::new();
                    if let Some(mut err) = child.stderr.take() {
                        use std::io::Read;
                        let _ = err.read_to_string(&mut buf);
                    }
                    buf
                };
                if !status.success() {
                    return Err(PulseError::CommandFailed {
                        code: status.code(),
                        stderr,
                    });
                }
                return parse_payload(&stdout);
            }
            None if Instant::now() >= deadline => {
                let pgid = format!("-{}", child.id());
                let _ = Command::new("kill")
                    .arg("-TERM")
                    .arg("--")
                    .arg(&pgid)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
                std::thread::sleep(Duration::from_millis(50));
                if child.try_wait()?.is_none() {
                    let _ = Command::new("kill")
                        .arg("-KILL")
                        .arg("--")
                        .arg(&pgid)
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status();
                    let _ = child.wait();
                }
                return Err(PulseError::Timeout {
                    secs: timeout_secs,
                });
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}

/// Один опрос по конфигу: команда или файл.
pub fn fetch(
    project_dir: &Path,
    pulse: &PulseConfig,
    env: &BTreeMap<String, String>,
) -> Result<PulsePayload, PulseError> {
    if let Some(cmd) = resolve_pulse_command(pulse) {
        #[cfg(unix)]
        {
            return fetch_from_command(
                project_dir,
                cmd,
                Duration::from_secs(pulse.timeout_secs),
                env,
            );
        }
        #[cfg(not(unix))]
        {
            let _ = (cmd, env);
            return Err(PulseError::Io(
                "pulse.command is only supported on Unix".into(),
            ));
        }
    }
    if let Some(file) = resolve_pulse_file(pulse) {
        return fetch_from_file(project_dir, file);
    }
    Err(PulseError::NoSource)
}

/// Последняя успешная запись пульса для проекта.
#[derive(Debug, Clone)]
pub struct PulseRecord {
    pub payload: PulsePayload,
    pub fetched_at: Instant,
}

/// Хранилище последних пульсов по ключу проекта (путь / имя).
#[derive(Debug, Default)]
pub struct PulseStore {
    inner: Mutex<BTreeMap<String, PulseRecord>>,
}

impl PulseStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    pub fn get(&self, key: &str) -> Option<PulseRecord> {
        self.inner.lock().ok()?.get(key).cloned()
    }

    pub fn set(&self, key: impl Into<String>, payload: PulsePayload) {
        if let Ok(mut map) = self.inner.lock() {
            map.insert(
                key.into(),
                PulseRecord {
                    payload,
                    fetched_at: Instant::now(),
                },
            );
        }
    }

    /// Нужен ли новый опрос по `interval_secs`.
    pub fn is_due(&self, key: &str, interval: Duration) -> bool {
        match self.get(key) {
            None => true,
            Some(rec) => rec.fetched_at.elapsed() >= interval,
        }
    }

    /// Опрашивает при необходимости; при ошибке оставляет предыдущую запись.
    /// Возвращает актуальный снимок (новый или кэш).
    pub fn poll(
        &self,
        key: &str,
        project_dir: &Path,
        pulse: &PulseConfig,
        env: &BTreeMap<String, String>,
    ) -> Result<PulsePayload, PulseError> {
        let interval = Duration::from_secs(pulse.interval_secs.max(1));
        if !self.is_due(key, interval) {
            if let Some(rec) = self.get(key) {
                return Ok(rec.payload);
            }
        }
        match fetch(project_dir, pulse, env) {
            Ok(payload) => {
                self.set(key, payload.clone());
                Ok(payload)
            }
            Err(e) => {
                if let Some(rec) = self.get(key) {
                    Ok(rec.payload)
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Принудительный опрос (игнорирует interval); при ошибке без кэша — Err.
    pub fn refresh(
        &self,
        key: &str,
        project_dir: &Path,
        pulse: &PulseConfig,
        env: &BTreeMap<String, String>,
    ) -> Result<PulsePayload, PulseError> {
        let payload = fetch(project_dir, pulse, env)?;
        self.set(key, payload.clone());
        Ok(payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "demerzel-pulse-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn parses_full_payload() {
        let p = parse_payload(
            r#"{
              "summary": "1500 новых лемм, 450 проверено",
              "metrics": { "parsed": 1500, "checked": 450 },
              "eta_seconds": 600,
              "state": "running"
            }"#,
        )
        .unwrap();
        assert_eq!(p.summary, "1500 новых лемм, 450 проверено");
        assert_eq!(p.metrics.get("parsed"), Some(&1500));
        assert_eq!(p.eta_seconds, Some(600));
        let snap = p.to_project_pulse();
        assert_eq!(snap.summary, "1500 новых лемм, 450 проверено");
        assert_eq!(snap.metrics.as_ref().unwrap().get("checked"), Some(&450));
    }

    #[test]
    fn rejects_empty_summary() {
        assert_eq!(
            parse_payload(r#"{ "summary": "  " }"#),
            Err(PulseError::EmptySummary)
        );
    }

    #[test]
    fn fetch_file_relative() {
        let dir = tmp("file");
        fs::write(
            dir.join("pulse.json"),
            r#"{"summary":"ok","metrics":{"n":1},"eta_seconds":30}"#,
        )
        .unwrap();
        let p = fetch_from_file(&dir, "pulse.json").unwrap();
        assert_eq!(p.summary, "ok");
        assert_eq!(p.metrics.get("n"), Some(&1));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rejects_parent_dir_in_file() {
        let dir = tmp("parent");
        assert!(matches!(
            fetch_from_file(&dir, "../secret.json"),
            Err(PulseError::Io(_))
        ));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn command_stdout_json() {
        let dir = tmp("cmd");
        let p = fetch_from_command(
            &dir,
            r#"printf '%s' '{"summary":"from cmd","eta_seconds":10}'"#,
            Duration::from_secs(2),
            &BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(p.summary, "from cmd");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn command_timeout() {
        let dir = tmp("timeout");
        let err = fetch_from_command(
            &dir,
            "sleep 5",
            Duration::from_millis(200),
            &BTreeMap::new(),
        )
        .unwrap_err();
        assert!(matches!(err, PulseError::Timeout { .. }), "{err}");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn command_nonzero_exit() {
        let dir = tmp("fail");
        let err = fetch_from_command(&dir, "exit 7", Duration::from_secs(2), &BTreeMap::new())
            .unwrap_err();
        assert!(
            matches!(err, PulseError::CommandFailed { code: Some(7), .. }),
            "{err}"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn store_poll_caches_until_interval() {
        let dir = tmp("store");
        fs::write(dir.join("a.json"), r#"{"summary":"one"}"#).unwrap();
        let cfg = PulseConfig {
            command: None,
            file: Some("a.json".into()),
            interval_secs: 3600,
            timeout_secs: 5,
        };
        let store = PulseStore::new();
        let first = store
            .poll("parser", &dir, &cfg, &BTreeMap::new())
            .unwrap();
        assert_eq!(first.summary, "one");
        fs::write(dir.join("a.json"), r#"{"summary":"two"}"#).unwrap();
        let cached = store
            .poll("parser", &dir, &cfg, &BTreeMap::new())
            .unwrap();
        assert_eq!(cached.summary, "one");
        let forced = store
            .refresh("parser", &dir, &cfg, &BTreeMap::new())
            .unwrap();
        assert_eq!(forced.summary, "two");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn store_keeps_last_on_error() {
        let dir = tmp("keeplast");
        fs::write(dir.join("p.json"), r#"{"summary":"ok"}"#).unwrap();
        let cfg = PulseConfig {
            command: None,
            file: Some("p.json".into()),
            interval_secs: 1,
            timeout_secs: 5,
        };
        let store = PulseStore::new();
        store.refresh("x", &dir, &cfg, &BTreeMap::new()).unwrap();
        fs::remove_file(dir.join("p.json")).unwrap();
        assert!(store.refresh("x", &dir, &cfg, &BTreeMap::new()).is_err());
        assert_eq!(store.get("x").unwrap().payload.summary, "ok");
        // poll после истечения interval тоже не затирает кэш
        std::thread::sleep(Duration::from_millis(1100));
        let kept = store.poll("x", &dir, &cfg, &BTreeMap::new()).unwrap();
        assert_eq!(kept.summary, "ok");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn fetch_dispatches_file() {
        let dir = tmp("dispatch");
        fs::write(dir.join("pulse.json"), r#"{"summary":"via fetch"}"#).unwrap();
        let cfg = PulseConfig {
            command: None,
            file: Some("pulse.json".into()),
            interval_secs: 30,
            timeout_secs: 5,
        };
        assert_eq!(
            fetch(&dir, &cfg, &BTreeMap::new()).unwrap().summary,
            "via fetch"
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
