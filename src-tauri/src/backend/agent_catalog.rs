//! Реестр CLI-агентов на машине (#95).
//!
//! Детект: наличие бинарника в `PATH` (или override `DEMERZEL_*_BIN`), затем
//! лёгкий `--version` — без headless-сессии агента. Для тестов — фейковые
//! скрипты в изолированном `PATH`.

use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use super::agents::AgentRunner;

/// Таймаут `--version` при детектe (не headless-прогон).
pub const VERSION_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// Запись реестра: установлен ли агент и как его поставить, если нет.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCatalogEntry {
    pub runner: AgentRunner,
    /// Имя бинарника в PATH (`cursor-agent`, `claude`, …).
    pub binary: String,
    pub available: bool,
    /// Абсолютный путь, если найден.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Первая строка `--version`, если удалось снять.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Подсказка установки (всегда, чтобы UI мог показать при `available: false`).
    pub install_hint: String,
    /// Headless-прогон уже реализован (сейчас Claude / Ollama).
    pub headless_supported: bool,
}

/// Метаданные известного CLI (#95).
#[derive(Debug, Clone, Copy)]
pub struct AgentCliMeta {
    pub runner: AgentRunner,
    pub binary: &'static str,
    pub env_override: &'static str,
    pub install_hint: &'static str,
    pub headless_supported: bool,
}

/// Все поддерживаемые в реестре агенты (порядок стабильный для UI).
pub fn all_agent_meta() -> &'static [AgentCliMeta] {
    &AGENT_META
}

const AGENT_META: &[AgentCliMeta] = &[
    AgentCliMeta {
        runner: AgentRunner::Claude,
        binary: "claude",
        env_override: "DEMERZEL_CLAUDE_BIN",
        install_hint: "Установите Claude Code CLI: https://docs.anthropic.com/en/docs/claude-code",
        headless_supported: true,
    },
    AgentCliMeta {
        runner: AgentRunner::Codex,
        binary: "codex",
        env_override: "DEMERZEL_CODEX_BIN",
        install_hint: "Установите OpenAI Codex CLI: npm i -g @openai/codex — https://github.com/openai/codex",
        headless_supported: false,
    },
    AgentCliMeta {
        runner: AgentRunner::Gemini,
        binary: "gemini",
        env_override: "DEMERZEL_GEMINI_BIN",
        install_hint: "Установите Gemini CLI: npm i -g @google/gemini-cli — https://github.com/google-gemini/gemini-cli",
        headless_supported: false,
    },
    AgentCliMeta {
        runner: AgentRunner::Aider,
        binary: "aider",
        env_override: "DEMERZEL_AIDER_BIN",
        install_hint: "Установите aider: pipx install aider-chat — https://aider.chat",
        headless_supported: false,
    },
    AgentCliMeta {
        runner: AgentRunner::Opencode,
        binary: "opencode",
        env_override: "DEMERZEL_OPENCODE_BIN",
        install_hint: "Установите OpenCode CLI: https://github.com/opencode-ai/opencode",
        headless_supported: false,
    },
    AgentCliMeta {
        runner: AgentRunner::CursorAgent,
        binary: "cursor-agent",
        env_override: "DEMERZEL_CURSOR_AGENT_BIN",
        install_hint: "Установите Cursor Agent CLI из Cursor IDE / документации Cursor",
        headless_supported: false,
    },
    AgentCliMeta {
        runner: AgentRunner::Ollama,
        binary: "ollama",
        env_override: "DEMERZEL_OLLAMA_BIN",
        install_hint: "Установите Ollama: https://ollama.com",
        headless_supported: true,
    },
];

impl AgentRunner {
    /// Метаданные CLI для раннера.
    pub fn meta(self) -> AgentCliMeta {
        all_agent_meta()
            .iter()
            .copied()
            .find(|m| m.runner == self)
            .expect("every AgentRunner has catalog meta")
    }

    pub fn binary_name(self) -> &'static str {
        self.meta().binary
    }

    pub fn install_hint(self) -> &'static str {
        self.meta().install_hint
    }

    pub fn headless_supported(self) -> bool {
        self.meta().headless_supported
    }

    pub fn env_bin_override(self) -> &'static str {
        self.meta().env_override
    }
}

/// Путь к бинарнику: env override → иначе имя для PATH.
pub fn resolve_agent_bin(runner: AgentRunner) -> PathBuf {
    if let Some(override_bin) = std::env::var_os(runner.env_bin_override()) {
        if !override_bin.is_empty() {
            return PathBuf::from(override_bin);
        }
    }
    PathBuf::from(runner.binary_name())
}

/// Ищет исполняемый файл в `PATH` (или в `path_env` для тестов).
pub fn find_on_path(bin_name: &str, path_env: Option<&OsStr>) -> Option<PathBuf> {
    if bin_name.is_empty() {
        return None;
    }
    let as_path = Path::new(bin_name);
    if as_path.is_absolute() || bin_name.contains('/') {
        return is_executable(as_path).then(|| as_path.to_path_buf());
    }

    let path_os = path_env
        .map(std::ffi::OsString::from)
        .or_else(|| std::env::var_os("PATH"))?;
    for dir in std::env::split_paths(&path_os) {
        let candidate = dir.join(bin_name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Снимает `--version` с коротким таймаутом (не headless-агент).
pub fn probe_version(bin: &Path) -> Option<String> {
    probe_version_with_timeout(bin, VERSION_PROBE_TIMEOUT)
}

pub fn probe_version_with_timeout(bin: &Path, timeout: Duration) -> Option<String> {
    let mut child = Command::new(bin)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;

    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => return None,
        }
    }

    let output = child.wait_with_output().ok()?;
    let text = if !output.stdout.is_empty() {
        String::from_utf8_lossy(&output.stdout)
    } else {
        String::from_utf8_lossy(&output.stderr)
    };
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())?
        .to_string();
    Some(line)
}

/// Детект одного раннера: PATH / override, затем `--version`.
///
/// `path_env` — подмена `PATH` в тестах. Override `DEMERZEL_*_BIN` имеет приоритет.
pub fn detect_agent(runner: AgentRunner, path_env: Option<&OsStr>) -> AgentCatalogEntry {
    let meta = runner.meta();
    let override_bin = std::env::var_os(meta.env_override).filter(|v| !v.is_empty());

    let found = if let Some(ref ov) = override_bin {
        let p = PathBuf::from(ov);
        is_executable(&p).then_some(p)
    } else {
        find_on_path(meta.binary, path_env)
    };

    match found {
        Some(path) => {
            let version = probe_version(&path);
            AgentCatalogEntry {
                runner,
                binary: meta.binary.to_string(),
                available: true,
                path: Some(path.display().to_string()),
                version,
                install_hint: meta.install_hint.to_string(),
                headless_supported: meta.headless_supported,
            }
        }
        None => AgentCatalogEntry {
            runner,
            binary: meta.binary.to_string(),
            available: false,
            path: None,
            version: None,
            install_hint: meta.install_hint.to_string(),
            headless_supported: meta.headless_supported,
        },
    }
}

/// Полный реестр известных агентов на текущем `PATH`.
pub fn list_available_agents(path_env: Option<&OsStr>) -> Vec<AgentCatalogEntry> {
    all_agent_meta()
        .iter()
        .map(|m| detect_agent(m.runner, path_env))
        .collect()
}

/// Сообщение, если агент не установлен или headless ещё не поддержан.
pub fn runner_readiness_error(runner: AgentRunner, path_env: Option<&OsStr>) -> Option<String> {
    let entry = detect_agent(runner, path_env);
    if !entry.available {
        return Some(format!(
            "агент '{}' не найден в PATH ({})",
            runner.as_str(),
            entry.install_hint
        ));
    }
    if !entry.headless_supported {
        return Some(format!(
            "агент '{}' обнаружен, но headless-прогон пока не поддержан (реестр #95; запуск — Claude/Ollama)",
            runner.as_str()
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn write_fake_bin(dir: &Path, name: &str, script: &str) -> PathBuf {
        fs::create_dir_all(dir).unwrap();
        let bin = dir.join(name);
        fs::write(&bin, script).unwrap();
        let mut perms = fs::metadata(&bin).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&bin, perms).unwrap();
        bin
    }

    #[test]
    fn detects_fake_binaries_on_path_with_version() {
        let _lock = ENV_LOCK.lock().unwrap();
        let root = std::env::temp_dir().join(format!(
            "demerzel-agent-catalog-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        write_fake_bin(
            &root,
            "claude",
            "#!/bin/sh\necho 'Claude Code 1.2.3'\n",
        );
        write_fake_bin(
            &root,
            "aider",
            "#!/bin/sh\necho 'aider 0.99.0'\n",
        );
        // Нет gemini — должен быть unavailable + hint.

        for key in [
            "DEMERZEL_CLAUDE_BIN",
            "DEMERZEL_AIDER_BIN",
            "DEMERZEL_GEMINI_BIN",
            "DEMERZEL_CODEX_BIN",
            "DEMERZEL_OPENCODE_BIN",
            "DEMERZEL_CURSOR_AGENT_BIN",
            "DEMERZEL_OLLAMA_BIN",
        ] {
            unsafe { std::env::remove_var(key) };
        }

        let path = root.as_os_str();
        let claude = detect_agent(AgentRunner::Claude, Some(path));
        assert!(claude.available);
        assert!(claude.path.as_ref().unwrap().contains("claude"));
        assert_eq!(claude.version.as_deref(), Some("Claude Code 1.2.3"));
        assert!(claude.headless_supported);

        let aider = detect_agent(AgentRunner::Aider, Some(path));
        assert!(aider.available);
        assert_eq!(aider.version.as_deref(), Some("aider 0.99.0"));
        assert!(!aider.headless_supported);

        let gemini = detect_agent(AgentRunner::Gemini, Some(path));
        assert!(!gemini.available);
        assert!(gemini.install_hint.contains("Gemini"));
        assert!(runner_readiness_error(AgentRunner::Gemini, Some(path))
            .unwrap()
            .contains("не найден"));

        let catalog = list_available_agents(Some(path));
        assert_eq!(catalog.len(), all_agent_meta().len());
        assert!(catalog.iter().any(|e| e.runner == AgentRunner::Claude && e.available));
        assert!(catalog.iter().any(|e| e.runner == AgentRunner::Gemini && !e.available));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_agent_install_hint_and_override_bin() {
        let _lock = ENV_LOCK.lock().unwrap();
        let root = std::env::temp_dir().join(format!(
            "demerzel-agent-override-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&root);
        let bin = write_fake_bin(
            &root,
            "my-codex",
            "#!/bin/sh\necho 'codex fake 0.1'\n",
        );
        unsafe {
            std::env::set_var("DEMERZEL_CODEX_BIN", &bin);
        }

        let empty_path = OsStr::new("");
        let entry = detect_agent(AgentRunner::Codex, Some(empty_path));
        assert!(entry.available);
        assert_eq!(entry.version.as_deref(), Some("codex fake 0.1"));

        unsafe {
            std::env::remove_var("DEMERZEL_CODEX_BIN");
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn find_on_path_skips_non_executable() {
        let root = std::env::temp_dir().join(format!(
            "demerzel-agent-nox-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let bin = root.join("gemini");
        fs::write(&bin, "not executable").unwrap();
        let mut perms = fs::metadata(&bin).unwrap().permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&bin, perms).unwrap();

        assert!(find_on_path("gemini", Some(root.as_os_str())).is_none());
        let _ = fs::remove_dir_all(&root);
    }
}
