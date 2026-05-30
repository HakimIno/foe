//! Per-domain compatibility rules. Lets us paper over Servo bugs on
//! specific sites without polluting the global baseline polyfill/CSS.
//!
//! ## How it ships
//!
//! All rules are bundled into a single JS dispatcher built at startup by
//! [`runtime_dispatcher_js()`]. That dispatcher is appended to the global
//! polyfill bundle and runs on every page load — it reads
//! `window.location.hostname` and applies the CSS/JS of any rule whose
//! domain matches. Top-frame only; iframes are skipped to avoid double-
//! application when a page embeds itself (common analytics pattern).
//!
//! ## Adding a rule
//!
//! Append to [`RULES`] below. Domain matches the host itself or any
//! subdomain (`SiteRule { domain_pattern: "example.com", .. }` matches
//! both `example.com` and `foo.example.com`).
//!
//! ```ignore
//! SiteRule {
//!     domain_pattern: "example.com",
//!     inject_css: Some("nav.broken-flex { display: block; }"),
//!     inject_js: Some("/* runs once after DOM ready */"),
//! },
//! ```
//!
//! Keep rules minimal — anything that helps every site belongs in
//! `compat::stylesheets::BASELINE` or `compat::polyfills` instead.

/// A compatibility patch scoped to one domain (and its subdomains).
#[derive(Debug, Clone, Copy)]
pub struct SiteRule {
    /// Bare domain (e.g. `"chatgpt.com"`). Matches the host itself or any
    /// subdomain (`"foo.chatgpt.com"`).
    pub domain_pattern: &'static str,
    /// CSS appended to the page via a <style> tag, if any.
    pub inject_css: Option<&'static str>,
    /// JS evaluated once after DOMContentLoaded, if any. Wrapped in
    /// try/catch by the dispatcher so a bad rule can't tank the page.
    pub inject_js: Option<&'static str>,
}

/// Active per-domain rules.
///
/// Entries here should be the smallest patch that unbreaks the site —
/// anything broader probably belongs in the global baseline.
const RULES: &[SiteRule] = &[
    // Wikipedia: their responsive image grid relies on `image-rendering:
    // crisp-edges` for retina thumbnails — Servo's value of `auto` makes
    // them look slightly blurry on HiDPI. Forcing crisp-edges matches
    // Chrome/Firefox visual behavior.
    SiteRule {
        domain_pattern: "wikipedia.org",
        inject_css: Some(
            "img.mw-file-element { image-rendering: auto; } \
             .mw-parser-output figure { max-width: 100%; }",
        ),
        inject_js: None,
    },
    // MDN docs: code blocks rely on `scrollbar-gutter: stable` for layout
    // stability (Servo doesn't support it yet). Reserving a fixed gutter
    // via padding keeps the line numbers from jumping when content
    // overflows.
    SiteRule {
        domain_pattern: "developer.mozilla.org",
        inject_css: Some(
            "pre.notranslate { scrollbar-gutter: auto; padding-right: 12px; }",
        ),
        inject_js: None,
    },
    // GitHub: their Primer design system uses `:has()` selectors for
    // styling hover/focus states on nested elements. Servo's `:has()` is
    // partial — fall back to a static hover style so nav items don't look
    // dead. Scoped to the global navigation only.
    SiteRule {
        domain_pattern: "github.com",
        inject_css: Some(
            ".AppHeader-globalBar a:hover, header[role='banner'] a:hover { \
             text-decoration: underline; }",
        ),
        inject_js: None,
    },
];

/// All rules that apply to the given host (matches the host itself or any
/// suffix of it). Returns a `Vec` because the closure capturing `host`
/// otherwise leaks a borrow lifetime into the opaque `impl Iterator` type,
/// which is awkward at call sites. The rule list is tiny so this is fine.
pub fn for_domain(host: &str) -> Vec<&'static SiteRule> {
    RULES
        .iter()
        .filter(|rule| domain_matches(host, rule.domain_pattern))
        .collect()
}

fn domain_matches(host: &str, pattern: &str) -> bool {
    if host == pattern {
        return true;
    }
    host.len() > pattern.len()
        && host.ends_with(pattern)
        && host[..host.len() - pattern.len()].ends_with('.')
}

/// Build the JS dispatcher that picks the right rule at page load. Called
/// once at startup from [`crate::compat::polyfill_bundle()`] and appended
/// to the polyfill stream so every WebView receives it via the global
/// UserContentManager.
///
/// The generated script:
/// 1. Skips iframes (`window.top !== window`) to avoid double-apply.
/// 2. Reads `location.hostname` and matches against the embedded rule list.
/// 3. Injects a `<style>` element for each matching rule's CSS.
/// 4. Evaluates each rule's JS inside try/catch.
///
/// Runs the work synchronously if `document.head` is already present;
/// otherwise defers to `DOMContentLoaded`. Either way each rule applies
/// at most once per navigation.
pub fn runtime_dispatcher_js() -> String {
    // Generate one entry per rule as a JS object literal. We serialize CSS
    // and JS as JSON-escaped strings to handle quotes/newlines safely.
    let mut entries = String::new();
    for rule in RULES {
        entries.push_str("    { domain: ");
        entries.push_str(&js_string_literal(rule.domain_pattern));
        entries.push_str(", css: ");
        match rule.inject_css {
            Some(css) => entries.push_str(&js_string_literal(css)),
            None => entries.push_str("null"),
        }
        entries.push_str(", js: ");
        match rule.inject_js {
            Some(js) => entries.push_str(&js_string_literal(js)),
            None => entries.push_str("null"),
        }
        entries.push_str(" },\n");
    }

    format!(
        r#"// === foe compat site_rules dispatcher ===
(function () {{
    // Skip iframes — top-level navigation is where the rule should apply.
    try {{ if (window.top !== window) return; }} catch (e) {{ /* cross-origin parent: treat as top */ }}

    var host = '';
    try {{ host = (location && location.hostname) || ''; }} catch (e) {{ return; }}
    if (!host) return;

    var RULES = [
{entries}    ];

    function matches(h, pattern) {{
        if (h === pattern) return true;
        return h.length > pattern.length
            && h.slice(-pattern.length) === pattern
            && h.charAt(h.length - pattern.length - 1) === '.';
    }}

    function apply() {{
        for (var i = 0; i < RULES.length; i++) {{
            var rule = RULES[i];
            if (!matches(host, rule.domain)) continue;
            if (rule.css) {{
                try {{
                    var style = document.createElement('style');
                    style.setAttribute('data-foe-site-rule', rule.domain);
                    style.textContent = rule.css;
                    (document.head || document.documentElement).appendChild(style);
                }} catch (e) {{ /* swallow */ }}
            }}
            if (rule.js) {{
                try {{ (new Function(rule.js))(); }} catch (e) {{ /* swallow */ }}
            }}
        }}
    }}

    if (document.head) {{
        apply();
    }} else if (typeof document.addEventListener === 'function') {{
        document.addEventListener('DOMContentLoaded', apply, {{ once: true }});
    }}
}})();
"#,
        entries = entries
    )
}

/// Escape an arbitrary string into a JS double-quoted literal. Handles the
/// characters that would otherwise break the literal: `\`, `"`, control
/// chars (newline, tab, CR), and Unicode line separators that JS treats as
/// line terminators inside strings.
fn js_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str(r"\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_exact_and_subdomain() {
        assert!(domain_matches("example.com", "example.com"));
        assert!(domain_matches("foo.example.com", "example.com"));
        assert!(domain_matches("a.b.example.com", "example.com"));
    }

    #[test]
    fn rejects_unrelated_hosts() {
        assert!(!domain_matches("notexample.com", "example.com"));
        assert!(!domain_matches("example.com.evil.com", "example.com"));
        assert!(!domain_matches("example.org", "example.com"));
    }

    #[test]
    fn dispatcher_embeds_every_rule_domain() {
        let js = runtime_dispatcher_js();
        for rule in RULES {
            assert!(
                js.contains(rule.domain_pattern),
                "dispatcher missing domain {}",
                rule.domain_pattern
            );
        }
    }

    #[test]
    fn js_string_literal_escapes_dangerous_chars() {
        assert_eq!(js_string_literal("a"), "\"a\"");
        assert_eq!(js_string_literal("a\"b"), "\"a\\\"b\"");
        assert_eq!(js_string_literal("a\\b"), "\"a\\\\b\"");
        assert_eq!(js_string_literal("a\nb"), "\"a\\nb\"");
    }
}
