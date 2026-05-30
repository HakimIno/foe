use rusqlite::{Connection, Result};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryItem {
    pub id: Option<i64>,
    pub url: String,
    pub title: String,
    pub visit_time: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BookmarkItem {
    pub id: Option<i64>,
    pub url: String,
    pub title: String,
    pub added_time: i64,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Database { conn };
        db.init_tables()?;
        Ok(db)
    }

    fn init_tables(&self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT NOT NULL,
                title TEXT NOT NULL,
                visit_time INTEGER NOT NULL
            )",
            [],
        )?;

        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS bookmarks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT NOT NULL,
                title TEXT NOT NULL,
                added_time INTEGER NOT NULL
            )",
            [],
        )?;

        Ok(())
    }

    pub fn add_history_entry(&self, url: &str, title: &str) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        self.conn.execute(
            "INSERT INTO history (url, title, visit_time) VALUES (?1, ?2, ?3)",
            rusqlite::params![url, title, now],
        )?;
        Ok(())
    }

    pub fn get_history(&self, limit: usize) -> Result<Vec<HistoryItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, url, title, visit_time FROM history ORDER BY visit_time DESC LIMIT ?",
        )?;
        let rows = stmt.query_map([limit], |row| {
            Ok(HistoryItem {
                id: Some(row.get(0)?),
                url: row.get(1)?,
                title: row.get(2)?,
                visit_time: row.get(3)?,
            })
        })?;

        let mut history = Vec::new();
        for r in rows {
            history.push(r?);
        }
        Ok(history)
    }
}
