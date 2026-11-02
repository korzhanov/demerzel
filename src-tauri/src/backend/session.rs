//! Сохранённые процессы: Demerzel помнит, что запустил или к чему присоединился,
//! и после перезапуска приложения снова берёт процессы под управление (#присоединение).
//!
//! Файл лежит в папке данных приложения. Запись считается действующей, только если
//! PID жив и его командная строка совпадает с записанной. Так переиспользованный PID
//! не будет принят за наш процесс.

use std::io;
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

pub const SESSION_FILE: &str = "processes.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub id: u64,
    pub project: String,
    pub command: String,
    pub path: String,
    pub port: Option<u16>,
    pub pid: u32,
    pub pgid: u32,
    /// Запущен Demerzel (true) или присоединён извне (false). От этого зависит, что останавливать.
    pub owned: bool,
    /// Присоединён вручную, а не запущен Demerzel.
    #[serde(default)]
    pub attached: bool,
    pub out_log: Option<String>,
    pub err_log: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub next_id: u64,
    #[serde(default)]
    pub records: Vec<Record>,
}

/// Читает состояние. Нет файла или он испорчен — пустое состояние, без ошибки:
/// процессы потом можно присоединить заново.
pub fn load(path: &Path) -> State {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Пишет через временный файл, чтобы обрыв записи не испортил состояние.
pub fn save(path: &Path, state: &State) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(state).map_err(io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(tmp, path)
}

/// Жив ли процесс (сигнал 0 без доставки).
#[cfg(unix)]
pub fn is_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(not(unix))]
pub fn is_alive(_pid: u32) -> bool {
    false
}

/// Командная строка процесса, если он существует.
#[cfg(unix)]
pub fn command_line(pid: u32) -> Option<String> {
    ps_field(pid, "command")
}

/// Process group процесса. Для процесса, запущенного Demerzel, совпадает с его PID.
#[cfg(unix)]
pub fn pgid_of(pid: u32) -> Option<u32> {
    ps_field(pid, "pgid").and_then(|s| s.parse().ok())
}

#[cfg(unix)]
fn ps_field(pid: u32, field: &str) -> Option<String> {
    let out = Command::new("ps")
        .args(["-o", &format!("{field}="), "-p", &pid.to_string()])
        .output()
        .ok()?;
    let text = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// Совпадает ли командная строка процесса с записанной командой.
pub fn same_process(command: &str, cmdline: &str) -> bool {
    !command.trim().is_empty() && cmdline.contains(command.trim())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp_file(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("demerzel-session-{name}-{nanos}")).join("processes.json")
    }

    fn sample(id: u64) -> Record {
        Record {
            id,
            project: "api".into(),
            command: "bun run dev".into(),
            path: "/work/api".into(),
            port: Some(5173),
            pid: 4242,
            pgid: 4242,
            owned: true,
            attached: false,
            out_log: Some("/logs/1.out.log".into()),
            err_log: Some("/logs/1.err.log".into()),
        }
    }

    #[test]
    fn save_then_load_roundtrip() {
        let path = tmp_file("roundtrip");
        let state = State { next_id: 3, records: vec![sample(1), sample(2)] };
        save(&path, &state).unwrap();
        assert_eq!(load(&path), state);
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn missing_or_broken_file_loads_empty() {
        assert_eq!(load(&tmp_file("missing")), State::default());
        let path = tmp_file("broken");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(load(&path), State::default());
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn same_process_checks_command_text() {
        assert!(same_process("bun run dev", "sh -c bun run dev"));
        assert!(!same_process("bun run dev", "/usr/bin/vim notes.md"));
        assert!(!same_process("   ", "anything"));
    }

    #[test]
    #[cfg(unix)]
    fn dead_pid_is_not_alive() {
        assert!(!is_alive(4_000_000));
    }
}
