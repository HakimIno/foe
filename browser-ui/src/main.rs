slint::include_modules!();

mod handlers;

#[cfg(feature = "engine-servo")]
pub mod servo_engine;
#[cfg(feature = "engine-servo")]
pub use servo_engine::ServoEngine as Engine;
#[cfg(feature = "engine-servo")]
mod rendering_setup;

#[cfg(feature = "engine-wry")]
pub mod wry_engine;
#[cfg(feature = "engine-wry")]
pub use wry_engine::WryEngine as Engine;

use browser_core::storage::Database;
use browser_core::shields::ShieldsEngine;
use browser_core::downloader::DownloadManager;
use std::sync::{Arc, Mutex};
use std::rc::Rc;
use std::cell::RefCell;
use i_slint_backend_winit::WinitWindowAccessor;
use slint::Model;
use std::sync::atomic::{AtomicI32, Ordering};

/// ตัวนับ id ของแท็บ — เริ่มที่ 1 เพราะแท็บเริ่มต้นจาก .slint literal default id = 0
static NEXT_TAB_ID: AtomicI32 = AtomicI32::new(1);

/// คืน id ใหม่ที่ไม่ซ้ำสำหรับแท็บ ใช้ผูก TabInfo (model) กับแท็บใน engine
pub fn next_tab_id() -> i32 {
    NEXT_TAB_ID.fetch_add(1, Ordering::Relaxed)
}

#[tokio::main]
async fn main() -> Result<(), slint::PlatformError> {
    let mut log_builder = env_logger::Builder::from_default_env();
    let js_log_enabled = matches!(
        std::env::var("FOE_JS_LOG").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    );
    if !js_log_enabled {
        log_builder
            .filter_module("script::script_runtime", log::LevelFilter::Off)
            .filter_module("script::dom::globalscope", log::LevelFilter::Off);
    } else {
        eprintln!("[foe] FOE_JS_LOG=1 — JS error/rejection logs are ON");
    }
    log_builder.init();

    #[cfg(target_os = "macos")]
    {
        let mut backend = i_slint_backend_winit::Backend::new()?;
        backend.window_attributes_hook = Some(Box::new(|attrs| {
            use i_slint_backend_winit::winit::platform::macos::WindowAttributesExtMacOS;
            attrs
                .with_fullsize_content_view(true)
                .with_title_hidden(true)
                .with_titlebar_transparent(true)
                .with_maximized(true)
        }));
        slint::platform::set_platform(Box::new(backend)).unwrap();
    }

    let db_path = "browser_data.db";
    let db = Arc::new(Mutex::new(Database::new(db_path).expect("Failed to initialize database")));
    let shields = Arc::new(Mutex::new(ShieldsEngine::new()));
    let download_manager = Arc::new(DownloadManager::new());

    let window = AppWindow::new()?;

    #[cfg(target_os = "macos")]
    window.set_has_titlebar_spacing(true);

    window.show().expect("Failed to show window");

    let _ = window.window().with_winit_window(|winit_window| {
        winit_window.set_maximized(true);
    });

    let engine = Rc::new(RefCell::new(Engine::new()));
    
    engine.borrow_mut().initialize(&window);

    // Sync engine layout ให้ตรงกับ tab-layout ปัจจุบันของ UI (Slint property คือ
    // single source of truth). Engine::new() อาจ default คนละค่ากับ UI ซึ่งจะทำให้
    // native webview ถูกวาง bounds ผิด layout ตอน startup (chrome "top" แต่ webview
    // ใช้ค่าของ "left" เป็นต้น) — sync ก่อนสร้าง webview ใดๆ
    {
        let layout = window.get_tab_layout();
        engine.borrow_mut().set_tab_layout(layout.as_str(), &window);
    }

    {
        let tabs_model = window.get_tabs();
        let mut eng = engine.borrow_mut();
        for i in 0..tabs_model.row_count() {
            if let Some(tab) = tabs_model.row_data(i) {
                eng.add_tab(&tab.url, tab.id, &window);
            }
        }
    }

    #[cfg(feature = "engine-servo")]
    let _heartbeat = rendering_setup::setup_rendering(&window, engine.clone());

    {
        let engine_clone = engine.clone();
        window.window().on_winit_window_event(move |winit_window, event| {
            let scale = winit_window.scale_factor() as f32;
            engine_clone.borrow_mut().handle_winit_event(event, scale);
            
            #[cfg(feature = "engine-wry")]
            if let i_slint_backend_winit::winit::event::WindowEvent::Resized(size) = event {
                engine_clone.borrow_mut().resize_from_event(*size, scale as f64);
            }
            
            i_slint_backend_winit::EventResult::Propagate
        });
    }

    handlers::navigation::setup(&window, db, shields, engine.clone());
    handlers::tabs::setup(&window, engine.clone());
    handlers::downloader::setup(&window, download_manager);
    handlers::command_bar::setup(&window);

    let window_weak = window.as_weak();
    window.on_start_drag_window(move || {
        if let Some(window) = window_weak.upgrade() {
            use i_slint_backend_winit::WinitWindowAccessor;
            let _ = window.window().with_winit_window(|winit_window| {
                let _ = winit_window.drag_window();
            });
        }
    });

    let window_weak = window.as_weak();
    window.on_double_click_titlebar(move || {
        if let Some(window) = window_weak.upgrade() {
            use i_slint_backend_winit::WinitWindowAccessor;
            let _ = window.window().with_winit_window(|winit_window| {
                let is_maximized = winit_window.is_maximized();
                winit_window.set_maximized(!is_maximized);
            });
        }
    });

    let window_weak = window.as_weak();
    let engine_clone = engine.clone();
    window.on_tab_layout_changed(move |layout| {
        if let Some(window) = window_weak.upgrade() {
            #[cfg(feature = "engine-wry")]
            engine_clone.borrow_mut().set_tab_layout(layout.as_str(), &window);
        }
    });

    #[cfg(feature = "engine-servo")]
    println!("[Bootstrap] foe initialized with Servo Engine 🚀");
    #[cfg(feature = "engine-wry")]
    println!("[Bootstrap] foe initialized with Wry Engine 🚀");
    
    window.run()?;
    Ok(())
}
