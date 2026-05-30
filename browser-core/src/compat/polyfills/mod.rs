//! JavaScript polyfills injected into every page via Servo's
//! `UserContentManager`. Each polyfill is kept in its own `.js` file for
//! readability and is concatenated at runtime into a single bundle.
//!
//! Order matters: foundational shims (IntersectionObserver, MutationObserver-
//! based scanners) must register before higher-level ones that depend on
//! them (Custom Elements walks the DOM with MutationObserver).

const INTERSECTION_OBSERVER: &str = include_str!("intersection_observer.js");
const RESIZE_OBSERVER: &str = include_str!("resize_observer.js");
const SVG_A_ELEMENT: &str = include_str!("svg_a_element.js");
const WEB_ANIMATIONS: &str = include_str!("web_animations.js");
const CUSTOM_ELEMENTS: &str = include_str!("custom_elements.js");
const POINTER_EVENTS: &str = include_str!("pointer_events.js");

/// Return the full polyfill bundle as a single string, ready to hand to
/// `servo::UserScript::from(...)`.
pub fn bundle() -> String {
    [
        "// === foe compat polyfills (injected by browser-core::compat) ===",
        INTERSECTION_OBSERVER,
        RESIZE_OBSERVER,
        SVG_A_ELEMENT,
        WEB_ANIMATIONS,
        CUSTOM_ELEMENTS,
        POINTER_EVENTS,
    ]
    .join("\n")
}
