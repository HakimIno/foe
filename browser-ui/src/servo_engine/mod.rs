// Servo Engine Module for foe Browser
//
// This module integrates the Servo web rendering engine into the foe browser.
// It replaces the previous wry/WebView-based approach with Servo's native
// rendering pipeline.

pub mod delegate;
pub mod gpu_context;
pub mod input;
pub mod rendering;
pub mod waker;

use servo::{
    Preferences, RenderingContext, Servo, UserContentManager, UserScript,
    WebView as ServoWebView, WebViewBuilder,
};
use std::rc::Rc;

/// Chrome 131 desktop UA on macOS. Many sites server-side sniff UA and serve
/// a stripped-down mobile/no-JS markup to unrecognized agents (Servo's default
/// "Firefox 140 / Servo" string trips this). Pretending to be Chrome avoids
/// that and lets us focus on real engine differences.
const DESKTOP_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
                          AppleWebKit/537.36 (KHTML, like Gecko) \
                          Chrome/131.0.0.0 Safari/537.36";

/// Minimal JS shims for the Web APIs Servo is missing that modern sites rely
/// on. These don't fully implement the APIs — they just satisfy the existence
/// checks so first-paint code paths don't throw and degrade the layout:
///   * IntersectionObserver: lazy-load triggers fire immediately, so `<img>`
///     placeholders actually load.
///   * Element.prototype.animate: returns a stub Animation that's "finished"
///     so animation chains don't reject.
///   * SVGAElement: empty class so `instanceof SVGAElement` checks return false
///     instead of throwing ReferenceError.
const POLYFILLS_JS: &str = r#"
(function() {
    if (typeof IntersectionObserver === 'undefined') {
        globalThis.IntersectionObserver = class {
            constructor(cb, opts) { this._cb = cb; this._opts = opts || {}; }
            observe(target) {
                Promise.resolve().then(() => {
                    var rect = (target && target.getBoundingClientRect)
                        ? target.getBoundingClientRect()
                        : { top:0, left:0, right:0, bottom:0, width:0, height:0, x:0, y:0 };
                    var entry = {
                        target: target,
                        isIntersecting: true,
                        intersectionRatio: 1,
                        boundingClientRect: rect,
                        intersectionRect: rect,
                        rootBounds: null,
                        time: (typeof performance !== 'undefined' && performance.now) ? performance.now() : Date.now()
                    };
                    try { this._cb([entry], this); } catch (e) {}
                });
            }
            unobserve() {}
            disconnect() {}
            takeRecords() { return []; }
        };
    }
    if (typeof Element !== 'undefined' && !Element.prototype.animate) {
        Element.prototype.animate = function(_keyframes, _options) {
            var finished = Promise.resolve();
            return {
                play(){}, pause(){}, reverse(){}, cancel(){}, finish(){}, persist(){},
                commitStyles(){}, updatePlaybackRate(){},
                addEventListener(){}, removeEventListener(){}, dispatchEvent(){ return true; },
                id: '', playState: 'finished', playbackRate: 1, startTime: 0, currentTime: 0,
                timeline: null, effect: null,
                finished: finished, ready: finished,
                onfinish: null, oncancel: null, onremove: null
            };
        };
    }
    if (typeof SVGAElement === 'undefined') {
        globalThis.SVGAElement = function SVGAElement() {};
        globalThis.SVGAElement.prototype = Object.create(
            (typeof SVGGraphicsElement !== 'undefined' ? SVGGraphicsElement : Object).prototype
        );
    }
    if (typeof ResizeObserver === 'undefined') {
        globalThis.ResizeObserver = class {
            constructor(cb) { this._cb = cb; }
            observe(target) {
                Promise.resolve().then(() => {
                    var rect = (target && target.getBoundingClientRect)
                        ? target.getBoundingClientRect()
                        : { top:0, left:0, right:0, bottom:0, width:0, height:0 };
                    try {
                        this._cb([{
                            target: target,
                            contentRect: rect,
                            borderBoxSize: [{ inlineSize: rect.width, blockSize: rect.height }],
                            contentBoxSize: [{ inlineSize: rect.width, blockSize: rect.height }],
                            devicePixelContentBoxSize: [{ inlineSize: rect.width, blockSize: rect.height }]
                        }], this);
                    } catch (e) {}
                });
            }
            unobserve() {}
            disconnect() {}
        };
    }
})();
"#;

use dpi::PhysicalSize;
use i_slint_backend_winit::WinitWindowAccessor;
use url::Url;

use std::sync::atomic::{AtomicBool, Ordering};

static ACTIVE_WEBVIEW_DIRTY: AtomicBool = AtomicBool::new(true);

pub fn set_active_dirty(dirty: bool) {
    ACTIVE_WEBVIEW_DIRTY.store(dirty, Ordering::SeqCst);
}

pub fn take_active_dirty() -> bool {
    ACTIVE_WEBVIEW_DIRTY.swap(false, Ordering::SeqCst)
}

fn is_native_url(url: &str) -> bool {
    url == "about:newtab" || url == "about:blank" || url.is_empty()
}

fn render_scale_for_window(window_scale: f32) -> f32 {
    if let Ok(value) = std::env::var("FOE_RENDER_SCALE") {
        if let Ok(scale) = value.parse::<f32>() {
            return scale.clamp(0.5, window_scale.max(0.5));
        }
    }

    // Render at the window's native HiDPI scale on every platform. On macOS
    // Retina this was previously clamped to 1.0 to avoid the cost of the CPU
    // read-back path, but with IOSurface zero-copy that cost is gone — rendering
    // at 1.0 only makes content look blurry and unnaturally small when Slint
    // upscales the texture back to physical pixels.
    window_scale.max(0.5)
}

use crate::AppWindow;
use slint::ComponentHandle;

/// A tab representation for the Servo engine
pub struct ServoTab {
    pub webview: Option<ServoWebView>,
    pub rendering_context: Option<Rc<gpu_context::GpuSharedRenderingContext>>,
    pub url: String,
    pub title: String,
}

/// Main Servo engine manager — replaces WebViewManager
/// Manages the Servo instance and multiple WebView tabs
pub struct ServoEngine {
    servo: Option<Servo>,
    /// Shared across all tabs: holds the JS polyfill UserScript and forwards
    /// it to each WebView so missing-API errors don't tank page layout.
    user_content_manager: Option<Rc<UserContentManager>>,
    tabs: Vec<ServoTab>,
    active_index: usize,
    input_state: input::InputState,
    /// Current HiDPI scale factor (e.g. 2.0 on Retina displays)
    scale_factor: f32,
    render_scale_factor: f32,
}

impl ServoEngine {
    /// Create a new ServoEngine (Servo not yet initialized)
    pub fn new() -> Self {
        ServoEngine {
            servo: None,
            user_content_manager: None,
            tabs: Vec::new(),
            active_index: 0,
            input_state: input::InputState::new(),
            scale_factor: 1.0,
            render_scale_factor: 1.0,
        }
    }

    /// Update the HiDPI scale factor and propagate to all tabs
    pub fn update_scale_factor(&mut self, new_scale: f32) {
        let new_render_scale = render_scale_for_window(new_scale);
        if (self.scale_factor - new_scale).abs() < 0.001
            && (self.render_scale_factor - new_render_scale).abs() < 0.001
        {
            return;
        }
        self.scale_factor = new_scale;
        self.render_scale_factor = new_render_scale;
        self.input_state
            .set_scale_factors(new_scale as f64, new_render_scale as f64);
        log::info!(
            "[ServoEngine] Scale factor → {:.2}, render scale → {:.2}",
            new_scale,
            new_render_scale
        );
        for tab in &self.tabs {
            if let Some(webview) = &tab.webview {
                webview.set_hidpi_scale_factor(euclid::Scale::new(new_render_scale));
            }
        }
        set_active_dirty(true);
    }

    /// Initialize Servo with the Slint window context
    /// This must be called after the window is shown and has valid handles
    pub fn initialize(&mut self, _window: &AppWindow) {
        log::info!("[ServoEngine] Initializing Servo engine...");

        // Create the event loop waker
        let waker = Box::new(waker::SlintWaker::new());

        // Customize Servo preferences before construction so the very first
        // network request already carries our desktop UA.
        let mut prefs = Preferences::default();
        prefs.user_agent = DESKTOP_UA.to_string();

        // Build the Servo instance with our preferences
        let servo_instance = servo::ServoBuilder::default()
            .preferences(prefs)
            .event_loop_waker(waker)
            .build();

        // Create the shared UserContentManager and register our Web-API
        // polyfill once. Each WebViewBuilder will receive the same Rc.
        let ucm = Rc::new(UserContentManager::new(&servo_instance));
        ucm.add_script(Rc::new(UserScript::from(POLYFILLS_JS)));

        log::info!("[ServoEngine] Servo engine initialized successfully");
        self.servo = Some(servo_instance);
        self.user_content_manager = Some(ucm);
    }

    fn create_webview_for_tab(
        servo: &Servo,
        user_content_manager: Option<&Rc<UserContentManager>>,
        tab_index: usize,
        url: Url,
        window: &AppWindow,
    ) -> (ServoWebView, Rc<gpu_context::GpuSharedRenderingContext>) {
        let window_weak = window.as_weak();
        let tab_delegate = delegate::FoeWebViewDelegate::new(window_weak, tab_index);

        let window_scale_factor = window
            .window()
            .with_winit_window(|w| w.scale_factor() as f32)
            .unwrap_or(1.0);
        let render_scale_factor = render_scale_for_window(window_scale_factor);

        // window.window().size() returns PhysicalSize. Convert to logical pixels
        // first, then scale by render_scale_factor — mirroring the math in
        // handle_winit_event::Resized. Treating physical as logical here was
        // multiplying the framebuffer by window_scale * render_scale, blowing the
        // CSS viewport up (e.g. 1440 → 2880 CSS px on Retina) and making every
        // page look tiny because layout sized for a "2880px desktop".
        let physical_size = window.window().size();
        let logical_w = physical_size.width as f32 / window_scale_factor.max(0.1);
        let logical_h = physical_size.height as f32 / window_scale_factor.max(0.1);
        let init_w = (logical_w * render_scale_factor).round().max(1.0) as u32;
        let init_h = ((logical_h - 76.0).max(1.0) * render_scale_factor)
            .round()
            .max(1.0) as u32;

        log::info!(
            "[ServoEngine] Tab {} render size: {}x{} (window scale={:.1}, render scale={:.1})",
            tab_index,
            init_w,
            init_h,
            window_scale_factor,
            render_scale_factor
        );

        let size = PhysicalSize::new(init_w.max(1), init_h.max(1));
        let rendering_context = Rc::new(
            gpu_context::GpuSharedRenderingContext::new(size)
                .expect("Failed to create GpuSharedRenderingContext"),
        );

        let mut builder = WebViewBuilder::new(
            servo,
            rendering_context.clone() as Rc<dyn servo::RenderingContext>,
        )
        .url(url)
        .hidpi_scale_factor(euclid::Scale::new(render_scale_factor))
        .delegate(Rc::new(tab_delegate));

        if let Some(ucm) = user_content_manager {
            builder = builder.user_content_manager(Rc::clone(ucm));
        }

        let webview = builder.build();

        webview.show();
        (webview, rendering_context)
    }

    /// Add a new tab. Native tabs do not allocate a Servo WebView until navigation.
    pub fn add_tab(&mut self, url_str: &str, window: &AppWindow) {
        let Some(ref servo) = self.servo else {
            log::error!("[ServoEngine] Cannot add tab — Servo not initialized");
            return;
        };

        let scale_factor = window
            .window()
            .with_winit_window(|w| w.scale_factor() as f32)
            .unwrap_or(1.0);
        let render_scale_factor = render_scale_for_window(scale_factor);
        self.input_state
            .set_scale_factors(scale_factor as f64, render_scale_factor as f64);

        let tab_index = self.tabs.len();
        if is_native_url(url_str) {
            log::info!("[ServoEngine] Created native tab {} → {}", tab_index, url_str);
            self.tabs.push(ServoTab {
                webview: None,
                rendering_context: None,
                url: url_str.to_string(),
                title: "Google".to_string(),
            });
            return;
        }

        let url = match Url::parse(url_str) {
            Ok(u) => u,
            Err(_) => {
                log::warn!("[ServoEngine] Invalid URL: {}, using native new tab", url_str);
                self.tabs.push(ServoTab {
                    webview: None,
                    rendering_context: None,
                    url: "about:newtab".to_string(),
                    title: "Google".to_string(),
                });
                return;
            }
        };

        let (webview, rendering_context) = Self::create_webview_for_tab(
            servo,
            self.user_content_manager.as_ref(),
            tab_index,
            url.clone(),
            window,
        );

        log::info!(
            "[ServoEngine] Created WebView for tab {} → {}",
            tab_index,
            url_str
        );
        self.tabs.push(ServoTab {
            webview: Some(webview),
            rendering_context: Some(rendering_context),
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
    pub fn navigate(&mut self, url_str: &str, window: &AppWindow) {
        let Ok(url) = Url::parse(url_str) else {
            return;
        };

        let Some(ref servo) = self.servo else {
            return;
        };

        // Snapshot the UCM Rc before borrowing self.tabs mutably below; both
        // fields are separate so the borrow checker is happy, but the explicit
        // local makes the lifetime obvious.
        let ucm = self.user_content_manager.clone();
        let active_index = self.active_index;

        let Some(tab) = self.tabs.get_mut(active_index) else {
            return;
        };

        log::info!("[ServoEngine] Navigating to: {}", url_str);
        if let Some(webview) = &tab.webview {
            webview.load(url);
        } else {
            let (webview, rendering_context) = Self::create_webview_for_tab(
                servo,
                ucm.as_ref(),
                active_index,
                url,
                window,
            );
            tab.webview = Some(webview);
            tab.rendering_context = Some(rendering_context);
        }
        tab.url = url_str.to_string();
        set_active_dirty(true);
    }

    /// Go back in history for the active tab
    pub fn go_back(&self) {
        if let Some(tab) = self.tabs.get(self.active_index) {
            if let Some(webview) = &tab.webview {
                set_active_dirty(true);
                webview.go_back(1);
            }
        }
    }

    /// Go forward in history for the active tab
    pub fn go_forward(&self) {
        if let Some(tab) = self.tabs.get(self.active_index) {
            if let Some(webview) = &tab.webview {
                set_active_dirty(true);
                webview.go_forward(1);
            }
        }
    }

    /// Reload the active tab
    pub fn reload(&self) {
        if let Some(tab) = self.tabs.get(self.active_index) {
            if let Some(webview) = &tab.webview {
                set_active_dirty(true);
                webview.reload();
            }
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
        self.tabs
            .get(self.active_index)
            .and_then(|t| t.webview.as_ref())
    }

    pub fn has_active_webview(&self) -> bool {
        self.get_active_webview().is_some()
    }

    /// Get the active index
    pub fn active_index(&self) -> usize {
        self.active_index
    }

    /// Get the active tab (if any)
    pub fn get_active_tab(&self) -> Option<&ServoTab> {
        self.tabs.get(self.active_index)
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
            if let Some(webview) = &tab.webview {
                webview.paint();
            }
        }
    }

    /// Get the current rendered frame of the active tab as a Slint Image.
    /// Reads pixels from Servo's offscreen FBO via CPU (glReadPixels).
    /// Note: BorrowedOpenGLTextureBuilder cannot be used here because Servo
    /// renders into a separate surfman GL context; Slint uses the winit GL
    /// context. Texture IDs are not portable across contexts without IOSurface.
    pub fn get_active_frame(&self) -> Option<slint::Image> {
        self.tabs.get(self.active_index).and_then(|tab| {
            let rendering_context = tab.rendering_context.as_ref()?;
            let size = rendering_context.size();
            let w = size.width;
            let h = size.height;
            if w == 0 || h == 0 {
                return None;
            }
            let rect =
                servo::DeviceIntRect::from_size(servo::DeviceIntSize::new(w as i32, h as i32));
            if let Some(image_buffer) = rendering_context.read_to_image(rect) {
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
            if let Some(webview) = &tab.webview {
                let size = PhysicalSize::new(width, height);
                set_active_dirty(true);
                webview.resize(size);
            }
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

        let render_scale = render_scale_for_window(scale);
        self.input_state
            .set_scale_factors(scale as f64, render_scale as f64);

        match event {
            WindowEvent::Resized(physical_size) => {
                log::debug!(
                    "[ServoEngine] Window resized: {:?} (scale={:.2})",
                    physical_size,
                    scale
                );
                self.update_scale_factor(scale);

                let logical_width = physical_size.width as f32 / scale.max(0.1);
                let logical_height = physical_size.height as f32 / scale.max(0.1);
                let width = (logical_width * render_scale).round().max(1.0) as u32;
                let height = ((logical_height - 76.0).max(1.0) * render_scale)
                    .round()
                    .max(1.0) as u32;

                if let Some(tab) = self.tabs.get(self.active_index) {
                    if let Some(webview) = &tab.webview {
                        webview.set_hidpi_scale_factor(euclid::Scale::new(render_scale));
                    }
                }
                self.resize_active(width, height);
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let scale = *scale_factor as f32;
                let render_scale = render_scale_for_window(scale);
                self.update_scale_factor(scale);
                self.input_state
                    .set_scale_factors(*scale_factor, render_scale as f64);
            }
            _ => {
                if let Some(servo_event) = input::translate_event(event, &mut self.input_state) {
                    if let Some(webview) = self.get_active_webview() {
                        webview.notify_input_event(servo_event);
                    }
                    // Pump Servo's event loop right here so the input event is
                    // processed in the same iteration. Without this, hover/click/
                    // scroll feedback would wait up to ~16ms for the heartbeat
                    // (and longer in debug builds when paint takes >16ms), which
                    // makes the page feel completely unresponsive.
                    self.spin_event_loop();
                }
            }
        }
    }
}
