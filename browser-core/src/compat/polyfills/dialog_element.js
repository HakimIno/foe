// HTMLDialogElement show/showModal/close stubs.
//
// Modern UI libraries (Radix UI, headless-ui, ARIA dialogs) use <dialog>
// for modal panels because it gives top-layer rendering and focus
// trapping for free. Servo's HTMLDialogElement may be missing the
// imperative methods — when that happens, dialogs render but never open
// because nothing toggles the [open] attribute.
//
// We pair this with the baseline.css rules `dialog:not([open]) { display:
// none; }` and `dialog[open] { display: block; }` so toggling the
// attribute is enough to make the dialog visible. True modal semantics
// (top-layer, ::backdrop, focus trap, ESC-to-close) aren't reproducible
// from userland JS — sites that depend on those still won't trap focus,
// but at least their open/close calls don't silently fail.
if (typeof HTMLDialogElement !== 'undefined') {
    const proto = HTMLDialogElement.prototype;

    if (typeof proto.show !== 'function') {
        proto.show = function () {
            this.setAttribute('open', '');
        };
    }

    if (typeof proto.showModal !== 'function') {
        proto.showModal = function () {
            this.setAttribute('open', '');
            // Real showModal also pushes onto the top layer. We can't do
            // that from JS — best-effort: bump z-index so it overlays
            // typical page content. Sites that set their own z-index
            // override this, which is fine.
            try {
                if (!this.style.zIndex) this.style.zIndex = '2147483647';
                if (!this.style.position) this.style.position = 'fixed';
            } catch (e) {}
        };
    }

    if (typeof proto.close !== 'function') {
        proto.close = function (returnValue) {
            this.removeAttribute('open');
            if (returnValue !== undefined) {
                try { this.returnValue = String(returnValue); } catch (e) {}
            }
            try { this.dispatchEvent(new Event('close')); } catch (e) {}
        };
    }
}
