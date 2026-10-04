/**
 * List view: one row per child of the current folder, with size, share of
 * the parent and a proportional bar.
 */
(function () {
    'use strict';

    const { formatBytes, escapeHtml, accessDeniedBadge, makeActivatable } = window.DiscRevealUtil;
    const { t, nodeName, formatDecimal } = window.DiscRevealI18n;

    const MAX_STAGGER_ROWS = 24;
    const STAGGER_MS = 12;

    function scanningDot() {
        return `<span class="scan-dot" title="${escapeHtml(t('stillScanning'))}"></span>`;
    }

    /**
     * Renders `node.children` into `container`.
     *
     * Clicking a row calls `onItemClick(item)`; right-clicking calls
     * `onItemContextMenu(item, clientX, clientY)`. The synthetic overflow node
     * (empty `path`) has no filesystem entry to open or right-click, but
     * clicking it calls `onExpandOverflow(node)` to re-read the folded
     * children on demand. With `animate: false` rows appear at once, which
     * live updates need.
     */
    function renderList(container, node, onItemClick, onItemContextMenu, { animate = true, onExpandOverflow } = {}) {
        const children = node.children || [];
        const parentTotal = node.size || 0;

        container.replaceChildren();

        if (children.length === 0) {
            const empty = document.createElement('div');
            empty.className = 'list-empty';
            empty.textContent = t(node.inProgress ? 'folderStillScanning' : 'emptyFolder');
            container.appendChild(empty);
            return;
        }

        children.forEach((item, index) => {
            const row = document.createElement('div');
            row.className = `list-row ${item.isDir ? 'is-dir' : 'is-file'}${item.path ? '' : ' is-tail'}`;
            if (animate) row.style.transitionDelay = `${Math.min(index, MAX_STAGGER_ROWS) * STAGGER_MS}ms`;

            // The bar's fill matches the percentage label exactly (both are the item's
            // share of the parent's total) — it previously scaled to the largest sibling
            // instead, so two similarly-sized top items both rendered as a full bar even
            // at a single-digit percentage, which users reported as a display bug.
            const share = parentTotal > 0 ? (item.size / parentTotal) * 100 : 0;
            const barWidth = share;

            row.innerHTML = `
                <span class="list-row-type">${item.isDir ? '▸' : ''}</span>
                ${item.accessDenied ? accessDeniedBadge() : ''}
                <span class="list-row-name" title="${escapeHtml(nodeName(item))}">${escapeHtml(nodeName(item))}</span>
                ${item.inProgress ? scanningDot() : ''}
                <span class="list-row-bar-track"><span class="list-row-bar-fill"></span></span>
                <span class="list-row-pct">${formatDecimal(share)}%</span>
                <span class="list-row-size" title="${escapeHtml(window.DiscRevealUtil.storageSizeDescription(item))}">${window.DiscRevealUtil.storageSizeLabel(item)}</span>
            `;

            if (item.path) {
                makeActivatable(
                    row,
                    () => onItemClick(item),
                    (x, y) => onItemContextMenu(item, x, y)
                );
                row.oncontextmenu = (event) => {
                    event.preventDefault();
                    onItemContextMenu(item, event.clientX, event.clientY);
                };
            } else if (onExpandOverflow) {
                makeActivatable(row, () => onExpandOverflow(node));
            }

            container.appendChild(row);

            const reveal = () => {
                row.classList.add('in');
                row.querySelector('.list-row-bar-fill').style.width = `${barWidth}%`;
            };
            // Deferred one frame so the CSS transitions run.
            if (animate) requestAnimationFrame(reveal);
            else reveal();
        });
    }

    window.DiscRevealList = Object.freeze({ renderList });
})();
