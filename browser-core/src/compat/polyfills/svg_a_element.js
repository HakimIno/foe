// Stub SVGAElement so `instanceof SVGAElement` checks don't throw
// ReferenceError. We don't implement the actual SVG <a> behavior — the
// element already works for clicks via standard event bubbling.
if (typeof SVGAElement === 'undefined') {
    const base = (typeof SVGGraphicsElement !== 'undefined')
        ? SVGGraphicsElement
        : (typeof SVGElement !== 'undefined' ? SVGElement : Object);
    globalThis.SVGAElement = function SVGAElement() {};
    globalThis.SVGAElement.prototype = Object.create(base.prototype);
}
