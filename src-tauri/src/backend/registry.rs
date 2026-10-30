//! Реестр запущенных процессов для UI (#10).
//!
//! Держит `Managed` по идентификатору. Команды Tauri работают через этот реестр.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;
use std::time::Duration;

use super::logs::LogLine;
use super::process::{Managed, Status};

pub struct Entry {
    pub project: String,
    pub command: String,
    pub managed: Managed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Row {
    pub id: u64,
    pub project: String,
    pub command: String,
    pub status: Status,
}

#[derive(Default)]
pub struct Registry {
    next_id: u64,
    entries: BTreeMap<u64, Entry>,
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
        let managed = Managed::spawn(command, cwd, env, port)?;
        self.next_id += 1;
        let id = self.next_id;
        self.entries.insert(
            id,
            Entry { project: project.to_string(), command: command.to_string(), managed },
        );
        Ok(id)
    }

    #[cfg(unix)]
    pub fn stop(&mut self, id: u64, grace: Duration) -> io::Result<Option<Status>> {
        match self.entries.get_mut(&id) {
            Some(e) => e.managed.stop(grace).map(Some),
            None => Ok(None),
        }
    }

    /// Обновляет статусы и возвращает строки для таблицы.
    pub fn list(&mut self) -> io::Result<Vec<Row>> {
        let mut rows = Vec::with_capacity(self.entries.len());
        for (id, e) in self.entries.iter_mut() {
            let status = e.managed.poll()?;
            rows.push(Row {
                id: *id,
                project: e.project.clone(),
                command: e.command.clone(),
                status,
            });
        }
        Ok(rows)
    }

    pub fn logs(&self, id: u64) -> Option<Vec<LogLine>> {
        self.entries.get(&id).map(|e| e.managed.logs())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    #[cfg(unix)]
    fn start_list_stop_roundtrip() {
        let mut reg = Registry::default();
        let cwd = std::env::current_dir().unwrap();
        let id = reg.start("api", "sleep 30", &cwd, &BTreeMap::new(), None).unwrap();
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
}
