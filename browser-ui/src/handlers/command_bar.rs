use crate::AppWindow;
use slint::ComponentHandle;

pub fn setup(window: &AppWindow) {
    let window_weak = window.as_weak();
    
    window.on_command_bar_submit(move |query| {
        let window = window_weak.upgrade().unwrap();
        let query_str = query.trim().to_string();
        println!("[CommandBar] Query submitted: {}", query_str);
        
        if query_str.starts_with("http://") || query_str.starts_with("https://") {
            window.set_current_url(query_str.clone().into());
            window.set_current_title(format!("Loading - {}", query_str).into());
        } else {
            let search_url = format!("https://duckduckgo.com/?q={}", query_str);
            window.set_current_url(search_url.into());
            window.set_current_title(format!("Search - {}", query_str).into());
        }
    });
}
