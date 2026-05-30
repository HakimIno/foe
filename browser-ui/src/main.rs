slint::include_modules!();

mod handlers;
pub mod servo_engine;
mod rendering_setup;

use browser_core::storage::Database;
use browser_core::shields::ShieldsEngine;
use browser_core::downloader::DownloadManager;
use std::sync::{Arc, Mutex};
use std::rc::Rc;
use std::cell::RefCell;
use servo_engine::ServoEngine;
use i_slint_backend_winit::WinitWindowAccessor;
use slint::Model;

#[tokio::main]
async fn main() -> Result<(), slint::PlatformError> {
    // Initialize logging
    env_logger::init();

    // 0. Configure macOS titlebar transparency and full-size content view
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

    // 1. Initialize Backend Core Services
    let db_path = "browser_data.db";
    let db = Arc::new(Mutex::new(Database::new(db_path).expect("Failed to initialize database")));
    let shields = Arc::new(Mutex::new(ShieldsEngine::new()));
    let download_manager = Arc::new(DownloadManager::new());

    // 2. Initialize UI Application Window
    let window = AppWindow::new()?;

    #[cfg(target_os = "macos")]
    window.set_has_titlebar_spacing(true);

    // Show the window first to realize the winit window handles and acquire valid sizes
    window.show().expect("Failed to show window");

    // Open maximized on all platforms (macOS hook above also sets this at creation time)
    let _ = window.window().with_winit_window(|winit_window| {
        winit_window.set_maximized(true);
    });

    // 3. Initialize Servo Engine
    let servo_engine = Rc::new(RefCell::new(ServoEngine::new()));
    
    // Initialize Servo with window context
    servo_engine.borrow_mut().initialize(&window);

    // Create initial tabs with Servo WebViews
    {
        let tabs_model = window.get_tabs();
        let mut engine = servo_engine.borrow_mut();
        for i in 0..tabs_model.row_count() {
            if let Some(tab) = tabs_model.row_data(i) {
                engine.add_tab(&tab.url, &window);
            }
        }
    }

    // 4. Set up rendering callbacks, event-driven paint trigger, and heartbeat timer
    let _heartbeat = rendering_setup::setup_rendering(&window, servo_engine.clone());


    // 5. Window event handler (resize and inputs)
    {
        let engine_clone = servo_engine.clone();
        window.window().on_winit_window_event(move |winit_window, event| {
            let scale = winit_window.scale_factor() as f32;
            engine_clone.borrow_mut().handle_winit_event(event, scale);
            i_slint_backend_winit::EventResult::Propagate
        });
    }

    // 6. Delegate UI Event Bindings to Modular Handlers
    handlers::navigation::setup(&window, db, shields, servo_engine.clone());
    handlers::tabs::setup(&window, servo_engine.clone());
    handlers::downloader::setup(&window, download_manager);
    handlers::command_bar::setup(&window);

    // Bind Custom Window Dragging & Double-Click to Maximize
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

    // 7. Start App Event Loop
    println!("[Bootstrap] foe initialized with Servo Engine 🚀");
    window.run()?;
    Ok(())
}
