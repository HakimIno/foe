pub mod navigation;
pub mod tabs;
pub mod downloader;
pub mod command_bar;

pub fn get_site_type(url: &str) -> String {
    let lower = url.to_lowercase();
    if lower.contains("google.com") || lower.contains("google") || lower == "about:newtab" {
        "google".into()
    } else {
        "generic".into()
    }
}
