/* discReveal by markMarkinson: check everything supported, choose broad areas, confirm once. */
(function () {
    'use strict';
    const { t, describeError, formatNumber } = window.DiscRevealI18n;
    const { formatBytes, escapeHtml } = window.DiscRevealUtil;
    const model = { rules: [], groups: [], bins: [], phase: 'idle', planId: null, loaded: false,
        selected: new Set(), open: new Set(), cancelling: false, checked: 0, notice: '', limited: [], issues: {}, completed: false, loadError: '' };
    const system = { result: null, progress: null };
    let ui, loading, progressTimer = null, progressStarted = 0;
    const categories = ['windows', 'browsers', 'apps', 'shaders', 'diagnostics', 'system', 'rollback', 'recycle'];
    const categoryOf = rule => rule.category === 'development' ? 'apps' : rule.category || 'apps';
    const rollbackId = id => ['update', 'previous'].includes(id);
    function node(tag, className, text) {
        const element = document.createElement(tag); element.className = className || '';
        if (text !== undefined) element.textContent = text; return element;
    }
    function button(label, onClick, primary = false) {
        const element = node('button', 'pill-btn ' + (primary ? 'pill-btn-primary' : 'pill-btn-ghost'), t(label));
        element.type = 'button'; element.dataset.focusKey = label; element.onclick = onClick; return element;
    }
    function isBusy() { return ['analyzing', 'system-scan', 'cleaning', 'system-clean', 'recycling'].includes(model.phase); }
    function areas() {
        return categories.map(id => {
            const rules = model.rules.filter(rule => rule.available && categoryOf(rule) === id);
            const cache = model.groups.filter(group => group.count > 0 && rules.some(rule => rule.id === group.id && !rule.blocked));
            const native = (system.result?.groups || []).filter(group => (id === 'rollback' ? rollbackId(group.id) : id === 'system' && !rollbackId(group.id)));
            const ready = native.filter(group => group.status === 'ready' && group.size > 0);
            const bins = id === 'recycle' ? model.bins.filter(bin => bin.count > 0) : [];
            return { id, rules, cache, native, ready, bins,
                size: cache.reduce((sum, group) => sum + group.allocated, 0) + ready.reduce((sum, group) => sum + group.size, 0) + bins.reduce((sum, bin) => sum + bin.size, 0),
                available: cache.length + ready.length + bins.length > 0 };
        }).filter(area => area.rules.length || area.native.length || area.bins.length || area.id === 'system' && !system.result);
    }
    function title(id) { return t(id === 'system' ? 'cleanupNativeTitle' : id === 'rollback' ? 'cleanupSimpleRollback' : id === 'recycle' ? 'cleanupRecycleBin' : 'cleanupSimple_' + id); }
    function selectedAreas() { return areas().filter(area => area.available && model.selected.has(area.id)); }
    function selectionSize() { return selectedAreas().reduce((sum, area) => sum + area.size, 0); }
    async function loadRules() {
        if (loading) return loading;
        loading = Promise.all([window.discreveal.cleanupRules(), window.discreveal.cleanupRecycleBins()]).then(([rules, bins]) => {
            model.rules = rules; model.bins = bins; model.loaded = true; model.loadError = '';
        }).finally(() => { loading = null; });
        return loading;
    }
    function renderControls() {
        ui.controls.replaceChildren();
        const check = button(model.loadError ? 'retry' : 'cleanupCheckEverything', model.loadError ? refresh : analyze, true); check.disabled = isBusy() || ui.otherBusy() || (!model.loaded && !model.loadError);
        ui.controls.appendChild(check);
        if (model.phase !== 'results') return;
        const clean = button('cleanupClean', cleanSelected, true); clean.textContent = t('cleanupCleanSize', { size: formatBytes(selectionSize()) });
        clean.disabled = isBusy() || ui.otherBusy() || !selectedAreas().length || model.completed;
        ui.controls.appendChild(clean);
    }
    function updateProgress() {
        let progress = ui.body.querySelector('.cleanup-progress');
        if ((model.loaded || model.loadError) && !isBusy()) {
            progress?.remove();
            if (progressTimer !== null) clearInterval(progressTimer);
            progressTimer = null;
            return;
        }
        if (!progress) {
            progress = window.DiscRevealProgress.create('cleanup-progress', cancelWork);
            ui.body.insertBefore(progress, ui.body.querySelector('.cleanup-content'));
            progressStarted = Date.now();
            progressTimer = setInterval(() => {
                if (ui.active()) updateProgress();
                else if ((model.loaded || model.loadError) && !isBusy()) {
                    clearInterval(progressTimer); progressTimer = null;
                }
            }, 1000);
        }
        const systemBusy = model.phase.startsWith('system-');
        const label = !model.loaded ? 'cleanupLoading' : model.cancelling ? 'cleanupCancelling'
            : systemBusy ? model.phase === 'system-clean' ? 'cleanupSystemCleaning' : 'cleanupSystemChecking'
            : model.phase === 'native' ? 'cleanupNativeRunning' : model.phase === 'registering' ? 'cleanupPickingProject'
            : model.phase === 'recycling' ? 'cleanupEmptyingBin' : model.phase === 'cleaning' ? 'cleanupCleaning' : 'cleanupAnalyzing';
        window.DiscRevealProgress.update(progress, {
            status: t(label, { count: formatNumber(model.checked) }),
            detail: systemBusy && system.progress ? t('cleanupSystem_' + system.progress.groupId) + ' · ' + formatBytes(system.progress.bytes) : '',
            elapsed: t('dupElapsed', { seconds: formatNumber(Math.floor((Date.now() - progressStarted) / 1000)) }),
            cancelling: model.cancelling,
            cancelVisible: model.loaded && !['recycling', 'native', 'registering'].includes(model.phase),
        });
    }
    function renderBody() {
        let body = ui.body.querySelector('.cleanup-content');
        if (!body) {
            const intro = node('div', 'cleanup-intro'); intro.append(node('h2'), node('p'));
            body = node('div', 'cleanup-content'); ui.body.replaceChildren(intro, body);
        }
        const intro = ui.body.querySelector('.cleanup-intro');
        intro.querySelector('h2').textContent = t('cleanupTitle'); intro.querySelector('p').textContent = t('cleanupSimpleIntro');
        updateProgress(); body.replaceChildren();
        if (model.loadError) { const error = node('p', 'cleanup-notice', t('cleanupLoadFailed', { detail: model.loadError })); error.setAttribute('role', 'alert'); body.appendChild(error); }
        if (!model.loaded) return;
        if (model.notice) {
            const notice = node('p', 'cleanup-notice', typeof model.notice === 'function' ? model.notice() : model.notice);
            notice.setAttribute('role', 'status'); body.appendChild(notice);
        }
        const analyzed = model.phase === 'results';
        if (analyzed && !model.completed) {
            const stats = node('div', 'cleanup-stats'); stats.append(node('strong', '', formatBytes(selectionSize())), node('span', '', t('cleanupSelectedPotential'))); body.appendChild(stats);
        }
        const cards = node('div', 'cleanup-categories cleanup-simple-areas'); body.appendChild(cards);
        for (const area of areas()) {
            if (analyzed && !area.available && !area.native.some(group => group.status === 'admin') && !area.rules.some(rule => rule.blocked)) continue;
            const host = node('details', 'cleanup-section' + (analyzed && model.selected.has(area.id) ? ' cleanup-section-selected' : ''));
            host.dataset.category = area.id; host.open = model.open.has(area.id);
            host.ontoggle = () => { if (host.open) model.open.add(area.id); else model.open.delete(area.id); };
            const summary = node('summary', 'cleanup-section-summary');
            summary.dataset.focusKey = 'area-summary-' + area.id;
            if (analyzed && !model.completed) {
                const check = node('input', 'cleanup-category-check'); check.type = 'checkbox'; check.setAttribute('aria-label', title(area.id));
                check.dataset.focusKey = 'area-check-' + area.id;
                check.checked = model.selected.has(area.id); check.disabled = !area.available || isBusy() || ui.otherBusy();
                check.onclick = event => event.stopPropagation();
                check.onchange = () => { if (isBusy() || ui.otherBusy()) return; if (check.checked) model.selected.add(area.id); else model.selected.delete(area.id); render(); };
                summary.appendChild(check);
            }
            const label = node('span', 'cleanup-section-label'); label.append(node('strong', '', title(area.id)), node('span', '', t('cleanupAreaHint_' + area.id)));
            summary.appendChild(label);
            if (analyzed) summary.appendChild(node('span', 'cleanup-section-total', area.available ? formatBytes(area.size) : t('cleanupUnavailable')));
            summary.appendChild(node('span', 'cleanup-chevron')); host.appendChild(summary); cards.appendChild(host);
            const details = node('div', 'cleanup-area-details'); host.appendChild(details);
            for (const rule of area.rules) {
                const group = area.cache.find(group => group.id === rule.id);
                const row = node('div', 'cleanup-area-rule');
                row.append(node('strong', '', t(rule.label)), node('span', 'cleanup-muted', rule.blocked ? t('cleanupCloseApp') : analyzed ? group ? formatBytes(group.allocated) : t('cleanupNoFiles') : t('cleanupMinimumAge', { days: rule.minDays })));
                row.appendChild(node('p', 'cleanup-muted', t(rule.effect)));
                if (model.limited.includes(rule.id)) row.appendChild(node('p', 'cleanup-muted', t('cleanupAreaLimited')));
                if (model.issues[rule.id]) row.appendChild(node('p', 'cleanup-muted', t('cleanupAreaSkipped', { count: formatNumber(model.issues[rule.id]) })));
                details.appendChild(row);
            }
            for (const group of area.native) {
                details.appendChild(node('p', 'cleanup-muted', t('cleanupSystem_' + group.id) + ' · ' + (group.status === 'ready' ? formatBytes(group.size) : t('cleanupSystemStatus_' + group.status))));
                if (rollbackId(group.id)) details.appendChild(node('p', 'cleanup-muted', t('cleanupSystemEffect_' + group.id)));
            }
            for (const bin of area.bins) details.appendChild(node('p', 'cleanup-muted', bin.drive + ' · ' + formatBytes(bin.size)));
        }
        if (system.result?.groups.some(group => group.status === 'admin')) {
            const admin = node('div', 'cleanup-admin-note'); admin.appendChild(node('span', 'cleanup-muted', t('cleanupSystemAdminHint')));
            const restart = button('cleanupSystemAdmin', restartAdmin); restart.disabled = isBusy() || ui.otherBusy(); admin.appendChild(restart); body.appendChild(admin);
        }
        body.appendChild(node('p', 'cleanup-footnote', t('cleanupSimplePolicy')));
    }
    function render() { ui?.onBusyChange?.(); if (!ui?.active()) return; const restoreBody = window.DiscRevealUtil.preserveFocus?.(ui.body), restoreControls = window.DiscRevealUtil.preserveFocus?.(ui.controls); renderControls(); renderBody(); restoreBody?.(); restoreControls?.(); }
    async function analyze() {
        if (isBusy() || ui.otherBusy()) return;
        model.groups = []; model.selected.clear(); model.planId = null; system.result = null; system.progress = null;
        model.completed = false; model.limited = []; model.issues = {}; model.phase = 'analyzing'; model.checked = 0; model.cancelling = false; model.notice = ''; render();
        const started = Date.now(); let nativeError = null;
        try {
            await loadRules();
            if (model.cancelling) return;
            const ids = model.rules.filter(rule => rule.available && !rule.blocked).map(rule => rule.id);
            if (ids.length) {
                const result = await window.discreveal.analyzeCleanup({ ruleIds: ids, minDays: 1 });
                model.planId = result.planId; model.groups = result.groups; model.limited = result.limitedGroups || []; model.issues = result.issues || {};
                model.cancelling = model.cancelling || Boolean(result.cancelled);
            }
            if (!model.cancelling) {
                model.phase = 'system-scan'; render();
                try { system.result = await window.discreveal.analyzeWindowsCleanup(); model.cancelling = model.cancelling || Boolean(system.result.cancelled); }
                catch (error) { nativeError = error; }
            }
            if (!model.cancelling) for (const area of areas()) if (area.available) model.selected.add(area.id);
            const cancelled = model.cancelling;
            const seconds = window.DiscRevealI18n.formatDecimal((Date.now() - started) / 1000);
            model.notice = () => t(cancelled ? 'cleanupPartial' : 'cleanupSimpleChecked', { seconds }) + (nativeError ? '\n' + t('cleanupSystemUnavailable') + ': ' + describeError(nativeError) : '');
        } catch (error) { ui.onError(describeError(error)); model.notice = () => t('cleanupPartial'); }
        finally { model.phase = 'results'; model.cancelling = false; render(); }
    }
    async function cleanSelected() {
        if (isBusy() || ui.otherBusy() || model.completed) return;
        const chosen = selectedAreas(); if (!chosen.length) return;
        const planId = model.planId, systemId = system.result?.planId;
        const cacheIds = chosen.flatMap(area => area.cache.map(group => group.id));
        const nativeIds = chosen.flatMap(area => area.ready.map(group => group.id));
        const bins = chosen.flatMap(area => area.bins);
        const rollback = nativeIds.some(rollbackId);
        const confirmed = await window.DiscRevealModal.confirm({ title: t('cleanupConfirmTitle'), danger: true,
            messageHtml: '<div class="cleanup-confirm-content">' + escapeHtml(t('cleanupSimpleConfirm')) + '<ul>' + chosen.map(area => '<li>' + escapeHtml(title(area.id)) + ' · ' + escapeHtml(formatBytes(area.size)) + '</li>').join('') + '</ul>'
                + (bins.length ? '<p>' + escapeHtml(t('cleanupSimpleBinWarning', { drives: bins.map(bin => bin.drive).join(', ') })) + '</p>' : '')
                + (chosen.some(area => area.id === 'shaders' || area.id === 'apps') ? '<p>' + escapeHtml(t('cleanupSimpleCacheWarning')) + '</p>' : '')
                + (rollback ? '<p>' + escapeHtml(t('cleanupSystemRollbackAck')) + '</p>' : '')
                + (nativeIds.includes('previous') ? '<p>' + escapeHtml(t('cleanupSystemEffect_previous')) + '</p>' : '') + '</div>',
            acknowledgement: t(rollback ? 'cleanupSystemRollbackAck' : 'cleanupAcknowledge'), confirmLabel: t('cleanupClean') });
        if (!confirmed || isBusy() || ui.otherBusy() || model.planId !== planId || system.result?.planId !== systemId) return;
        model.phase = 'cleaning'; model.cancelling = false; model.checked = 0; model.notice = ''; render();
        const messages = []; let removed = false;
        const observed = result => result.observedFreeChange === null || result.observedFreeChange === undefined ? ''
            : ' ' + t('cleanupObserved', { size: (result.observedFreeChange >= 0 ? '+' : '−') + formatBytes(Math.abs(result.observedFreeChange)) });
        try {
            if (cacheIds.length && !model.cancelling) {
                const result = await window.discreveal.executeCleanup(planId, cacheIds, []);
                model.groups = result.remaining; removed = result.removed > 0; model.cancelling = model.cancelling || Boolean(result.cancelled);
                messages.push(() => t('cleanupDone', { count: formatNumber(result.removed), size: formatBytes(result.estimatedFreed), skipped: formatNumber(result.skipped) })
                    + observed(result) + Object.entries(result.reasons || {}).map(([reason, count]) => ' · ' + t('cleanupReason_' + reason) + ': ' + formatNumber(count)).join(''));
            }
            if (nativeIds.length && !model.cancelling) {
                model.phase = 'system-clean'; system.progress = null; render();
                const result = await window.discreveal.executeWindowsCleanup(systemId, nativeIds, rollback);
                removed = removed || result.completed.length > 0; model.cancelling = model.cancelling || Boolean(result.cancelled);
                messages.push(() => t('cleanupSystemDone', { count: formatNumber(result.completed.length), failed: formatNumber(result.failed.length), size: formatBytes(result.reportedFreed) })
                    + observed(result) + result.failed.map(group => ' · ' + t('cleanupSystem_' + group.id) + ': ' + t('cleanupSystemStatus_' + group.status)).join(''));
                // Do not continue into the Recycle Bin after a failed servicing operation.
                if (result.failed.length) model.cancelling = true;
            }
            for (const bin of bins) {
                if (model.cancelling) break;
                model.phase = 'recycling'; render();
                model.bins = await window.discreveal.emptyCleanupRecycleBin(bin.drive); removed = true;
                messages.push(() => t('cleanupBinDone', { drive: bin.drive }));
            }
        } catch (error) { messages.push(() => describeError(error)); ui.onError(describeError(error)); }
        finally {
            const partial = model.cancelling;
            model.notice = () => messages.map(message => message()).join('\n') + (partial ? '\n' + t('cleanupPartial') : '');
            model.selected.clear(); model.completed = true; model.planId = null; system.result = null;
            model.phase = 'results'; model.cancelling = false; if (removed) ui.onRemoved(); render();
        }
    }
    async function cancelWork() {
        if (model.cancelling || !isBusy()) return; model.cancelling = true; renderBody();
        try { await window.discreveal.cancelCleanup(); } catch (error) { model.cancelling = false; ui.onError(describeError(error)); renderBody(); }
    }
    async function restartAdmin() {
        if (isBusy() || ui.otherBusy()) return;
        const confirmed = await window.DiscRevealModal.confirm({ title: t('cleanupSystemAdmin'), messageHtml: escapeHtml(t('cleanupSystemAdminConfirm')), confirmLabel: t('cleanupSystemAdmin') });
        if (!confirmed || isBusy() || ui.otherBusy()) return;
        try { await window.discreveal.restartCleanupAsAdmin(); } catch (error) { ui.onError(describeError(error)); }
    }
    function init(options) {
        ui = options;
        window.discreveal.onWindowsCleanupProgress(payload => {
            if (!model.phase.startsWith('system-') || model.phase === 'system-clean' && system.result?.planId !== payload.planId) return;
            system.progress = payload; if (ui.active()) updateProgress();
        });
        window.discreveal.onCleanupProgress(payload => {
            if (!['analyzing', 'cleaning'].includes(model.phase) || model.planId !== null && payload.planId !== model.planId) return;
            model.checked = payload.checked;
            if (model.phase === 'analyzing') model.groups = payload.groups;
            if (ui.active()) updateProgress();
        });
    }
    async function refresh() {
        if (!ui?.active()) return;
        model.loadError = ''; render();
        if (!isBusy()) try { await loadRules(); render(); } catch (error) { model.loadError = describeError(error); ui.onError(model.loadError); render(); }
    }
    window.DiscRevealCleanup = Object.freeze({ init, refresh, isBusy, isCleaning: () => ['cleaning', 'recycling', 'system-clean'].includes(model.phase) });
})();
