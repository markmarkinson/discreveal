/**
 * Context menu styled like the app's dropdowns, replacing the native menu.
 * Fully operable by keyboard: arrow keys move, Enter selects, Escape closes.
 */
(function () {
    'use strict';

    const VIEWPORT_MARGIN = 8;

    let discRevealActiveMenu = null;
    let previouslyFocused = null;

    function closeMenu({ restoreFocus = false } = {}) {
        discRevealActiveMenu?.remove();
        discRevealActiveMenu = null;
        document.removeEventListener('click', onOutsideEvent);
        document.removeEventListener('contextmenu', onOutsideEvent);
        if (restoreFocus && previouslyFocused?.isConnected) previouslyFocused.focus();
        previouslyFocused = null;
    }

    function onOutsideEvent() {
        closeMenu();
    }

    function onMenuKeydown(event) {
        const items = [...discRevealActiveMenu.querySelectorAll('.context-menu-item')];
        const index = items.indexOf(document.activeElement);
        if (event.key === 'Escape') {
            event.preventDefault();
            closeMenu({ restoreFocus: true });
        } else if (event.key === 'ArrowDown') {
            event.preventDefault();
            items[(index + 1) % items.length].focus();
        } else if (event.key === 'ArrowUp') {
            event.preventDefault();
            items[(index - 1 + items.length) % items.length].focus();
        } else if (event.key === 'Tab') {
            event.preventDefault();
        }
    }

    /**
     * Shows the menu at viewport position (`x`, `y`). Each item is either
     * `{ label, danger?, onSelect }` or `{ divider: true }`.
     */
    function show(x, y, items) {
        closeMenu();
        previouslyFocused = document.activeElement;

        const menu = document.createElement('div');
        menu.className = 'context-menu';
        menu.setAttribute('role', 'menu');

        for (const item of items) {
            if (item.divider) {
                const divider = document.createElement('div');
                divider.className = 'context-menu-divider';
                divider.setAttribute('role', 'separator');
                menu.appendChild(divider);
                continue;
            }

            const button = document.createElement('button');
            button.type = 'button';
            button.setAttribute('role', 'menuitem');
            button.className = `context-menu-item${item.danger ? ' danger' : ''}`;
            button.textContent = item.label;
            button.onclick = (event) => {
                event.stopPropagation();
                closeMenu({ restoreFocus: true });
                item.onSelect();
            };
            menu.appendChild(button);
        }

        menu.addEventListener('keydown', onMenuKeydown);
        document.body.appendChild(menu);

        // Keep the menu fully inside the viewport.
        const { width, height } = menu.getBoundingClientRect();
        const left = Math.min(x, window.innerWidth - width - VIEWPORT_MARGIN);
        const top = Math.min(y, window.innerHeight - height - VIEWPORT_MARGIN);
        menu.style.left = `${Math.max(VIEWPORT_MARGIN, left)}px`;
        menu.style.top = `${Math.max(VIEWPORT_MARGIN, top)}px`;

        discRevealActiveMenu = menu;
        menu.querySelector('.context-menu-item')?.focus();

        // Registered on the next tick so the event that opened the menu does
        // not close it again immediately.
        setTimeout(() => {
            document.addEventListener('click', onOutsideEvent);
            document.addEventListener('contextmenu', onOutsideEvent);
        }, 0);
    }

    window.DiscRevealContextMenu = Object.freeze({ show });
})();
