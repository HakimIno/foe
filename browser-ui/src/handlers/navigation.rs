use crate::AppWindow;
use crate::servo_engine::ServoEngine;
use browser_core::storage::Database;
use browser_core::shields::ShieldsEngine;
use std::sync::{Arc, Mutex};
use std::rc::Rc;
use std::cell::RefCell;
use slint::{ComponentHandle, Model};

fn update_active_tab(window: &AppWindow, url: &str, title: &str) {
    let tabs_model = window.get_tabs();
    let mut tabs: Vec<crate::TabInfo> = tabs_model.iter().collect();
    for tab in tabs.iter_mut() {
        if tab.active {
            tab.url = url.to_string().into();
            tab.title = title.to_string().into();
            tab.site_type = crate::handlers::get_site_type(url).into();
        }
    }
    window.set_tabs(slint::ModelRc::new(slint::VecModel::from(tabs)));
}

fn clean_url_input(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") || trimmed.starts_with("file://") {
        trimmed.to_string()
    } else if trimmed.contains('.') && !trimmed.contains(' ') {
        format!("https://{}", trimmed)
    } else {
        let encoded: String = url::form_urlencoded::byte_serialize(trimmed.as_bytes()).collect();
        format!("http://127.0.0.1:8089/static/custom-ui/index.html?q={}", encoded)
    }
}

pub fn setup(
    window: &AppWindow,
    db: Arc<Mutex<Database>>,
    shields: Arc<Mutex<ShieldsEngine>>,
    servo_engine: Rc<RefCell<ServoEngine>>,
) {
    let window_weak = window.as_weak();
    let db_clone = db.clone();
    let shields_clone = shields.clone();
    let engine_clone = servo_engine.clone();
    
    window.on_navigate_to(move |url| {
        let window = window_weak.upgrade().unwrap();
        let target_url = clean_url_input(&url);
        
        println!("[Navigation] Navigating to: {}", target_url);
        window.set_current_url(target_url.clone().into());

        // Shields check
        let is_blocked = shields_clone.lock().unwrap().should_block(&target_url);
        if is_blocked {
            println!("[Shields] Intercepted and blocked: {}", target_url);
            let blocked_count = window.get_blocked_count();
            window.set_blocked_count(blocked_count + 1);
            let title = "Blocked by Shields".to_string();
            window.set_current_title(title.clone().into());
            update_active_tab(&window, &target_url, &title);
            return;
        }

        // Add history entry
        let title = format!("Webpage - {}", target_url);
        if let Err(e) = db_clone.lock().unwrap().add_history_entry(&target_url, &title) {
            eprintln!("[Database] Failed to save history: {}", e);
        }
        window.set_current_title(title.clone().into());
        update_active_tab(&window, &target_url, &title);

        // Navigate using Servo Engine
        engine_clone.borrow_mut().navigate(&target_url, &window);
    });

    let engine_clone = servo_engine.clone();
    let window_weak = window.as_weak();
    window.on_back_clicked(move || {
        let _window = window_weak.upgrade().unwrap();
        println!("[Navigation] Back clicked");
        engine_clone.borrow().go_back();
    });

    let engine_clone = servo_engine.clone();
    let window_weak = window.as_weak();
    window.on_forward_clicked(move || {
        let _window = window_weak.upgrade().unwrap();
        println!("[Navigation] Forward clicked");
        engine_clone.borrow().go_forward();
    });

    let engine_clone = servo_engine.clone();
    let window_weak = window.as_weak();
    window.on_reload_clicked(move || {
        let window = window_weak.upgrade().unwrap();
        let url = window.get_current_url();
        println!("[Navigation] Reloading: {}", url);
        engine_clone.borrow().reload();
    });

    let shields_clone = shields.clone();
    let window_weak = window.as_weak();
    window.on_toggle_shields(move || {
        let window = window_weak.upgrade().unwrap();
        let current_state = window.get_shields_active();
        let new_state = !current_state;
        
        shields_clone.lock().unwrap().set_enabled(new_state);
        window.set_shields_active(new_state);
        println!("[Shields] Engine enabled set to: {}", new_state);
    });
}
