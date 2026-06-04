// crypto.randomUUID() polyfill.
//
// Ubiquitous in modern web code: Sentry/analytics use it for event IDs,
// React libraries for stable keys, form libraries for field IDs, and many
// state managers for instance tags. It shipped in Chrome 92 / Firefox 95
// but older Servo SpiderMonkey builds expose `crypto.getRandomValues`
// without `randomUUID`, so any top-level `crypto.randomUUID()` reference
// throws and takes the whole module down with it.
//
// We build the v4 UUID from `crypto.getRandomValues` (present in Servo) so
// the result is still cryptographically random — not a Math.random() stand
// in. The version (4) and variant (8–b) nibbles are set per RFC 4122.
if (typeof crypto !== 'undefined' &&
    typeof crypto.getRandomValues === 'function' &&
    typeof crypto.randomUUID !== 'function') {
    var _hex = [];
    for (var i = 0; i < 256; i++) {
        _hex[i] = (i + 0x100).toString(16).slice(1);
    }
    try {
        Object.defineProperty(crypto, 'randomUUID', {
            value: function randomUUID() {
                var b = new Uint8Array(16);
                crypto.getRandomValues(b);
                // Version 4 — high nibble of byte 6 is 0x4.
                b[6] = (b[6] & 0x0f) | 0x40;
                // Variant 1 (RFC 4122) — top two bits of byte 8 are 10.
                b[8] = (b[8] & 0x3f) | 0x80;
                return (
                    _hex[b[0]] + _hex[b[1]] + _hex[b[2]] + _hex[b[3]] + '-' +
                    _hex[b[4]] + _hex[b[5]] + '-' +
                    _hex[b[6]] + _hex[b[7]] + '-' +
                    _hex[b[8]] + _hex[b[9]] + '-' +
                    _hex[b[10]] + _hex[b[11]] + _hex[b[12]] +
                    _hex[b[13]] + _hex[b[14]] + _hex[b[15]]
                );
            },
            writable: true,
            configurable: true
        });
    } catch (e) {
        // crypto is frozen/non-configurable on this build — best effort,
        // skip rather than throw at injection time.
    }
}
