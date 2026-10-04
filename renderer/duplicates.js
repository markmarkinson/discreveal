/**
 * Duplicate finder: a drive picker, a live progress view and the results
 * (one card per group of files with identical content, each file
 * individually selectable for deletion). Stateless like `list.js`/`grid.js`
 * — all state (selection, progress, which drives are checked) lives in the
 * caller and is passed in.
 */
(function () {
    'use strict';

    const { formatBytes } = window.DiscRevealUtil;
    const { t, formatNumber } = window.DiscRevealI18n;

    /**
     * Total used bytes across `drives` — the denominator for the walk
     * phase's real progress percentage (`bytesScanned` / this, the exact
     * metric `start_scan`'s own progress bar already uses). An earlier
     * version instead *guessed* a total duration from this number times an
     * assumed throughput; that guess was consistently far too pessimistic
     * (confirmed live: a ~67-minute estimate for a search that was already
     * past 300,000 files within 6 seconds) because neither phase actually
     * reads through a drive's used space the way the guess assumed — the
     * walk phase only reads metadata, and the analyze phase (since the
     * fast-hash change) only samples a bounded prefix of each candidate, not
     * whole files. A live percentage instead of an upfront promise sidesteps
     * the whole problem.
     */
    function totalUsedBytes(drives) {
        return drives.reduce((total, drive) => total + drive.usedSize, 0);
    }

    /** Short "media · file system" description, matching app.js's own `driveDetails`. */
    function driveDetails(drive) {
        const details = [];
        if (drive.mediaType !== 'Unknown') details.push(drive.busType === 'NVMe' ? 'NVMe' : drive.mediaType);
        if (drive.fileSystem !== 'Unknown') details.push(drive.fileSystem);
        return details.join(' · ');
    }

    /**
     * Renders the drive-picker step. `selectedPaths` is a `Set` of drive
     * paths currently checked. `cacheEnabled` is the current opt-in state of
     * the persistent hash cache; `cacheStats` is `null` while its size is
     * still loading, otherwise `{ exists, entryCount, fileSizeBytes }` from
     * `hash_cache_stats`. Callbacks: `onToggleDrive(path)`, `onStart()`,
     * `onToggleCache()`, `onClearCache()`.
     */
    function renderDrivePicker(container, drives, selectedPaths, { onToggleDrive, onStart, cacheEnabled, cacheStats, onToggleCache, onClearCache, filter, onFilterChange }) {
        const filtersOpen = Boolean(container.querySelector('.dup-filter-disclosure')?.open);
        container.replaceChildren();
        const setup = document.createElement('div'); setup.className = 'dup-setup'; container.appendChild(setup);
        const heading = document.createElement('h2'); heading.className = 'dup-setup-title'; heading.textContent = t('dupSetupTitle'); setup.appendChild(heading);

        const intro = document.createElement('div');
        intro.className = 'dup-summary';
        intro.textContent = t('dupPickIntro');
        setup.appendChild(intro);

        const list = document.createElement('div');
        list.className = 'dup-drive-list';
        for (const drive of drives) {
            const row = document.createElement('label');
            row.className = 'dup-drive-row';
            row.classList.toggle('is-selected', selectedPaths.has(drive.path));

            const checkbox = document.createElement('input');
            checkbox.type = 'checkbox';
            checkbox.checked = selectedPaths.has(drive.path);
            checkbox.onchange = () => onToggleDrive(drive.path);

            const info = document.createElement('span');
            info.className = 'dup-drive-info';
            const path = document.createElement('span');
            path.className = 'dup-drive-path';
            path.textContent = drive.path;
            const meta = document.createElement('span');
            meta.className = 'dup-drive-meta';
            meta.textContent = [driveDetails(drive), formatBytes(drive.usedSize)].filter(Boolean).join(' · ');
            info.append(path, meta);

            row.append(checkbox, info);
            list.appendChild(row);
        }
        setup.appendChild(list);
        if (filter) {
            const option = document.createElement('label'); option.className = 'dup-size-toggle dup-cache-toggle';
            const checkbox = document.createElement('input'); checkbox.type = 'checkbox'; checkbox.checked = filter.minEnabled !== false;
            const text = document.createElement('span'); text.textContent = t('dupSkipSmallFiles');
            checkbox.onchange = () => {
                onFilterChange('minEnabled', checkbox.checked);
                for (const input of setup.querySelectorAll('.dup-min-control')) input.disabled = !checkbox.checked;
            };
            option.append(checkbox, text); setup.appendChild(option);
            const hint = document.createElement('p'); hint.className = 'dup-size-hint'; hint.textContent = t('dupSkipSmallHint'); setup.appendChild(hint);
        }
        if (filter) { renderFilterControls(setup, filter, onFilterChange, true); setup.querySelector('.dup-filter-disclosure').open = filtersOpen; }

        const cacheOption = document.createElement('div');
        cacheOption.className = 'dup-cache-option';

        const cacheLabel = document.createElement('label');
        cacheLabel.className = 'dup-cache-toggle';
        const cacheCheckbox = document.createElement('input');
        cacheCheckbox.type = 'checkbox';
        cacheCheckbox.checked = cacheEnabled;
        cacheCheckbox.onchange = onToggleCache;
        const cacheText = document.createElement('span');
        cacheText.textContent = t('dupCacheLabel');
        cacheLabel.append(cacheCheckbox, cacheText);
        cacheOption.appendChild(cacheLabel);

        cacheLabel.title = t('dupCacheHint');

        if (cacheStats?.exists) {
            const cacheStatsRow = document.createElement('div');
            cacheStatsRow.className = 'dup-cache-stats';
            const cacheStatsText = document.createElement('span');
            cacheStatsText.textContent = t('dupCacheStats', {
                count: formatNumber(cacheStats.entryCount),
                size: formatBytes(cacheStats.fileSizeBytes),
            });
            const clearButton = document.createElement('button');
            clearButton.type = 'button';
            clearButton.className = 'link-btn';
            clearButton.textContent = t('dupCacheClear');
            clearButton.onclick = onClearCache;
            cacheStatsRow.append(cacheStatsText, clearButton);
            cacheOption.appendChild(cacheStatsRow);
        }
        setup.appendChild(cacheOption);
        const settings = setup.querySelector('.dup-filter-disclosure');
        if (settings) {
            const panel = document.createElement('div'); panel.className = 'dup-options-panel';
            panel.appendChild(settings.querySelector('.dup-filters'));
            for (const node of [...setup.querySelectorAll('.dup-size-toggle, .dup-size-hint, .dup-cache-option')]) panel.appendChild(node);
            settings.appendChild(panel);
        }

        const selectedDrives = drives.filter((drive) => selectedPaths.has(drive.path));
        const summary = document.createElement('div');
        summary.className = 'dup-eta';
        summary.textContent =
            selectedDrives.length > 0 ? t('dupSelectedSummary', { size: formatBytes(totalUsedBytes(selectedDrives)) }) : '';
        const footer = document.createElement('div'); footer.className = 'dup-setup-footer';
        footer.appendChild(summary);

        const startButton = document.createElement('button');
        startButton.type = 'button';
        startButton.className = 'pill-btn pill-btn-primary';
        startButton.textContent = t('dupStart');
        startButton.disabled = selectedDrives.length === 0;
        startButton.onclick = onStart;
        footer.appendChild(startButton); setup.appendChild(footer);
    }

    function buildProgressSkeleton(onCancel) {
        return window.DiscRevealProgress.create('dup-progress', onCancel);
    }

    /** Matches `MAX_SCAN_PROGRESS_PERCENT` in app.js: excluded/unreadable data can keep a byte-based percentage from ever reaching 100. */
    const MAX_WALK_PROGRESS_PERCENT = 97;

    /**
     * Renders the live-progress step, updating the existing DOM in place on
     * repeated calls rather than rebuilding it. This matters: a fresh
     * `.progress-bar-fill` element on every call (progress events arrive up
     * to ~8/s, plus a once-a-second elapsed-time tick) restarted its CSS
     * animation just as often, which is what made the indeterminate bar look
     * broken rather than smoothly busy. `progress` is `null` before the
     * first event arrives. `totalBytes` is the selected drives' combined
     * used size — the walk phase shows `bytesScanned / totalBytes` as a
     * real, live percentage (the exact metric the main scan's own progress
     * bar already uses) rather than the upfront duration guess an earlier
     * version showed, which was consistently far too pessimistic.
     * `cancelling` is true from the moment the user clicks "Abbrechen" until
     * the backend's `duplicates:done` actually arrives — the cancel button
     * disables and the status line says so, since cancelling still has to
     * wait for the current file to finish hashing before results (whatever
     * was already confirmed) show up. Callback: `onCancel()`.
     */
    function renderProgress(container, progress, totalBytes, elapsedSeconds, { onCancel, cancelling }) {
        let box = container.querySelector('.dup-progress');
        if (!box) {
            container.replaceChildren();
            box = buildProgressSkeleton(onCancel);
            container.appendChild(box);
        }

        const analyzing = Boolean(progress && progress.phase === 'analyze' && progress.candidatesTotal > 0);
        const walking = Boolean(progress && progress.phase === 'walk' && totalBytes > 0);
        let status = t('dupStarting');
        if (cancelling) status = t('dupCancelling');
        else if (progress) {
            const phaseLabel = t(progress.phase === 'analyze' ? 'dupPhaseAnalyze' : 'dupPhaseWalk');
            const detail = analyzing
                ? t('dupProgressAnalyze', { done: formatNumber(progress.candidatesDone), total: formatNumber(progress.candidatesTotal) })
                : t('dupProgressWalk', { count: formatNumber(progress.filesScanned) });
            status = `${phaseLabel} — ${detail}`;
        }
        window.DiscRevealProgress.update(box, {
            status,
            detail: (progress?.currentPath ?? '').replace(/^\\\\\?\\/, ''),
            title: progress?.currentPath ?? '',
            elapsed: t('dupElapsed', { seconds: formatNumber(Math.floor(elapsedSeconds)) }),
            percentage: analyzing ? progress.candidatesDone / progress.candidatesTotal * 100
                : walking ? Math.min(MAX_WALK_PROGRESS_PERCENT, progress.bytesScanned / totalBytes * 100) : null,
            cancelling: Boolean(cancelling), hint: cancelling ? t('dupCancelHint') : '',
        });
    }

    /** Total bytes reclaimed if every file but one in each group is deleted. */
    function wastedBytes(groups) {
        return groups.reduce((sum, group) => {
            const keeper = oldestFile(group);
            return sum + group.files.reduce((bytes, file) => bytes + (file === keeper ? 0 : reclaimableBytes(file, group)), 0);
        }, 0);
    }

    function reclaimableBytes(file, group) {
        // Old in-memory fixtures/providers lack this field. Current backend
        // always sends it: null is explicitly unknown and must never inflate savings.
        return file.reclaimableBytes === undefined ? group.size : file.reclaimableBytes ?? 0;
    }

    /** The file with the oldest `modified` time in a group — the copy a "keep only the oldest" action leaves alone. */
    function oldestFile(group) {
        return group.files.reduce((oldest, file) => (file.modified && (!oldest.modified || file.modified < oldest.modified) ? file : oldest));
    }

    function renderGroup(group, selectedPaths, { onToggleSelect, onOpenPath, onKeepOnlyOldest, readOnly = false, fileLimit = 25, expanded = false }) {
        const card = document.createElement('details');
        card.className = 'dup-group'; card.open = expanded;

        const header = document.createElement('summary');
        header.dataset.focusKey = 'summary-' + group.files[0].path;
        header.className = 'dup-group-header';
        const title = document.createElement('span');
        title.className = 'dup-group-title';
        title.textContent = oldestFile(group).name;
        const meta = document.createElement('span'); meta.className = 'dup-group-meta';
        meta.textContent = `${t('dupGroupCopies', {count:formatNumber(group.files.length)})} · ${formatBytes(group.size)} ${t('dupPerFile')}`;
        const identity = document.createElement('div'); identity.className = 'dup-group-identity'; identity.append(title, meta);
        const savings = document.createElement('span'); savings.className = 'dup-group-savings'; savings.textContent = formatBytes(wastedBytes([group])); savings.title = t('dupPotentialLabel');
        const eligible = [...suggestedPaths([group])];
        const select = document.createElement('input'); select.type = 'checkbox';
        select.dataset.focusKey = 'group-' + group.files[0].path;
        select.checked = eligible.length > 0 && eligible.every(path => selectedPaths.has(path));
        select.indeterminate = !select.checked && eligible.some(path => selectedPaths.has(path));
        select.disabled = readOnly || eligible.length === 0;
        if (!eligible.length) select.title = t('dupNotEligible');
        select.setAttribute('aria-label', t('dupKeepOldest'));
        select.onclick = event => event.stopPropagation();
        select.onchange = () => onKeepOnlyOldest(group);
        const chevron = document.createElement('i'); chevron.className = 'cleanup-chevron'; chevron.setAttribute('aria-hidden', 'true');
        const retained = document.createElement('span'); retained.className = 'dup-retained'; retained.textContent = t('dupOriginalKept');
        meta.textContent += ` · ${t(sameNames(group) ? 'dupSameNames' : 'dupDifferentNames')}`;
        header.append(select, chevron, identity, retained, savings);
        card.appendChild(header);
        const body = document.createElement('div'); body.className = 'dup-group-body';
        card.appendChild(body);
        let populated = false;
        const populate = () => {
            if (populated || !card.open) return;
            populated = true;
            card._fileLimit = fileLimit;
            const oldest = oldestFile(group);
            const ordered = [oldest, ...group.files.slice(0, fileLimit).filter(file=>file !== oldest)];
            for (const file of ordered.slice(0, fileLimit)) {
                const row = document.createElement('label');
                row.className = 'dup-file-row';

                const checkbox = document.createElement('input');
                checkbox.type = 'checkbox';
                checkbox.dataset.focusKey = 'file-' + file.path;
                checkbox.checked = selectedPaths.has(file.path);
                checkbox.disabled = readOnly || file === oldest || !file.cleanupEligible;
                if (file === oldest || !file.cleanupEligible) { row.title = file !== oldest && file.cleanupReason ? window.DiscRevealI18n.describeError({code:file.cleanupReason}) : t(file === oldest ? 'dupOriginalKept' : 'dupNotEligible'); checkbox.setAttribute('aria-label', file.name + ' · ' + row.title); }
                checkbox.onchange = () => onToggleSelect(file.path);
                row.appendChild(checkbox);

                const name = document.createElement('span');
                name.className = 'dup-file-name';
                const basename = document.createElement('span');
                basename.className = 'dup-file-basename';
                basename.textContent = file.name;
                name.appendChild(basename);
                name.title = `${file.name} · ${formatBytes(group.size)}${file.modified ? ` · ${window.DiscRevealI18n.formatDate(file.modified * 1000)}` : ''}`;
                if (file === oldest) {
                    const badge = document.createElement('span');
                    badge.className = 'dup-file-oldest-badge';
                    badge.textContent = t('dupOriginalBadge');
                    name.appendChild(badge);
                }

                const path = document.createElement('span');
                path.className = 'dup-file-path';
                path.textContent = file.path.replace(/^\\\\\?\\/, '');
                path.title = file.path;

                const open = document.createElement('button');
                open.type = 'button';
                open.className = 'dup-file-open';
                open.dataset.focusKey = 'open-' + file.path;
                open.title = t('menuOpenPath');
                open.setAttribute('aria-label', t('menuOpenPath') + ' · ' + file.name);
                open.textContent = '↗';
                open.onclick = (event) => {
                    event.preventDefault();
                    onOpenPath(file.path);
                };

                row.append(name, path, open);
                body.appendChild(row);
            }
            if (group.files.length > fileLimit) {
                const more=document.createElement('button'); more.type='button'; more.className='link-btn';
                more.dataset.focusKey = 'more-files-' + group.files[0].path;
                more.textContent=t('dupMoreFiles',{count:formatNumber(group.files.length-fileLimit)});
                more.onclick=()=>{ const restore = window.DiscRevealUtil.preserveFocus?.(body); fileLimit += 25; populated = false; body.replaceChildren(); populate(); restore?.(); if (document.activeElement === document.body) header.focus(); };
                body.appendChild(more);
            }
            const proof = document.createElement('details');
            proof.className = 'dup-proof';
            const proofTitle = document.createElement('summary'); proofTitle.textContent = t('dupProofDetails'); proof.appendChild(proofTitle);
            const proofBody = document.createElement('div'); proofBody.className = 'dup-proof-body';
            const types=[...new Set(group.files.map(file=>file.name.includes('.') ? file.name.split('.').at(-1).toUpperCase() : '—'))].slice(0,6).join(', ');
            proofBody.textContent = `${t('dupVerified')} · ${t(sameNames(group) ? 'dupSameNames' : 'dupDifferentNames')} · ${formatBytes(group.size)} ${t('dupPerFile')} · ${types} · ${t('dupHash')}: ${group.sampleHash || '—'}`;
            proof.appendChild(proofBody);
            proof.title = t('dupHashHint');
            body.appendChild(proof);
        };
        card.ontoggle = populate; populate();
        return card;
    }

    function sameNames(group) { return group.files.every(file => file.name.toLowerCase() === group.files[0].name.toLowerCase()); }

    function extensions(text) { return text.toLowerCase().split(/[\s,;]+/).map(value=>value.replace(/^\*?\./, '')).filter(Boolean); }

    function filterGroups(groups, filter) {
        const types = extensions(filter.types || '');
        const min = Number(filter.min || 0) * (filter.minUnit === 'GB' ? 1024 ** 3 : 1024 ** 2);
        const max = Number(filter.max || 0) * (filter.maxUnit === 'GB' ? 1024 ** 3 : 1024 ** 2);
        const name = (filter.name || '').toLowerCase();
        return groups.filter(group => group.size >= min && (!max || group.size <= max) &&
            (filter.category !== 'same' || sameNames(group)) && (filter.category !== 'different' || !sameNames(group)) &&
            group.files.some(file => (!name || `${file.name} ${file.path}`.toLowerCase().includes(name)) &&
                (!types.length || types.some(type => file.name.toLowerCase().endsWith(`.${type}`)))));
    }

    function discRevealDuplicateStats(groups, selectedPaths = new Set()) {
        let copies = 0, bytes = 0, eligibleBytes = 0, selected = 0, selectedBytes = 0;
        for (const group of groups) {
            copies += group.files.length - 1;
            bytes += wastedBytes([group]);
            const keeper = oldestFile(group);
            for (const file of group.files) if (file !== keeper && file.cleanupEligible) eligibleBytes += reclaimableBytes(file, group);
            if (selectedPaths.size) for (const file of group.files) if (file !== keeper && selectedPaths.has(file.path)) { selected++; selectedBytes += reclaimableBytes(file, group); }
        }
        return {groups:groups.length, copies, bytes, eligibleBytes, selected, selectedBytes};
    }

    function suggestedPaths(groups) {
        const selected = new Set();
        for (const group of groups) {
            const keeper = oldestFile(group);
            for (const file of group.files) if (file !== keeper && file.cleanupEligible) selected.add(file.path);
        }
        return selected;
    }

    function renderFilterControls(container, filter, onChange, scan = false) {
        const disclosure = document.createElement('details'); disclosure.className = 'dup-filter-disclosure';
        const toggle = document.createElement('summary'); toggle.className = 'pill-btn pill-btn-ghost'; toggle.textContent = t(scan ? 'dupSearchOptions' : 'dupFilterButton'); disclosure.appendChild(toggle);
        const box = document.createElement('div'); box.className = 'dup-filters';
        const field = (key, label, choices) => {
            const wrapper = document.createElement('label'); wrapper.textContent = t(label);
            const input = document.createElement(choices ? 'select' : 'input');
            if (choices) for (const [value, text] of choices) { const option = document.createElement('option'); option.value=value; option.textContent=t(text); input.appendChild(option); }
            else { input.type = key === 'min' || key === 'max' ? 'number' : 'text'; if (input.type === 'number') { input.min='0'; input.step='any'; } }
            input.value = filter[key] || '';
            if (scan && (key === 'min' || key === 'minUnit')) { input.className = 'dup-min-control'; input.disabled = filter.minEnabled === false; }
            input.oninput = () => onChange(key, input.value);
            wrapper.appendChild(input); box.appendChild(wrapper);
        };
        if (!scan) {
            const search=document.createElement('input'); search.type='search'; search.className='dup-search'; search.value=filter.name || ''; search.placeholder=t('dupSearchPlaceholder'); search.setAttribute('aria-label',t('dupFilterName')); search.oninput=()=>onChange('name',search.value); container.appendChild(search);
        }
        field('min', 'dupFilterMin'); field('minUnit', 'dupUnit', [['MB','dupMB'],['GB','dupGB']]);
        field('max', 'dupFilterMax'); field('maxUnit', 'dupUnit', [['MB','dupMB'],['GB','dupGB']]);
        if (scan) field('typeMode','dupTypeMode',[['all','dupAllTypes'],['include','dupIncludeTypes'],['exclude','dupExcludeTypes']]);
        field('types', 'dupFilterTypes');
        if (!scan) field('category','dupFilterCategory',[['all','dupAllCategories'],['same','dupSameNames'],['different','dupDifferentNames']]);
        disclosure.appendChild(box); container.appendChild(disclosure);
    }

    function renderStats(container, groups, selection, visibleCount, notice = '') {
        let box = container.querySelector('.dup-stats');
        if (!box) {
            box=document.createElement('div'); box.className='dup-stats'; box.setAttribute('role','status');
            for (const key of ['bytes','copies','groups']) {
                const metric=document.createElement('div'); metric.className=`dup-metric dup-metric-${key}`;
                const value=document.createElement('strong'); value.className='dup-metric-value';
                const label=document.createElement('span'); label.className='dup-metric-label'; label.textContent=t({bytes:'dupEligibleLabel',copies:'dupCopiesLabel',groups:'dupGroupsLabel'}[key]); metric.append(value,label); box.appendChild(metric);
            }
            const detail=document.createElement('div'); detail.className='dup-stats-detail'; box.appendChild(detail);
            container.appendChild(box);
        }
        const stats = discRevealDuplicateStats(groups, selection);
        for (const key of ['bytes','copies','groups']) box.querySelector(`.dup-metric-${key}`).querySelector('.dup-metric-value').textContent = key === 'bytes' ? formatBytes(stats.eligibleBytes) : formatNumber(stats[key]);
        box.querySelector('.dup-stats-detail').textContent = (notice || (stats.selected ? t('dupSelectionStats',{count:formatNumber(stats.selected),size:formatBytes(stats.selectedBytes)}) : t('dupVisibleGroups',{count:formatNumber(visibleCount)}))) + ' · ' + t('dupFoundBytes', { size: formatBytes(stats.bytes) }) + (groups.some(group => group.files.some(file => file.reclaimableBytes === null)) ? ' · ' + t('dupStorageUnknown') : '');
    }

    /**
     * Renders `groups` into `container`. `selectedPaths` is a `Set` of file
     * paths currently checked. `cancelled` is true when the search behind
     * these groups was stopped early by the user — `groups` is then whatever
     * had already been confirmed at that point, not necessarily everything
     * the selected drives contain, and the summary wording says so instead
     * of implying a completed search. Callbacks: `onToggleSelect(path)`,
     * `onOpenPath(path)`, `onKeepOnlyOldest(group)` (selects every file in the
     * group except the oldest one).
     */
    function renderDuplicates(container, groups, selectedPaths, { cancelled, ...callbacks }) {
        if (!container.querySelector('.dup-results')) container.replaceChildren();
        renderResultPanel(container, groups, selectedPaths, {...callbacks, cancelled}, false);
    }

    /** Appends only new confirmed groups, leaving the live progress elements untouched. */
    function renderLiveGroups(container, groups, onOpenPath, changedGroups = groups, options = {}) {
        renderResultPanel(container, groups, new Set(), {...options, onOpenPath}, true);
    }

    function renderResultPanel(container, groups, selection, options, live) {
        const filter = options.filter || {};
        const filtered = filterGroups(groups, filter);
        const visible = filtered.slice(0, options.limit || 50);
        renderStats(container, groups, selection, filtered.length, options.notice);
        let tools = container.querySelector('.dup-tools');
        if (!tools && options.onFilterChange) {
            tools = document.createElement('div'); tools.className='dup-tools';
            renderFilterControls(tools, filter, options.onFilterChange);
            const actions=document.createElement('div'); actions.className='dup-bulk-actions';
            for (const [key, label] of [['onSelectAll','dupSelectAll'],['onClearSelection','dupClearSelection'],['onClean','dupClean']]) {
                const button=document.createElement('button'); button.type='button'; button.className='pill-btn pill-btn-ghost';
                button.dataset.action=key; button.textContent=t(label); if (key==='onClean') button.className='pill-btn pill-btn-primary'; button.onclick=()=>tools._options[key]?.(); actions.appendChild(button);
            }
            const policy=document.createElement('details'); policy.className='dup-policy';
            const hintTitle=document.createElement('summary'); hintTitle.textContent=t('dupSafetyShort');
            const hint=document.createElement('p'); hint.textContent=t('dupCleanupPolicy'); policy.append(hintTitle,hint);
            tools.appendChild(actions); container.appendChild(tools); container.appendChild(policy);
        }
        if (tools) { tools._options = options; tools.querySelector('.dup-bulk-actions').classList.toggle('hidden', live); }
        if (tools) for (const button of tools.querySelectorAll('button')) {
            button.disabled = live || options.busy || (button.dataset.action === 'onClean' && selection.size === 0);
        }
        let results = container.querySelector(live ? '.dup-live-results' : '.dup-results');
        if (!results) {
            results = document.createElement('div');
            results.className = live ? 'dup-live-results' : 'dup-results';
            results._cards = new Map();
            container.appendChild(results);
        }
        const keys = new Set(visible.map(group=>group.files[0].path));
        for (const [key, previous] of results._cards) if (!keys.has(key)) { previous.card.remove(); results._cards.delete(key); }
        for (const group of visible) {
            const key = group.files[0].path;
            const previous = results._cards.get(key);
            if (live && previous?.group === group) continue;
            const restore = previous && window.DiscRevealUtil.preserveFocus?.(previous.card);
            const card = renderGroup(group, selection, {...options,readOnly:live || options.busy, expanded: Boolean(previous?.card.open), fileLimit: previous?.card._fileLimit || 25});
            if (previous) previous.card.replaceWith(card);
            else results.appendChild(card);
            results._cards.set(key, { group, card });
            restore?.(card);
        }
        let footer=container.querySelector('.dup-results-footer');
        if (!footer) { footer=document.createElement('div'); footer.className='dup-results-footer'; container.appendChild(footer); }
        footer.replaceChildren();
        if (options.cancelled) { const hint=document.createElement('p'); hint.textContent=t('dupCancelledSummary',{groups:formatNumber(groups.length),size:formatBytes(wastedBytes(groups))}); footer.appendChild(hint); }
        if (!filtered.length) { const hint=document.createElement('p'); hint.textContent=t(groups.length ? 'dupNoFilterMatches' : live ? 'dupWaitingMatches' : 'dupNoneFound'); footer.appendChild(hint); }
        if (filtered.length > visible.length) { const more=document.createElement('button'); more.type='button'; more.className='link-btn'; more.textContent=t('dupMoreGroups',{shown:formatNumber(visible.length),total:formatNumber(filtered.length)}); more.onclick=()=>options.onMore?.(); footer.appendChild(more); }
    }

    window.DiscRevealDuplicates = Object.freeze({ renderDrivePicker, renderProgress, renderDuplicates, renderLiveGroups, oldestFile, totalUsedBytes, filterGroups, reclaimableBytes, duplicateStats:discRevealDuplicateStats, suggestedPaths, extensions });
})();
