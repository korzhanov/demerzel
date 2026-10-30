//! Схема хранилища SQLite (#7). Здесь только имена таблиц и версия миграций.

pub const SCHEMA_VERSION: u32 = 1;

pub const TABLES: &[&str] = &[
    "projects",
    "processes",
    "port_leases",
    "tasks",
    "agent_runs",
    "alerts",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_has_all_tables() {
        assert_eq!(TABLES.len(), 6);
        assert!(TABLES.contains(&"agent_runs"));
    }
}
