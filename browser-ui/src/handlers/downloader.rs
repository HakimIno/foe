use crate::AppWindow;
use browser_core::downloader::DownloadManager;
use std::sync::Arc;
use slint::ComponentHandle;

pub fn setup(window: &AppWindow, download_manager: Arc<DownloadManager>) {
    let window_weak = window.as_weak();
    let dm_clone = download_manager.clone();
    
    window.on_trigger_download(move |url| {
        let window = window_weak.upgrade().unwrap();
        let filename = url.split('/').last().unwrap_or("downloaded_file").to_string();
        
        println!("[Downloader] Grabbing media stream: {} -> {}", url, filename);
        let dm = dm_clone.clone();
        
        tokio::spawn(async move {
            let id = dm.add_task(&url, &filename).await;
            println!("[Downloader] Registered task id: {}", id);
        });

        window.set_show_downloader_panel(true);
    });
}
