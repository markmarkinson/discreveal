/**
 * Icon-grid view: folders and files as icon cells, files with their system
 * icon for the file type.
 *
 * Icons depend on the extension only, so they are fetched once per extension
 * and cached for the lifetime of the window. A folder with 300 files of five
 * types costs five lookups.
 */
(function () {
    'use strict';

    const { formatBytes, escapeHtml, accessDeniedBadge, makeActivatable } = window.DiscRevealUtil;
    const { t, nodeName } = window.DiscRevealI18n;

    const MAX_STAGGER_CELLS = 40;
    const STAGGER_MS = 8;

    const FOLDER_ICON = `<svg viewBox="0 0 24 24" width="40" height="40" fill="none" xmlns="http://www.w3.org/2000/svg">
        <path d="M3 6.5A1.5 1.5 0 0 1 4.5 5h4.379a1.5 1.5 0 0 1 1.06.44L11.06 6.56a1.5 1.5 0 0 0 1.06.44H19.5A1.5 1.5 0 0 1 21 8.5v9A1.5 1.5 0 0 1 19.5 19h-15A1.5 1.5 0 0 1 3 17.5v-11Z" fill="var(--gold)"/>
    </svg>`;

    const GENERIC_FILE_ICON = `<svg viewBox="0 0 24 24" width="36" height="36" fill="none" xmlns="http://www.w3.org/2000/svg">
        <path d="M6.5 2A1.5 1.5 0 0 0 5 3.5v17A1.5 1.5 0 0 0 6.5 22h11a1.5 1.5 0 0 0 1.5-1.5V8l-6-6H6.5Z" fill="var(--surface-4)" stroke="var(--border-soft-hi)" stroke-width="1"/>
        <path d="M13 2v5a1.5 1.5 0 0 0 1.5 1.5H19" stroke="var(--border-soft-hi)" stroke-width="1" fill="none"/>
    </svg>`;

    /** Extension (with dot, lower case) to icon data URL. */
    const iconCache = new Map();

    function extensionOf(name) {
        const dot = name.lastIndexOf('.');
        return dot > 0 ? name.slice(dot).toLowerCase() : '';
    }

    function scanningDot() {
        return `<span class="scan-dot" title="${escapeHtml(t('stillScanning'))}"></span>`;
    }

    /** Loads icons for the extensions in `files` that are not cached yet. */
    async function loadMissingIcons(files) {
        const samplePathByExtension = new Map();
        for (const file of files) {
            const extension = extensionOf(file.name);
            if (extension && !iconCache.has(extension) && !samplePathByExtension.has(extension)) {
                samplePathByExtension.set(extension, file.path);
            }
        }
        if (samplePathByExtension.size === 0) return;

        const extensions = [...samplePathByExtension.keys()];
        try {
            const results = await window.discreveal.getFileIcons([...samplePathByExtension.values()]);
            results.forEach((result, index) => {
                if (result.dataUrl) iconCache.set(extensions[index], result.dataUrl);
            });
        } catch {
            // Icons are cosmetic; the generic file icon stays in place.
        }
    }

    /**
     * Renders `node.children` into `container` as icon cells.
     *
     * Handlers work as in the list view, including `onExpandOverflow(node)` for
     * the synthetic overflow cell. Placeholder icons render at once and are
     * swapped for the real ones when the lookup resolves. With
     * `animate: false` cells appear at once, which live updates need.
     */
    async function renderGrid(container, node, onItemClick, onItemContextMenu, { animate = true, onExpandOverflow } = {}) {
        const children = node.children || [];
        container.replaceChildren();

        if (children.length === 0) {
            const empty = document.createElement('div');
            empty.className = 'list-empty';
            empty.textContent = t(node.inProgress ? 'folderStillScanning' : 'emptyFolder');
            container.appendChild(empty);
            return;
        }

        const cells = children.map((item, index) => {
            const cell = document.createElement('div');
            cell.className = `grid-cell${item.path ? '' : ' is-tail'}`;
            if (animate) cell.style.transitionDelay = `${Math.min(index, MAX_STAGGER_CELLS) * STAGGER_MS}ms`;

            cell.innerHTML = `
                <div class="grid-cell-icon">
                    <span class="grid-cell-icon-glyph">${item.isDir ? FOLDER_ICON : GENERIC_FILE_ICON}</span>
                    ${item.accessDenied ? accessDeniedBadge('grid-cell-badge') : ''}
                </div>
                <div class="grid-cell-name" title="${escapeHtml(nodeName(item))}">${escapeHtml(nodeName(item))}${item.inProgress ? scanningDot() : ''}</div>
                <div class="grid-cell-size" title="${escapeHtml(window.DiscRevealUtil.storageSizeDescription(item))}">${window.DiscRevealUtil.storageSizeLabel(item)}</div>
            `;

            if (item.path) {
                makeActivatable(
                    cell,
                    () => onItemClick(item),
                    (x, y) => onItemContextMenu(item, x, y)
                );
                cell.oncontextmenu = (event) => {
                    event.preventDefault();
                    onItemContextMenu(item, event.clientX, event.clientY);
                };
            } else if (onExpandOverflow) {
                makeActivatable(cell, () => onExpandOverflow(node));
            }

            container.appendChild(cell);
            if (animate) requestAnimationFrame(() => cell.classList.add('in'));
            else cell.classList.add('in');
            return cell;
        });

        await loadMissingIcons(children.filter((child) => !child.isDir && child.path));

        // A newer render may have replaced the container while icons loaded.
        if (!cells[0]?.isConnected) return;

        children.forEach((item, index) => {
            const dataUrl = item.isDir ? undefined : iconCache.get(extensionOf(item.name));
            if (!dataUrl) return;
            // Only the glyph is replaced, so the access-denied badge survives.
            const image = document.createElement('img');
            image.src = dataUrl;
            image.alt = '';
            cells[index].querySelector('.grid-cell-icon-glyph').replaceChildren(image);
        });
    }

    window.DiscRevealGrid = Object.freeze({ renderGrid });
})();
