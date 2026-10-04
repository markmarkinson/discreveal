// Minimal connected DOM and focus model, not a browser/layout substitute.
function createDocument() {
    const document = { activeElement: null };
    class Element {
        constructor(tag) { this.tag = tag; this.children = []; this.dataset = {}; this.style = {}; this.className = ''; this.attributes = {}; this.listeners = {}; this.textContent = ''; this.connected = false; this.ownerDocument = document; }
        get isConnected() { return this.connected; }
        get parentElement() { return this.parent; }
        get classList() { return { contains: key => this.className.split(' ').includes(key), add: key => this.classList.toggle(key, true), remove: key => this.classList.toggle(key, false), toggle: (key, force) => { const values = new Set(this.className.split(' ').filter(Boolean)); if (force ?? !values.has(key)) values.add(key); else values.delete(key); this.className = [...values].join(' '); } }; }
        connect(value) { this.connected = value; for (const child of this.children) child.connect(value); if (!value && document.activeElement === this) document.activeElement = document.body; }
        append(...nodes) { nodes.forEach(node => this.appendChild(node)); }
        appendChild(node) { if (node.parent) node.parent.children.splice(node.parent.children.indexOf(node), 1); node.parent = this; this.children.push(node); node.connect(this.connected); return node; }
        replaceChildren(...nodes) { for (const child of this.children) { child.connect(false); child.parent = null; } this.children = []; this.append(...nodes); }
        insertBefore(node, before) { this.appendChild(node); const target = this.children.indexOf(before); if (target >= 0) { this.children.pop(); this.children.splice(target, 0, node); } }
        remove() { this.connect(false); if (this.parent) this.parent.children.splice(this.parent.children.indexOf(this), 1); this.parent = null; }
        replaceWith(node) { const parent = this.parent, index = parent.children.indexOf(this); this.connect(false); this.parent = null; node.parent = parent; node.connect(parent.connected); parent.children[index] = node; }
        setAttribute(key, value) { this.attributes[key] = String(value); }
        getAttribute(key) { return this.attributes[key]; }
        removeAttribute(key) { delete this.attributes[key]; }
        addEventListener(type, action) { (this.listeners[type] ||= []).push(action); }
        focus() { if (this.connected && !this.disabled) document.activeElement = this; }
        contains(node) { return this === node || this.children.some(child => child.contains(node)); }
        matches(selector) { if (selector === '[data-focus-key]') return Boolean(this.dataset.focusKey); if (selector.startsWith('.')) return selector.slice(1).split('.').every(key => this.classList.contains(key)); return this.tag === selector; }
        querySelectorAll(selector) { return this.children.flatMap(child => [...(selector.split(',').some(part => child.matches(part.trim())) ? [child] : []), ...child.querySelectorAll(selector)]); }
        querySelector(selector) { return this.querySelectorAll(selector)[0] || null; }
        getBoundingClientRect() { return { left: 0, top: 0, bottom: 20, width: 900, height: 600 }; }
    }
    document.createElement = tag => new Element(tag);
    document.createElementNS = (_, tag) => new Element(tag);
    document.body = new Element('body'); document.body.connect(true); document.activeElement = document.body;
    document.documentElement = { lang: 'en' };
    document.querySelectorAll = selector => document.body.querySelectorAll(selector);
    return document;
}
module.exports = { createDocument };
