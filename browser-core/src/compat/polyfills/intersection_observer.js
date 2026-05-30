// Minimal IntersectionObserver shim.
// Pretends every observed element is fully visible. That's enough for the
// common pattern of "load this image / hydrate this component once it
// scrolls into view" — Servo lacks the real API, so without this shim
// lazy-loaded content never appears.
if (typeof IntersectionObserver === 'undefined') {
    globalThis.IntersectionObserver = class IntersectionObserver {
        constructor(cb, opts) {
            this._cb = cb;
            this._opts = opts || {};
            this._targets = new Set();
        }
        observe(target) {
            if (!target) return;
            this._targets.add(target);
            var self = this;
            Promise.resolve().then(function () {
                var rect = (target && target.getBoundingClientRect)
                    ? target.getBoundingClientRect()
                    : { top: 0, left: 0, right: 0, bottom: 0, width: 0, height: 0, x: 0, y: 0 };
                var entry = {
                    target: target,
                    isIntersecting: true,
                    intersectionRatio: 1,
                    boundingClientRect: rect,
                    intersectionRect: rect,
                    rootBounds: null,
                    time: (typeof performance !== 'undefined' && performance.now) ? performance.now() : Date.now()
                };
                try { self._cb([entry], self); } catch (e) {}
            });
        }
        unobserve(target) { this._targets.delete(target); }
        disconnect() { this._targets.clear(); }
        takeRecords() { return []; }
        get root() { return this._opts.root || null; }
        get rootMargin() { return this._opts.rootMargin || '0px'; }
        get thresholds() {
            var t = this._opts.threshold;
            return Array.isArray(t) ? t : (typeof t === 'number' ? [t] : [0]);
        }
    };
}
