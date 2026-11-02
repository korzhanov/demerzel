//! Менеджер процессов: статусы и переходы (#8, #9).

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Status {
    Stopped,
    Starting,
    Running,
    Exited(i32),
    Crashed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Start,
    Started,
    Stop,
    Exit(i32),
    Signal,
}

/// Чистая функция переходов: не запускает процессы, только считает статус.
pub fn next(status: Status, event: Event) -> Status {
    use Event::*;
    use Status::*;
    match (status, event) {
        (Stopped | Exited(_) | Crashed, Start) => Starting,
        (Starting, Started) => Running,
        (Running | Starting, Stop) => Stopped,
        (Running | Starting, Exit(0)) => Exited(0),
        (Running | Starting, Exit(code)) => Exited(code),
        (Running | Starting, Signal) => Crashed,
        (s, _) => s,
    }
}

use std::collections::BTreeMap;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::logs::{SharedLog, Stream};
use super::session;

/// Файлы, куда пишутся stdout и stderr процесса.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogFiles {
    pub out: PathBuf,
    pub err: PathBuf,
}

/// Управляемый процесс: запускается в своей process group, останавливается целиком.
///
/// Бывает трёх видов:
/// - запущен этим экземпляром Demerzel (`child` есть, `owned`);
/// - восстановлен после перезапуска приложения (нет `child`, но `owned` и группа та же);
/// - присоединён извне по PID (`owned = false`): останавливаем только сам процесс.
pub struct Managed {
    child: Option<Child>,
    pid: u32,
    pgid: u32,
    owned: bool,
    status: Status,
    log: SharedLog,
    logs: Option<LogFiles>,
    done: Arc<AtomicBool>,
}

impl Managed {
    /// Запускает команду через `sh -c` в папке проекта. Вывод пишется в `logs`.
    /// `port` передаётся как `PORT`, остальные переменные из `env`.
    #[cfg(unix)]
    pub fn spawn(
        command: &str,
        cwd: &Path,
        env: &BTreeMap<String, String>,
        port: Option<u16>,
        logs: &LogFiles,
    ) -> io::Result<Self> {
        use std::os::unix::process::CommandExt;
        if let Some(dir) = logs.out.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let out = File::create(&logs.out)?;
        let err = File::create(&logs.err)?;
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command).current_dir(cwd)
            .stdin(Stdio::null()).stdout(Stdio::from(out)).stderr(Stdio::from(err))
            .process_group(0);
        for (k, v) in env {
            cmd.env(k, v);
        }
        if !env.contains_key("PATH") {
            let home = std::env::var_os("HOME").map(PathBuf::from);
            let current = std::env::var("PATH").ok();
            cmd.env("PATH", super::env::augmented_path(current.as_deref(), home.as_deref()));
        }
        if let Some(p) = port {
            cmd.env("PORT", p.to_string());
        }
        let child = cmd.spawn()?;
        let pid = child.id();
        let log = super::logs::new_shared(super::logs::DEFAULT_CAPACITY);
        let done = Arc::new(AtomicBool::new(false));
        start_tailers(logs, &log, &done, 0, 0);
        let status = next(Status::Stopped, Event::Start);
        let status = next(status, Event::Started);
        Ok(Self {
            child: Some(child),
            pid,
            pgid: pid,
            owned: true,
            status,
            log,
            logs: Some(logs.clone()),
            done,
        })
    }

    /// Берёт под управление уже работающий процесс (после перезапуска или по PID).
    /// Логи дочитываются из файлов, если они заданы: с конца, последние строки.
    pub fn adopt(pid: u32, pgid: u32, owned: bool, logs: Option<LogFiles>) -> Self {
        let log = super::logs::new_shared(super::logs::DEFAULT_CAPACITY);
        let done = Arc::new(AtomicBool::new(false));
        if let Some(files) = &logs {
            let out_start = super::logs::tail_start(&files.out);
            let err_start = super::logs::tail_start(&files.err);
            start_tailers(files, &log, &done, out_start, err_start);
        }
        Self {
            child: None,
            pid,
            pgid,
            owned,
            status: Status::Running,
            log,
            logs,
            done,
        }
    }

    pub fn status(&self) -> Status {
        self.status
    }

    /// Последние строки логов процесса.
    pub fn logs(&self) -> Vec<super::logs::LogLine> {
        self.log.lock().map(|b| b.snapshot()).unwrap_or_default()
    }

    /// PID, пока процесс жив.
    pub fn pid(&self) -> Option<u32> {
        match self.status {
            Status::Running | Status::Starting => Some(self.pid),
            _ => None,
        }
    }

    pub fn pgid(&self) -> u32 {
        self.pgid
    }

    pub fn owned(&self) -> bool {
        self.owned
    }

    pub fn log_files(&self) -> Option<&LogFiles> {
        self.logs.as_ref()
    }

    /// Проверяет, завершился ли процесс, и обновляет статус.
    pub fn poll(&mut self) -> io::Result<Status> {
        if matches!(self.status, Status::Running | Status::Starting) {
            match self.child.as_mut() {
                Some(child) => {
                    if let Some(exit) = child.try_wait()? {
                        let code = exit.code().unwrap_or(-1);
                        self.status = next(self.status, Event::Exit(code));
                        self.child = None;
                        self.done.store(true, Ordering::SeqCst);
                    }
                }
                None => {
                    if !session::is_alive(self.pid) {
                        // Завершился, пока за ним не было наблюдения: код неизвестен.
                        self.status = next(self.status, Event::Stop);
                        self.done.store(true, Ordering::SeqCst);
                    }
                }
            }
        }
        Ok(self.status)
    }

    fn is_alive(&mut self) -> bool {
        match self.child.as_mut() {
            Some(child) => matches!(child.try_wait(), Ok(None)),
            None => session::is_alive(self.pid),
        }
    }

    /// Останавливает процесс: сначала TERM, через таймаут KILL.
    /// Запущенный Demerzel процесс останавливается вместе с группой, присоединённый — только сам.
    #[cfg(unix)]
    pub fn stop(&mut self, grace: Duration) -> io::Result<Status> {
        if !self.is_alive() {
            self.finish();
            return Ok(self.status);
        }
        let target = if self.owned {
            format!("-{}", self.pgid)
        } else {
            self.pid.to_string()
        };
        signal(&target, "TERM")?;
        let deadline = Instant::now() + grace;
        while Instant::now() < deadline && self.is_alive() {
            std::thread::sleep(Duration::from_millis(50));
        }
        if self.is_alive() {
            signal(&target, "KILL")?;
        }
        if let Some(mut child) = self.child.take() {
            let _ = child.wait();
        }
        self.finish();
        Ok(self.status)
    }

    fn finish(&mut self) {
        self.status = next(self.status, Event::Stop);
        self.done.store(true, Ordering::SeqCst);
    }
}

/// Запускает читатели stdout и stderr с заданных смещений.
fn start_tailers(logs: &LogFiles, log: &SharedLog, done: &Arc<AtomicBool>, out_start: u64, err_start: u64) {
    super::logs::spawn_file_tailer(logs.out.clone(), Stream::Stdout, log.clone(), done.clone(), out_start);
    super::logs::spawn_file_tailer(logs.err.clone(), Stream::Stderr, log.clone(), done.clone(), err_start);
}

#[cfg(unix)]
impl Managed {
    /// Присоединяет процесс, запущенный не Demerzel, по PID. Логов у такого процесса нет.
    pub fn attach(pid: u32) -> Self {
        Self::adopt(pid, pgid_or(pid), false, None)
    }
}

#[cfg(unix)]
fn pgid_or(pid: u32) -> u32 {
    session::pgid_of(pid).unwrap_or(pid)
}

#[cfg(unix)]
fn signal(target: &str, sig: &str) -> io::Result<()> {
    Command::new("kill")
        .arg(format!("-{sig}"))
        .arg("--")
        .arg(target)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_then_started_is_running() {
        let s = next(Status::Stopped, Event::Start);
        assert_eq!(s, Status::Starting);
        assert_eq!(next(s, Event::Started), Status::Running);
    }

    #[test]
    fn nonzero_exit_is_recorded_with_code() {
        assert_eq!(next(Status::Running, Event::Exit(1)), Status::Exited(1));
    }

    #[test]
    fn stop_from_running_returns_to_stopped() {
        assert_eq!(next(Status::Running, Event::Stop), Status::Stopped);
    }
}

#[cfg(test)]
mod process_tests {
    use super::*;
    use std::time::Duration;

    fn here() -> std::path::PathBuf {
        std::env::current_dir().unwrap()
    }

    fn logs_in(name: &str) -> LogFiles {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("demerzel-proc-{name}-{nanos}"));
        LogFiles { out: dir.join("out.log"), err: dir.join("err.log") }
    }

    fn wait_exit(m: &mut Managed) {
        for _ in 0..100 {
            if m.poll().unwrap() != Status::Running {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    #[cfg(unix)]
    fn clean_exit_reports_code_zero() {
        let mut m = Managed::spawn("exit 0", &here(), &BTreeMap::new(), None, &logs_in("ok")).unwrap();
        wait_exit(&mut m);
        assert_eq!(m.status(), Status::Exited(0));
    }

    #[test]
    #[cfg(unix)]
    fn nonzero_exit_is_recorded() {
        let mut m = Managed::spawn("exit 3", &here(), &BTreeMap::new(), None, &logs_in("code")).unwrap();
        wait_exit(&mut m);
        assert_eq!(m.status(), Status::Exited(3));
    }

    #[test]
    #[cfg(unix)]
    fn stop_terminates_long_running_process() {
        let mut m = Managed::spawn("sleep 30", &here(), &BTreeMap::new(), None, &logs_in("stop")).unwrap();
        assert_eq!(m.status(), Status::Running);
        let status = m.stop(Duration::from_secs(2)).unwrap();
        assert_eq!(status, Status::Stopped);
    }

    #[test]
    #[cfg(unix)]
    fn port_is_passed_as_env() {
        let mut m = Managed::spawn(
            "test \"$PORT\" = 5123",
            &here(),
            &BTreeMap::new(),
            Some(5123),
            &logs_in("port"),
        )
        .unwrap();
        wait_exit(&mut m);
        assert_eq!(m.status(), Status::Exited(0));
    }

    #[test]
    #[cfg(unix)]
    fn output_goes_to_log_files_and_is_read_back() {
        let files = logs_in("files");
        let mut m = Managed::spawn("echo hello; echo oops 1>&2", &here(), &BTreeMap::new(), None, &files).unwrap();
        wait_exit(&mut m);
        std::thread::sleep(Duration::from_millis(400));
        let texts: Vec<String> = m.logs().into_iter().map(|l| l.text).collect();
        assert!(texts.contains(&"hello".to_string()));
        assert!(texts.contains(&"oops".to_string()));
    }

    #[test]
    #[cfg(unix)]
    fn adopted_owned_process_keeps_running_and_stops_as_group() {
        use std::os::unix::process::CommandExt;
        let mut child = Command::new("sleep").arg("30").process_group(0).spawn().unwrap();
        let pid = child.id();
        let mut m = Managed::adopt(pid, pid, true, None);
        assert_eq!(m.status(), Status::Running);
        assert_eq!(m.pid(), Some(pid));
        assert_eq!(m.stop(Duration::from_secs(2)).unwrap(), Status::Stopped);
        let _ = child.wait();
    }

    #[test]
    #[cfg(unix)]
    fn attached_external_process_is_stopped_by_pid_only() {
        let mut child = Command::new("sleep").arg("30").spawn().unwrap();
        let pid = child.id();
        let mut m = Managed::attach(pid);
        assert!(!m.owned());
        assert_eq!(m.stop(Duration::from_secs(2)).unwrap(), Status::Stopped);
        let _ = child.wait();
    }
}
