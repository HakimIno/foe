use crate::{AppWindow, TabInfo};
use i_slint_backend_winit::WinitWindowAccessor;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::path::PathBuf;
use wry::{WebContext, WebView, WebViewBuilder, Rect};

const DESKTOP_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15";

/// JS ฝังตอนต้นเอกสาร — อ่าน <link rel=icon> ของหน้าจริง (เลือกตัวที่ใหญ่สุด)
/// แล้วส่ง host + URL ของ favicon กลับ Rust ผ่าน window.ipc.postMessage
/// รูปแบบข้อความ: "favicon\n<host>\n<absolute-favicon-url>"
const FAVICON_JS: &str = r#"(function(){
  function pick(){
    try {
      var links = document.querySelectorAll("link[rel~='icon'],link[rel='shortcut icon'],link[rel='apple-touch-icon']");
      var href = "", best = -1;
      links.forEach(function(l){
        var s = 0;
        if (l.sizes && l.sizes.value){ var m = /(\d+)x\d+/.exec(l.sizes.value); if(m){ s = parseInt(m[1],10); } }
        if (s >= best && l.href){ best = s; href = l.href; }
      });
      if (!href) href = location.origin + "/favicon.ico";
      if (href === window.__foeLastIcon) return; // กันยิงซ้ำ href เดิม
      window.__foeLastIcon = href;
      if (window.ipc && window.ipc.postMessage) {
        window.ipc.postMessage("favicon\n" + location.host + "\n" + href);
      }
    } catch(e) {}
  }
  if (document.readyState === "interactive" || document.readyState === "complete") pick();
  document.addEventListener("DOMContentLoaded", pick);
  window.addEventListener("load", pick);
})();"#;

/// favicon URL ที่ราก (origin + /favicon.ico) จาก URL ของหน้า — fallback ที่เชื่อถือได้
fn root_favicon(page_url: &str) -> Option<String> {
    url::Url::parse(page_url)
        .ok()?
        .join("/favicon.ico")
        .ok()
        .map(|u| u.to_string())
}

/// ดึง favicon ของแท็บ id ที่กำหนด (blocking ใน std::thread เพื่อไม่พึ่ง tokio
/// runtime context ของ wry callback) แล้วตั้งลง model ผ่าน Slint event loop.
/// ใช้ eprintln เพื่อให้เห็น log ได้โดยไม่ต้องตั้ง RUST_LOG
/// is_fallback = true (origin/favicon.ico) → ไม่เขียนทับถ้าแท็บมี favicon จาก
/// <link> จริงอยู่แล้ว (declared icon แม่นกว่า)
fn fetch_and_set_favicon(tab_id: i32, url: String, weak: slint::Weak<AppWindow>, is_fallback: bool) {
    std::thread::spawn(move || {
        eprintln!("[Favicon] tab {} fetching {}", tab_id, url);

        // UA เบราว์เซอร์ — บาง server ตอบต่าง/บล็อกถ้าไม่มี
        let client = match reqwest::blocking::Client::builder()
            .user_agent(DESKTOP_USER_AGENT)
            .timeout(std::time::Duration::from_secs(10))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[Favicon] tab {} client build failed: {}", tab_id, e);
                return;
            }
        };

        let bytes = match client.get(&url).send() {
            Ok(resp) => {
                let status = resp.status();
                match resp.bytes() {
                    Ok(b) => {
                        eprintln!("[Favicon] tab {} got {} bytes (HTTP {})", tab_id, b.len(), status);
                        b
                    }
                    Err(e) => {
                        eprintln!("[Favicon] tab {} read body failed: {}", tab_id, e);
                        return;
                    }
                }
            }
            Err(e) => {
                eprintln!("[Favicon] tab {} request failed: {}", tab_id, e);
                return;
            }
        };

        // image crate รองรับ png/ico/jpeg/gif/webp/bmp (ไม่รองรับ svg)
        let decoded = match image::load_from_memory(&bytes) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("[Favicon] tab {} decode failed (svg/unsupported?): {}", tab_id, e);
                return;
            }
        };
        let rgba = decoded.to_rgba8();
        let (w, h) = rgba.dimensions();
        if w == 0 || h == 0 {
            return;
        }
        let raw = rgba.into_raw();

        let _ = slint::invoke_from_event_loop(move || {
            let Some(window) = weak.upgrade() else { return };
            let buffer =
                slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&raw, w, h);
            let image = slint::Image::from_rgba8(buffer);

            let model = window.get_tabs();
            let mut tabs: Vec<TabInfo> = model.iter().collect();
            let mut changed = false;
            for tab in tabs.iter_mut() {
                if tab.id == tab_id {
                    // fallback (favicon.ico) ไม่ทับ declared icon ที่ตั้งไว้แล้ว
                    if is_fallback && tab.has_favicon {
                        continue;
                    }
                    tab.favicon = image.clone();
                    tab.has_favicon = true;
                    changed = true;
                }
            }
            if changed {
                window.set_tabs(ModelRc::new(VecModel::from(tabs)));
                eprintln!("[Favicon] tab {} applied ✓", tab_id);
            } else {
                eprintln!("[Favicon] tab {} no longer in model (closed?)", tab_id);
            }
        });
    });
}

/// ตั้งสถานะ loading ของแท็บ id ที่กำหนด (true ตอนเริ่มโหลด → สปินเนอร์หมุน)
fn set_tab_loading(tab_id: i32, loading: bool, weak: slint::Weak<AppWindow>) {
    let _ = slint::invoke_from_event_loop(move || {
        let Some(window) = weak.upgrade() else { return };
        let model = window.get_tabs();
        let mut tabs: Vec<TabInfo> = model.iter().collect();
        let mut changed = false;
        for tab in tabs.iter_mut() {
            if tab.id == tab_id && tab.loading != loading {
                tab.loading = loading;
                changed = true;
            }
        }
        if changed {
            window.set_tabs(ModelRc::new(VecModel::from(tabs)));
        }
    });
}

/// ตั้ง title จริงจากหน้าเว็บ (document.title) ลงแท็บ id ที่กำหนด
fn set_tab_title(tab_id: i32, title: String, weak: slint::Weak<AppWindow>) {
    if title.trim().is_empty() {
        return;
    }
    let _ = slint::invoke_from_event_loop(move || {
        let Some(window) = weak.upgrade() else { return };
        let model = window.get_tabs();
        let mut tabs: Vec<TabInfo> = model.iter().collect();
        let mut changed = false;
        for tab in tabs.iter_mut() {
            if tab.id == tab_id {
                tab.title = title.clone().into();
                if tab.active {
                    window.set_current_title(title.clone().into());
                }
                changed = true;
            }
        }
        if changed {
            window.set_tabs(ModelRc::new(VecModel::from(tabs)));
        }
    });
}

/// JS injected เมื่อ tab ถูกพับไปอยู่ background. WKWebView/WebView2 ยังรัน
/// timer/media ของ tab ที่ซ่อนต่อไป — ตัวที่กิน CPU/แบตจริงคือ media ที่เล่น
/// ค้างไว้ เพราะงั้น pause ทุก <video>/<audio> แล้วจำตัวที่กำลังเล่นไว้ใน
/// window.__foeSuspended เพื่อ resume ตอนกลับมา.
const SUSPEND_JS: &str = r#"(function(){
  try {
    var paused = [];
    document.querySelectorAll('video,audio').forEach(function(m){
      if(!m.paused){ paused.push(m); m.pause(); }
    });
    window.__foeSuspended = paused;
  } catch(e) {}
})();"#;

/// JS injected เมื่อ tab กลับมา active — resume เฉพาะ media ที่เรา pause ไว้
/// ตอน suspend (ไม่ไปเล่น media ที่ user หยุดเอง).
const RESUME_JS: &str = r#"(function(){
  try {
    (window.__foeSuspended||[]).forEach(function(m){ m.play().catch(function(){}); });
    window.__foeSuspended = [];
  } catch(e) {}
})();"#;

pub struct WryTab {
    pub webview: Option<WebView>,
    pub url: String,
    pub title: String,
    /// id เสถียร ตรงกับ TabInfo.id ใน Slint model — ใช้ผูก callback ของ webview
    /// (title/favicon) กลับไปยังแท็บที่ถูกต้องแม้ลำดับจะถูกสลับ
    pub id: i32,
}

pub struct WryEngine {
    tabs: Vec<WryTab>,
    active_index: usize,
    tab_layout: String,
    bounds: Rect,
    /// WebContext เดียวที่แชร์ข้ามทุก tab — cache/cookie/network stack กองเดียว
    /// แทนที่จะแยกต่อ webview ประหยัด RAM/disk และทำให้ session ต่อเนื่องกัน.
    web_context: WebContext,
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
            tab_layout: "left".to_string(),
            bounds: Rect {
                position: wry::dpi::Position::Logical(wry::dpi::LogicalPosition::new(0.0, y_pos)),
                size: wry::dpi::Size::Logical(wry::dpi::LogicalSize::new(800.0, 600.0)),
            },
            // data dir ถาวรไว้ที่ cwd (ข้างๆ browser_data.db) เพื่อให้ cache/
            // cookie อยู่ข้าม session; ใช้บน WebKitGTK (Linux) เป็นหลัก ส่วน
            // macOS/Windows ใช้ default store แต่การแชร์ context ตัวเดียวยัง
            // ช่วยให้ทุก tab อยู่บน data store เดียวกัน.
            web_context: WebContext::new(Some(PathBuf::from("wry_data"))),
        }
    }

    pub fn set_tab_layout(&mut self, layout: &str, window: &AppWindow) {
        self.tab_layout = layout.to_string();
        self.update_bounds(window);
    }

    pub fn initialize(&mut self, window: &AppWindow) {
        log::info!("[WryEngine] Initializing Wry engine...");
        self.update_bounds(window);
    }

    /// คำนวณ bounds ของ webview จากขนาด logical ของหน้าต่าง + tab_layout ปัจจุบัน.
    /// รวม magic number ของ chrome ไว้ที่เดียว เพื่อให้ update_bounds และ
    /// resize_from_event ใช้ค่าตรงกัน (เดิม resize ใช้ค่าคงที่ 76 ไม่สน layout).
    fn compute_bounds(&self, logical_w: f64, logical_h: f64) -> Rect {
        let mut x = 0.0;
        let mut y = 0.0;
        let mut w = logical_w;
        let mut h = logical_h;

        match self.tab_layout.as_str() {
            "top" => {
                // TabBar (40) + Navbar (40) + toolbar bottom padding (4)
                y = 84.0;
                h = logical_h - 84.0;
            }
            "bottom" => {
                y = 40.0; // Navbar only
                h = logical_h - 80.0; // Navbar (40) + TabBar (40)
            }
            "left" => {
                x = 230.0;
                y = 40.0; // Navbar only
                w = logical_w - 230.0;
                h = logical_h - 40.0;
            }
            "right" => {
                x = 0.0;
                y = 40.0; // Navbar only
                w = logical_w - 230.0;
                h = logical_h - 40.0;
            }
            _ => {}
        }

        // wry บน macOS วาง webview ด้วยพิกัด bottom-left origin (Cocoa) ส่วน y ที่
        // คำนวณข้างบนเป็น top-left (ระยะจากขอบบน) จึงต้องแปลงกลับ ไม่งั้น webview
        // จะไปโผล่ผิดด้าน (chrome หายไป + เกิดแถบว่างที่ก้นหน้าต่าง).
        #[cfg(target_os = "macos")]
        let y = logical_h - (y + h);

        Rect {
            position: wry::dpi::Position::Logical(wry::dpi::LogicalPosition::new(x, y)),
            size: wry::dpi::Size::Logical(wry::dpi::LogicalSize::new(w.max(1.0), h.max(1.0))),
        }
    }

    fn update_bounds(&mut self, window: &AppWindow) {
        if let Some(size) = window.window().with_winit_window(|w: &i_slint_backend_winit::winit::window::Window| w.inner_size()) {
            let scale = window.window().with_winit_window(|w: &i_slint_backend_winit::winit::window::Window| w.scale_factor()).unwrap_or(1.0);
            let logical_w = size.width as f64 / scale;
            let logical_h = size.height as f64 / scale;

            self.bounds = self.compute_bounds(logical_w, logical_h);

            if let Some(tab) = self.tabs.get(self.active_index) {
                if let Some(wv) = &tab.webview {
                    let _ = wv.set_bounds(self.bounds.clone());
                }
            }
        }
    }

    pub fn add_tab(&mut self, url_str: &str, id: i32, window: &AppWindow) {
        let is_native = url_str == "about:newtab" || url_str == "about:blank" || url_str.is_empty();

        if is_native {
            self.tabs.push(WryTab {
                webview: None,
                url: url_str.to_string(),
                title: "Google".to_string(),
                id,
            });
            return;
        }

        let webview = self.create_webview(url_str, id, window);
        self.tabs.push(WryTab {
            webview,
            url: url_str.to_string(),
            title: "New Tab".to_string(),
            id,
        });
    }

    fn create_webview(&mut self, url: &str, id: i32, window: &AppWindow) -> Option<WebView> {
        // ดึงค่าออกมาเป็น local ก่อน เพื่อไม่ให้ closure capture self ทั้ง &
        // และ &mut พร้อมกัน (web_context ต้องยืมแบบ &mut ตอน build).
        let bounds = self.bounds.clone();
        let ctx = &mut self.web_context;
        let ipc_weak = window.as_weak();
        let title_weak = window.as_weak();
        let pageload_weak = window.as_weak();
        window.window().with_winit_window(move |winit_window: &i_slint_backend_winit::winit::window::Window| {
            let builder = WebViewBuilder::new_as_child(winit_window)
                .with_url(url)
                .with_bounds(bounds)
                .with_user_agent(DESKTOP_USER_AGENT)
                // swipe trackpad ย้อน/ไปหน้า แบบ native (wry 0.44 ไม่มี back()/
                // forward() ให้เรียกตรงๆ — ปุ่มยังต้องใช้ JS history ใน go_back).
                .with_back_forward_navigation_gestures(true)
                // title จริงจากหน้าเว็บ → อัปเดตชื่อแท็บ (wry รายงานผ่าน callback นี้)
                .with_document_title_changed_handler(move |title| {
                    set_tab_title(id, title, title_weak.clone());
                })
                // favicon เส้นหลัก (native callback ที่เชื่อถือได้ เหมือน title):
                // หน้าโหลดเสร็จ → ดึง origin/favicon.ico
                .with_on_page_load_handler(move |event, page_url| {
                    match event {
                        wry::PageLoadEvent::Started => {
                            set_tab_loading(id, true, pageload_weak.clone());
                        }
                        wry::PageLoadEvent::Finished => {
                            set_tab_loading(id, false, pageload_weak.clone());
                            if let Some(fav) = root_favicon(&page_url) {
                                fetch_and_set_favicon(id, fav, pageload_weak.clone(), true);
                            }
                        }
                    }
                })
                // favicon เส้นเสริม: อ่าน <link rel=icon> จริงผ่าน JS + IPC (แม่นกว่า)
                .with_initialization_script(FAVICON_JS)
                .with_ipc_handler(move |req| {
                    let body = req.body();
                    if let Some(rest) = body.strip_prefix("favicon\n") {
                        // host อยู่บรรทัดกลาง, favicon url บรรทัดท้าย
                        let mut parts = rest.splitn(2, '\n');
                        let _host = parts.next().unwrap_or("");
                        let fav_url = parts.next().unwrap_or("").to_string();
                        if !fav_url.is_empty() {
                            fetch_and_set_favicon(id, fav_url, ipc_weak.clone(), false);
                        }
                    }
                })
                .with_web_context(ctx);

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
                let active = i == self.active_index;
                let _ = wv.set_visible(active);
                if active {
                    let _ = wv.set_bounds(self.bounds.clone());
                    let _ = wv.evaluate_script(RESUME_JS);
                } else {
                    // tab พื้นหลัง: pause media เพื่อตัด CPU/แบตที่เสียไปเปล่าๆ.
                    let _ = wv.evaluate_script(SUSPEND_JS);
                }
            }
        }
    }

    pub fn navigate(&mut self, url_str: &str, window: &AppWindow) {
        let active_tab = self.tabs.get(self.active_index);
        let needs_webview = active_tab.map(|t| t.webview.is_none()).unwrap_or(false);
        let active_id = active_tab.map(|t| t.id).unwrap_or(0);

        let new_webview = if needs_webview {
            self.create_webview(url_str, active_id, window)
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
                // native load_url แทน JS location.reload() — ทำงานได้แม้หน้า
                // ค้าง/JS error. หมายเหตุ: tab.url เป็น URL ที่ navigate ครั้ง
                // ล่าสุด ถ้าเป็น SPA ที่เปลี่ยน path ฝั่ง client อาจ reload กลับ
                // ไป URL ตั้งต้น (trade-off ที่ยอมรับได้เพื่อความทนทาน).
                let _ = wv.load_url(&tab.url);
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

        // ใช้ compute_bounds ตัวเดียวกับ update_bounds เพื่อให้ y-offset/layout
        // ตรงกันทุกเส้นทาง (เดิม resize hardcode 76 + y=0 ไม่สน tab_layout จึงไป
        // วาง webview ทับขอบน้ำเงิน 12px ใต้ navbar ทำให้ขอบหาย)
        self.bounds = self.compute_bounds(logical_w, logical_h);

        if let Some(tab) = self.tabs.get(self.active_index) {
            if let Some(wv) = &tab.webview {
                let _ = wv.set_bounds(self.bounds.clone());
            }
        }
    }
}
