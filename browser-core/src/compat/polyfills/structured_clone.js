// structuredClone polyfill.
//
// Used directly by Redux Toolkit (immer cloning), Zustand persist
// middleware, several state libraries, and React 19's transition state.
// Missing API → JSON.parse(JSON.stringify(x)) fallback used by older code
// loses Map/Set/Date and crashes on circular refs.
//
// Our recursive walker handles Date, RegExp, Map, Set, ArrayBuffer,
// TypedArray, Array, plain objects, and circular references via a WeakMap.
// Functions, DOM nodes, and other host objects are returned as-is or
// throw — matching native structuredClone behavior closely enough that
// libraries that depend on it work.
if (typeof structuredClone === 'undefined') {
    const _clone = function (v, seen) {
        if (v === null || typeof v !== 'object') return v;
        if (seen.has(v)) return seen.get(v);

        // Date
        if (v instanceof Date) return new Date(v.getTime());

        // RegExp
        if (v instanceof RegExp) {
            const r = new RegExp(v.source, v.flags);
            r.lastIndex = v.lastIndex;
            return r;
        }

        // Map
        if (typeof Map !== 'undefined' && v instanceof Map) {
            const m = new Map();
            seen.set(v, m);
            v.forEach(function (val, key) {
                m.set(_clone(key, seen), _clone(val, seen));
            });
            return m;
        }

        // Set
        if (typeof Set !== 'undefined' && v instanceof Set) {
            const s = new Set();
            seen.set(v, s);
            v.forEach(function (val) { s.add(_clone(val, seen)); });
            return s;
        }

        // ArrayBuffer
        if (typeof ArrayBuffer !== 'undefined' && v instanceof ArrayBuffer) {
            return v.slice(0);
        }

        // TypedArray / DataView — view of an ArrayBuffer; copy the bytes
        if (typeof ArrayBuffer !== 'undefined' && ArrayBuffer.isView && ArrayBuffer.isView(v)) {
            const Ctor = v.constructor;
            const out = new Ctor(v.length !== undefined ? v.length : v.byteLength);
            if (out.set) out.set(v);
            return out;
        }

        // Array — preserves length and sparse holes via index loop
        if (Array.isArray(v)) {
            const a = new Array(v.length);
            seen.set(v, a);
            for (let i = 0; i < v.length; i++) {
                if (i in v) a[i] = _clone(v[i], seen);
            }
            return a;
        }

        // Plain object — preserve prototype chain so class instances
        // (Error, custom classes) still report the right type after clone
        const o = Object.create(Object.getPrototypeOf(v));
        seen.set(v, o);
        const keys = Object.keys(v);
        for (let i = 0; i < keys.length; i++) {
            o[keys[i]] = _clone(v[keys[i]], seen);
        }
        return o;
    };

    globalThis.structuredClone = function (value, _opts) {
        return _clone(value, new WeakMap());
    };
}
