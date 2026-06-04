use crate::{AppWindow, TabInfo};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::rc::Rc;
use std::cell::RefCell;

pub fn setup(window: &AppWindow, servo_engine: Rc<RefCell<crate::Engine>>) {
    setup_select_tab(window, servo_engine.clone());
    setup_new_tab(window, servo_engine.clone());
    setup_close_tab(window, servo_engine.clone());
    setup_move_tab(window, servo_engine.clone());
}

fn setup_select_tab(window: &AppWindow, servo_engine: Rc<RefCell<crate::Engine>>) {
    let window_weak = window.as_weak();
    let engine_clone = servo_engine.clone();

    window.on_select_tab(move |index| {
        let window = window_weak.upgrade().unwrap();
        let tabs_model = window.get_tabs();
        let mut tabs: Vec<TabInfo> = tabs_model.iter().collect();

        for (i, tab) in tabs.iter_mut().enumerate() {
            tab.active = i == index as usize;
            if tab.active {
                window.set_current_url(tab.url.clone());
                window.set_current_title(tab.title.clone());
            }
        }

        window.set_tabs(ModelRc::new(VecModel::from(tabs)));
        engine_clone.borrow_mut().set_active_tab(index as usize);
    });
}

fn setup_new_tab(window: &AppWindow, servo_engine: Rc<RefCell<crate::Engine>>) {
    let window_weak = window.as_weak();
    let engine_clone = servo_engine.clone();

    window.on_new_tab(move || {
        let window = window_weak.upgrade().unwrap();
        let tabs_model = window.get_tabs();
        let mut tabs: Vec<TabInfo> = tabs_model.iter().collect();

        for tab in tabs.iter_mut() {
            tab.active = false;
        }

        let default_url = "about:newtab";
        tabs.push(TabInfo {
            title: "Google".into(),
            url: default_url.into(),
            active: true,
            is_pinned: false,
            site_type: "google".into(),
            has_favicon: false,
            favicon: slint::Image::default(),
        });

        window.set_current_url(default_url.into());
        window.set_current_title("Google".into());
        window.set_tabs(ModelRc::new(VecModel::from(tabs)));

        // Add Servo WebView for new tab and activate it
        let mut engine = engine_clone.borrow_mut();
        engine.add_tab(default_url, &window);
        let idx = engine.len() - 1;
        engine.set_active_tab(idx);
    });
}

fn setup_close_tab(window: &AppWindow, servo_engine: Rc<RefCell<crate::Engine>>) {
    let window_weak = window.as_weak();
    let engine_clone = servo_engine.clone();

    window.on_close_tab(move |index| {
        let window = window_weak.upgrade().unwrap();
        let tabs_model = window.get_tabs();
        let mut tabs: Vec<TabInfo> = tabs_model.iter().collect();

        if tabs.len() <= 1 {
            return;
        }

        tabs.remove(index as usize);

        let mut has_active = false;
        for tab in &tabs {
            if tab.active {
                has_active = true;
                window.set_current_url(tab.url.clone());
                window.set_current_title(tab.title.clone());
                break;
            }
        }

        if !has_active && !tabs.is_empty() {
            tabs[0].active = true;
            window.set_current_url(tabs[0].url.clone());
            window.set_current_title(tabs[0].title.clone());
        }

        window.set_tabs(ModelRc::new(VecModel::from(tabs)));

        // Close Servo WebView
        engine_clone.borrow_mut().close_tab(index as usize);
    });
}

fn setup_move_tab(window: &AppWindow, servo_engine: Rc<RefCell<crate::Engine>>) {
    let window_weak = window.as_weak();
    let engine_clone = servo_engine.clone();

    window.on_move_tab(move |from, to| {
        let from = from as usize;
        let to = to as usize;
        if from == to {
            return;
        }

        let window = window_weak.upgrade().unwrap();
        let tabs_model = window.get_tabs();
        let mut tabs: Vec<TabInfo> = tabs_model.iter().collect();

        if from >= tabs.len() || to >= tabs.len() {
            return;
        }

        let tab = tabs.remove(from);
        tabs.insert(to, tab);

        window.set_tabs(ModelRc::new(VecModel::from(tabs)));

        // Move Servo WebView
        engine_clone.borrow_mut().move_tab(from, to);
    });
}
