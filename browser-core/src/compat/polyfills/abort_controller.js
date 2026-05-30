// AbortController / AbortSignal polyfill.
//
// Modern fetch-based libraries (Apollo Client, SWR, TanStack Query, axios
// recent versions) pass an AbortSignal into fetch() and read fields like
// signal.aborted / signal.reason / signal.throwIfAborted(). When the
// runtime lacks AbortController these libraries throw at module init
// because they reference the constructor at the top level.
//
// We can't make Servo's native fetch honor abort — that would require
// wiring into the network stack. But we can satisfy the surface area so
// the libraries load, and so application code that calls signal.abort()
// reaches its own abort handlers via the 'abort' event.
if (typeof AbortSignal === 'undefined') {
    const makeAbortError = function (reason) {
        if (reason !== undefined) return reason;
        try { return new DOMException('Aborted', 'AbortError'); }
        catch (e) {
            const err = new Error('Aborted');
            err.name = 'AbortError';
            return err;
        }
    };

    // Base class — extend EventTarget when available so addEventListener
    // works for the 'abort' event. EventTarget should be present in Servo;
    // the typeof guard is defensive.
    const Base = (typeof EventTarget !== 'undefined') ? EventTarget : Object;

    globalThis.AbortSignal = class AbortSignal extends Base {
        constructor() {
            super();
            this.aborted = false;
            this.reason = undefined;
            this.onabort = null;
        }

        throwIfAborted() {
            if (this.aborted) throw this.reason;
        }

        static abort(reason) {
            const s = new AbortSignal();
            s.aborted = true;
            s.reason = makeAbortError(reason);
            return s;
        }

        static timeout(ms) {
            const s = new AbortSignal();
            setTimeout(function () {
                if (s.aborted) return;
                s.aborted = true;
                try {
                    s.reason = new DOMException('Timeout', 'TimeoutError');
                } catch (e) {
                    const err = new Error('Timeout');
                    err.name = 'TimeoutError';
                    s.reason = err;
                }
                if (typeof s.onabort === 'function') {
                    try { s.onabort(); } catch (e) {}
                }
                try { s.dispatchEvent(new Event('abort')); } catch (e) {}
            }, ms);
            return s;
        }

        // AbortSignal.any([sig1, sig2, ...]) — abort the combined signal
        // when any input signal aborts. Used by some recent libs.
        static any(signals) {
            const combined = new AbortSignal();
            const onAny = function (src) {
                if (combined.aborted) return;
                combined.aborted = true;
                combined.reason = src.reason;
                if (typeof combined.onabort === 'function') {
                    try { combined.onabort(); } catch (e) {}
                }
                try { combined.dispatchEvent(new Event('abort')); } catch (e) {}
            };
            for (let i = 0; i < signals.length; i++) {
                const sig = signals[i];
                if (!sig) continue;
                if (sig.aborted) {
                    onAny(sig);
                    break;
                }
                try {
                    sig.addEventListener('abort', function () { onAny(sig); }, { once: true });
                } catch (e) {}
            }
            return combined;
        }
    };
}

if (typeof AbortController === 'undefined') {
    globalThis.AbortController = class AbortController {
        constructor() {
            this.signal = new AbortSignal();
        }

        abort(reason) {
            const s = this.signal;
            if (s.aborted) return;
            s.aborted = true;
            s.reason = (function (r) {
                if (r !== undefined) return r;
                try { return new DOMException('Aborted', 'AbortError'); }
                catch (e) {
                    const err = new Error('Aborted');
                    err.name = 'AbortError';
                    return err;
                }
            })(reason);
            if (typeof s.onabort === 'function') {
                try { s.onabort(); } catch (e) {}
            }
            try { s.dispatchEvent(new Event('abort')); } catch (e) {}
        }
    };
}
