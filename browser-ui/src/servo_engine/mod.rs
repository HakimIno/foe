// Servo Engine Module for foe Browser
//
// This module integrates the Servo web rendering engine into the foe browser.
// It replaces the previous wry/WebView-based approach with Servo's native
// rendering pipeline.

pub mod waker;
pub mod delegate;
pub mod rendering;
pub mod input;
pub mod gpu_context;

use servo::{Servo, WebView as ServoWebView, WebViewBuilder, RenderingContext};
use std::rc::Rc;
use std::cell::Cell;
use url::Url;
use dpi::PhysicalSize;
use i_slint_backend_winit::WinitWindowAccessor;

thread_local! {
    static ACTIVE_WEBVIEW_DIRTY: Cell<bool> = Cell::new(true);
}

pub fn set_active_dirty(dirty: bool) {
    ACTIVE_WEBVIEW_DIRTY.with(|c| c.set(dirty));
}

pub fn take_active_dirty() -> bool {
    ACTIVE_WEBVIEW_DIRTY.with(|c| c.replace(false))
}

use crate::AppWindow;
use slint::ComponentHandle;

/// A tab representation for the Servo engine
pub struct ServoTab {
    pub webview: ServoWebView,
    pub rendering_context: Rc<gpu_context::GpuSharedRenderingContext>,
    pub url: String,
    pub title: String,
}

/// Main Servo engine manager — replaces WebViewManager
/// Manages the Servo instance and multiple WebView tabs
pub struct ServoEngine {
    servo: Option<Servo>,
    tabs: Vec<ServoTab>,
    active_index: usize,
    input_state: input::InputState,
    /// Current HiDPI scale factor (e.g. 2.0 on Retina displays)
    scale_factor: f32,
}

impl ServoEngine {
    /// Create a new ServoEngine (Servo not yet initialized)
    pub fn new() -> Self {
        ServoEngine {
            servo: None,
            tabs: Vec::new(),
            active_index: 0,
            input_state: input::InputState::new(),
            scale_factor: 1.0,
        }
    }

    /// Update the HiDPI scale factor and propagate to all tabs
    pub fn update_scale_factor(&mut self, new_scale: f32) {
        if (self.scale_factor - new_scale).abs() < 0.001 {
            return;
        }
        self.scale_factor = new_scale;
        log::info!("[ServoEngine] Scale factor → {:.2}", new_scale);
        for tab in &self.tabs {
            tab.webview.set_hidpi_scale_factor(euclid::Scale::new(new_scale));
        }
        set_active_dirty(true);
    }

    /// Initialize Servo with the Slint window context
    /// This must be called after the window is shown and has valid handles
    pub fn initialize(&mut self, _window: &AppWindow) {
        log::info!("[ServoEngine] Initializing Servo engine...");

        // Create the event loop waker
        let waker = Box::new(waker::SlintWaker::new());

        // Build the Servo instance
        let servo_instance = servo::ServoBuilder::default()
            .event_loop_waker(waker)
            .build();

        log::info!("[ServoEngine] Servo engine initialized successfully");
        self.servo = Some(servo_instance);
    }

    /// Add a new tab and create a Servo WebView for it
    pub fn add_tab(&mut self, url_str: &str, window: &AppWindow) {
        let Some(ref servo) = self.servo else {
            log::error!("[ServoEngine] Cannot add tab — Servo not initialized");
            return;
        };

        let url = match Url::parse(url_str) {
            Ok(u) => u,
            Err(_) => {
                log::warn!("[ServoEngine] Invalid URL: {}, using about:blank", url_str);
                Url::parse("about:blank").unwrap()
            }
        };

        // Create WebView delegate for this tab
        let tab_index = self.tabs.len();
        let window_weak = window.as_weak();
        let tab_delegate = delegate::FoeWebViewDelegate::new(window_weak, tab_index);

        // Get scale factor from the winit window (for HiDPI/Retina support)
        let scale_factor = window
            .window()
            .with_winit_window(|w| w.scale_factor() as f32)
            .unwrap_or(1.0);

        // Use actual window size minus chrome height (TabBar 38 + Navbar 38 = 76 logical → physical)
        let logical_size = window.window().size();
        let chrome_height_physical = (76.0 * scale_factor) as u32;
        let init_w = ((logical_size.width as f32) * scale_factor) as u32;
        let init_h = (((logical_size.height as f32) * scale_factor) as u32)
            .saturating_sub(chrome_height_physical)
            .max(1);

        log::info!(
            "[ServoEngine] Tab {} initial size: {}x{} (scale={:.1})",
            tab_index, init_w, init_h, scale_factor
        );

        let size = PhysicalSize::new(init_w.max(1), init_h.max(1));
        let rendering_context = Rc::new(
            gpu_context::GpuSharedRenderingContext::new(size)
                .expect("Failed to create GpuSharedRenderingContext")
        );

        // Build the WebView with correct HiDPI scale
        let webview = WebViewBuilder::new(servo, rendering_context.clone() as Rc<dyn servo::RenderingContext>)
            .url(url.clone())
            .hidpi_scale_factor(euclid::Scale::new(scale_factor))
            .delegate(Rc::new(tab_delegate))
            .build();

        // Make the WebView visible so Servo includes it in the render display list
        webview.show();

        log::info!("[ServoEngine] Created WebView for tab {} → {}", tab_index, url_str);
        self.tabs.push(ServoTab {
            webview,
            rendering_context,
            url: url_str.to_string(),
            title: "New Tab".to_string(),
        });
    }

    /// Close a tab and destroy its WebView
    pub fn close_tab(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }

        log::info!("[ServoEngine] Closing tab {}", index);
        self.tabs.remove(index);

        // Adjust active index
        if self.active_index >= self.tabs.len() && !self.tabs.is_empty() {
            self.active_index = self.tabs.len() - 1;
        }
    }

    /// Set the active (visible) tab
    pub fn set_active_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active_index = index;
            set_active_dirty(true);
            log::info!("[ServoEngine] Active tab set to {}", index);
        }
    }

    /// Navigate the active tab to a URL
    pub fn navigate(&self, url_str: &str) {
        if let Some(tab) = self.tabs.get(self.active_index) {
            if let Ok(url) = Url::parse(url_str) {
                log::info!("[ServoEngine] Navigating to: {}", url_str);
                set_active_dirty(true);
                tab.webview.load(url);
            }
        }
    }

    /// Go back in history for the active tab
    pub fn go_back(&self) {
        if let Some(tab) = self.tabs.get(self.active_index) {
            set_active_dirty(true);
            tab.webview.go_back(1);
        }
    }

    /// Go forward in history for the active tab
    pub fn go_forward(&self) {
        if let Some(tab) = self.tabs.get(self.active_index) {
            set_active_dirty(true);
            tab.webview.go_forward(1);
        }
    }

    /// Reload the active tab
    pub fn reload(&self) {
        if let Some(tab) = self.tabs.get(self.active_index) {
            set_active_dirty(true);
            tab.webview.reload();
        }
    }

    /// Move a tab from one position to another
    pub fn move_tab(&mut self, from: usize, to: usize) {
        if from >= self.tabs.len() || to >= self.tabs.len() || from == to {
            return;
        }
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);

        // Update active index
        if self.active_index == from {
            self.active_index = to;
        } else if from < self.active_index && to >= self.active_index {
            self.active_index -= 1;
        } else if from > self.active_index && to <= self.active_index {
            self.active_index += 1;
        }
    }

    /// Get the number of tabs
    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    /// Get the active tab's WebView (if any)
    pub fn get_active_webview(&self) -> Option<&ServoWebView> {
        self.tabs.get(self.active_index).map(|t| &t.webview)
    }

    /// Pump the Servo event loop — should be called periodically
    pub fn spin_event_loop(&self) {
        if let Some(ref servo) = self.servo {
            servo.spin_event_loop();
        }
    }

    /// Paint the active webview
    pub fn paint_active(&self) {
        if let Some(tab) = self.tabs.get(self.active_index) {
            tab.webview.paint();
        }
    }

    /// Get the current rendered frame of the active tab as a Slint Image.
    /// Reads pixels from Servo's offscreen FBO via CPU (glReadPixels).
    /// Note: BorrowedOpenGLTextureBuilder cannot be used here because Servo
    /// renders into a separate surfman GL context; Slint uses the winit GL
    /// context. Texture IDs are not portable across contexts without IOSurface.
    pub fn get_active_frame(&self) -> Option<slint::Image> {
        self.tabs.get(self.active_index).and_then(|tab| {
            let size = tab.rendering_context.size();
            let w = size.width;
            let h = size.height;
            if w == 0 || h == 0 {
                return None;
            }
            let rect = servo::DeviceIntRect::from_size(
                servo::DeviceIntSize::new(w as i32, h as i32),
            );
            if let Some(image_buffer) = tab.rendering_context.read_to_image(rect) {
                let (iw, ih) = image_buffer.dimensions();
                let pixel_buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                    image_buffer.as_raw(),
                    iw,
                    ih,
                );
                Some(slint::Image::from_rgba8(pixel_buffer))
            } else {
                None
            }
        })
    }

    /// Resize the active tab
    pub fn resize_active(&self, width: u32, height: u32) {
        if let Some(tab) = self.tabs.get(self.active_index) {
            let size = PhysicalSize::new(width, height);
            set_active_dirty(true);
            tab.webview.resize(size);
        }
    }

    /// Handle a winit window event for resizing and input delivery.
    /// `scale` is the current window scale factor (from `winit_window.scale_factor()`).
    pub fn handle_winit_event(
        &mut self,
        event: &i_slint_backend_winit::winit::event::WindowEvent,
        scale: f32,
    ) {
        use i_slint_backend_winit::winit::event::WindowEvent;

        match event {
            WindowEvent::Resized(physical_size) => {
                log::debug!("[ServoEngine] Window resized: {:?} (scale={:.2})", physical_size, scale);
                // Update scale factor first so viewport calculations are correct
                self.update_scale_factor(scale);

                let width = physical_size.width;
                // 90 logical px chrome (TabBar 38 + Navbar 38 ≈ 76 + extra) → physical
                let chrome_h = (90.0 * scale) as u32;
                let height = physical_size.height.saturating_sub(chrome_h).max(1);

                // Propagate scale to active webview before resize so Servo uses correct viewport
                if let Some(tab) = self.tabs.get(self.active_index) {
                    tab.webview.set_hidpi_scale_factor(euclid::Scale::new(scale));
                }
                self.resize_active(width, height);
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.update_scale_factor(*scale_factor as f32);
            }
            _ => {
                if let Some(servo_event) = input::translate_event(event, &mut self.input_state) {
                    if let Some(webview) = self.get_active_webview() {
                        webview.notify_input_event(servo_event);
                    }
                }
            }
        }
    }
}
