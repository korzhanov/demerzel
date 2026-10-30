//! Реестр портов: диапазоны из конфига, аренда и проверка конфликтов (#13).

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

use serde::Serialize;

/// Диапазон портов, который проект резервирует в `orchestrator.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortRange {
    pub start: u16,
    pub end: u16,
}

impl PortRange {
    pub fn new(start: u16, end: u16) -> Option<Self> {
        (start > 0 && start <= end).then_some(Self { start, end })
    }

    pub fn contains(&self, port: u16) -> bool {
        (self.start..=self.end).contains(&port)
    }

    pub fn overlaps(&self, other: &PortRange) -> bool {
        self.start <= other.end && other.start <= self.end
    }
}

/// Выделение портов внутри зарезервированных диапазонов проектов.
#[derive(Default)]
pub struct Allocator {
    pub ranges: BTreeMap<String, PortRange>,
    /// port → process id (аренда).
    pub leases: BTreeMap<u16, u64>,
}

impl Allocator {
    /// Резервирует диапазон за проектом. Ошибка, если пересекается с другим.
    pub fn reserve(&mut self, project: &str, range: PortRange) -> Result<(), String> {
        for (other, existing) in &self.ranges {
            if other != project && existing.overlaps(&range) {
                return Err(format!("port ranges overlap: {project} and {other}"));
            }
        }
        self.ranges.insert(project.into(), range);
        Ok(())
    }

    /// Свободный порт в диапазоне проекта: preferred (если свободен), иначе первый свободный.
    /// `occupied` — порты, занятые снаружи (например ОС); аренды учитываются отдельно.
    pub fn available(
        &self,
        project: &str,
        preferred: Option<u16>,
        occupied: &BTreeSet<u16>,
    ) -> Result<u16, String> {
        let range = self
            .ranges
            .get(project)
            .ok_or_else(|| format!("no range for {project}"))?;
        let free = |port: &u16| !self.leases.contains_key(port) && !occupied.contains(port);
        preferred
            .filter(|p| range.contains(*p))
            .into_iter()
            .chain(range.start..=range.end)
            .find(free)
            .ok_or_else(|| {
                format!(
                    "no free ports for {project} ({}–{})",
                    range.start, range.end
                )
            })
    }

    /// Снимает все аренды процесса.
    pub fn release(&mut self, process: u64) {
        self.leases.retain(|_, owner| *owner != process);
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Listener {
    pub pid: u32,
    pub command: String,
    pub port: u16,
}

/// lsof repeats a socket for IPv4/IPv6; inventory is unique per (pid, port).
pub fn parse_lsof(text: &str) -> Vec<Listener> {
    let mut pid = None;
    let mut command = String::new();
    let mut sockets = BTreeMap::new();
    for line in text.lines() {
        if let Some(p) = line.strip_prefix('p') {
            pid = p.parse::<u32>().ok();
            command.clear();
        } else if let Some(c) = line.strip_prefix('c') {
            command = c.to_owned();
        } else if let Some(n) = line.strip_prefix('n') {
            if let (Some(pid), Some(port)) = (
                pid,
                n.rsplit(':').next().and_then(|p| p.parse::<u16>().ok()),
            ) {
                sockets.insert(
                    (pid, port),
                    Listener {
                        pid,
                        port,
                        command: command.clone(),
                    },
                );
            }
        }
    }
    sockets.into_values().collect()
}

pub fn listeners() -> Result<Vec<Listener>, String> {
    let out = Command::new("lsof")
        .args(["-iTCP", "-sTCP:LISTEN", "-nP", "-Fpcn"])
        .output()
        .map_err(|e| format!("lsof: {e}"))?;
    // lsof exits 1 when there are no matching sockets.
    if !out.status.success() && !(out.status.code() == Some(1) && out.stderr.is_empty()) {
        return Err(format!("lsof: {}", String::from_utf8_lossy(&out.stderr)));
    }
    Ok(parse_lsof(&String::from_utf8_lossy(&out.stdout)))
}

#[derive(Debug, Clone, Serialize)]
pub struct PortRow {
    pub port: u16,
    pub project: Option<String>,
    pub command: String,
    pub pid: u32,
    pub status: String,
    pub managed_id: Option<u64>,
}

pub fn inventory(
    sockets: Vec<Listener>,
    managed: &BTreeMap<u32, (u64, String)>,
    ranges: &BTreeMap<String, PortRange>,
) -> Vec<PortRow> {
    sockets
        .iter()
        .map(|socket| {
            let owner = managed.get(&socket.pid);
            let conflict = sockets
                .iter()
                .any(|other| other.port == socket.port && other.pid != socket.pid)
                || owner
                    .and_then(|(_, project)| ranges.get(project))
                    .is_some_and(|r| !r.contains(socket.port));
            PortRow {
                port: socket.port,
                pid: socket.pid,
                command: socket.command.clone(),
                project: owner.map(|(_, project)| project.clone()),
                managed_id: owner.map(|(id, _)| *id),
                status: if conflict {
                    "conflict"
                } else if owner.is_some() {
                    "ok"
                } else {
                    "foreign"
                }
                .into(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_inverted_range() {
        assert!(PortRange::new(5200, 5100).is_none());
    }

    #[test]
    fn rejects_zero_start() {
        assert!(PortRange::new(0, 10).is_none());
    }

    #[test]
    fn detects_overlap() {
        let a = PortRange::new(5100, 5199).unwrap();
        let b = PortRange::new(5150, 5250).unwrap();
        let c = PortRange::new(5200, 5299).unwrap();
        assert!(a.overlaps(&b));
        assert!(!a.overlaps(&c));
        assert!(a.contains(5199));
    }

    #[test]
    fn allocation_skips_busy_and_releases() {
        let mut a = Allocator::default();
        a.reserve("api", PortRange::new(5100, 5102).unwrap())
            .unwrap();
        assert!(a
            .reserve("web", PortRange::new(5102, 5200).unwrap())
            .is_err());
        a.leases.insert(5100, 1);
        assert_eq!(
            a.available("api", Some(5100), &BTreeSet::from([5101]))
                .unwrap(),
            5102
        );
        a.leases.insert(5102, 2);
        assert!(a
            .available("api", None, &BTreeSet::from([5101]))
            .is_err());
        a.release(1);
        assert_eq!(a.available("api", None, &BTreeSet::new()).unwrap(), 5100);
    }

    #[test]
    fn preferred_must_be_in_range() {
        let mut a = Allocator::default();
        a.reserve("api", PortRange::new(5100, 5102).unwrap())
            .unwrap();
        assert_eq!(
            a.available("api", Some(5200), &BTreeSet::new()).unwrap(),
            5100
        );
    }

    #[test]
    fn parses_ipv6_deduplicates_and_ignores_bad_records() {
        let rows = parse_lsof(
            "p42\ncnode\nn*:5100\nn[::1]:5100\npbad\nn*:20\np43\ncserver\nn127.0.0.1:5101\n",
        );
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0],
            Listener {
                pid: 42,
                command: "node".into(),
                port: 5100
            }
        );
    }

    #[test]
    fn foreign_and_out_of_range_are_distinguished() {
        let rows = inventory(
            parse_lsof("p1\ncnode\nn*:5200\np2\ncssh\nn*:22"),
            &BTreeMap::from([(1, (8, "api".into()))]),
            &BTreeMap::from([("api".into(), PortRange::new(5100, 5199).unwrap())]),
        );
        assert_eq!(rows[0].status, "conflict");
        assert_eq!(rows[1].status, "foreign");
        assert_eq!(rows[1].managed_id, None);
    }
}
