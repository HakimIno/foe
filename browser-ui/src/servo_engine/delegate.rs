// WebViewDelegate implementation for Servo → Slint UI callbacks
//
// This delegate receives callbacks from Servo when:
// - A new frame is ready to paint
// - The page title changes
// - The URL changes (navigation)
// - History state changes (back/forward availability)

use crate::AppWindow;
use servo::{WebView as ServoWebView, WebViewDelegate};
use slint::{ComponentHandle, Model, Weak};

/// Delegate that receives events from a Servo WebView and updates the Slint UI
pub struct FoeWebViewDelegate {
    window_weak: Weak<AppWindow>,
    tab_index: usize,
}

impl FoeWebViewDelegate {
    pub fn new(window_weak: Weak<AppWindow>, tab_index: usize) -> Self {
        FoeWebViewDelegate {
            window_weak,
            tab_index,
        }
    }
}

impl WebViewDelegate for FoeWebViewDelegate {
    /// Called when Servo has a new frame ready to display
    fn notify_new_frame_ready(&self, _webview: ServoWebView) {
        log::trace!("[Delegate] New frame ready for tab {}", self.tab_index);
        let w_weak = self.window_weak.clone();
        let _ = slint::invoke_from_event_loop(move || {
            crate::servo_engine::set_active_dirty(true);
            if let Some(window) = w_weak.upgrade() {
                window.window().request_redraw();
            }
        });
    }

    /// Called when the page title changes
    fn notify_page_title_changed(&self, _webview: ServoWebView, title: Option<String>) {
        let new_title = title.unwrap_or_else(|| "Untitled".to_string());
        log::info!(
            "[Delegate] Title changed for tab {}: {}",
            self.tab_index,
            new_title
        );

        let w_weak = self.window_weak.clone();
        let title_clone = new_title.clone();
        let tab_idx = self.tab_index;
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(window) = w_weak.upgrade() {
                window.set_current_title(title_clone.clone().into());

                // Update the tab model
                let tabs_model = window.get_tabs();
                let mut tabs: Vec<crate::TabInfo> = tabs_model.iter().collect();
                if let Some(tab) = tabs.get_mut(tab_idx) {
                    tab.title = title_clone.into();
                }
                window.set_tabs(slint::ModelRc::new(slint::VecModel::from(tabs)));
            }
        });
    }

    /// Called when the URL changes (e.g., after navigation or redirect)
    fn notify_url_changed(&self, _webview: ServoWebView, url: url::Url) {
        let url_str = url.to_string();
        log::info!(
            "[Delegate] URL changed for tab {}: {}",
            self.tab_index,
            url_str
        );

        let w_weak = self.window_weak.clone();
        let url_clone = url_str.clone();
        let tab_idx = self.tab_index;
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(window) = w_weak.upgrade() {
                window.set_current_url(url_clone.clone().into());

                // Update the tab model
                let tabs_model = window.get_tabs();
                let mut tabs: Vec<crate::TabInfo> = tabs_model.iter().collect();
                if let Some(tab) = tabs.get_mut(tab_idx) {
                    tab.url = url_clone.clone().into();
                    tab.site_type = crate::handlers::get_site_type(&url_clone).into();
                }
                window.set_tabs(slint::ModelRc::new(slint::VecModel::from(tabs)));
            }
        });
    }

    /// Called when history state changes (back/forward buttons)
    fn notify_history_changed(
        &self,
        _webview: ServoWebView,
        entries: Vec<url::Url>,
        current: usize,
    ) {
        log::debug!(
            "[Delegate] History changed for tab {}: {} entries, current={}",
            self.tab_index,
            entries.len(),
            current
        );

        if let Some(url) = entries.get(current) {
            let url_str = url.to_string();
            let w_weak = self.window_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(window) = w_weak.upgrade() {
                    window.set_current_url(url_str.into());
                }
            });
        }
    }
}
