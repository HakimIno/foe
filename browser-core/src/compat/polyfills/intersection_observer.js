// IntersectionObserver shim that actually computes intersections.
//
// The original stub here just marked every observed element as "fully
// visible". That works for the simplest pattern ("load this image once it
// scrolls into view") but breaks every other use case:
//   - "lazy hydrate components on view" — hydrates every component at once
//   - "infinite scroll trigger" — fires immediately, can loop
//   - "track which section is visible" — every section reports visible
//
// This implementation:
//   - computes intersection between target.getBoundingClientRect() and the
//     root (or viewport) on each tick, including rootMargin expansion
//   - listens to scroll/resize via capture-phase passive listeners so it
//     catches every scroll container without enumerating ancestors
//   - batches recomputation through requestAnimationFrame to avoid running
//     callbacks per scroll event (libraries often observe hundreds of
//     elements at once)
//   - fires the initial entry once after observe() so existing code paths
//     that wait for the first callback unblock immediately
//   - only fires subsequent entries when a threshold is actually crossed
//     or isIntersecting flips
if (typeof IntersectionObserver === 'undefined') {
    const ACTIVE = new Set();
    let rafScheduled = false;

    const schedule = function () {
        if (rafScheduled || ACTIVE.size === 0) return;
        rafScheduled = true;
        const run = function () {
            rafScheduled = false;
            ACTIVE.forEach(function (o) { o._check(); });
        };
        if (typeof requestAnimationFrame === 'function') {
            requestAnimationFrame(run);
        } else {
            setTimeout(run, 16);
        }
    };

    if (typeof window !== 'undefined' && window.addEventListener) {
        const opts = { passive: true, capture: true };
        try { window.addEventListener('scroll', schedule, opts); } catch (e) {}
        try { window.addEventListener('resize', schedule, opts); } catch (e) {}
    }

    // Parse a rootMargin string like "10px 20px" into [top, right, bottom,
    // left]. Percentages aren't supported (would need root size to resolve
    // and most rules use px); they fall back to 0.
    const parseMargin = function (margin) {
        if (!margin) return [0, 0, 0, 0];
        const parts = String(margin).trim().split(/\s+/).map(function (p) {
            const n = parseFloat(p);
            return isNaN(n) ? 0 : n;
        });
        if (parts.length === 1) return [parts[0], parts[0], parts[0], parts[0]];
        if (parts.length === 2) return [parts[0], parts[1], parts[0], parts[1]];
        if (parts.length === 3) return [parts[0], parts[1], parts[2], parts[1]];
        return parts.slice(0, 4);
    };

    const getRootRect = function (root) {
        if (root && typeof root.getBoundingClientRect === 'function') {
            return root.getBoundingClientRect();
        }
        const w = (typeof window !== 'undefined' && window.innerWidth) || 0;
        const h = (typeof window !== 'undefined' && window.innerHeight) || 0;
        return { top: 0, left: 0, right: w, bottom: h, width: w, height: h };
    };

    globalThis.IntersectionObserver = class IntersectionObserver {
        constructor(callback, options) {
            options = options || {};
            this._callback = callback;
            this._root = options.root || null;
            this._rootMargin = options.rootMargin || '0px';
            this._margin = parseMargin(this._rootMargin);
            const t = options.threshold;
            this._thresholds = Array.isArray(t)
                ? t.slice().sort(function (a, b) { return a - b; })
                : (typeof t === 'number' ? [t] : [0]);
            // target → { lastRatio: number, lastIntersecting: bool, firstFire: bool }
            this._targets = new Map();
        }

        observe(target) {
            if (!target || this._targets.has(target)) return;
            this._targets.set(target, { lastRatio: -1, lastIntersecting: false, firstFire: true });
            ACTIVE.add(this);
            schedule();
        }

        unobserve(target) {
            this._targets.delete(target);
            if (this._targets.size === 0) ACTIVE.delete(this);
        }

        disconnect() {
            this._targets.clear();
            ACTIVE.delete(this);
        }

        takeRecords() { return []; }

        get root() { return this._root; }
        get rootMargin() { return this._rootMargin; }
        get thresholds() { return this._thresholds.slice(); }

        _check() {
            const rootRect = getRootRect(this._root);
            const m = this._margin;
            // Expand the root by rootMargin — spec semantics are "this many
            // pixels outside the root counts as intersecting".
            const expanded = {
                top: rootRect.top - m[0],
                right: rootRect.right + m[1],
                bottom: rootRect.bottom + m[2],
                left: rootRect.left - m[3]
            };

            const entries = [];
            const now = (typeof performance !== 'undefined' && performance.now)
                ? performance.now() : Date.now();
            const self = this;

            this._targets.forEach(function (state, target) {
                if (!target || typeof target.getBoundingClientRect !== 'function') return;
                if (target.isConnected === false) return;
                const tr = target.getBoundingClientRect();

                const interTop = Math.max(tr.top, expanded.top);
                const interBottom = Math.min(tr.bottom, expanded.bottom);
                const interLeft = Math.max(tr.left, expanded.left);
                const interRight = Math.min(tr.right, expanded.right);

                let ratio = 0;
                let isIntersecting = false;
                let interRect = { top: 0, left: 0, right: 0, bottom: 0, width: 0, height: 0, x: 0, y: 0 };

                if (interBottom > interTop && interRight > interLeft) {
                    const iw = interRight - interLeft;
                    const ih = interBottom - interTop;
                    const ia = iw * ih;
                    const ta = (tr.right - tr.left) * (tr.bottom - tr.top);
                    ratio = ta > 0 ? ia / ta : 0;
                    isIntersecting = ratio > 0;
                    interRect = {
                        top: interTop, left: interLeft, right: interRight, bottom: interBottom,
                        width: iw, height: ih, x: interLeft, y: interTop
                    };
                }

                // Decide whether this entry needs to fire:
                //   - first observe call always fires once (spec behavior)
                //   - any threshold crossing in either direction fires
                //   - isIntersecting flipping fires even without a threshold
                let shouldFire = state.firstFire;
                if (!shouldFire) {
                    if (isIntersecting !== state.lastIntersecting) {
                        shouldFire = true;
                    } else {
                        for (let i = 0; i < self._thresholds.length; i++) {
                            const th = self._thresholds[i];
                            if ((state.lastRatio < th && ratio >= th) ||
                                (state.lastRatio >= th && ratio < th)) {
                                shouldFire = true;
                                break;
                            }
                        }
                    }
                }

                if (shouldFire) {
                    state.firstFire = false;
                    state.lastRatio = ratio;
                    state.lastIntersecting = isIntersecting;
                    entries.push({
                        target: target,
                        isIntersecting: isIntersecting,
                        intersectionRatio: ratio,
                        boundingClientRect: tr,
                        intersectionRect: interRect,
                        rootBounds: self._root ? rootRect : null,
                        time: now
                    });
                }
            });

            if (entries.length > 0) {
                try { this._callback(entries, this); } catch (e) { /* swallow */ }
            }
        }
    };
}
