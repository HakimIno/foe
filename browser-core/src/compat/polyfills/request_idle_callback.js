// requestIdleCallback / cancelIdleCallback polyfill.
//
// Originally Chrome-only, now in most engines but Servo may lack it.
// Used by analytics, deferred-hydration frameworks (Next.js, Astro), and
// React's scheduler in older builds. Missing API → libraries fall back to
// blocking code paths or crash entirely.
//
// We can't replicate the "idle" semantics — that requires deep main-thread
// scheduling knowledge — so we approximate with setTimeout(1). Real idle
// callbacks fire when the browser has spare time; setTimeout fires after
// the current task drains, which is a reasonable proxy for "soon, but not
// blocking".
if (typeof requestIdleCallback === 'undefined') {
    globalThis.requestIdleCallback = function (cb, opts) {
        const start = Date.now();
        const timeout = opts && typeof opts.timeout === 'number' ? opts.timeout : 0;
        return setTimeout(function () {
            cb({
                didTimeout: timeout > 0 && (Date.now() - start) >= timeout,
                timeRemaining: function () {
                    // Real impl returns up to 50ms. We always claim 50ms
                    // remaining since we can't measure scheduler pressure.
                    return Math.max(0, 50 - (Date.now() - start));
                }
            });
        }, 1);
    };
    globalThis.cancelIdleCallback = function (id) { clearTimeout(id); };
}
