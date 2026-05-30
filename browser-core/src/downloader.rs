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
