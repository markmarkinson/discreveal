/**
 * Statistics view: composition by file type, size distribution, age
 * distribution and the largest files across the whole scanned tree.
 * Computed client-side from the tree already loaded — no backend command.
 */
(function () {
    'use strict';

    const { formatBytes, makeActivatable } = window.DiscRevealUtil;
    const { t, formatNumber, formatDecimal } = window.DiscRevealI18n;

    const TOP_TYPES = 10;
    const TOP_LARGEST_FILES = 15;
    const SECONDS_PER_DAY = 86400;

    const SIZE_BUCKETS = [
        { max: 1024 * 1024, labelKey: 'chartSizeUnder1MB' },
        { max: 10 * 1024 * 1024, labelKey: 'chartSize1to10MB' },
        { max: 100 * 1024 * 1024, labelKey: 'chartSize10to100MB' },
        { max: 1024 * 1024 * 1024, labelKey: 'chartSize100MBto1GB' },
        { max: Infinity, labelKey: 'chartSizeOver1GB' },
    ];

    const AGE_BUCKETS = [
        { maxDays: 30, labelKey: 'chartAgeUnder1Month' },
        { maxDays: 182, labelKey: 'chartAge1to6Months' },
        { maxDays: 365, labelKey: 'chartAge6to12Months' },
        { maxDays: 730, labelKey: 'chartAge1to2Years' },
        { maxDays: Infinity, labelKey: 'chartAgeOver2Years' },
    ];

    function extensionOf(name) {
        const dot = name.lastIndexOf('.');
        return dot > 0 ? name.slice(dot).toLowerCase() : '';
    }

    /**
     * Walks the tree once, splitting it into individually addressable files
     * and the total bytes/count hidden inside folded overflow nodes (which
     * carry no per-file breakdown, so they can't enter the charts below —
     * shown instead as one honest "folded" line rather than left out silently).
     */
    function collectFiles(tree) {
        const files = [];
        let foldedBytes = 0;
        let foldedCount = 0;
        (function walk(node) {
            for (const child of node.children || []) {
                if (!child.path) {
                    foldedBytes += child.size;
                    foldedCount += child.overflowCount || 0;
                } else if (child.isDir) {
                    walk(child);
                } else {
                    files.push(child);
                }
            }
        })(tree);
        return { files, foldedBytes, foldedCount };
    }

    function compositionByType(files) {
        const byExtension = new Map();
        for (const file of files) {
            const key = extensionOf(file.name) || t('chartNoExtension');
            const entry = byExtension.get(key) || { label: key, size: 0, count: 0 };
            entry.size += file.size;
            entry.count += 1;
            byExtension.set(key, entry);
        }
        const sorted = [...byExtension.values()].sort((a, b) => b.size - a.size);
        const top = sorted.slice(0, TOP_TYPES);
        const rest = sorted.slice(TOP_TYPES);
        if (rest.length > 0) {
            top.push({
                label: t('chartOtherTypes'),
                size: rest.reduce((sum, entry) => sum + entry.size, 0),
                count: rest.reduce((sum, entry) => sum + entry.count, 0),
            });
        }
        return top;
    }

    function bucketBy(files, buckets, keyOf) {
        const rows = buckets.map((bucket) => ({ label: t(bucket.labelKey), size: 0, count: 0 }));
        for (const file of files) {
            const key = keyOf(file);
            if (key === null) continue;
            const index = buckets.findIndex((bucket) => key < (bucket.max ?? bucket.maxDays));
            rows[index === -1 ? rows.length - 1 : index].size += file.size;
            rows[index === -1 ? rows.length - 1 : index].count += 1;
        }
        return rows;
    }

    function sizeDistribution(files) {
        return bucketBy(files, SIZE_BUCKETS, (file) => file.size);
    }

    function ageDistribution(files) {
        const nowSeconds = Date.now() / 1000;
        // A file with no reliable modified time (0) is left out of the age chart
        // rather than dumped into "over 2 years", which would misrepresent it.
        return bucketBy(files, AGE_BUCKETS, (file) => (file.modified ? (nowSeconds - file.modified) / SECONDS_PER_DAY : null));
    }

    function largestFiles(files) {
        return [...files].sort((a, b) => b.size - a.size).slice(0, TOP_LARGEST_FILES);
    }

    const SVG_NS = 'http://www.w3.org/2000/svg';
    /** Hue of `--accent` in style.css (#e50914) — chart colours are built around it. */
    const ACCENT_HUE = 357;

    /**
     * A shared floating tooltip, positioned relative to `root` (must be
     * `position: relative`). The caller appends `.element` wherever it needs
     * to paint above everything else — this factory does not insert it.
     */
    function createTooltip(root) {
        const bubble = document.createElement('div');
        bubble.className = 'chart-tooltip hidden';
        return {
            element: bubble,
            show(text, clientX, clientY) {
                bubble.textContent = text;
                bubble.classList.remove('hidden');
                this.move(clientX, clientY);
            },
            move(clientX, clientY) {
                const rootRect = root.getBoundingClientRect();
                bubble.style.left = `${Math.max(0, Math.min(clientX - rootRect.left + 14, rootRect.width - bubble.offsetWidth))}px`;
                bubble.style.top = `${Math.max(0, Math.min(clientY - rootRect.top + 14, rootRect.height - bubble.offsetHeight))}px`;
            },
            hide() {
                bubble.classList.add('hidden');
            },
        };
    }

    /** Distinct categorical colours spaced around the wheel, starting at the app's accent hue. */
    function categoricalColors(count) {
        return Array.from({ length: count }, (_, i) => `hsl(${Math.round(ACCENT_HUE + (i * 360) / count) % 360}, 70%, 56%)`);
    }

    /** A light-to-dark ramp of the accent hue, for ordered buckets (size/age ranges). */
    function sequentialColors(count) {
        return Array.from({ length: count }, (_, i) => {
            const lightness = count > 1 ? 72 - (i * 38) / (count - 1) : 55;
            return `hsl(${ACCENT_HUE}, 65%, ${lightness.toFixed(0)}%)`;
        });
    }

    function polarToCartesian(cx, cy, r, angleDeg) {
        const angleRad = ((angleDeg - 90) * Math.PI) / 180;
        return { x: cx + r * Math.cos(angleRad), y: cy + r * Math.sin(angleRad) };
    }

    /** SVG path for one ring wedge, from `startAngle` to `endAngle` (degrees, clockwise from top). */
    function ringWedgePath(cx, cy, outerR, innerR, startAngle, endAngle) {
        const start = Math.min(endAngle - startAngle, 359.99) + startAngle;
        const outerStart = polarToCartesian(cx, cy, outerR, start);
        const outerEnd = polarToCartesian(cx, cy, outerR, startAngle);
        const innerStart = polarToCartesian(cx, cy, innerR, start);
        const innerEnd = polarToCartesian(cx, cy, innerR, startAngle);
        const largeArc = start - startAngle > 180 ? 1 : 0;
        return [
            `M ${outerStart.x} ${outerStart.y}`,
            `A ${outerR} ${outerR} 0 ${largeArc} 0 ${outerEnd.x} ${outerEnd.y}`,
            `L ${innerEnd.x} ${innerEnd.y}`,
            `A ${innerR} ${innerR} 0 ${largeArc} 1 ${innerStart.x} ${innerStart.y}`,
            'Z',
        ].join(' ');
    }

    /** A legend row synced with a chart element: hovering either highlights both. */
    function legendRow(color, label, valueText, onHighlight, onUnhighlight) {
        const row = document.createElement('div');
        row.className = 'chart-legend-row';
        const swatch = document.createElement('span');
        swatch.className = 'chart-legend-swatch';
        swatch.style.background = color;
        const labelEl = document.createElement('span');
        labelEl.className = 'chart-legend-label';
        labelEl.textContent = label;
        const valueEl = document.createElement('span');
        valueEl.className = 'chart-legend-value';
        valueEl.textContent = valueText;
        row.append(swatch, labelEl, valueEl);
        row.tabIndex = 0;
        row.setAttribute('aria-label', label + ' · ' + valueText);
        row.addEventListener('mouseenter', onHighlight);
        row.addEventListener('mouseleave', onUnhighlight);
        row.addEventListener('focus', () => { const rect = row.getBoundingClientRect(); onHighlight({ clientX: rect.left, clientY: rect.bottom }); });
        row.addEventListener('blur', onUnhighlight);
        return row;
    }

    /** Composition-by-type as an interactive donut: hover a wedge or its legend row to highlight both. */
    function renderDonutSection(title, rows, tooltip) {
        const section = document.createElement('section');
        section.className = 'chart-section';
        const heading = document.createElement('h3');
        heading.className = 'chart-section-title';
        heading.textContent = title;
        section.appendChild(heading);

        const total = rows.reduce((sum, row) => sum + row.size, 0);
        const size = 168;
        const cx = size / 2;
        const cy = size / 2;
        const outerR = 78;
        const innerR = 50;
        const colors = categoricalColors(rows.length);
        const isOther = (row) => row.label === t('chartOtherTypes');

        const body = document.createElement('div');
        body.className = 'chart-donut-body';
        const svg = document.createElementNS(SVG_NS, 'svg');
        svg.setAttribute('viewBox', `0 0 ${size} ${size}`);
        svg.setAttribute('class', 'chart-donut');
        const legend = document.createElement('div');
        legend.className = 'chart-legend';

        let angle = 0;
        rows.forEach((row, index) => {
            const share = total ? row.size / total : 0;
            const sweep = share * 360;
            const color = isOther(row) ? 'var(--text-3)' : colors[index];

            const path = document.createElementNS(SVG_NS, 'path');
            path.setAttribute('d', ringWedgePath(cx, cy, outerR, innerR, angle, angle + sweep));
            path.setAttribute('fill', color);
            path.setAttribute('class', 'chart-donut-segment');

            const valueText = `${formatBytes(row.size)} · ${formatNumber(row.count)} (${formatDecimal(share * 100)}%)`;
            const highlight = (event) => {
                path.classList.add('is-active');
                row_.classList.add('is-active');
                tooltip.show(`${row.label} — ${valueText}`, event.clientX, event.clientY);
            };
            const move = (event) => tooltip.move(event.clientX, event.clientY);
            const unhighlight = () => {
                path.classList.remove('is-active');
                row_.classList.remove('is-active');
                tooltip.hide();
            };
            path.addEventListener('mouseenter', highlight);
            path.addEventListener('mousemove', move);
            path.addEventListener('mouseleave', unhighlight);
            svg.appendChild(path);

            const row_ = legendRow(
                color,
                row.label,
                valueText,
                (event) => highlight(event),
                unhighlight
            );
            legend.appendChild(row_);

            angle += sweep;
        });

        const centerLabel = document.createElement('div');
        centerLabel.className = 'chart-donut-center';
        centerLabel.innerHTML = `<span class="chart-donut-center-value">${formatBytes(total)}</span><span class="chart-donut-center-hint">${t('chartTypeTitle')}</span>`;

        const donutFrame = document.createElement('div');
        donutFrame.className = 'chart-donut-frame';
        donutFrame.append(svg, centerLabel);

        body.append(donutFrame, legend);
        section.appendChild(body);
        return section;
    }

    /** A size/age distribution as one interactive stacked bar plus a legend. */
    function renderStackedBarSection(title, rows, tooltip) {
        const section = document.createElement('section');
        section.className = 'chart-section';
        const heading = document.createElement('h3');
        heading.className = 'chart-section-title';
        heading.textContent = title;
        section.appendChild(heading);

        const total = rows.reduce((sum, row) => sum + row.size, 0);
        if (total === 0) {
            const empty = document.createElement('div');
            empty.className = 'chart-empty';
            empty.textContent = t('chartNoData');
            section.appendChild(empty);
            return section;
        }

        const colors = sequentialColors(rows.length);
        const bar = document.createElement('div');
        bar.className = 'chart-stacked-bar';
        const legend = document.createElement('div');
        legend.className = 'chart-legend';

        rows.forEach((row, index) => {
            if (row.size === 0) return;
            const share = row.size / total;
            const color = colors[index];
            const valueText = `${formatBytes(row.size)} · ${formatNumber(row.count)} (${formatDecimal(share * 100)}%)`;

            const segment = document.createElement('span');
            segment.className = 'chart-stacked-segment';
            segment.style.width = `${(share * 100).toFixed(2)}%`;
            segment.style.background = color;

            const highlight = (event) => {
                segment.classList.add('is-active');
                row_.classList.add('is-active');
                tooltip.show(`${row.label} — ${valueText}`, event.clientX, event.clientY);
            };
            const move = (event) => tooltip.move(event.clientX, event.clientY);
            const unhighlight = () => {
                segment.classList.remove('is-active');
                row_.classList.remove('is-active');
                tooltip.hide();
            };
            segment.addEventListener('mouseenter', highlight);
            segment.addEventListener('mousemove', move);
            segment.addEventListener('mouseleave', unhighlight);
            bar.appendChild(segment);

            const row_ = legendRow(color, row.label, valueText, (event) => highlight(event), unhighlight);
            legend.appendChild(row_);
        });

        section.append(bar, legend);
        return section;
    }

    function renderLargestFilesSection(title, files, onOpenFolder) {
        const section = document.createElement('section');
        section.className = 'chart-section';

        const heading = document.createElement('h3');
        heading.className = 'chart-section-title';
        heading.textContent = title;
        section.appendChild(heading);

        if (files.length === 0) {
            const empty = document.createElement('div');
            empty.className = 'chart-empty';
            empty.textContent = t('chartNoData');
            section.appendChild(empty);
            return section;
        }

        for (const file of files) {
            const row = document.createElement('div');
            row.className = 'chart-file-row';

            const name = document.createElement('span');
            name.className = 'chart-file-name';
            name.textContent = file.name;
            name.title = file.path;

            const size = document.createElement('span');
            size.className = 'chart-file-size';
            size.textContent = formatBytes(file.size);

            row.append(name, size);
            makeActivatable(row, () => onOpenFolder(file));
            section.appendChild(row);
        }
        return section;
    }

    /** Renders the full statistics view for `tree` into `container`. */
    function renderCharts(container, tree, onOpenFolder) {
        container.replaceChildren();
        if (!tree) return;

        const { files, foldedBytes, foldedCount } = collectFiles(tree);

        if (foldedBytes > 0 || foldedCount > 0) {
            const note = document.createElement('div'); note.className = 'charts-fold-note';
            note.textContent = t('chartsFoldedNote', { count: formatNumber(foldedCount), size: formatBytes(foldedBytes) });
            container.appendChild(note);
        }

        if (files.length === 0) {
            const empty = document.createElement('div');
            empty.className = 'chart-empty';
            empty.textContent = t('chartNoData');
            container.appendChild(empty);
            return;
        }

        // Created before the sections that need it, appended after: the tooltip must paint
        // above everything else, and DOM order (not just z-index) decides that among siblings.
        const tooltip = createTooltip(container);
        container.append(
            renderDonutSection(t('chartTypeTitle'), compositionByType(files), tooltip),
            renderStackedBarSection(t('chartSizeTitle'), sizeDistribution(files), tooltip),
            renderStackedBarSection(t('chartAgeTitle'), ageDistribution(files), tooltip),
            renderLargestFilesSection(t('chartLargestTitle'), largestFiles(files), onOpenFolder)
        );
        container.appendChild(tooltip.element);
    }

    window.DiscRevealCharts = Object.freeze({ renderCharts });
})();
