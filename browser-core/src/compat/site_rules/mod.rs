//! Per-domain compatibility rules. Lets us paper over Servo bugs on
//! specific sites without polluting the global baseline polyfill/CSS.
//!
//! Currently a skeleton — populate `RULES` as we encounter sites that need
//! targeted fixes. The navigation handler can call [`for_domain`] just
//! before load and apply each matching rule's CSS + JS via the WebView's
//! [`servo::UserContentManager`], or as a one-shot `evaluate_javascript`
//! after the page settles.
//!
//! ## Adding a rule
//!
//! ```ignore
//! const RULES: &[SiteRule] = &[
//!     SiteRule {
//!         domain_pattern: "example.com",
//!         inject_css: Some("nav.broken-flex { display: block; }"),
//!         inject_js: Some("/* one-off shim */"),
//!     },
//! ];
//! ```

/// A compatibility patch scoped to one domain (and its subdomains).
#[derive(Debug, Clone, Copy)]
pub struct SiteRule {
    /// Bare domain (e.g. `"chatgpt.com"`). Matches the host itself or any
    /// subdomain (`"foo.chatgpt.com"`).
    pub domain_pattern: &'static str,
    /// CSS appended to the page via UserStyleSheet, if any.
    pub inject_css: Option<&'static str>,
    /// JS appended to the page via UserScript, if any.
    pub inject_js: Option<&'static str>,
}

/// Add entries here as we discover site-specific compat issues.
const RULES: &[SiteRule] = &[
    // TODO: populate. Leave empty for now; the API exists so callers can
    // wire it up without further plumbing changes when rules show up.
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

#[cfg(test)]
mod tests {
    use super::domain_matches;

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
}
