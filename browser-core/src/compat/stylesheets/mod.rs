//! User stylesheets injected into every page via Servo's
//! `UserContentManager.add_stylesheet()`. Keep additions here minimal and
//! generic; per-site CSS belongs in `compat::site_rules`.

/// Cross-browser CSS baseline that papers over common rendering
/// inconsistencies between Servo and Chrome/Safari.
pub const BASELINE: &str = include_str!("baseline.css");
