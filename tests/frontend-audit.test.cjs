const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { createDocument } = require('./helpers/audit-dom.cjs');
const source = name => fs.readFileSync(path.join(__dirname, '../renderer', name), 'utf8');
const app = source('app.js');
function fn(name) {
    const start = app.indexOf(`function ${name}(`), opening = app.indexOf('{', start); let depth = 0;
    for (let index = opening; index < app.length; index++) {
        if (app[index] === '{') depth++;
        if (app[index] === '}' && --depth === 0) return app.slice(app.slice(start - 6, start) === 'async ' ? start - 6 : start, index + 1);
    }
    throw Error('Missing function: ' + name);
}
function views(...files) {
    const document = createDocument();
    const window = { DiscRevealI18n: { t: (key, params) => key + (params ? ':' + JSON.stringify(params) : ''), nodeName: node => node.name, formatNumber: String, formatDecimal: value => Number(value).toFixed(1), formatDate: String }, discreveal: {} };
    const context = { window, document, requestAnimationFrame: action => action() };
    vm.runInNewContext(source('util.js'), context);
    for (const file of files) vm.runInNewContext(source(file), context);
    const root = document.createElement('div'); document.body.appendChild(root);
    return { document, window, root, context };
}

test('rejected scan cancellation preserves accepted scan identity and restores cancellation controls', async () => {
    const state = { scanning: true, scanCancelling: false, activeScanId: 7 }, errors = [];
    const context = { state, el: { btnCancel: {}, progressCancel: {}, progressText: {} }, t: key => key, describeError: String, showError: error => errors.push(error), endScanUi: () => assert.fail('scan must remain active'), window: { discreveal: { cancelScan: async () => { throw Error('transport'); } } } };
    vm.runInNewContext(fn('cancelActiveScan'), context); await context.cancelActiveScan();
    assert.equal(state.activeScanId, 7); assert.equal(state.scanning, true); assert.equal(state.scanCancelling, false);
    assert.equal(context.el.btnCancel.disabled, false); assert.deepEqual(errors, ['cancelFailed']);
});

test('a completed scan wins a race with a late cancellation rejection', async () => {
    let reject; const state = { scanning: true, activeScanId: 7 };
    const context = { state, el: { btnCancel: {}, progressCancel: {}, progressText: {} }, t: key => key, describeError: String, showError: () => assert.fail('late error must not overwrite completed result'), endScanUi: () => assert.fail('already completed'), window: { discreveal: { cancelScan: () => new Promise((_, reject_) => { reject = reject_; }) } } };
    vm.runInNewContext(fn('cancelActiveScan'), context); const pending = context.cancelActiveScan();
    state.activeScanId = null; state.scanning = false; reject(Error('late')); await pending;
    assert.equal(state.activeScanId, null); assert.equal(state.scanning, false);
});

test('failed startup retains safe settings and gives an explicit retry without saving defaults', async () => {
    const prefix = app.slice(app.indexOf('const state = {'), app.indexOf('\n};', app.indexOf('const state = {')) + 3);
    const context = { navigator: { language: 'en-US' }, document: { body: { classList: { remove() {} } } }, el: { appVersion: {} }, t: key => key, describeError: String, showError() {}, showInitRetry() { context.retries++; }, applyLanguage(language) { context.state.settings.language = language; }, retries: 0,
        window: { discreveal: { getVersion: async () => '0.3.11', loadSettings: async () => { throw Error('unreadable settings'); }, listDrives: async () => [] } } };
    vm.runInNewContext(prefix + '\nthis.state = state;\n' + fn('init'), context);
    await context.init(); assert.equal(context.state.settings.language, 'en'); assert.deepEqual([...context.state.settings.excludes], []);
    assert.equal(context.state.settings.cacheHashesEnabled, false); assert.equal(context.state.initialized, false); assert.equal(context.retries, 1);
});

test('search reports all matching names separately from the 200 rendered largest results', () => {
    const view = views('search.js');
    const tree = { children: Array.from({ length: 250 }, (_, index) => ({ name: 'match-' + index, path: 'C:\\' + index, size: index, isDir: false })) };
    const results = view.window.DiscRevealSearch.searchTree(tree, 'match');
    assert.equal(results.total, 250); assert.equal(results.length, 200); assert.equal(results[0].size, 249);
    view.window.DiscRevealSearch.renderSearchResults(view.root, results, () => {});
    assert.ok(view.root.querySelector('.search-summary').textContent.includes('"total":"250"'));
    assert.ok(view.root.querySelectorAll('p').some(node => node.textContent === 'searchLimitHint'));
});

test('active live search is refreshed on partial and final scan events', () => {
    const actions = {};
    const state = { activeScanId: 7, scanning: true, scannedDrive: { path: 'C:\\' }, activeScanSettingsKey: 'x' };
    let searches = 0;
    const context = { state, scanCache: new Map(), scanSnapshots: { merge: tree => tree }, adoptTree: tree => { state.fullTree = tree; }, endScanUi() {}, updateScanCoverage() {}, renderCurrentLevel() {}, refreshActiveSearch: immediate => { searches += immediate ? 10 : 1; }, isCurrentScan: id => id === state.activeScanId,
        window: { discreveal: { onScanPartial: action => { actions.partial = action; }, onScanDone: action => { actions.done = action; } } } };
    const start = app.indexOf('window.discreveal.onScanPartial('), end = app.indexOf('window.discreveal.onScanError(', start);
    vm.runInNewContext(app.slice(start, end), context);
    actions.partial({ scanId: 7, tree: {} }); actions.done({ scanId: 7, tree: {} }); assert.equal(searches, 11);
});

test('duplicate selection preserves focused copy and its 75-file page after rebuilding', () => {
    const view = views('duplicates.js'), { renderDuplicates } = view.window.DiscRevealDuplicates;
    const group = { size: 1024 * 1024, sampleHash: 'a', files: Array.from({ length: 80 }, (_, index) => ({ path: 'C:\\copy' + index, name: 'copy' + index, modified: index + 1, cleanupEligible: true })) };
    const selection = new Set(), options = { cancelled: false, onToggleSelect: file => { selection.add(file); renderDuplicates(view.root, [group], selection, options); }, onOpenPath() {}, onKeepOnlyOldest() {} };
    renderDuplicates(view.root, [group], selection, options);
    let card = view.root.querySelector('.dup-group'); card.open = true; card.ontoggle();
    while (card._fileLimit < 75) card.querySelector('.link-btn').onclick();
    const checkbox = card.querySelectorAll('input').find(node => node.dataset.focusKey === 'file-C:\\copy40');
    checkbox.focus(); checkbox.checked = true; checkbox.onchange();
    card = view.root.querySelector('.dup-group');
    assert.equal(card._fileLimit, 75); assert.equal(card.querySelectorAll('.dup-file-row').length, 75);
    assert.equal(view.document.activeElement.dataset.focusKey, 'file-C:\\copy40'); assert.equal(view.document.activeElement.checked, true);
});

test('selectable duplicate potential excludes protected copies and originals', () => {
    const view = views('duplicates.js');
    const stats = view.window.DiscRevealDuplicates.duplicateStats([{ size: 100, files: [{ path: 'original', modified: 1 }, { path: 'personal', modified: 2, cleanupEligible: true }, { path: 'system', modified: 3, cleanupEligible: false }] }]);
    assert.equal(stats.bytes, 200); assert.equal(stats.eligibleBytes, 100);
});

test('cooperative duplicate cleanup stop leaves current and later copies selected and processes no next file', async () => {
    const keeper = { path: 'original', modified: 1 }, files = [keeper, { path: 'first', modified: 2, cleanupEligible: true }, { path: 'later', modified: 3, cleanupEligible: true }];
    const group = { size: 10, files }, state = { duplicatesPhase: 'results', duplicatesResultId: 7, duplicateGroups: [group], duplicateSelection: new Set(['first', 'later']) };
    let deleteCalls = 0;
    const context = { state, oldestFile: () => keeper, duplicatesScope: () => ({ kind: 'duplicates', id: 7 }), t: key => key, formatNumber: String, formatBytes: String, escapeHtml: String, plainText: String, describeError: String, showError() {}, updateBusyUi() {}, renderDuplicatesView() {}, renderCurrentLevel() {}, removeItemFromTree() {},
        window: { DiscRevealDuplicates: views('duplicates.js').window.DiscRevealDuplicates, DiscRevealModal: { confirm: async () => true }, discreveal: { getProtection: async () => null, deleteDuplicate: async () => { deleteCalls++; throw { code: 'duplicateCleanupCancelled' }; } } } };
    vm.runInNewContext(fn('cleanSelectedDuplicates'), context); await context.cleanSelectedDuplicates();
    assert.equal(deleteCalls, 1); assert.equal(group.files.length, 3); assert.equal(state.duplicateSelection.size, 2);
    assert.ok(state.duplicatesCleanupNotice.includes('dupCleanupStopped'));
});

test('duplicate savings use allocated bytes and never count unknown or multiply linked copies as logical savings', () => {
    const { duplicateStats } = views('duplicates.js').window.DiscRevealDuplicates;
    const group = { size: 1024 * 1024, files: [
        { path: 'original', modified: 1, cleanupEligible: true, reclaimableBytes: 4096 },
        { path: 'compressed', modified: 2, cleanupEligible: true, reclaimableBytes: 8192 },
        { path: 'shared', modified: 3, cleanupEligible: false, reclaimableBytes: 0 },
        { path: 'unknown', modified: 4, cleanupEligible: true, reclaimableBytes: null }
    ] };
    const stats = duplicateStats([group], new Set(['original', 'compressed', 'shared', 'unknown']));
    assert.equal(stats.bytes, 8192); assert.equal(stats.eligibleBytes, 8192);
    assert.equal(stats.selectedBytes, 8192); assert.equal(stats.copies, 3);
});

test('allocated scan rows expose logical length and unknown measurement instead of presenting zero as measured', () => {
    const view = views();
    const item = { path: 'C:\\unknown.bin', name: 'unknown.bin', size: 0, logicalSize: 8193, allocationUnknown: true };
    assert.equal(view.window.DiscRevealUtil.storageSizeLabel(item), 'storageUnknown');
    assert.ok(view.window.DiscRevealUtil.storageSizeDescription(item).includes('8.0 KB'));
    assert.ok(view.window.DiscRevealUtil.storageSizeDescription(item).includes('storageUnknown'));
    assert.ok(source('list.js').includes('storageSizeLabel(item)'));
    assert.ok(source('grid.js').includes('storageSizeLabel(item)'));
});

test('common lifecycle failures reset only the matching read operation and preserve newer results', () => {
    const state = { activeScanId: 4, activeDuplicatesScanId: 7, duplicatesPhase: 'scanning' };
    let scanEnds = 0, renders = 0, errors = 0;
    const context = { state, duplicatesElapsedTimer: null, isCurrentScan: id => id === state.activeScanId,
        endScanUi() { scanEnds++; }, renderDuplicatesView() { renders++; }, updateBusyUi() {},
        showError() { errors++; }, t: String, clearInterval() {} };
    const lifecycleStart = app.indexOf('function handleOperationStatus(');
    const lifecycleEnd = app.indexOf('\nwindow.discreveal.onOperationStatus', lifecycleStart);
    vm.runInNewContext(app.slice(lifecycleStart, lifecycleEnd), context);
    context.handleOperationStatus({ kind: 'scan', requestId: 3, status: 'abandoned' });
    assert.equal(scanEnds, 0);
    context.handleOperationStatus({ kind: 'scan', requestId: 4, status: 'abandoned' });
    assert.equal(scanEnds, 1); assert.equal(state.activeScanId, null);
    context.handleOperationStatus({ kind: 'duplicates', requestId: 7, status: 'failed' });
    assert.equal(state.duplicatesPhase, 'results'); assert.equal(state.activeDuplicatesScanId, null);
    assert.equal(state.duplicatesWereCancelled, true); assert.equal(renders, 1);
    context.handleOperationStatus({ kind: 'duplicates', requestId: 7, status: 'abandoned' });
    assert.equal(errors, 2); assert.equal(renders, 1);
});

test('folded-only charts explain known bytes and zero-byte charts keep a real zero total', () => {
    const view = views('charts.js');
    view.window.DiscRevealCharts.renderCharts(view.root, { children: [{ path: '', size: 100, overflowCount: 10 }] }, () => {});
    assert.ok(view.root.querySelector('.charts-fold-note').textContent.includes('"count":"10"'));
    view.window.DiscRevealCharts.renderCharts(view.root, { children: [{ path: 'C:\\zero.txt', name: 'zero.txt', size: 0, isDir: false }] }, () => {});
    assert.ok(view.root.querySelector('.chart-donut-center').innerHTML.includes('0 B'));
});

test('chart legends expose counts and percentages without hover and are keyboard focusable', () => {
    const view = views('charts.js');
    view.window.DiscRevealCharts.renderCharts(view.root, { children: [{ path: 'C:\\a.txt', name: 'a.txt', size: 100, modified: 1 }] }, () => {});
    const legend = view.root.querySelector('.chart-legend-row');
    assert.equal(legend.tabIndex, 0); assert.ok(legend.getAttribute('aria-label').includes('100.0%'));
    assert.ok(legend.querySelector('.chart-legend-value').textContent.includes(' · 1 '));
});

test('tree exposes expandable buttons and returns keyboard focus after toggling', () => {
    const view = views('tree.js'), paths = new Set(['C:\\']);
    const tree = { path: 'C:\\', name: 'C:\\', size: 100, children: [{ path: 'C:\\child', name: 'child', size: 100, isDir: true }] };
    const options = { expandedPaths: paths, onSelect() {}, onToggle: path => { paths.delete(path); view.window.DiscRevealTree.renderTree(view.root, tree, options); } };
    view.window.DiscRevealTree.renderTree(view.root, tree, options);
    const toggle = view.root.querySelector('button'); toggle.focus(); toggle.onclick({ stopPropagation() {} });
    assert.equal(view.document.activeElement.dataset.focusKey, 'tree-toggle-C:\\'); assert.equal(view.document.activeElement.getAttribute('aria-expanded'), 'false');
});

test('Danger button text contrast stays above 4.5:1 in normal and hover states', () => {
    const css = source('style.css');
    const luminance = hex => { const rgb = hex.match(/[0-9a-f]{2}/gi).map(pair => parseInt(pair, 16) / 255).map(value => value <= .04045 ? value / 12.92 : ((value + .055) / 1.055) ** 2.4); return rgb[0] * .2126 + rgb[1] * .7152 + rgb[2] * .0722; };
    for (const selector of ['.pill-btn-danger', '.pill-btn-danger:hover']) {
        const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
        const color = new RegExp(escaped + ' \\{\\s*background: (#[0-9a-f]+)', 'i').exec(css)[1];
        assert.ok(1.05 / (luminance(color.slice(1)) + .05) >= 4.5);
    }
});
