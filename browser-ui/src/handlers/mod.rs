pub mod navigation;
pub mod tabs;
pub mod downloader;
pub mod command_bar;

pub fn get_site_type(url: &str) -> String {
    let lower = url.to_lowercase();
    if lower.contains("servo.org") {
        "servo".into()
    } else if lower.contains("brave.com") {
        "brave".into()
    } else if lower.contains("arc.net") {
        "arc".into()
    } else if lower.contains("google.com") || lower.contains("google") {
        "google".into()
    } else {
        "generic".into()
    }
}
