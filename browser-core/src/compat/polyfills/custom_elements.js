// Minimal Custom Elements registry shim.
// Real Custom Elements require deep platform integration (upgrade chains
// during HTML parsing, lifecycle callbacks, autonomous vs customized built-
// ins). We can't reproduce that from JS alone, but we can:
//   - keep customElements.define() from throwing
//   - upgrade elements that are already in the DOM when define() runs
//   - upgrade elements added later via a MutationObserver scan
//   - fire connectedCallback when an upgraded element is in the document
// This is enough for many web-component libraries to render their content
// instead of leaving raw <custom-tag> empty in the DOM.

if (typeof customElements === 'undefined' && typeof MutationObserver !== 'undefined') {
    const registry = new Map(); // tag → { ctor, observedAttributes, prototype }
    const upgraded = new WeakSet();

    function upgradeElement(el, def) {
        if (upgraded.has(el)) return;
        upgraded.add(el);
        try {
            Object.setPrototypeOf(el, def.prototype);
            // Call constructor manually — note this is best-effort, real CE
            // semantics call ctor before insertion.
            try { def.ctor.call(el); } catch (e) {}
            if (typeof el.connectedCallback === 'function' && el.isConnected) {
                try { el.connectedCallback(); } catch (e) {}
            }
        } catch (e) {}
    }

    function scan(root) {
        registry.forEach(function (def, tag) {
            const matches = root.querySelectorAll ? root.querySelectorAll(tag) : [];
            for (let i = 0; i < matches.length; i++) upgradeElement(matches[i], def);
            // Also check root itself if it matches.
            if (root.matches && root.matches(tag)) upgradeElement(root, def);
        });
    }

    globalThis.customElements = {
        define: function (tag, ctor, _options) {
            if (registry.has(tag)) return;
            const def = {
                ctor: ctor,
                prototype: ctor.prototype,
                observedAttributes: ctor.observedAttributes || []
            };
            registry.set(tag, def);
            // Upgrade existing DOM elements with this tag.
            if (typeof document !== 'undefined') {
                const matches = document.querySelectorAll(tag);
                for (let i = 0; i < matches.length; i++) upgradeElement(matches[i], def);
            }
        },
        get: function (tag) {
            const def = registry.get(tag);
            return def ? def.ctor : undefined;
        },
        whenDefined: function (tag) {
            return registry.has(tag) ? Promise.resolve(registry.get(tag).ctor) : new Promise(function () {});
        },
        upgrade: function (root) { scan(root); }
    };

    // Watch the DOM for newly-added elements that match registered tags.
    if (typeof document !== 'undefined' && document.body) {
        const mo = new MutationObserver(function (mutations) {
            for (let i = 0; i < mutations.length; i++) {
                const added = mutations[i].addedNodes;
                for (let j = 0; j < added.length; j++) {
                    const node = added[j];
                    if (node && node.nodeType === 1) scan(node);
                }
            }
        });
        try { mo.observe(document.documentElement || document.body, { childList: true, subtree: true }); } catch (e) {}
    }

    // HTMLElement constructor shim — Custom Element constructors do
    // `super()` which delegates to HTMLElement. Without proper platform
    // support, that throws. We can't fix it completely from JS, but at
    // least make HTMLElement callable without throwing for derived ctors
    // (the prototype chain still works because of Object.setPrototypeOf
    // in upgradeElement).
    if (typeof HTMLElement !== 'undefined') {
        const OriginalHTMLElement = HTMLElement;
        try {
            globalThis.HTMLElement = function HTMLElement() {
                return Reflect.construct(OriginalHTMLElement, [], this.constructor || HTMLElement);
            };
            globalThis.HTMLElement.prototype = OriginalHTMLElement.prototype;
        } catch (e) {}
    }
}
