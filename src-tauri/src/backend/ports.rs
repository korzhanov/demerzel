//! Реестр портов и проверка конфликтов (#13, #14).

/// Диапазон портов, который проект резервирует в `orchestrator.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortRange {
    pub start: u16,
    pub end: u16,
}

impl PortRange {
    pub fn new(start: u16, end: u16) -> Option<Self> {
        (start <= end).then_some(Self { start, end })
    }

    pub fn contains(&self, port: u16) -> bool {
        (self.start..=self.end).contains(&port)
    }

    pub fn overlaps(&self, other: &PortRange) -> bool {
        self.start <= other.end && other.start <= self.end
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
    fn detects_overlap() {
        let a = PortRange::new(5100, 5199).unwrap();
        let b = PortRange::new(5150, 5250).unwrap();
        let c = PortRange::new(5200, 5299).unwrap();
        assert!(a.overlaps(&b));
        assert!(!a.overlaps(&c));
        assert!(a.contains(5199));
    }
}
