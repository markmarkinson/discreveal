/**
 * Whole-tree search over every scanned file and folder, not only the folder
 * currently shown. Results are sorted by size and capped, since the largest
 * matches are the relevant ones on a full-drive scan.
 */
(function () {
    'use strict';

    const { formatBytes, escapeHtml, makeActivatable } = window.DiscRevealUtil;
    const { t, formatNumber } = window.DiscRevealI18n;

    const MAX_RESULTS = 200;
    const MAX_STAGGER_ROWS = 24;
    const STAGGER_MS = 10;

    /**
     * Returns nodes whose name contains `query` (case-insensitive), largest
     * first. The synthetic overflow node (empty path) never matches.
     */
    function searchTree(root, query) {
        const needle = query.trim().toLowerCase();
        if (!needle) return [];

        const results = [];
        const visit = (node) => {
            for (const child of node.children || []) {
                if (!child.path) continue;
                if (child.name.toLowerCase().includes(needle)) results.push(child);
                if (child.isDir) visit(child);
            }
        };
        visit(root);

        results.sort((a, b) => b.size - a.size);
        const visible = results.slice(0, MAX_RESULTS);
        visible.total = results.length;
        return visible;
    }

    /** Directory part of a Windows path; a drive root keeps its trailing backslash (`C:\`). */
    function dirnameOf(fullPath) {
        const index = Math.max(fullPath.lastIndexOf('\\'), fullPath.lastIndexOf('/'));
        if (index <= 0) return fullPath;
        const parent = fullPath.slice(0, index);
        return /^[A-Za-z]:$/.test(parent) ? `${parent}\\` : parent;
    }

    /** Renders `results` into `container`; `onItemClick(item)` handles selection. */
    function renderSearchResults(container, results, onItemClick) {
        container.replaceChildren();
        const summary = document.createElement('p'); summary.className = 'search-summary';
        summary.setAttribute('role', 'status');
        summary.textContent = t('searchSummary', { shown: formatNumber(results.length), total: formatNumber(results.total ?? results.length) });
        container.appendChild(summary);
        const scopeHint = document.createElement('p'); scopeHint.className = 'search-summary'; scopeHint.textContent = t('searchScopeHint'); container.appendChild(scopeHint);
        if ((results.total ?? results.length) > results.length) { const hint = document.createElement('p'); hint.className = 'search-summary'; hint.textContent = t('searchLimitHint'); container.appendChild(hint); }

        if (results.length === 0) {
            const empty = document.createElement('div');
            empty.className = 'list-empty';
            empty.textContent = t('noHits');
            container.appendChild(empty);
            return;
        }

        results.forEach((item, index) => {
            const row = document.createElement('div');
            row.className = 'search-result-row';
            row.style.transitionDelay = `${Math.min(index, MAX_STAGGER_ROWS) * STAGGER_MS}ms`;

            row.innerHTML = `
                <span class="search-result-type">${item.isDir ? '▸' : ''}</span>
                <div class="search-result-main">
                    <div class="search-result-name">${escapeHtml(item.name)}</div>
                    <div class="search-result-path" title="${escapeHtml(item.path)}">${escapeHtml(dirnameOf(item.path))}</div>
                </div>
                <span class="search-result-size" title="${escapeHtml(window.DiscRevealUtil.storageSizeDescription(item))}">${window.DiscRevealUtil.storageSizeLabel(item)}</span>
            `;

            makeActivatable(row, () => onItemClick(item));
            container.appendChild(row);
            requestAnimationFrame(() => row.classList.add('in'));
        });
    }

    window.DiscRevealSearch = Object.freeze({ searchTree, renderSearchResults, dirnameOf });
})();
