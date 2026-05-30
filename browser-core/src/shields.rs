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
