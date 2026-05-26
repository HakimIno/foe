pub mod storage {
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
}

pub mod shields {
    // Simple adblocking / tracker blocking engine.
    // In production we can compile with `adblock` crate.
    // For now we will support a simple host-based filtering.
    use std::collections::HashSet;

    pub struct ShieldsEngine {
        blocked_domains: HashSet<String>,
        enabled: bool,
    }

    impl ShieldsEngine {
        pub fn new() -> Self {
            let mut blocked = HashSet::new();
            // Stub blocklist
            blocked.insert("doubleclick.net".to_string());
            blocked.insert("google-analytics.com".to_string());
            blocked.insert("ads.youtube.com".to_string());
            blocked.insert("adservice.google.com".to_string());
            
            ShieldsEngine {
                blocked_domains: blocked,
                enabled: true,
            }
        }

        pub fn set_enabled(&mut self, enabled: bool) {
            self.enabled = enabled;
        }

        pub fn is_enabled(&self) -> bool {
            self.enabled
        }

        pub fn should_block(&self, url: &str) -> bool {
            if !self.enabled {
                return false;
            }
            
            // Extract domain from URL
            if let Ok(parsed_url) = url::Url::parse(url) {
                if let Some(host) = parsed_url.host_str() {
                    for blocked in &self.blocked_domains {
                        if host == blocked || host.ends_with(&format!(".{}", blocked)) {
                            return true;
                        }
                    }
                }
            }
            false
        }
    }
}

pub mod downloader {
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use serde::{Serialize, Deserialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct DownloadTask {
        pub id: String,
        pub url: String,
        pub filename: String,
        pub total_size: u64,
        pub downloaded_size: u64,
        pub status: String, // "Pending", "Downloading", "Completed", "Failed"
    }

    pub struct DownloadManager {
        tasks: Arc<Mutex<Vec<DownloadTask>>>,
    }

    impl DownloadManager {
        pub fn new() -> Self {
            DownloadManager {
                tasks: Arc::new(Mutex::new(Vec::new())),
            }
        }

        pub async fn add_task(&self, url: &str, filename: &str) -> String {
            let id = uuid::Uuid::new_v4().to_string();
            let task = DownloadTask {
                id: id.clone(),
                url: url.to_string(),
                filename: filename.to_string(),
                total_size: 0,
                downloaded_size: 0,
                status: "Pending".to_string(),
            };
            self.tasks.lock().await.push(task);
            id
        }

        pub async fn get_tasks(&self) -> Vec<DownloadTask> {
            self.tasks.lock().await.clone()
        }
    }
}
