//! Реестр запущенных процессов для UI (#10).
//!
//! Держит `Managed` по идентификатору. Команды Tauri работают через этот реестр.
//! При старте выделяет порт из диапазона конфига и передаёт `PORT` (#13).
//! Занятость = аренды ∪ слушающие сокеты ОС через lsof (#14).

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::logs::LogLine;
use super::process::{LogFiles, Managed, Status};
use super::session::{self, Record, State};

/// Откуда процесс в реестре: запущен здесь, восстановлен после перезапуска или присоединён по PID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    Started,
    Restored,
    Attached,
}

pub struct Entry {
    pub project: String,
    pub command: String,
    pub path: PathBuf,
    pub port: Option<u16>,
    pub origin: Origin,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub managed: Managed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Row {
    pub id: u64,
    /// Имя проекта. Не уникально: подпроекты часто называются одинаково (www, frontend).
    pub project: String,
    /// Папка процесса. По ней сопоставляется с проектом.
    pub path: String,
    pub command: String,
    pub status: Status,
    pub pid: Option<u32>,
    pub port: Option<u16>,
    pub origin: Origin,
    pub owned: bool,
    pub started_at: String,
    pub finished_at: Option<String>,
}

pub struct Registry {
    next_id: u64,
    entries: BTreeMap<u64, Entry>,
    pub ports: super::ports::Allocator,
    /// Файл состояния. Без него реестр живёт только в памяти (тесты).
    state_path: Option<PathBuf>,
    /// Папка для логов процессов: по два файла на процесс.
    logs_dir: PathBuf,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            next_id: 0,
            entries: BTreeMap::new(),
            ports: super::ports::Allocator::default(),
            state_path: None,
            logs_dir: std::env::temp_dir().join("demerzel-process-logs"),
        }
    }
}

fn is_live(status: Status) -> bool {
    matches!(status, Status::Running | Status::Starting)
}

impl Registry {
    /// Задаёт файл состояния и папку логов. Вызывается один раз при старте приложения.
    pub fn configure(&mut self, state_path: PathBuf, logs_dir: PathBuf) {
        self.state_path = Some(state_path);
        self.logs_dir = logs_dir;
    }

    fn log_files(&self, id: u64) -> LogFiles {
        LogFiles {
            out: self.logs_dir.join(format!("{id}.out.log")),
            err: self.logs_dir.join(format!("{id}.err.log")),
        }
    }

    fn has_live_at(&self, cwd: &Path) -> bool {
        self.entries
            .values()
            .any(|e| e.path == cwd && is_live(e.managed.status()))
    }

    #[cfg(unix)]
    pub fn start(
        &mut self,
        project: &str,
        command: &str,
        cwd: &Path,
        env: &BTreeMap<String, String>,
        port: Option<u16>,
    ) -> io::Result<u64> {
        self.list()?;
        if self.has_live_at(cwd) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "project already running",
            ));
        }
        let id = self.next_id + 1;
        let logs = self.log_files(id);
        let managed = Managed::spawn(command, cwd, env, port, &logs)?;
        self.next_id = id;
        self.entries.insert(
            id,
            Entry {
                project: project.to_string(),
                command: command.to_string(),
                path: cwd.into(),
                port,
                origin: Origin::Started,
                started_at: now_rfc3339(),
                finished_at: None,
                managed,
            },
        );
        if let Some(port) = port {
            self.ports.leases.insert(port, id);
        }
        self.persist();
        Ok(id)
    }

    /// Восстанавливает процессы, которые остались работать после прошлого запуска приложения.
    /// Запись берётся, только если PID жив и командная строка совпадает.
    pub fn restore(&mut self) {
        let Some(path) = self.state_path.clone() else {
            return;
        };
        let state = session::load(&path);
        self.next_id = self.next_id.max(state.next_id);
        for rec in state.records {
            let alive = session::is_alive(rec.pid);
            let same = session::command_line(rec.pid)
                .is_some_and(|c| session::same_process(&rec.command, &c));
            if !(alive && same) {
                continue;
            }
            let logs = match (rec.out_log, rec.err_log) {
                (Some(out), Some(err)) => Some(LogFiles { out: out.into(), err: err.into() }),
                _ => None,
            };
            let origin = if rec.attached { Origin::Attached } else { Origin::Restored };
            let managed = Managed::adopt(rec.pid, rec.pgid, rec.owned, logs);
            self.entries.insert(
                rec.id,
                Entry {
                    project: rec.project,
                    command: rec.command,
                    path: rec.path.into(),
                    port: rec.port,
                    origin,
                    started_at: rec.started_at.unwrap_or_else(now_rfc3339),
                    finished_at: None,
                    managed,
                },
            );
            if let Some(port) = rec.port {
                self.ports.leases.insert(port, rec.id);
            }
        }
        self.persist();
    }

    /// Присоединяет процесс, запущенный не Demerzel, по PID (например, найденный в списке портов).
    #[cfg(unix)]
    pub fn attach(&mut self, project: &str, pid: u32, cwd: &Path, port: Option<u16>) -> Result<u64, String> {
        self.list().map_err(|e| e.to_string())?;
        if !session::is_alive(pid) {
            return Err(format!("процесс {pid} уже не работает"));
        }
        let already = self
            .entries
            .values()
            .any(|e| e.managed.pid() == Some(pid));
        if already {
            return Err(format!("процесс {pid} уже под управлением Demerzel"));
        }
        let command = session::command_line(pid).unwrap_or_else(|| pid.to_string());
        let managed = Managed::attach(pid);
        self.next_id += 1;
        let id = self.next_id;
        self.entries.insert(
            id,
            Entry {
                project: project.to_string(),
                command,
                path: cwd.into(),
                port,
                origin: Origin::Attached,
                started_at: now_rfc3339(),
                finished_at: None,
                managed,
            },
        );
        self.persist();
        Ok(id)
    }

    /// Записывает действующие процессы в файл состояния.
    fn persist(&self) {
        let Some(path) = &self.state_path else {
            return;
        };
        let records = self
            .entries
            .iter()
            .filter(|(_, e)| is_live(e.managed.status()))
            .filter_map(|(id, e)| {
                let pid = e.managed.pid()?;
                Some(Record {
                    id: *id,
                    project: e.project.clone(),
                    command: e.command.clone(),
                    path: e.path.to_string_lossy().into_owned(),
                    port: e.port,
                    pid,
                    pgid: e.managed.pgid(),
                    owned: e.managed.owned(),
                    attached: e.origin == Origin::Attached,
                    started_at: Some(e.started_at.clone()),
                    out_log: e.managed.log_files().map(|f| f.out.to_string_lossy().into_owned()),
                    err_log: e.managed.log_files().map(|f| f.err.to_string_lossy().into_owned()),
                })
            })
            .collect();
        let state = State { next_id: self.next_id, records };
        if let Err(err) = session::save(path, &state) {
            eprintln!("demerzel: не удалось сохранить список процессов: {err}");
        }
    }

    #[cfg(unix)]
    pub fn stop(&mut self, id: u64, grace: Duration) -> io::Result<Option<Status>> {
        match self.entries.get_mut(&id) {
            Some(e) => {
                let was_live = is_live(e.managed.status());
                let status = e.managed.stop(grace)?;
                if was_live && e.finished_at.is_none() {
                    e.finished_at = Some(now_rfc3339());
                }
                self.ports.release(id);
                self.persist();
                Ok(Some(status))
            }
            None => Ok(None),
        }
    }

    /// Обновляет статусы и возвращает строки для таблицы.
    pub fn list(&mut self) -> io::Result<Vec<Row>> {
        let live_before = self.entries.values().filter(|e| is_live(e.managed.status())).count();
        let mut rows = Vec::with_capacity(self.entries.len());
        for (id, e) in self.entries.iter_mut() {
            let was_live = is_live(e.managed.status());
            let status = e.managed.poll()?;
            if was_live && !is_live(status) && e.finished_at.is_none() {
                e.finished_at = Some(now_rfc3339());
            }
            if !is_live(status) {
                self.ports.release(*id);
            }
            rows.push(Row {
                id: *id,
                project: e.project.clone(),
                path: e.path.to_string_lossy().into_owned(),
                command: e.command.clone(),
                status,
                pid: e.managed.pid(),
                port: e.port,
                origin: e.origin,
                owned: e.managed.owned(),
                started_at: e.started_at.clone(),
                finished_at: e.finished_at.clone(),
            });
        }
        let live_after = self.entries.values().filter(|e| is_live(e.managed.status())).count();
        if live_after != live_before {
            self.persist();
        }
        Ok(rows)
    }

    /// Старт с выделением порта из `orchestrator.json` (если задан `ports`).
    pub fn start_configured(
        &mut self,
        project: &str,
        command: &str,
        cwd: &Path,
    ) -> Result<u64, String> {
        self.start_service(project, command, cwd, None)
    }

    /// Старт с опциональным запрошенным портом; иначе default/первый свободный в диапазоне.
    pub fn start_service(
        &mut self,
        project: &str,
        command: &str,
        cwd: &Path,
        requested_port: Option<u16>,
    ) -> Result<u64, String> {
        self.list().map_err(|e| e.to_string())?;
        let cfg = super::config::load_checked(cwd)?;
        let env = cfg.as_ref().map(|c| c.env.clone()).unwrap_or_default();
        let key = cwd.to_string_lossy().into_owned();
        // occupied = слушающие порты ОС; аренды Allocator учитывает отдельно.
        let occupied: BTreeSet<u16> = super::ports::listeners()?
            .into_iter()
            .map(|s| s.port)
            .collect();
        let port = if let Some(ports) = cfg.and_then(|c| c.ports) {
            let range = super::ports::PortRange::new(ports.range[0], ports.range[1])
                .ok_or("invalid range")?;
            self.ports.reserve(&key, range)?;
            if requested_port.is_some_and(|p| {
                !range.contains(p) || occupied.contains(&p) || self.ports.leases.contains_key(&p)
            }) {
                return Err(format!("requested port unavailable for {project}"));
            }
            Some(self.ports.available(
                &key,
                requested_port.or(ports.default),
                &occupied,
            )?)
        } else if let Some(port) = requested_port {
            if port == 0
                || self.ports.leases.contains_key(&port)
                || occupied.contains(&port)
            {
                return Err(format!("port {port} unavailable"));
            }
            Some(port)
        } else {
            None
        };
        self.start(project, command, cwd, &env, port)
            .map_err(|e| e.to_string())
    }

    /// Инвентарь портов. `known_projects` — пути со скана: cwd слушателя
    /// сопоставляется с ними (не даёт `managed_id`, только проект и статус ok).
    pub fn port_inventory(
        &mut self,
        known_projects: &[std::path::PathBuf],
    ) -> Result<Vec<super::ports::PortRow>, String> {
        self.list().map_err(|e| e.to_string())?;
        let sockets = super::ports::listeners()?;
        let mut managed = BTreeMap::new();
        // Управляемые — по process group. cwd сам по себе не делает процесс managed.
        for socket in &sockets {
            let pgid = std::process::Command::new("ps")
                .args(["-o", "pgid=", "-p", &socket.pid.to_string()])
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .and_then(|s| s.trim().parse::<u32>().ok());
            for (id, e) in &self.entries {
                if let Some(pid) = e.managed.pid()
                    && (socket.pid == pid || pgid == Some(pid))
                {
                    managed.insert(socket.pid, (*id, e.path.to_string_lossy().into_owned()));
                }
            }
        }
        let need_cwd: Vec<u32> = sockets
            .iter()
            .filter(|s| !managed.contains_key(&s.pid))
            .map(|s| s.pid)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let cwds = super::ports::process_cwds(&need_cwd);
        Ok(super::ports::inventory(
            sockets,
            &managed,
            &self.ports.ranges,
            known_projects,
            &cwds,
        ))
    }

    pub fn logs(&self, id: u64) -> Option<Vec<LogLine>> {
        self.entries.get(&id).map(|e| e.managed.logs())
    }

    /// Контекст для автофикса (#96): путь, команда, логи по имени проекта.
    pub fn autofix_context_by_project(
        &self,
        project: &str,
    ) -> Option<(std::path::PathBuf, String, Vec<LogLine>)> {
        // Сначала по пути (однозначно), потом по имени для старых отправителей.
        let entry = self
            .entries
            .values()
            .find(|e| e.path.to_string_lossy() == project)
            .or_else(|| self.entries.values().find(|e| e.project == project))?;
        Some((entry.path.clone(), entry.command.clone(), entry.managed.logs()))
    }

    /// Контекст для автофикса (#96) по выделенному порту процесса.
    pub fn autofix_context_by_port(
        &self,
        port: u16,
    ) -> Option<(std::path::PathBuf, String, Vec<LogLine>)> {
        self.entries
            .values()
            .find(|e| e.port == Some(port))
            .map(|e| (e.path.clone(), e.command.clone(), e.managed.logs()))
    }

    /// Путь проекта по id управляемого процесса.
    /// (id, папка, команда, статус) по всем запускам; вызывать после `list`, чтобы статусы были свежими.
    pub fn runs(&self) -> Vec<(u64, std::path::PathBuf, String, Status)> {
        self.entries
            .iter()
            .map(|(id, e)| (*id, e.path.clone(), e.command.clone(), e.managed.status()))
            .collect()
    }

    pub fn path_for_id(&self, id: u64) -> Option<std::path::PathBuf> {
        self.entries.get(&id).map(|e| e.path.clone())
    }
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("demerzel-reg-{name}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    #[cfg(unix)]
    fn start_list_stop_roundtrip() {
        let mut reg = Registry::default();
        let cwd = std::env::current_dir().unwrap();
        let id = reg
            .start("api", "sleep 30", &cwd, &BTreeMap::new(), None)
            .unwrap();
        let rows = reg.list().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, Status::Running);
        assert_eq!(rows[0].project, "api");
        let status = reg.stop(id, Duration::from_secs(2)).unwrap();
        assert_eq!(status, Some(Status::Stopped));
    }

    #[test]
    #[cfg(unix)]
    fn restart_restores_running_process_and_stops_it() {
        let dir = tmp("restore");
        let state = dir.join("processes.json");
        let logs = dir.join("logs");
        let cwd = std::env::current_dir().unwrap();
        // Первый экземпляр приложения запускает процесс и «закрывается».
        let id = {
            let mut first = Registry::default();
            first.configure(state.clone(), logs.clone());
            first.start("api", "sleep 30", &cwd, &BTreeMap::new(), None).unwrap()
        };
        // Новый экземпляр должен взять процесс под управление, а не считать его чужим.
        let mut second = Registry::default();
        second.configure(state.clone(), logs.clone());
        second.restore();
        let rows = second.list().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, id);
        assert_eq!(rows[0].origin, Origin::Restored);
        assert_eq!(rows[0].status, Status::Running);
        assert!(rows[0].owned);
        assert_eq!(second.stop(id, Duration::from_secs(2)).unwrap(), Some(Status::Stopped));
    }

    #[test]
    #[cfg(unix)]
    fn restore_drops_records_whose_process_is_gone() {
        let dir = tmp("gone");
        let state = dir.join("processes.json");
        let record = session::Record {
            id: 7,
            project: "api".into(),
            command: "bun run dev".into(),
            path: dir.to_string_lossy().into_owned(),
            port: Some(5173),
            pid: 4_000_000,
            pgid: 4_000_000,
            owned: true,
            attached: false,
            started_at: None,
            out_log: None,
            err_log: None,
        };
        session::save(&state, &session::State { next_id: 7, records: vec![record] }).unwrap();
        let mut reg = Registry::default();
        reg.configure(state, dir.join("logs"));
        reg.restore();
        assert!(reg.list().unwrap().is_empty());
    }

    #[test]
    #[cfg(unix)]
    fn attach_external_process_by_pid_and_stop_it() {
        let mut child = std::process::Command::new("sleep").arg("30").spawn().unwrap();
        let mut reg = Registry::default();
        let id = reg.attach("ext", child.id(), Path::new("/tmp"), Some(5555)).unwrap();
        let rows = reg.list().unwrap();
        assert_eq!(rows[0].origin, Origin::Attached);
        assert!(!rows[0].owned);
        assert_eq!(reg.stop(id, Duration::from_secs(2)).unwrap(), Some(Status::Stopped));
        let _ = child.wait();
        assert!(reg.attach("ext", 4_000_000, Path::new("/tmp"), None).is_err());
    }

    #[test]
    fn unknown_id_is_none() {
        let reg = Registry::default();
        assert_eq!(reg.logs(42), None);
    }

    #[test]
    #[cfg(unix)]
    fn start_configured_leases_port_and_sets_env() {
        let dir = tmp("ports");
        fs::write(
            dir.join(super::super::config::CONFIG_FILE),
            r#"{ "name": "api", "type": "app", "ports": { "range": [6100, 6102], "default": 6100 }, "commands": { "dev": "sleep 30" } }"#,
        )
        .unwrap();
        let mut reg = Registry::default();
        let id = reg
            .start_configured("api", "test \"$PORT\" = 6100 && sleep 30", &dir)
            .unwrap();
        assert_eq!(reg.ports.leases.get(&6100), Some(&id));
        let rows = reg.list().unwrap();
        assert_eq!(rows[0].port, Some(6100));
        // Второй проект с пересекающимся диапазоном — ошибка.
        let dir2 = tmp("ports2");
        fs::write(
            dir2.join(super::super::config::CONFIG_FILE),
            r#"{ "name": "web", "type": "app", "ports": { "range": [6101, 6200] } }"#,
        )
        .unwrap();
        let err = reg
            .start_configured("web", "sleep 1", &dir2)
            .unwrap_err();
        assert!(err.contains("overlap"), "{err}");
        reg.stop(id, Duration::from_secs(2)).unwrap();
        assert!(!reg.ports.leases.contains_key(&6100));
        fs::remove_dir_all(&dir).unwrap();
        fs::remove_dir_all(&dir2).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn start_configured_skips_os_listener_ports() {
        use std::net::TcpListener;
        let dir = tmp("lsof-skip");
        // Высокий диапазон; держим default занятым через реальный LISTEN.
        let bind = TcpListener::bind("127.0.0.1:48100").expect("bind 48100");
        fs::write(
            dir.join(super::super::config::CONFIG_FILE),
            r#"{ "name": "api", "type": "app", "ports": { "range": [48100, 48102], "default": 48100 }, "commands": { "dev": "sleep 30" } }"#,
        )
        .unwrap();
        let mut reg = Registry::default();
        let id = reg
            .start_configured("api", "test \"$PORT\" = 48101 && sleep 30", &dir)
            .unwrap();
        assert_eq!(reg.ports.leases.get(&48101), Some(&id));
        assert!(!reg.ports.leases.contains_key(&48100));
        reg.stop(id, Duration::from_secs(2)).unwrap();
        drop(bind);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn requested_port_unavailable_when_os_listens() {
        use std::net::TcpListener;
        let dir = tmp("lsof-req");
        let bind = TcpListener::bind("127.0.0.1:48110").expect("bind 48110");
        fs::write(
            dir.join(super::super::config::CONFIG_FILE),
            r#"{ "name": "api", "type": "app", "ports": { "range": [48110, 48112] }, "commands": { "dev": "sleep 1" } }"#,
        )
        .unwrap();
        let mut reg = Registry::default();
        let err = reg
            .start_service("api", "sleep 1", &dir, Some(48110))
            .unwrap_err();
        assert!(err.contains("unavailable"), "{err}");
        drop(bind);
        fs::remove_dir_all(&dir).unwrap();
    }
}
