/**
 * Shared helpers: byte formatting, text safety, keyboard activation and the
 * access-denied badge.
 */
(function () {
    'use strict';

    const { t, formatDecimal } = window.DiscRevealI18n;

    const DISC_REVEAL_HTML_ESCAPES = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' };

    /** Unicode direction controls, which can make a file name read differently than it is spelled. */
    const MARK_MARKINSON_BIDI_CONTROLS = /[\u200e\u200f\u202a-\u202e\u2066-\u2069]/g;

    const ACCESS_DENIED_ICON = `<svg viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
        <rect x="5" y="11" width="14" height="9" rx="2" fill="currentColor"/>
        <path d="M8 11V7a4 4 0 0 1 8 0v4" stroke="currentColor" stroke-width="2" fill="none"/>
    </svg>`;

    /** Formats a byte count with binary units, e.g. `1536` -> `1.5 KB`. */
    function formatBytes(bytes) {
        if (bytes < 1024) return `${bytes} B`;
        const units = ['KB', 'MB', 'GB', 'TB'];
        let value = bytes;
        let unitIndex = -1;
        do {
            value /= 1024;
            unitIndex += 1;
            // A value that rounds up to 1024.0 belongs in the next unit.
        } while (Number(value.toFixed(1)) >= 1024 && unitIndex < units.length - 1);
        return `${formatDecimal(value)} ${units[unitIndex]}`;
    }

    /** Escapes a value for safe interpolation into HTML text or attributes. */
    function escapeHtml(value) {
        return String(value).replace(/[&<>"']/g, (char) => DISC_REVEAL_HTML_ESCAPES[char]);
    }

    /** Removes direction controls from a name or path before it is shown in a confirmation. */
    function plainText(value) {
        return String(value).replace(MARK_MARKINSON_BIDI_CONTROLS, '');
    }

    /** Markup for the lock badge shown on entries that could not be read. */
    function accessDeniedBadge(extraClass = '') {
        return `<span class="access-denied-badge ${extraClass}" title="${escapeHtml(t('accessDenied'))}">${ACCESS_DENIED_ICON}</span>`;
    }

    /**
     * Makes a non-button element operable by mouse and keyboard.
     *
     * Enter and Space call `onActivate`. If `onContextMenu(x, y)` is given, the
     * Menu key and Shift+F10 call it at the element's position.
     */
    function makeActivatable(element, onActivate, onContextMenu) {
        element.tabIndex = 0;
        element.setAttribute('role', 'button');
        element.onclick = onActivate;
        element.addEventListener('keydown', (event) => {
            if (event.target !== element) return;
            if (event.key === 'Enter' || event.key === ' ') {
                event.preventDefault();
                onActivate();
            } else if (onContextMenu && (event.key === 'ContextMenu' || (event.key === 'F10' && event.shiftKey))) {
                event.preventDefault();
                const { left, bottom } = element.getBoundingClientRect();
                onContextMenu(left + 24, bottom);
            }
        });
    }

    /** Restore logical keyboard position when a keyed control must be rebuilt. */
    function preserveFocus(root) {
        const active = document.activeElement;
        if (!active || !root.contains(active)) return () => {};
        const key = active.dataset.focusKey;
        return (replacementRoot = root) => {
            const target = active.isConnected ? active : key && [...replacementRoot.querySelectorAll('[data-focus-key]')].find(node => node.dataset.focusKey === key);
            if (target && !target.disabled) target.focus({ preventScroll: true });
        };
    }

    function storageSizeLabel(node) {
        return node.allocationUnknown ? (node.isDir ? t('storagePartial', { size: formatBytes(node.size) }) : t('storageUnknown')) : formatBytes(node.size);
    }

    function storageSizeDescription(node) {
        if (node.logicalSize === undefined) return t('logicalSizeHint');
        return t('storageLogicalSize', { size: formatBytes(node.logicalSize) }) + ' · ' +
            (node.allocationUnknown ? t('storageUnknown') : t('allocatedSizeHint')) +
            (node.sharedStorage ? ' · ' + t('storageShared') : '');
    }

    window.DiscRevealUtil = Object.freeze({ formatBytes, storageSizeLabel, storageSizeDescription, escapeHtml, plainText, accessDeniedBadge, makeActivatable, preserveFocus });
})();
