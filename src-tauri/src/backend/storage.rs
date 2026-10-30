//! SQLite-хранилище состояния (#7).
//!
//! Миграции версионируются через `PRAGMA user_version`. Каждая миграция
//! выполняется в транзакции.

use rusqlite::{Connection, Result};

pub const SCHEMA_VERSION: u32 = 1;

/// Миграции по порядку. Индекс + 1 — версия.
pub const MIGRATIONS: &[&str] = &[include_str!("../../migrations/0001_init.sql")];

pub const TABLES: &[&str] = &[
    "projects",
    "processes",
    "port_leases",
    "tasks",
    "agent_runs",
    "alerts",
];

/// Открывает базу и применяет недостающие миграции.
pub fn open(path: &str) -> Result<Connection> {
    let mut conn = Connection::open(path)?;
    migrate(&mut conn)?;
    Ok(conn)
}

pub fn migrate(conn: &mut Connection) -> Result<()> {
    let current: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as u32)?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table_names(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
            .unwrap();
        stmt.query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    }

    #[test]
    fn migrate_creates_all_tables() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        let mut expected: Vec<String> = TABLES.iter().map(|s| s.to_string()).collect();
        expected.sort();
        assert_eq!(table_names(&conn), expected);
        let v: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, SCHEMA_VERSION);
    }

    #[test]
    fn migrate_is_idempotent() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        migrate(&mut conn).unwrap();
        assert_eq!(table_names(&conn).len(), TABLES.len());
    }

    #[test]
    fn port_lease_is_unique() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        conn.execute("INSERT INTO projects (name, path, kind) VALUES ('a','/a','app')", []).unwrap();
        conn.execute("INSERT INTO port_leases (port, project_id) VALUES (5100, 1)", []).unwrap();
        let dup = conn.execute("INSERT INTO port_leases (port, project_id) VALUES (5100, 1)", []);
        assert!(dup.is_err());
    }
}
