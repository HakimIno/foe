// Minimal ResizeObserver shim.
// Real ResizeObserver fires whenever an element's content box changes size.
// Servo's missing API → modern UI libraries (Material, Vuetify, headless
// component libs) bail out on init. We fire one entry per observed element
// at registration time so first-render logic completes; subsequent resizes
// won't notify but most "init once" code paths get unblocked.
if (typeof ResizeObserver === 'undefined') {
    globalThis.ResizeObserver = class ResizeObserver {
        constructor(cb) { this._cb = cb; this._targets = new Set(); }
        observe(target) {
            if (!target) return;
            this._targets.add(target);
            var self = this;
            Promise.resolve().then(function () {
                var rect = (target && target.getBoundingClientRect)
                    ? target.getBoundingClientRect()
                    : { top: 0, left: 0, right: 0, bottom: 0, width: 0, height: 0 };
                var sizeEntry = { inlineSize: rect.width, blockSize: rect.height };
                try {
                    self._cb([{
                        target: target,
                        contentRect: rect,
                        borderBoxSize: [sizeEntry],
                        contentBoxSize: [sizeEntry],
                        devicePixelContentBoxSize: [sizeEntry]
                    }], self);
                } catch (e) {}
            });
        }
        unobserve(target) { this._targets.delete(target); }
        disconnect() { this._targets.clear(); }
    };
}
