//! Web-compatibility shim layer that sits on top of Servo.
//!
//! Servo's web platform implementation has gaps (some Web APIs missing,
//! certain CSS features incomplete, UA-sniffing servers serving mobile
//! markup to anything that isn't Chrome/Firefox). This module is where we
//! paper over those gaps without touching Servo itself — everything here is
//! purely additive and easy to remove as Servo catches up upstream.
//!
//! ### Pieces
//!
//! * [`user_agent()`] — UA string we present to remote servers
//! * [`polyfill_bundle()`] — JS shims for missing Web APIs, registered via
//!   Servo's `UserContentManager.add_script()`
//! * [`baseline_stylesheet()`] — CSS rules registered via
//!   `UserContentManager.add_stylesheet()`
//! * [`site_rules`] — per-domain CSS/JS patches; empty skeleton today,
//!   ready for entries as we find sites that need them
//!
//! ### Runtime overrides
//!
//! These env vars let us toggle the compat layer at startup without a
//! rebuild — useful when diagnosing whether a rendering bug lives in Servo
//! itself or in our shims:
//!
//! * `FOE_USER_AGENT` — override the UA string entirely
//! * `FOE_DISABLE_POLYFILLS=1` — skip the JS polyfill bundle
//! * `FOE_DISABLE_BASELINE_CSS=1` — skip the baseline stylesheet
//! * `FOE_DISABLE_COMPAT=1` — shortcut: disables both polyfills and baseline

pub mod polyfills;
pub mod site_rules;
pub mod stylesheets;

/// Chrome 131 desktop UA on macOS. Many sites server-side sniff UA and
/// serve a stripped-down mobile/no-JS markup to unrecognized agents
/// (Servo's default "Firefox 140 / Servo" string trips this). Pretending to
/// be Chrome avoids that and lets us focus on real engine differences.
pub const DESKTOP_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
                              AppleWebKit/537.36 (KHTML, like Gecko) \
                              Chrome/131.0.0.0 Safari/537.36";

/// The UA string foe presents to remote servers. Reads `FOE_USER_AGENT`
/// from the environment to allow runtime overrides without a rebuild;
/// falls back to [`DESKTOP_UA`] when unset or empty.
pub fn user_agent() -> String {
    match std::env::var("FOE_USER_AGENT") {
        Ok(val) if !val.trim().is_empty() => val,
        _ => DESKTOP_UA.to_string(),
    }
}

fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    )
}

/// True when the JS polyfill bundle should be skipped at engine init.
/// Honors `FOE_DISABLE_POLYFILLS` and the umbrella `FOE_DISABLE_COMPAT`.
pub fn polyfills_disabled() -> bool {
    env_flag("FOE_DISABLE_POLYFILLS") || env_flag("FOE_DISABLE_COMPAT")
}

/// True when the baseline stylesheet should be skipped at engine init.
/// Honors `FOE_DISABLE_BASELINE_CSS` and the umbrella `FOE_DISABLE_COMPAT`.
pub fn baseline_css_disabled() -> bool {
    env_flag("FOE_DISABLE_BASELINE_CSS") || env_flag("FOE_DISABLE_COMPAT")
}

/// JavaScript polyfill bundle to register via
/// `UserContentManager.add_script()`. Allocates a fresh `String` per call —
/// call once at engine init, not per request.
pub fn polyfill_bundle() -> String {
    polyfills::bundle()
}

/// Cross-browser CSS baseline to register via
/// `UserContentManager.add_stylesheet()`.
pub fn baseline_stylesheet() -> &'static str {
    stylesheets::BASELINE
}
