//! JavaScript polyfills injected into every page via Servo's
//! `UserContentManager`. Each polyfill is kept in its own `.js` file for
//! readability and is concatenated at runtime into a single bundle.
//!
//! Order matters: foundational shims (queueMicrotask, AbortController,
//! IntersectionObserver) must register before higher-level ones that
//! depend on them or run in DOM scan loops (Custom Elements walks the DOM
//! with MutationObserver; site_rules dispatcher uses listeners that may
//! end up scheduled via the polyfilled APIs).

// Tier 1 — foundational primitives. These have no internal dependencies
// on other polyfills and must load first so anything below can rely on
// them being present.
const QUEUE_MICROTASK: &str = include_str!("queue_microtask.js");
const STRUCTURED_CLONE: &str = include_str!("structured_clone.js");
const ABORT_CONTROLLER: &str = include_str!("abort_controller.js");
const REQUEST_IDLE_CALLBACK: &str = include_str!("request_idle_callback.js");

// Tier 2 — observer APIs. The Custom Elements polyfill below uses
// MutationObserver to scan added nodes, so observers must initialize
// first if any of them are JS-based.
const INTERSECTION_OBSERVER: &str = include_str!("intersection_observer.js");
const RESIZE_OBSERVER: &str = include_str!("resize_observer.js");

// Tier 3 — DOM/global API patches. Safe to load after observers since none
// of these rely on the observer machinery.
const SVG_A_ELEMENT: &str = include_str!("svg_a_element.js");
const WEB_ANIMATIONS: &str = include_str!("web_animations.js");
const DIALOG_ELEMENT: &str = include_str!("dialog_element.js");
const CUSTOM_ELEMENTS: &str = include_str!("custom_elements.js");
const POINTER_EVENTS: &str = include_str!("pointer_events.js");
const CRYPTO_RANDOM_UUID: &str = include_str!("crypto_random_uuid.js");
const URL_CAN_PARSE: &str = include_str!("url_can_parse.js");

// Opt-in DevTools loader. Only appended to the bundle when FOE_ERUDA=1
// at startup so production runs don't pay the (one-time) CDN fetch.
const ERUDA_DEVTOOLS: &str = include_str!("eruda_devtools.js");

fn eruda_enabled() -> bool {
    matches!(
        std::env::var("FOE_ERUDA").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    )
}

/// Return the full polyfill bundle as a single string, ready to hand to
/// `servo::UserScript::from(...)`. Includes the per-domain site_rules
/// dispatcher tail so site-specific patches ship through the same script
/// injection — no extra plumbing needed on the embedder side.
pub fn bundle() -> String {
    let mut out = [
        "// === foe compat polyfills (injected by browser-core::compat) ===",
        QUEUE_MICROTASK,
        STRUCTURED_CLONE,
        ABORT_CONTROLLER,
        REQUEST_IDLE_CALLBACK,
        INTERSECTION_OBSERVER,
        RESIZE_OBSERVER,
        SVG_A_ELEMENT,
        WEB_ANIMATIONS,
        DIALOG_ELEMENT,
        CUSTOM_ELEMENTS,
        POINTER_EVENTS,
        CRYPTO_RANDOM_UUID,
        URL_CAN_PARSE,
    ]
    .join("\n");
    out.push('\n');
    out.push_str(&super::site_rules::runtime_dispatcher_js());
    if eruda_enabled() {
        out.push('\n');
        out.push_str(ERUDA_DEVTOOLS);
    }
    out
}
