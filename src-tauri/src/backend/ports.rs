//! Реестр портов: диапазоны из конфига, аренда и проверка конфликтов (#13).

use std::collections::{BTreeMap, BTreeSet};

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
}
