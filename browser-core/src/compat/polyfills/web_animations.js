// Element.prototype.animate shim that applies the final keyframe state
// immediately. This is not a real animation — Servo doesn't drive the
// timing — but it leaves the element at the visually-correct end state,
// which is good enough for hover/reveal patterns that just want
// `el.animate(..., {fill: 'forwards'}).finished.then(...)` to work.
if (typeof Element !== 'undefined' && !Element.prototype.animate) {
    Element.prototype.animate = function (keyframes, _options) {
        try {
            if (Array.isArray(keyframes) && keyframes.length > 0) {
                // keyframe array → apply last frame's properties
                const last = keyframes[keyframes.length - 1];
                for (const prop in last) {
                    if (prop === 'offset' || prop === 'easing' || prop === 'composite') continue;
                    try { this.style[prop] = last[prop]; } catch (e) {}
                }
            } else if (keyframes && typeof keyframes === 'object') {
                // keyframe-by-property object → apply last value of each
                for (const prop in keyframes) {
                    const values = keyframes[prop];
                    const final = Array.isArray(values) ? values[values.length - 1] : values;
                    try { this.style[prop] = final; } catch (e) {}
                }
            }
        } catch (e) {}

        const finished = Promise.resolve();
        return {
            play() {}, pause() {}, reverse() {}, cancel() {}, finish() {},
            persist() {}, commitStyles() {}, updatePlaybackRate() {},
            addEventListener() {}, removeEventListener() {}, dispatchEvent() { return true; },
            id: '', playState: 'finished', playbackRate: 1, startTime: 0, currentTime: 0,
            timeline: null, effect: null,
            finished: finished, ready: finished,
            onfinish: null, oncancel: null, onremove: null
        };
    };
}

// Document.getAnimations / Element.getAnimations — return empty list so
// callers iterating animations don't crash.
if (typeof Document !== 'undefined' && !Document.prototype.getAnimations) {
    Document.prototype.getAnimations = function () { return []; };
}
if (typeof Element !== 'undefined' && !Element.prototype.getAnimations) {
    Element.prototype.getAnimations = function () { return []; };
}
