// queueMicrotask polyfill.
//
// React's internal scheduler, MobX reactions, and promise-based libraries
// call queueMicrotask directly. Servo has it in recent builds but older
// versions don't, and feature detection at module init means the call
// either works or the entire library throws.
//
// Fallback uses Promise.resolve().then() which delivers the callback in
// the same microtask phase as native queueMicrotask. Rethrows are routed
// through setTimeout so they surface as uncaught exceptions instead of
// being swallowed by the promise chain — matches spec behavior.
if (typeof queueMicrotask === 'undefined') {
    globalThis.queueMicrotask = function (cb) {
        Promise.resolve().then(function () {
            try { cb(); } catch (e) {
                setTimeout(function () { throw e; }, 0);
            }
        });
    };
}
