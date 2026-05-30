// Pointer Events alias for engines without native PointerEvent support.
// Many modern UIs (drag-and-drop libraries, charting libs, design systems)
// assume PointerEvent exists and won't fall back to MouseEvent. We expose
// PointerEvent as an alias of MouseEvent, plus translate pointer event
// names in addEventListener so library code paths work.
//
// CRITICAL: every patch below must be gated on `typeof PointerEvent ===
// 'undefined'`. If we rewrite addEventListener even when native
// PointerEvent exists, real pointerdown/pointermove listeners get bound to
// mousedown/mousemove instead — silently breaking drag-and-drop, charting,
// and any code that reads PointerEvent-only fields like pointerId/pressure.
if (typeof PointerEvent === 'undefined' && typeof MouseEvent !== 'undefined') {
    globalThis.PointerEvent = MouseEvent;

    // Mapping of pointer event names → mouse event names. When a script adds a
    // listener for "pointerdown", we transparently bind it as "mousedown".
    const POINTER_TO_MOUSE = {
        pointerdown: 'mousedown',
        pointerup: 'mouseup',
        pointermove: 'mousemove',
        pointerenter: 'mouseenter',
        pointerleave: 'mouseleave',
        pointerover: 'mouseover',
        pointerout: 'mouseout',
        pointercancel: 'mouseup'
    };

    if (typeof EventTarget !== 'undefined' && EventTarget.prototype.addEventListener) {
        const origAdd = EventTarget.prototype.addEventListener;
        const origRemove = EventTarget.prototype.removeEventListener;
        EventTarget.prototype.addEventListener = function (type, listener, opts) {
            const mapped = POINTER_TO_MOUSE[type];
            if (mapped) {
                return origAdd.call(this, mapped, listener, opts);
            }
            return origAdd.call(this, type, listener, opts);
        };
        EventTarget.prototype.removeEventListener = function (type, listener, opts) {
            const mapped = POINTER_TO_MOUSE[type];
            if (mapped) {
                return origRemove.call(this, mapped, listener, opts);
            }
            return origRemove.call(this, type, listener, opts);
        };
    }
}
