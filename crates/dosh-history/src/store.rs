use anyhow::Result;
use rusqlite::{Connection, params};
use std::fs;
use std::path::Path;

use crate::search;

pub struct HistoryStore {
    entries: Vec<String>,
    conn: Option<Connection>,
}

impl HistoryStore {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            conn: None,
        }
    }

    pub fn new_persistent(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS history_entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                command TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            ",
        )?;

        let mut store = Self {
            entries: Vec::new(),
            conn: Some(conn),
        };
        store.reload_from_db()?;
        Ok(store)
    }

    pub fn add(&mut self, entry: &str) -> Result<()> {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            return Ok(());
        }

        self.entries.push(trimmed.to_string());

        if let Some(conn) = &self.conn {
            conn.execute(
                "INSERT INTO history_entries (command) VALUES (?1)",
                params![trimmed],
            )?;
        }

        Ok(())
    }

    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    pub fn suggest(&self, prefix: &str) -> Option<String> {
        search::suggest(&self.entries, prefix)
    }

    pub fn fuzzy_search(&self, query: &str, limit: usize) -> Vec<String> {
        search::fuzzy_search(&self.entries, query, limit)
    }

    fn reload_from_db(&mut self) -> Result<()> {
        let Some(conn) = &self.conn else {
            return Ok(());
        };

        let mut stmt = conn.prepare("SELECT command FROM history_entries ORDER BY id ASC")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;

        self.entries.clear();
        for row in rows {
            self.entries.push(row?);
        }
        Ok(())
    }
}

impl Default for HistoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggest_by_prefix() {
        let mut history = HistoryStore::new();
        history.add("cargo test").unwrap();
        history.add("cargo check").unwrap();
        assert_eq!(history.suggest("cargo t").as_deref(), Some("cargo test"));
    }

    #[test]
    fn persistent_store_roundtrip() {
        let tmp = std::env::temp_dir().join("dosh_history_test.sqlite3");
        if tmp.exists() {
            let _ = std::fs::remove_file(&tmp);
        }

        {
            let mut store = HistoryStore::new_persistent(&tmp).unwrap();
            store.add("echo persistent").unwrap();
        }

        let store = HistoryStore::new_persistent(&tmp).unwrap();
        assert!(store.entries().iter().any(|v| v == "echo persistent"));

        let _ = std::fs::remove_file(&tmp);
    }
}
