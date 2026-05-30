// ResizeObserver shim with real change detection.
//
// The original stub fired one entry at observe() time and never again. That
// unblocks "init once" patterns but breaks anything that depends on
// re-measuring after layout shifts: responsive charts, virtualized lists,
// auto-grow textareas.
//
// This polyfill polls getBoundingClientRect() of each observed target on
// requestAnimationFrame and fires the callback whenever width or height
// actually changed since the last tick. The RAF loop self-suspends when
// the last target is unobserved so idle pages don't keep waking the main
// thread. Cost is O(targets) per frame; targets are typically <100 even on
// component-heavy pages, far cheaper than the layout work the observer is
// reacting to.
if (typeof ResizeObserver === 'undefined') {
    const ACTIVE = new Set();
    let rafScheduled = false;

    const scheduleTick = function () {
        if (rafScheduled) return;
        rafScheduled = true;
        const tick = function () {
            rafScheduled = false;
            let anyTargets = false;
            ACTIVE.forEach(function (o) {
                if (o._targets.size > 0) {
                    o._poll();
                    anyTargets = true;
                }
            });
            if (anyTargets) scheduleTick();
        };
        if (typeof requestAnimationFrame === 'function') {
            requestAnimationFrame(tick);
        } else {
            setTimeout(tick, 16);
        }
    };

    globalThis.ResizeObserver = class ResizeObserver {
        constructor(callback) {
            this._callback = callback;
            // target → { w, h }; -1 sentinel so the first poll always fires
            this._targets = new Map();
        }

        observe(target) {
            if (!target || this._targets.has(target)) return;
            this._targets.set(target, { w: -1, h: -1 });
            ACTIVE.add(this);
            scheduleTick();
        }

        unobserve(target) {
            this._targets.delete(target);
            if (this._targets.size === 0) ACTIVE.delete(this);
        }

        disconnect() {
            this._targets.clear();
            ACTIVE.delete(this);
        }

        _poll() {
            const entries = [];
            this._targets.forEach(function (state, target) {
                if (!target || typeof target.getBoundingClientRect !== 'function') return;
                if (target.isConnected === false) return;
                const rect = target.getBoundingClientRect();
                if (rect.width !== state.w || rect.height !== state.h) {
                    state.w = rect.width;
                    state.h = rect.height;
                    const sizeEntry = { inlineSize: rect.width, blockSize: rect.height };
                    entries.push({
                        target: target,
                        contentRect: rect,
                        borderBoxSize: [sizeEntry],
                        contentBoxSize: [sizeEntry],
                        devicePixelContentBoxSize: [sizeEntry]
                    });
                }
            });
            if (entries.length > 0) {
                try { this._callback(entries, this); } catch (e) { /* swallow */ }
            }
        }
    };
}
