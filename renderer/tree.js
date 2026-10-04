/**
 * Folder tree panel: expandable directory tree next to the file list.
 * Files are left out, as they add nothing to navigation.
 */
(function () {
    'use strict';

    const { formatBytes, makeActivatable } = window.DiscRevealUtil;
    const { nodeName, t } = window.DiscRevealI18n;

    /**
     * Renders `rootNode` into `container`. `rootBadge`, if not empty, is shown
     * as a pill next to the root entry (e.g. the drive's media and file system).
     */
    function renderTree(container, rootNode, { expandedPaths, activePath, rootBadge, onToggle, onSelect }) {
        const restore = window.DiscRevealUtil.preserveFocus?.(container);
        container.replaceChildren();
        if (rootNode) {
            const rootElement = buildNodeElement(rootNode, expandedPaths, activePath, onToggle, onSelect);
            if (rootBadge) {
                const badge = document.createElement('span');
                badge.className = 'tree-node-badge';
                badge.textContent = rootBadge;
                rootElement.querySelector('.tree-node-row').appendChild(badge);
            }
            container.appendChild(rootElement);
        }
        restore?.();
    }

    function buildNodeElement(node, expandedPaths, activePath, onToggle, onSelect) {
        const wrapper = document.createElement('div');

        const row = document.createElement('div');
        row.className = `tree-node-row${node.path === activePath ? ' active' : ''}`;
        row.dataset.focusKey = 'tree-row-' + node.path;

        const directories = (node.children || []).filter((child) => child.isDir);
        const isExpanded = expandedPaths.has(node.path);

        const toggle = document.createElement(directories.length ? 'button' : 'span');
        toggle.className = `tree-node-toggle${directories.length === 0 ? ' empty' : isExpanded ? ' expanded' : ''}`;
        toggle.textContent = '▶';
        if (directories.length > 0) {
            toggle.type = 'button'; toggle.dataset.focusKey = 'tree-toggle-' + node.path;
            toggle.setAttribute('aria-expanded', String(isExpanded));
            toggle.setAttribute('aria-label', t(isExpanded ? 'collapseFolder' : 'expandFolder', { name: nodeName(node) || node.path }));
            toggle.onclick = (event) => {
                event.stopPropagation();
                onToggle(node.path);
            };
        }
        row.appendChild(toggle);

        const name = document.createElement('span');
        name.className = 'tree-node-name';
        name.textContent = nodeName(node) || node.path;
        row.appendChild(name);

        const size = document.createElement('span');
        size.className = 'tree-node-size';
        size.textContent = window.DiscRevealUtil.storageSizeLabel(node);
        size.title = window.DiscRevealUtil.storageSizeDescription(node);
        row.appendChild(size);

        makeActivatable(row, () => onSelect(node.path));
        row.addEventListener('keydown', event => {
            if (event.target !== row || !directories.length) return;
            if ((event.key === 'ArrowRight' && !isExpanded) || (event.key === 'ArrowLeft' && isExpanded)) {
                event.preventDefault(); onToggle(node.path);
            }
        });
        wrapper.appendChild(row);

        if (isExpanded && directories.length > 0) {
            const childContainer = document.createElement('div');
            childContainer.className = 'tree-node-children';
            for (const child of directories) {
                childContainer.appendChild(buildNodeElement(child, expandedPaths, activePath, onToggle, onSelect));
            }
            wrapper.appendChild(childContainer);
        }

        return wrapper;
    }

    window.DiscRevealTree = Object.freeze({ renderTree });
})();
