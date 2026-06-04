// URL.canParse() / URL.parse() polyfill.
//
// `URL.canParse(input, base)` (Chrome 120 / Firefox 115, 2023) is the cheap
// validity check routers and form-validation libraries reach for instead of
// the throw-and-catch dance around `new URL()`. The companion
// `URL.parse()` (Chrome 126, 2024) returns null instead of throwing. Both
// are recent enough that the pinned Servo build is missing them, and code
// that calls `URL.canParse(...)` at module scope dies on the undefined
// static.
//
// The fallback just funnels through the `new URL()` the engine already has,
// catching the throw — same observable result, no new parsing logic.
if (typeof URL === 'function') {
    if (typeof URL.canParse !== 'function') {
        try {
            URL.canParse = function canParse(url, base) {
                try {
                    // `new URL(url)` vs `new URL(url, base)` — passing an
                    // explicit `undefined` base is not the same as omitting
                    // it, so branch on argument count.
                    if (arguments.length > 1) { new URL(url, base); }
                    else { new URL(url); }
                    return true;
                } catch (e) {
                    return false;
                }
            };
        } catch (e) { /* non-writable static — skip */ }
    }

    if (typeof URL.parse !== 'function') {
        try {
            URL.parse = function parse(url, base) {
                try {
                    return arguments.length > 1 ? new URL(url, base) : new URL(url);
                } catch (e) {
                    return null;
                }
            };
        } catch (e) { /* non-writable static — skip */ }
    }
}
