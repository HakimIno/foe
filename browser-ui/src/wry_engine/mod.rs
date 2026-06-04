use crate::AppWindow;
use i_slint_backend_winit::WinitWindowAccessor;
use slint::ComponentHandle;
use wry::{WebView, WebViewBuilder, Rect};

pub struct WryTab {
    pub webview: Option<WebView>,
    pub url: String,
    pub title: String,
}

pub struct WryEngine {
    tabs: Vec<WryTab>,
    active_index: usize,
    bounds: Rect,
}

impl WryEngine {
    pub fn new() -> Self {
        #[cfg(target_os = "macos")]
        let y_pos = 0.0;
        #[cfg(not(target_os = "macos"))]
        let y_pos = 76.0;

        Self {
            tabs: Vec::new(),
            active_index: 0,
            bounds: Rect {
                position: wry::dpi::Position::Logical(wry::dpi::LogicalPosition::new(0.0, y_pos)),
                size: wry::dpi::Size::Logical(wry::dpi::LogicalSize::new(800.0, 600.0)),
            },
        }
    }

    pub fn initialize(&mut self, window: &AppWindow) {
        log::info!("[WryEngine] Initializing Wry engine...");
        self.update_bounds(window);
    }

    fn update_bounds(&mut self, window: &AppWindow) {
        if let Some(size) = window.window().with_winit_window(|w: &i_slint_backend_winit::winit::window::Window| w.inner_size()) {
            let scale = window.window().with_winit_window(|w: &i_slint_backend_winit::winit::window::Window| w.scale_factor()).unwrap_or(1.0);
            let logical_w = size.width as f64 / scale;
            let logical_h = size.height as f64 / scale;
            
            #[cfg(target_os = "macos")]
            let y_pos = 0.0;
            #[cfg(not(target_os = "macos"))]
            let y_pos = 76.0;
            
            self.bounds = Rect {
                position: wry::dpi::Position::Logical(wry::dpi::LogicalPosition::new(0.0, y_pos)),
                size: wry::dpi::Size::Logical(wry::dpi::LogicalSize::new(logical_w, (logical_h - 76.0).max(1.0))),
            };

            if let Some(tab) = self.tabs.get(self.active_index) {
                if let Some(wv) = &tab.webview {
                    let _ = wv.set_bounds(self.bounds.clone());
                }
            }
        }
    }

    pub fn add_tab(&mut self, url_str: &str, window: &AppWindow) {
        let is_native = url_str == "about:newtab" || url_str == "about:blank" || url_str.is_empty();
        
        let _tab_index = self.tabs.len();
        if is_native {
            self.tabs.push(WryTab {
                webview: None,
                url: url_str.to_string(),
                title: "Google".to_string(),
            });
            return;
        }

        let webview = self.create_webview(url_str, window);
        self.tabs.push(WryTab {
            webview,
            url: url_str.to_string(),
            title: "New Tab".to_string(),
        });
    }

    fn create_webview(&self, url: &str, window: &AppWindow) -> Option<WebView> {
        window.window().with_winit_window(|winit_window: &i_slint_backend_winit::winit::window::Window| {
            let builder = WebViewBuilder::new_as_child(winit_window)
                .with_url(url)
                .with_bounds(self.bounds.clone());
            
            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            match builder.build() {
                Ok(wv) => Some(wv),
                Err(e) => {
                    log::error!("[WryEngine] Failed to create WebView: {}", e);
                    None
                }
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
            None
        }).flatten()
    }

    pub fn close_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.tabs.remove(index);
            if self.active_index >= self.tabs.len() && !self.tabs.is_empty() {
                self.active_index = self.tabs.len() - 1;
            }
            self.update_visibility();
        }
    }

    pub fn set_active_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active_index = index;
            self.update_visibility();
        }
    }

    fn update_visibility(&self) {
        for (i, tab) in self.tabs.iter().enumerate() {
            if let Some(wv) = &tab.webview {
                let _ = wv.set_visible(i == self.active_index);
                if i == self.active_index {
                    let _ = wv.set_bounds(self.bounds.clone());
                }
            }
        }
    }

    pub fn navigate(&mut self, url_str: &str, window: &AppWindow) {
        let needs_webview = self.tabs.get(self.active_index).map(|t| t.webview.is_none()).unwrap_or(false);
        
        let new_webview = if needs_webview {
            self.create_webview(url_str, window)
        } else {
            None
        };

        if let Some(tab) = self.tabs.get_mut(self.active_index) {
            if let Some(wv) = new_webview {
                tab.webview = Some(wv);
            }
            if let Some(wv) = &tab.webview {
                let _ = wv.load_url(url_str);
            }
            tab.url = url_str.to_string();
        }
        self.update_visibility();
    }

    pub fn go_back(&self) {
        if let Some(tab) = self.tabs.get(self.active_index) {
            if let Some(wv) = &tab.webview {
                let _ = wv.evaluate_script("window.history.back()");
            }
        }
    }

    pub fn go_forward(&self) {
        if let Some(tab) = self.tabs.get(self.active_index) {
            if let Some(wv) = &tab.webview {
                let _ = wv.evaluate_script("window.history.forward()");
            }
        }
    }

    pub fn reload(&self) {
        if let Some(tab) = self.tabs.get(self.active_index) {
            if let Some(wv) = &tab.webview {
                let _ = wv.evaluate_script("window.location.reload()");
            }
        }
    }

    pub fn move_tab(&mut self, from: usize, to: usize) {
        if from < self.tabs.len() && to < self.tabs.len() && from != to {
            let tab = self.tabs.remove(from);
            self.tabs.insert(to, tab);
            
            if self.active_index == from {
                self.active_index = to;
            } else if from < self.active_index && to >= self.active_index {
                self.active_index -= 1;
            } else if from > self.active_index && to <= self.active_index {
                self.active_index += 1;
            }
        }
    }

    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    pub fn handle_winit_event(&mut self, event: &i_slint_backend_winit::winit::event::WindowEvent, _scale: f32) {
        use i_slint_backend_winit::winit::event::WindowEvent;
        if let WindowEvent::Resized(_) = event {
            // Need a window reference to get size... wait, event has size
            // But we don't have AppWindow here. Main passes event and scale.
            // Let's rely on bounds updates via AppWindow elsewhere if needed,
            // or just use the physical size from the event.
        }
    }

    pub fn resize_from_event(&mut self, physical_size: i_slint_backend_winit::winit::dpi::PhysicalSize<u32>, scale: f64) {
        let logical_w = physical_size.width as f64 / scale;
        let logical_h = physical_size.height as f64 / scale;
        
        #[cfg(target_os = "macos")]
        let y_pos = 0.0;
        #[cfg(not(target_os = "macos"))]
        let y_pos = 76.0;
        
        self.bounds = Rect {
            position: wry::dpi::Position::Logical(wry::dpi::LogicalPosition::new(0.0, y_pos)),
            size: wry::dpi::Size::Logical(wry::dpi::LogicalSize::new(logical_w, (logical_h - 76.0).max(1.0))),
        };

        if let Some(tab) = self.tabs.get(self.active_index) {
            if let Some(wv) = &tab.webview {
                let _ = wv.set_bounds(self.bounds.clone());
            }
        }
    }
}
