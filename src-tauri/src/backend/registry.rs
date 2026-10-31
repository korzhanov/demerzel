//! Реестр запущенных процессов для UI (#10).
//!
//! Держит `Managed` по идентификатору. Команды Tauri работают через этот реестр.
//! При старте выделяет порт из диапазона конфига и передаёт `PORT` (#13).
//! Занятость = аренды ∪ слушающие сокеты ОС через lsof (#14).

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;
use std::time::Duration;

use super::logs::LogLine;
use super::process::{Managed, Status};

pub struct Entry {
    pub project: String,
    pub command: String,
    pub path: std::path::PathBuf,
    pub port: Option<u16>,
    pub managed: Managed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Row {
    pub id: u64,
    pub project: String,
    pub command: String,
    pub status: Status,
    pub pid: Option<u32>,
    pub port: Option<u16>,
}

#[derive(Default)]
pub struct Registry {
    next_id: u64,
    entries: BTreeMap<u64, Entry>,
    pub ports: super::ports::Allocator,
}

impl Registry {
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
        if self
            .entries
            .values()
            .any(|e| e.path == cwd && e.managed.status() == Status::Running)
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "project already running",
            ));
        }
        let managed = Managed::spawn(command, cwd, env, port)?;
        self.next_id += 1;
        let id = self.next_id;
        self.entries.insert(
            id,
            Entry {
                project: project.to_string(),
                command: command.to_string(),
                path: cwd.into(),
                port,
                managed,
            },
        );
        if let Some(port) = port {
            self.ports.leases.insert(port, id);
        }
        Ok(id)
    }

    #[cfg(unix)]
    pub fn stop(&mut self, id: u64, grace: Duration) -> io::Result<Option<Status>> {
        match self.entries.get_mut(&id) {
            Some(e) => {
                let status = e.managed.stop(grace)?;
                self.ports.release(id);
                Ok(Some(status))
            }
            None => Ok(None),
        }
    }

    /// Обновляет статусы и возвращает строки для таблицы.
    pub fn list(&mut self) -> io::Result<Vec<Row>> {
        let mut rows = Vec::with_capacity(self.entries.len());
        for (id, e) in self.entries.iter_mut() {
            let status = e.managed.poll()?;
            if status != Status::Running && status != Status::Starting {
                self.ports.release(*id);
            }
            rows.push(Row {
                id: *id,
                project: e.project.clone(),
                command: e.command.clone(),
                status,
                pid: e.managed.pid(),
                port: e.port,
            });
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

    pub fn port_inventory(&mut self) -> Result<Vec<super::ports::PortRow>, String> {
        self.list().map_err(|e| e.to_string())?;
        let sockets = super::ports::listeners()?;
        let mut managed = BTreeMap::new();
        // Resolve child listeners through process groups, never through cwd alone:
        // another process in the same directory is still foreign.
        for socket in &sockets {
            let pgid = std::process::Command::new("ps")
                .args(["-o", "pgid=", "-p", &socket.pid.to_string()])
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .and_then(|s| s.trim().parse::<u32>().ok());
            for (id, e) in &self.entries {
                if let Some(pid) = e.managed.pid() {
                    if socket.pid == pid || pgid == Some(pid) {
                        managed.insert(socket.pid, (*id, e.path.to_string_lossy().into_owned()));
                    }
                }
            }
        }
        Ok(super::ports::inventory(sockets, &managed, &self.ports.ranges))
    }

    pub fn logs(&self, id: u64) -> Option<Vec<LogLine>> {
        self.entries.get(&id).map(|e| e.managed.logs())
    }

    /// Контекст для автофикса (#96): путь, команда, логи по имени проекта.
    pub fn autofix_context_by_project(
        &self,
        project: &str,
    ) -> Option<(std::path::PathBuf, String, Vec<LogLine>)> {
        self.entries.values().find(|e| e.project == project).map(|e| {
            (e.path.clone(), e.command.clone(), e.managed.logs())
        })
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
    pub fn path_for_id(&self, id: u64) -> Option<std::path::PathBuf> {
        self.entries.get(&id).map(|e| e.path.clone())
    }
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
