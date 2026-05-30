// Eruda DevTools loader.
//
// Servo has no built-in DevTools, which makes diagnosing "why is this page
// broken" painful. Eruda (https://github.com/liriliri/eruda) is a tiny
// in-page DevTools panel — Console, Elements, Network, Resources, Sources
// — that runs entirely as JS and can render in any modern engine. We
// inject it from jsDelivr's CDN so foe doesn't need to bundle the 200KB
// script for the common case where DevTools aren't needed.
//
// This file is only added to the polyfill bundle when FOE_ERUDA=1 is set
// at startup (see browser-core/src/compat/polyfills/mod.rs::bundle()), so
// in normal runs there's zero overhead.
//
// The loader is defensive about three things:
//   1. document.body may not exist yet at script eval time — we defer
//      injection until DOMContentLoaded.
//   2. CDN may be unreachable (offline, blocked by shields, …) — onerror
//      logs to console instead of throwing.
//   3. Multiple navigations re-evaluate the script — the early-return on
//      window.__foeErudaLoaded__ avoids double-mounting the panel.
(function () {
    if (typeof window === 'undefined' || typeof document === 'undefined') return;
    if (window.__foeErudaLoaded__) return;
    window.__foeErudaLoaded__ = true;

    const inject = function () {
        try {
            const script = document.createElement('script');
            script.src = 'https://cdn.jsdelivr.net/npm/eruda';
            script.async = true;
            script.crossOrigin = 'anonymous';
            script.onload = function () {
                try {
                    if (typeof window.eruda !== 'undefined' && typeof window.eruda.init === 'function') {
                        window.eruda.init();
                        // Auto-open on first load so users immediately see
                        // the panel — they can close it with the floating
                        // button if they want it dismissed.
                        try { window.eruda.show(); } catch (e) {}
                    }
                } catch (e) {
                    try { console.error('[foe] eruda init failed:', e); } catch (_) {}
                }
            };
            script.onerror = function () {
                try {
                    console.warn(
                        '[foe] Failed to load Eruda from CDN. Check network ' +
                        'access to cdn.jsdelivr.net or disable FOE_ERUDA.'
                    );
                } catch (e) {}
            };
            (document.body || document.documentElement).appendChild(script);
        } catch (e) {
            try { console.error('[foe] eruda injection error:', e); } catch (_) {}
        }
    };

    if (document.body) {
        inject();
    } else if (typeof document.addEventListener === 'function') {
        document.addEventListener('DOMContentLoaded', inject, { once: true });
    }
})();
