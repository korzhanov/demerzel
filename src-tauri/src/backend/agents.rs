//! Запуски агентов: статусы run, git worktree (#27), Claude Code CLI (#28)
//! и лимит параллелизма / памяти (#29).
//!
//! На каждый баг — отдельный worktree в `~/.orchestrator/worktrees/<project>/<run-id>`
//! с веткой `agent/<run-id>`, чтобы правки агента не попадали в основную ветку.
//! Override корня: `DEMERZEL_WORKTREES_DIR`.
//!
//! Headless-раннер (#28): `claude -p "<prompt>"` в каталоге worktree, таймаут,
//! отмена SIGTERM дереву процессов, промпт из задачи + `CLAUDE.md`.
//! Бинарник: `claude` или `DEMERZEL_CLAUDE_BIN` (для тестов — фейковый скрипт).
//!
//! Параллелизм (#29): по умолчанию 1 агент; перед стартом — свободная память
//! (`vm_stat`), с запасом под Ollama если процесс запущен (сам раннер — #30).

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Статусы прогона агента (#27–#31).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Queued,
    Running,
    NeedsReview,
    Failed,
    Discarded,
    Accepted,
}

/// Допустимые переходы. Accepted только после NeedsReview: слияние всегда вручную.
pub fn can_transition(from: RunStatus, to: RunStatus) -> bool {
    use RunStatus::*;
    matches!(
        (from, to),
        (Queued, Running)
            | (Running, NeedsReview)
            | (Running, Failed)
            | (NeedsReview, Accepted)
            | (NeedsReview, Discarded)
            | (Failed, Discarded)
            | (Queued, Discarded)
    )
}

/// Лимит параллельных агентов по умолчанию (#29).
pub const DEFAULT_CONCURRENCY: usize = 1;

/// Минимум доступной памяти (МиБ) перед стартом по умолчанию.
pub const DEFAULT_MIN_FREE_MEMORY_MB: u64 = 2048;

/// Запас под Ollama (МиБ), если процесс `ollama` запущен. Раннер Ollama — #30.
pub const DEFAULT_OLLAMA_RESERVE_MB: u64 = 4096;

fn default_max_concurrent() -> usize {
    DEFAULT_CONCURRENCY
}

fn default_min_free_memory_mb() -> u64 {
    DEFAULT_MIN_FREE_MEMORY_MB
}

fn default_ollama_reserve_mb() -> u64 {
    DEFAULT_OLLAMA_RESERVE_MB
}

/// Настройки агентов в `settings.json` → `agents` (#29).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentsConfig {
    /// Сколько агентов можно держать в Running одновременно.
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,
    /// Минимум доступной RAM (МиБ) после вычета запаса Ollama.
    #[serde(default = "default_min_free_memory_mb")]
    pub min_free_memory_mb: u64,
    /// Сколько МиБ резервировать, если `ollama` уже крутится.
    #[serde(default = "default_ollama_reserve_mb")]
    pub ollama_reserve_mb: u64,
}

impl Default for AgentsConfig {
    fn default() -> Self {
        Self {
            max_concurrent: DEFAULT_CONCURRENCY,
            min_free_memory_mb: DEFAULT_MIN_FREE_MEMORY_MB,
            ollama_reserve_mb: DEFAULT_OLLAMA_RESERVE_MB,
        }
    }
}

/// Почему задача осталась в `Queued` вместо `Running`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueReason {
    ConcurrencyLimit { running: usize, max: usize },
    InsufficientMemory {
        free_mb: u64,
        required_mb: u64,
        ollama_running: bool,
    },
}

/// Решение лимитера перед стартом агента.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartDecision {
    /// Слот занят — можно переводить Queued → Running.
    Start,
    /// Слот не выдан — оставить в очереди.
    Queue(QueueReason),
}

/// Источник доступной памяти (для тестов подменяется).
pub trait MemoryProbe {
    fn available_mb(&self) -> Result<u64, String>;
}

/// Проверка, запущен ли процесс Ollama (для тестов подменяется).
pub trait OllamaProbe {
    fn is_running(&self) -> bool;
}

/// Чтение доступной RAM через `vm_stat` (free + inactive + speculative).
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemMemoryProbe;

impl MemoryProbe for SystemMemoryProbe {
    fn available_mb(&self) -> Result<u64, String> {
        available_memory_mb()
    }
}

/// `pgrep -x ollama` — процесс есть / нет. Не запускает Ollama (#30).
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemOllamaProbe;

impl OllamaProbe for SystemOllamaProbe {
    fn is_running(&self) -> bool {
        ollama_process_running()
    }
}

/// Лимитер: concurrency + память (+ запас Ollama).
#[derive(Debug)]
pub struct AgentLimiter<M = SystemMemoryProbe, O = SystemOllamaProbe> {
    config: AgentsConfig,
    running: HashSet<String>,
    memory: M,
    ollama: O,
}

impl AgentLimiter<SystemMemoryProbe, SystemOllamaProbe> {
    pub fn new(config: AgentsConfig) -> Self {
        Self::with_probes(config, SystemMemoryProbe, SystemOllamaProbe)
    }

    pub fn from_defaults() -> Self {
        Self::new(AgentsConfig::default())
    }
}

impl<M: MemoryProbe, O: OllamaProbe> AgentLimiter<M, O> {
    pub fn with_probes(config: AgentsConfig, memory: M, ollama: O) -> Self {
        Self {
            config,
            running: HashSet::new(),
            memory,
            ollama,
        }
    }

    pub fn config(&self) -> &AgentsConfig {
        &self.config
    }

    pub fn running_count(&self) -> usize {
        self.running.len()
    }

    pub fn is_running(&self, run_id: &str) -> bool {
        self.running.contains(run_id)
    }

    /// Сколько МиБ нужно свободно с учётом Ollama.
    pub fn required_free_mb(&self) -> u64 {
        let mut required = self.config.min_free_memory_mb;
        if self.ollama.is_running() {
            required = required.saturating_add(self.config.ollama_reserve_mb);
        }
        required
    }

    /// Пытается занять слот для `run_id`. При отказе — `Queue` (статус Queued).
    pub fn try_acquire(&mut self, run_id: &str) -> Result<StartDecision, String> {
        let run_id = run_id.trim();
        if run_id.is_empty() {
            return Err("run_id must not be empty".into());
        }
        if self.running.contains(run_id) {
            return Ok(StartDecision::Start);
        }

        let max = self.config.max_concurrent;
        if self.running.len() >= max {
            return Ok(StartDecision::Queue(QueueReason::ConcurrencyLimit {
                running: self.running.len(),
                max,
            }));
        }

        let free_mb = self.memory.available_mb()?;
        let ollama_running = self.ollama.is_running();
        let required_mb = if ollama_running {
            self.config
                .min_free_memory_mb
                .saturating_add(self.config.ollama_reserve_mb)
        } else {
            self.config.min_free_memory_mb
        };

        if free_mb < required_mb {
            return Ok(StartDecision::Queue(QueueReason::InsufficientMemory {
                free_mb,
                required_mb,
                ollama_running,
            }));
        }

        self.running.insert(run_id.to_string());
        Ok(StartDecision::Start)
    }

    /// Освобождает слот после завершения / fail / discard.
    pub fn release(&mut self, run_id: &str) {
        self.running.remove(run_id);
    }
}

/// Доступная память (МиБ): pages free + inactive + speculative из `vm_stat`.
pub fn available_memory_mb() -> Result<u64, String> {
    let output = Command::new("vm_stat")
        .output()
        .map_err(|e| format!("vm_stat: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("vm_stat failed: {}", stderr.trim()));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_vm_stat_available_mb(&stdout)
}

/// Парсит вывод `vm_stat` → доступные МиБ (для тестов и SystemMemoryProbe).
pub fn parse_vm_stat_available_mb(stdout: &str) -> Result<u64, String> {
    let page_size = parse_vm_stat_page_size(stdout)?;
    let free = parse_vm_stat_pages(stdout, "Pages free")?;
    let inactive = parse_vm_stat_pages(stdout, "Pages inactive")?;
    let speculative = parse_vm_stat_pages(stdout, "Pages speculative").unwrap_or(0);
    let pages = free.saturating_add(inactive).saturating_add(speculative);
    let bytes = pages.saturating_mul(page_size);
    Ok(bytes / (1024 * 1024))
}

fn parse_vm_stat_page_size(stdout: &str) -> Result<u64, String> {
    // "Mach Virtual Memory Statistics: (page size of 16384 bytes)"
    for line in stdout.lines() {
        if let Some(idx) = line.find("page size of ") {
            let rest = &line[idx + "page size of ".len()..];
            let num: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = num.parse::<u64>() {
                if n > 0 {
                    return Ok(n);
                }
            }
        }
    }
    // Fallback: sysctl hw.pagesize
    let out = Command::new("sysctl")
        .args(["-n", "hw.pagesize"])
        .output()
        .map_err(|e| format!("sysctl hw.pagesize: {e}"))?;
    if !out.status.success() {
        return Err("cannot determine page size from vm_stat or sysctl".into());
    }
    let s = String::from_utf8_lossy(&out.stdout);
    s.trim()
        .parse::<u64>()
        .map_err(|e| format!("parse hw.pagesize: {e}"))
}

fn parse_vm_stat_pages(stdout: &str, label: &str) -> Result<u64, String> {
    let prefix = format!("{label}:");
    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(&prefix) {
            let digits: String = rest
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect();
            return digits
                .parse::<u64>()
                .map_err(|e| format!("parse {label}: {e}"));
        }
    }
    Err(format!("vm_stat: missing {label}"))
}

/// Есть ли процесс с именем `ollama` (`pgrep -x`).
pub fn ollama_process_running() -> bool {
    Command::new("pgrep")
        .args(["-x", "ollama"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Созданный worktree: путь, ветка и идентификаторы.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeHandle {
    pub project: String,
    pub run_id: String,
    pub path: PathBuf,
    pub branch: String,
}

/// Имя ветки агента: `agent/<run-id>`.
pub fn branch_name(run_id: &str) -> Result<String, String> {
    validate_segment("run_id", run_id)?;
    Ok(format!("agent/{run_id}"))
}

/// Корень каталога worktree (`~/.orchestrator/worktrees` или `DEMERZEL_WORKTREES_DIR`).
pub fn worktrees_root() -> Result<PathBuf, String> {
    if let Some(override_dir) = std::env::var_os("DEMERZEL_WORKTREES_DIR") {
        return Ok(PathBuf::from(override_dir));
    }
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    Ok(PathBuf::from(home).join(".orchestrator/worktrees"))
}

/// Путь worktree: `<root>/<project>/<run-id>`.
pub fn worktree_path(project: &str, run_id: &str) -> Result<PathBuf, String> {
    validate_segment("project", project)?;
    validate_segment("run_id", run_id)?;
    Ok(worktrees_root()?.join(project).join(run_id))
}

fn validate_segment(label: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if value.contains('/') || value.contains('\\') || value.contains("..") {
        return Err(format!("{label} must be a single path segment without '..'"));
    }
    if value.starts_with('.') {
        return Err(format!("{label} must not start with '.'"));
    }
    Ok(())
}

fn run_git(repo: &Path, args: &[&str]) -> Result<Output, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|e| format!("git {}: {e}", args.first().unwrap_or(&"")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "git {} failed: {}{}",
            args.join(" "),
            stderr.trim(),
            if stdout.trim().is_empty() {
                String::new()
            } else {
                format!(" ({})", stdout.trim())
            }
        ));
    }
    Ok(output)
}

/// Создаёт ветку `agent/<run-id>` и worktree в `~/.orchestrator/worktrees/...`.
///
/// `base_ref` — от чего ветвиться (по умолчанию `HEAD` репозитория проекта).
pub fn create_worktree(
    repo: &Path,
    project: &str,
    run_id: &str,
    base_ref: Option<&str>,
) -> Result<WorktreeHandle, String> {
    let path = worktree_path(project, run_id)?;
    let branch = branch_name(run_id)?;
    let base = base_ref.unwrap_or("HEAD");

    if path.exists() {
        return Err(format!("worktree path already exists: {}", path.display()));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // Новая ветка + каталог: правки только в worktree, основная ветка не трогается.
    run_git(
        repo,
        &[
            "worktree",
            "add",
            "-b",
            &branch,
            path.to_str().ok_or("worktree path is not UTF-8")?,
            base,
        ],
    )?;

    Ok(WorktreeHandle {
        project: project.to_string(),
        run_id: run_id.to_string(),
        path,
        branch,
    })
}

/// Удаляет worktree и ветку `agent/<run-id>` после завершения или отмены.
pub fn remove_worktree(repo: &Path, project: &str, run_id: &str) -> Result<(), String> {
    let path = worktree_path(project, run_id)?;
    let branch = branch_name(run_id)?;

    if path.exists() {
        run_git(
            repo,
            &[
                "worktree",
                "remove",
                "--force",
                path.to_str().ok_or("worktree path is not UTF-8")?,
            ],
        )?;
    } else {
        // Каталог уже снесён — подчистим запись worktree, если осталась.
        let _ = run_git(repo, &["worktree", "prune"]);
    }

    // Ветка агента больше не нужна; игнор, если уже удалена.
    let delete = Command::new("git")
        .args(["branch", "-D", &branch])
        .current_dir(repo)
        .output()
        .map_err(|e| format!("git branch -D: {e}"))?;
    if !delete.status.success() {
        let stderr = String::from_utf8_lossy(&delete.stderr);
        if !stderr.contains("not found") && !stderr.contains("doesn't exist") {
            return Err(format!("git branch -D {branch} failed: {}", stderr.trim()));
        }
    }

    // Пустой каталог проекта под worktrees — подчистить, если остался.
    if let Some(project_dir) = path.parent() {
        let _ = std::fs::remove_dir(project_dir);
    }

    Ok(())
}

// --- Claude Code CLI headless runner (#28) ---

/// Имя файла правил проекта в корне worktree.
pub const PROJECT_RULES_FILE: &str = "CLAUDE.md";

/// Таймаут прогона Claude по умолчанию (30 минут).
pub const DEFAULT_CLAUDE_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Grace после SIGTERM перед SIGKILL при отмене/таймауте.
const CANCEL_GRACE: Duration = Duration::from_secs(2);

/// Путь к бинарнику Claude Code CLI.
pub fn claude_bin() -> PathBuf {
    if let Some(override_bin) = std::env::var_os("DEMERZEL_CLAUDE_BIN") {
        return PathBuf::from(override_bin);
    }
    PathBuf::from("claude")
}

/// Промпт из описания задачи и `CLAUDE.md` в корне worktree (если есть).
pub fn build_prompt(task: &str, worktree: &Path) -> Result<String, String> {
    let task = task.trim();
    if task.is_empty() {
        return Err("task description must not be empty".into());
    }

    let mut parts: Vec<String> = Vec::new();
    let rules_path = worktree.join(PROJECT_RULES_FILE);
    if rules_path.is_file() {
        let rules = std::fs::read_to_string(&rules_path).map_err(|e| {
            format!("read {}: {e}", rules_path.display())
        })?;
        let rules = rules.trim();
        if !rules.is_empty() {
            parts.push(format!("# Project rules\n\n{rules}"));
        }
    }
    parts.push(format!("# Task\n\n{task}"));
    Ok(parts.join("\n\n"))
}

/// Итог headless-прогона Claude CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeRunResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub timed_out: bool,
    pub cancelled: bool,
}

/// Живой процесс `claude -p`; отмена — SIGTERM всей process group.
pub struct ClaudeHandle {
    child: Option<Child>,
    started: Instant,
    stdout_buf: Arc<Mutex<String>>,
    stderr_buf: Arc<Mutex<String>>,
}

impl ClaudeHandle {
    /// Запускает `claude -p <prompt>` в `worktree` (отдельная process group на Unix).
    #[cfg(unix)]
    pub fn spawn(worktree: &Path, prompt: &str) -> Result<Self, String> {
        use std::os::unix::process::CommandExt;

        if !worktree.is_dir() {
            return Err(format!("worktree is not a directory: {}", worktree.display()));
        }
        if prompt.trim().is_empty() {
            return Err("prompt must not be empty".into());
        }

        let bin = claude_bin();
        let mut cmd = Command::new(&bin);
        cmd.arg("-p")
            .arg(prompt)
            .current_dir(worktree)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);

        let mut child = cmd.spawn().map_err(|e| {
            format!("spawn {}: {e}", bin.display())
        })?;

        let stdout_buf = Arc::new(Mutex::new(String::new()));
        let stderr_buf = Arc::new(Mutex::new(String::new()));
        if let Some(out) = child.stdout.take() {
            spawn_pipe_reader(out, stdout_buf.clone());
        }
        if let Some(err) = child.stderr.take() {
            spawn_pipe_reader(err, stderr_buf.clone());
        }

        Ok(Self {
            child: Some(child),
            started: Instant::now(),
            stdout_buf,
            stderr_buf,
        })
    }

    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().map(|c| c.id())
    }

    /// SIGTERM дереву процессов (process group); при необходимости SIGKILL.
    #[cfg(unix)]
    pub fn cancel(&mut self) -> Result<(), String> {
        let Some(child) = self.child.as_mut() else {
            return Ok(());
        };
        let pgid = format!("-{}", child.id());
        signal_group(&pgid, "TERM")?;
        let deadline = Instant::now() + CANCEL_GRACE;
        while Instant::now() < deadline {
            if child.try_wait().map_err(|e| e.to_string())?.is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        if child.try_wait().map_err(|e| e.to_string())?.is_none() {
            let _ = signal_group(&pgid, "KILL");
            let _ = child.wait();
        }
        self.child = None;
        Ok(())
    }

    /// Ждёт завершения, таймаута или флага отмены; собирает stdout/stderr/код/длительность.
    #[cfg(unix)]
    pub fn wait_until(
        &mut self,
        deadline: Instant,
        cancel: Option<&AtomicBool>,
    ) -> Result<ClaudeRunResult, String> {
        let mut timed_out = false;
        let mut cancelled = false;

        loop {
            if cancel.is_some_and(|c| c.load(Ordering::SeqCst)) {
                cancelled = true;
                self.cancel()?;
                break;
            }
            if Instant::now() >= deadline {
                timed_out = true;
                self.cancel()?;
                break;
            }

            let Some(child) = self.child.as_mut() else {
                break;
            };
            match child.try_wait().map_err(|e| e.to_string())? {
                Some(status) => {
                    let exit_code = status.code();
                    self.child = None;
                    return Ok(self.finish(exit_code, false, false));
                }
                None => thread::sleep(Duration::from_millis(50)),
            }
        }

        // После cancel/timeout процесс уже снят; код выхода обычно отсутствует.
        Ok(self.finish(None, timed_out, cancelled))
    }

    fn finish(
        &self,
        exit_code: Option<i32>,
        timed_out: bool,
        cancelled: bool,
    ) -> ClaudeRunResult {
        // Дать ридерам дочитать хвост пайпа.
        thread::sleep(Duration::from_millis(20));
        let stdout = self
            .stdout_buf
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        let stderr = self
            .stderr_buf
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        ClaudeRunResult {
            stdout,
            stderr,
            exit_code,
            duration_ms: self.started.elapsed().as_millis() as u64,
            timed_out,
            cancelled,
        }
    }
}

fn spawn_pipe_reader<R: std::io::Read + Send + 'static>(reader: R, buf: Arc<Mutex<String>>) {
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if let Ok(mut g) = buf.lock() {
                        g.push_str(&line);
                    }
                }
                Err(_) => break,
            }
        }
    });
}

#[cfg(unix)]
fn signal_group(pgid: &str, sig: &str) -> Result<(), String> {
    let status = Command::new("kill")
        .arg(format!("-{sig}"))
        .arg("--")
        .arg(pgid)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("kill -{sig}: {e}"))?;
    if !status.success() {
        // Процесс мог уже завершиться — не считаем ошибкой для гонки try_wait.
        return Ok(());
    }
    Ok(())
}

/// Собирает промпт и запускает Claude headless с таймаутом и опциональной отменой.
#[cfg(unix)]
pub fn run_claude(
    worktree: &Path,
    task: &str,
    timeout: Duration,
    cancel: Option<&AtomicBool>,
) -> Result<ClaudeRunResult, String> {
    let prompt = build_prompt(task, worktree)?;
    let mut handle = ClaudeHandle::spawn(worktree, &prompt)?;
    let deadline = Instant::now() + timeout;
    handle.wait_until(deadline, cancel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use RunStatus::*;
    use std::fs;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    /// Сериализация тестов с `DEMERZEL_WORKTREES_DIR` / env.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn cannot_accept_without_review_inner() {
        assert!(!can_transition(Running, Accepted));
        assert!(can_transition(NeedsReview, Accepted));
    }

    #[test]
    fn cannot_accept_without_review() {
        cannot_accept_without_review_inner();
    }

    #[test]
    fn default_concurrency_is_one() {
        assert_eq!(DEFAULT_CONCURRENCY, 1);
    }

    #[test]
    fn branch_name_format() {
        assert_eq!(branch_name("run-42").unwrap(), "agent/run-42");
        assert!(branch_name("../x").is_err());
        assert!(branch_name("a/b").is_err());
        assert!(branch_name("").is_err());
    }

    #[test]
    fn worktree_path_under_override_root() {
        let _guard = ENV_LOCK.lock().unwrap();
        let root = std::env::temp_dir().join(format!("demerzel-wt-root-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        unsafe { std::env::set_var("DEMERZEL_WORKTREES_DIR", &root) };
        let path = worktree_path("demo", "r1").unwrap();
        assert_eq!(path, root.join("demo").join("r1"));
        unsafe { std::env::remove_var("DEMERZEL_WORKTREES_DIR") };
        let _ = fs::remove_dir_all(&root);
    }

    fn init_repo(dir: &Path) {
        fs::create_dir_all(dir).unwrap();
        run_git(dir, &["init"]).unwrap();
        run_git(dir, &["config", "user.email", "test@demerzel.local"]).unwrap();
        run_git(dir, &["config", "user.name", "Demerzel Test"]).unwrap();
        // Без default branch name на старых git.
        let _ = run_git(dir, &["checkout", "-b", "main"]);
        fs::write(dir.join("README.md"), "main\n").unwrap();
        run_git(dir, &["add", "README.md"]).unwrap();
        run_git(dir, &["commit", "-m", "init"]).unwrap();
    }

    fn git_stdout(repo: &Path, args: &[&str]) -> String {
        let out = run_git(repo, args).unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    #[test]
    fn create_and_remove_worktree_isolates_main() {
        let _guard = ENV_LOCK.lock().unwrap();
        let pid = std::process::id();
        let repo = std::env::temp_dir().join(format!("demerzel-repo-{pid}"));
        let wt_root = std::env::temp_dir().join(format!("demerzel-worktrees-{pid}"));
        let _ = fs::remove_dir_all(&repo);
        let _ = fs::remove_dir_all(&wt_root);
        init_repo(&repo);
        unsafe { std::env::set_var("DEMERZEL_WORKTREES_DIR", &wt_root) };

        let main_before = git_stdout(&repo, &["rev-parse", "HEAD"]);
        let status_before = git_stdout(&repo, &["status", "--porcelain"]);
        assert!(status_before.is_empty(), "repo must be clean before: {status_before}");

        let handle = create_worktree(&repo, "my-app", "run-100", None).unwrap();
        assert!(handle.path.is_dir());
        assert_eq!(handle.branch, "agent/run-100");
        assert_eq!(handle.path, wt_root.join("my-app").join("run-100"));

        // Правка только в worktree.
        fs::write(handle.path.join("agent-change.txt"), "from agent\n").unwrap();
        run_git(&handle.path, &["add", "agent-change.txt"]).unwrap();
        run_git(&handle.path, &["commit", "-m", "agent wip"]).unwrap();

        // Основной каталог и ветка main не изменились.
        let status_main = git_stdout(&repo, &["status", "--porcelain"]);
        assert!(
            status_main.is_empty(),
            "main worktree must stay clean, got: {status_main}"
        );
        assert_eq!(git_stdout(&repo, &["rev-parse", "HEAD"]), main_before);
        assert_eq!(
            git_stdout(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]),
            "main"
        );
        assert!(
            !repo.join("agent-change.txt").exists(),
            "agent file must not appear in main worktree"
        );
        // На ветке агента коммит есть.
        assert_ne!(
            git_stdout(&repo, &["rev-parse", "agent/run-100"]),
            main_before
        );

        remove_worktree(&repo, "my-app", "run-100").unwrap();
        assert!(!handle.path.exists());
        let branches = git_stdout(&repo, &["branch", "--list", "agent/run-100"]);
        assert!(branches.is_empty(), "agent branch must be deleted: {branches}");
        assert_eq!(git_stdout(&repo, &["rev-parse", "HEAD"]), main_before);

        unsafe { std::env::remove_var("DEMERZEL_WORKTREES_DIR") };
        let _ = fs::remove_dir_all(&repo);
        let _ = fs::remove_dir_all(&wt_root);
    }

    #[test]
    fn remove_is_idempotent_when_path_missing() {
        let _guard = ENV_LOCK.lock().unwrap();
        let pid = std::process::id();
        let repo = std::env::temp_dir().join(format!("demerzel-repo-rm-{pid}"));
        let wt_root = std::env::temp_dir().join(format!("demerzel-worktrees-rm-{pid}"));
        let _ = fs::remove_dir_all(&repo);
        let _ = fs::remove_dir_all(&wt_root);
        init_repo(&repo);
        unsafe { std::env::set_var("DEMERZEL_WORKTREES_DIR", &wt_root) };

        create_worktree(&repo, "p", "r2", Some("HEAD")).unwrap();
        remove_worktree(&repo, "p", "r2").unwrap();
        // Повторная очистка после отмены не должна падать.
        remove_worktree(&repo, "p", "r2").unwrap();

        unsafe { std::env::remove_var("DEMERZEL_WORKTREES_DIR") };
        let _ = fs::remove_dir_all(&repo);
        let _ = fs::remove_dir_all(&wt_root);
    }

    #[test]
    fn rejects_duplicate_worktree_path() {
        let _guard = ENV_LOCK.lock().unwrap();
        let pid = std::process::id();
        let repo = std::env::temp_dir().join(format!("demerzel-repo-dup-{pid}"));
        let wt_root = std::env::temp_dir().join(format!("demerzel-worktrees-dup-{pid}"));
        let _ = fs::remove_dir_all(&repo);
        let _ = fs::remove_dir_all(&wt_root);
        init_repo(&repo);
        unsafe { std::env::set_var("DEMERZEL_WORKTREES_DIR", &wt_root) };

        create_worktree(&repo, "p", "same", None).unwrap();
        let err = create_worktree(&repo, "p", "same", None).unwrap_err();
        assert!(err.contains("already exists"), "{err}");

        remove_worktree(&repo, "p", "same").unwrap();
        unsafe { std::env::remove_var("DEMERZEL_WORKTREES_DIR") };
        let _ = fs::remove_dir_all(&repo);
        let _ = fs::remove_dir_all(&wt_root);
    }

    #[test]
    fn build_prompt_task_only_without_rules() {
        let dir = std::env::temp_dir().join(format!("demerzel-prompt-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let prompt = build_prompt("  Fix the flaky test  ", &dir).unwrap();
        assert_eq!(prompt, "# Task\n\nFix the flaky test");
        assert!(build_prompt("   ", &dir).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn build_prompt_includes_claude_md() {
        let dir = std::env::temp_dir().join(format!("demerzel-prompt-rules-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("CLAUDE.md"), "Use bun, not npm.\n").unwrap();
        let prompt = build_prompt("Add logout button", &dir).unwrap();
        assert!(prompt.starts_with("# Project rules\n\nUse bun, not npm."));
        assert!(prompt.contains("# Task\n\nAdd logout button"));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Пишет исполняемый фейковый `claude` и выставляет `DEMERZEL_CLAUDE_BIN`.
    fn install_fake_claude(dir: &Path, body: &str) -> PathBuf {
        let bin = dir.join("fake-claude");
        fs::write(&bin, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&bin).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&bin, perms).unwrap();
        }
        unsafe { std::env::set_var("DEMERZEL_CLAUDE_BIN", &bin) };
        bin
    }

    #[test]
    #[cfg(unix)]
    fn claude_runner_collects_stdout_and_exit() {
        let _guard = ENV_LOCK.lock().unwrap();
        let pid = std::process::id();
        let dir = std::env::temp_dir().join(format!("demerzel-claude-ok-{pid}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        install_fake_claude(
            &dir,
            "#!/bin/sh\necho \"args:$*\"\necho hi-from-claude\nexit 0\n",
        );
        fs::write(dir.join("CLAUDE.md"), "Be concise.\n").unwrap();

        let result = run_claude(
            &dir,
            "Say hello",
            Duration::from_secs(5),
            None,
        )
        .unwrap();
        assert_eq!(result.exit_code, Some(0));
        assert!(!result.timed_out);
        assert!(!result.cancelled);
        assert!(result.stdout.contains("hi-from-claude"), "{}", result.stdout);
        assert!(result.stdout.contains("-p"), "{}", result.stdout);
        assert!(result.duration_ms < 5000);

        unsafe { std::env::remove_var("DEMERZEL_CLAUDE_BIN") };
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    #[cfg(unix)]
    fn claude_runner_times_out_and_kills() {
        let _guard = ENV_LOCK.lock().unwrap();
        let pid = std::process::id();
        let dir = std::env::temp_dir().join(format!("demerzel-claude-to-{pid}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        install_fake_claude(&dir, "#!/bin/sh\nsleep 30\n");

        let started = Instant::now();
        let result = run_claude(
            &dir,
            "slow task",
            Duration::from_millis(300),
            None,
        )
        .unwrap();
        assert!(result.timed_out);
        assert!(!result.cancelled);
        assert!(started.elapsed() < Duration::from_secs(5));

        unsafe { std::env::remove_var("DEMERZEL_CLAUDE_BIN") };
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    #[cfg(unix)]
    fn claude_runner_cancel_via_flag() {
        let _guard = ENV_LOCK.lock().unwrap();
        let pid = std::process::id();
        let dir = std::env::temp_dir().join(format!("demerzel-claude-cancel-{pid}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        install_fake_claude(&dir, "#!/bin/sh\nsleep 30\n");

        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_flag = cancel.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(150));
            cancel_flag.store(true, Ordering::SeqCst);
        });

        let started = Instant::now();
        let result = run_claude(
            &dir,
            "cancel me",
            Duration::from_secs(30),
            Some(cancel.as_ref()),
        )
        .unwrap();
        assert!(result.cancelled);
        assert!(!result.timed_out);
        assert!(started.elapsed() < Duration::from_secs(5));

        unsafe { std::env::remove_var("DEMERZEL_CLAUDE_BIN") };
        let _ = fs::remove_dir_all(&dir);
    }

    // --- concurrency + memory (#29) ---

    #[derive(Clone)]
    struct FixedMemory(u64);
    impl MemoryProbe for FixedMemory {
        fn available_mb(&self) -> Result<u64, String> {
            Ok(self.0)
        }
    }

    #[derive(Clone)]
    struct FixedOllama(bool);
    impl OllamaProbe for FixedOllama {
        fn is_running(&self) -> bool {
            self.0
        }
    }

    fn limiter(max: usize, free_mb: u64, ollama: bool) -> AgentLimiter<FixedMemory, FixedOllama> {
        let cfg = AgentsConfig {
            max_concurrent: max,
            min_free_memory_mb: 2048,
            ollama_reserve_mb: 4096,
        };
        AgentLimiter::with_probes(cfg, FixedMemory(free_mb), FixedOllama(ollama))
    }

    #[test]
    fn agents_config_defaults() {
        let cfg = AgentsConfig::default();
        assert_eq!(cfg.max_concurrent, 1);
        assert_eq!(cfg.min_free_memory_mb, 2048);
        assert_eq!(cfg.ollama_reserve_mb, 4096);
    }

    #[test]
    fn concurrency_limit_queues_second_agent() {
        let mut gate = limiter(1, 16_000, false);
        assert_eq!(gate.try_acquire("a").unwrap(), StartDecision::Start);
        assert_eq!(gate.running_count(), 1);
        match gate.try_acquire("b").unwrap() {
            StartDecision::Queue(QueueReason::ConcurrencyLimit { running, max }) => {
                assert_eq!(running, 1);
                assert_eq!(max, 1);
            }
            other => panic!("expected concurrency queue, got {other:?}"),
        }
        gate.release("a");
        assert_eq!(gate.try_acquire("b").unwrap(), StartDecision::Start);
        assert!(gate.is_running("b"));
    }

    #[test]
    fn concurrency_allows_up_to_max() {
        let mut gate = limiter(2, 16_000, false);
        assert_eq!(gate.try_acquire("a").unwrap(), StartDecision::Start);
        assert_eq!(gate.try_acquire("b").unwrap(), StartDecision::Start);
        assert!(matches!(
            gate.try_acquire("c").unwrap(),
            StartDecision::Queue(QueueReason::ConcurrencyLimit { max: 2, .. })
        ));
    }

    #[test]
    fn low_memory_queues_even_with_free_slot() {
        let mut gate = limiter(2, 500, false);
        match gate.try_acquire("a").unwrap() {
            StartDecision::Queue(QueueReason::InsufficientMemory {
                free_mb,
                required_mb,
                ollama_running,
            }) => {
                assert_eq!(free_mb, 500);
                assert_eq!(required_mb, 2048);
                assert!(!ollama_running);
            }
            other => panic!("expected memory queue, got {other:?}"),
        }
        assert_eq!(gate.running_count(), 0);
    }

    #[test]
    fn ollama_reserve_raises_memory_requirement() {
        // 5000 >= 2048, но с Ollama нужно 2048+4096=6144.
        let mut gate = limiter(1, 5000, true);
        match gate.try_acquire("a").unwrap() {
            StartDecision::Queue(QueueReason::InsufficientMemory {
                required_mb,
                ollama_running,
                ..
            }) => {
                assert_eq!(required_mb, 6144);
                assert!(ollama_running);
            }
            other => panic!("expected ollama memory queue, got {other:?}"),
        }
        let mut ok = limiter(1, 7000, true);
        assert_eq!(ok.try_acquire("a").unwrap(), StartDecision::Start);
        assert_eq!(ok.required_free_mb(), 6144);
    }

    #[test]
    fn parse_vm_stat_fixture() {
        let sample = "\
Mach Virtual Memory Statistics: (page size of 16384 bytes)
Pages free:                               1024.
Pages active:                             10000.
Pages inactive:                           2048.
Pages speculative:                         512.
Pages wired down:                         9000.
";
        // (1024+2048+512)*16384 / 1MiB = 3584*16384/1048576 = 56
        assert_eq!(parse_vm_stat_available_mb(sample).unwrap(), 56);
    }

    #[test]
    #[cfg(unix)]
    fn system_memory_probe_returns_positive() {
        let mb = available_memory_mb().unwrap();
        assert!(mb > 0, "expected positive free memory, got {mb}");
    }

    #[test]
    fn try_acquire_rejects_empty_run_id() {
        let mut gate = limiter(1, 16_000, false);
        assert!(gate.try_acquire("").is_err());
        assert!(gate.try_acquire("  ").is_err());
    }

    #[test]
    fn reacquire_same_run_id_is_idempotent() {
        let mut gate = limiter(1, 16_000, false);
        assert_eq!(gate.try_acquire("same").unwrap(), StartDecision::Start);
        assert_eq!(gate.try_acquire("same").unwrap(), StartDecision::Start);
        assert_eq!(gate.running_count(), 1);
    }
}
