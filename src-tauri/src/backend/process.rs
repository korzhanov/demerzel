//! Менеджер процессов: статусы и переходы (#8, #9).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
use std::io;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Управляемый процесс: запускается в своей process group, останавливается целиком.
pub struct Managed {
    child: Option<Child>,
    status: Status,
    log: super::logs::SharedLog,
}

impl Managed {
    /// Запускает команду через `sh -c` в папке проекта.
    /// `port` передаётся как `PORT`, остальные переменные из `env`.
    #[cfg(unix)]
    pub fn spawn(
        command: &str,
        cwd: &Path,
        env: &BTreeMap<String, String>,
        port: Option<u16>,
    ) -> io::Result<Self> {
        use std::os::unix::process::CommandExt;
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command).current_dir(cwd)
            .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
            .process_group(0);
        for (k, v) in env {
            cmd.env(k, v);
        }
        if let Some(p) = port {
            cmd.env("PORT", p.to_string());
        }
        let mut child = cmd.spawn()?;
        let log = super::logs::new_shared(super::logs::DEFAULT_CAPACITY);
        if let Some(out) = child.stdout.take() {
            super::logs::spawn_reader(out, super::logs::Stream::Stdout, log.clone());
        }
        if let Some(err) = child.stderr.take() {
            super::logs::spawn_reader(err, super::logs::Stream::Stderr, log.clone());
        }
        let status = next(Status::Stopped, Event::Start);
        let status = next(status, Event::Started);
        Ok(Self { child: Some(child), status, log })
    }

    pub fn status(&self) -> Status {
        self.status
    }

    /// Последние строки логов процесса.
    pub fn logs(&self) -> Vec<super::logs::LogLine> {
        self.log.lock().map(|b| b.snapshot()).unwrap_or_default()
    }

    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().map(|c| c.id())
    }

    /// Проверяет, завершился ли процесс, и обновляет статус.
    pub fn poll(&mut self) -> io::Result<Status> {
        if let Some(child) = self.child.as_mut() {
            if let Some(exit) = child.try_wait()? {
                let code = exit.code().unwrap_or(-1);
                self.status = next(self.status, Event::Exit(code));
                self.child = None;
            }
        }
        Ok(self.status)
    }

    /// Останавливает всю группу процессов: сначала TERM, через таймаут KILL.
    #[cfg(unix)]
    pub fn stop(&mut self, grace: Duration) -> io::Result<Status> {
        let Some(child) = self.child.as_mut() else {
            return Ok(self.status);
        };
        let pgid = format!("-{}", child.id());
        signal_group(&pgid, "TERM")?;
        let deadline = Instant::now() + grace;
        while Instant::now() < deadline {
            if child.try_wait()?.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        if child.try_wait()?.is_none() {
            signal_group(&pgid, "KILL")?;
            let _ = child.wait();
        }
        self.child = None;
        self.status = next(self.status, Event::Stop);
        Ok(self.status)
    }
}

#[cfg(unix)]
fn signal_group(pgid: &str, sig: &str) -> io::Result<()> {
    Command::new("kill")
        .arg(format!("-{sig}"))
        .arg("--")
        .arg(pgid)
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

    #[test]
    #[cfg(unix)]
    fn clean_exit_reports_code_zero() {
        let mut m = Managed::spawn("exit 0", &here(), &BTreeMap::new(), None).unwrap();
        for _ in 0..100 {
            if m.poll().unwrap() != Status::Running { break; }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(m.status(), Status::Exited(0));
    }

    #[test]
    #[cfg(unix)]
    fn nonzero_exit_is_recorded() {
        let mut m = Managed::spawn("exit 3", &here(), &BTreeMap::new(), None).unwrap();
        for _ in 0..100 {
            if m.poll().unwrap() != Status::Running { break; }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(m.status(), Status::Exited(3));
    }

    #[test]
    #[cfg(unix)]
    fn stop_terminates_long_running_process() {
        let mut m = Managed::spawn("sleep 30", &here(), &BTreeMap::new(), None).unwrap();
        assert_eq!(m.status(), Status::Running);
        let status = m.stop(Duration::from_secs(2)).unwrap();
        assert_eq!(status, Status::Stopped);
    }

    #[test]
    #[cfg(unix)]
    fn port_is_passed_as_env() {
        let mut m = Managed::spawn("test \"$PORT\" = 5123", &here(), &BTreeMap::new(), Some(5123)).unwrap();
        for _ in 0..100 {
            if m.poll().unwrap() != Status::Running { break; }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(m.status(), Status::Exited(0));
    }
}
