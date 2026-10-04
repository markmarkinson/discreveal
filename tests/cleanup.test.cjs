const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');
const { createDocument } = require('./helpers/audit-dom.cjs');

class Element {
    constructor(tag) { this.style = {}; this.tag = tag; this.children = []; this.dataset = {}; this.className = ''; this.isConnected = true; this.textContent = ''; }
    append(...children) { for (const child of children) this.appendChild(child); }
    appendChild(child) { child.parent = this; this.children.push(child); return child; }
    replaceChildren(...children) { this.children = []; this.append(...children); }
    setAttribute(key, value) { this[key] = value; }
    removeAttribute(key) { delete this[key]; }
    insertBefore(child, before) { child.parent = this; const index = this.children.indexOf(before); this.children.splice(index < 0 ? this.children.length : index, 0, child); }
    remove() { this.parent.children = this.parent.children.filter(child => child !== this); }
    querySelectorAll(selector) {
        const match = node => selector.startsWith('.') ? node.className.split(' ').includes(selector.slice(1)) : node.tag === selector;
        return this.children.flatMap(child => [...(match(child) ? [child] : []), ...child.querySelectorAll(selector)]);
    }
    querySelector(selector) { return this.querySelectorAll(selector)[0] || null; }
}
function fixture() {
    const document = createDocument();
    const body = document.createElement('div'), controls = document.createElement('div'); document.body.append(controls, body);
    let allow = false, executions = [], emptyCalls = [], errors = [], progress, languagePrefix = '';
    let nextCachePlan = 7, nextSystemPlan = 8, cachePlan = null, systemPlan = null;
    const rules = [
        { id: 'chrome', label: 'cleanupChrome', effect: 'cleanupBrowserEffect', category: 'browsers', recommended: true, available: true, blocked: false, minDays: 1 },
        { id: 'temp', label: 'cleanupTemp', effect: 'cleanupTempEffect', category: 'windows', recommended: false, available: true, blocked: false, minDays: 7 },
        { id: 'firefox', label: 'cleanupFirefox', effect: 'cleanupBrowserEffect', category: 'browsers', recommended: true, available: true, blocked: true, minDays: 1 },
    ];
    const groups = [{ id: 'chrome', count: 2, size: 15, allocated: 8192 }, { id: 'temp', count: 1, size: 10, allocated: 4096 }];
    const api = {
        cleanupRules: async () => rules,
        cleanupRecycleBins: async () => [{ drive: 'C:\\', count: 1, size: 500 }],
        analyzeCleanup: async options => { assert.equal(options.minDays, 1); cachePlan = nextCachePlan; nextCachePlan += 2; return { planId: cachePlan, groups, skipped: 1, elapsedMs: 2 }; },
        runWindowsCleanup: async () => {},
        analyzeWindowsCleanup: async drive => { systemPlan = nextSystemPlan; nextSystemPlan += 2; return { planId: systemPlan, drive, elevated: false, groups: [{ id: 'thumbnails', size: 1000, status: 'ready' }, { id: 'update', size: 0, status: 'admin' }] }; },
        executeWindowsCleanup: async id => { assert.equal(id, systemPlan); systemPlan = null; return { completed: ['thumbnails'], failed: [], reportedFreed: 0, observedFreeChange: null }; },
        restartCleanupAsAdmin: async () => {},
        onWindowsCleanupProgress: () => {},
        pickCleanupProject: async () => false,
        onCleanupProgress: callback => { progress = callback; },
        cleanupItems: async () => [{ id: 11, path: 'C:\\cache\\keep.bin', size: 5, allocated: 4096 }, { id: 12, path: 'C:\\cache\\remove.bin', size: 10, allocated: 4096 }],
        executeCleanup: async (...args) => { assert.equal(args[0], cachePlan); cachePlan = null; executions.push(args); return { remaining: [], removed: 1, estimatedFreed: 4096, skipped: 1, reasons: { changed: 1 }, observedFreeChange: 4096 }; },
        emptyCleanupRecycleBin: async drive => { emptyCalls.push(drive); return []; },
        cancelCleanup: async () => {},
    };
    const window = { discreveal: api, DiscRevealI18n: { t: key => languagePrefix + key, describeError: String, formatNumber: String, formatDecimal: String }, DiscRevealUtil: { formatBytes: String, escapeHtml: String }, DiscRevealModal: { confirm: async () => allow } };
    const timers = new Set();
    const context = { window, document, setInterval: callback => { timers.add(callback); return callback; }, clearInterval: callback => timers.delete(callback) };
    vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../renderer/util.js'), 'utf8'), context);
    // Retain fixture formatting while exercising the production focus restoration.
    window.DiscRevealUtil = { ...window.DiscRevealUtil, formatBytes: String, escapeHtml: String };
    vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../renderer/operationProgress.js'), 'utf8'), context);
    vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../renderer/cleanup.js'), 'utf8'), context);
    window.DiscRevealCleanup.init({ controls, body, active: () => true, otherBusy: () => false, onError: error => errors.push(error), onRemoved() {} });
    const find = (host, label) => host.querySelectorAll('button').find(button => button.textContent === label);
    return { document, timers, language: value => { languagePrefix = value; }, window, api, body, controls, find, executions, emptyCalls, errors, allow: value => { allow = value; }, progress: value => progress(value) };
}

const section = (view, id) => view.body.querySelectorAll('.cleanup-section').find(row => row.dataset.category === id);
const toggle = (view, id, checked) => { const input = section(view, id).querySelector('input'); input.checked = checked; input.onchange(); };
const check = view => view.find(view.controls, 'cleanupCheckEverything').onclick();
const clean = view => view.find(view.controls, 'cleanupCleanSize').onclick();

test('one check covers all supported areas and presets broad results without deleting anything', async () => {
    const view = fixture(); await view.window.DiscRevealCleanup.refresh();
    assert.equal(view.body.querySelectorAll('input').length, 0);
    assert.equal(view.controls.querySelectorAll('button').length, 1);
    let options, nativeChecks = 0;
    const original = view.api.analyzeCleanup; view.api.analyzeCleanup = async value => { options = value; return original(value); };
    view.api.analyzeWindowsCleanup = async drive => { assert.equal(drive, undefined); nativeChecks++; return { planId: 8, groups: [{ id: 'thumbnails', status: 'ready', size: 1000 }] }; };
    await check(view);
    assert.deepEqual(Array.from(options.ruleIds), ['chrome', 'temp']); assert.equal(nativeChecks, 1);
    assert.equal(view.executions.length, 0); assert.equal(view.emptyCalls.length, 0);
    assert.equal(section(view, 'browsers').querySelector('input').checked, true);
    assert.equal(section(view, 'windows').querySelector('input').checked, true);
    assert.equal(view.find(view.controls, 'cleanupCleanSize').disabled, false);
    await clean(view); assert.equal(view.executions.length, 0);
});

test('coarse opt-outs exclude whole areas from the single confirmed operation', async () => {
    const view = fixture(); await view.window.DiscRevealCleanup.refresh(); await check(view);
    toggle(view, 'browsers', false); toggle(view, 'system', false); toggle(view, 'recycle', false);
    let systemCalls = 0; view.api.executeWindowsCleanup = async () => { systemCalls++; };
    view.allow(true); await clean(view);
    assert.deepEqual(Array.from(view.executions[0][1]), ['temp']); assert.deepEqual(Array.from(view.executions[0][2]), []);
    assert.equal(systemCalls, 0); assert.equal(view.emptyCalls.length, 0);
});

test('expanded area details are read-only and retain only one checkbox per broad area', async () => {
    const view = fixture(); await view.window.DiscRevealCleanup.refresh(); await check(view);
    const count = view.body.querySelectorAll('input').length;
    const browsers = section(view, 'browsers'); browsers.open = true; browsers.ontoggle();
    await view.window.DiscRevealCleanup.refresh();
    assert.equal(section(view, 'browsers').open, true);
    assert.equal(section(view, 'browsers').querySelectorAll('input').length, 1);
    assert.equal(view.body.querySelectorAll('input').length, count);
    assert.equal(view.body.querySelectorAll('.cleanup-item-check').length, 0);
    assert.equal(view.controls.querySelectorAll('select').length, 0);
});

test('shader and developer caches are checked together without a second scan or hidden selection', async () => {
    const view = fixture(), original = view.api.cleanupRules;
    view.api.cleanupRules = async () => [...await original(), { id: 'shader', category: 'shaders', available: true, minDays: 30 }, { id: 'npm', category: 'development', available: true, minDays: 30 }];
    await view.window.DiscRevealCleanup.refresh();
    let options; const analyze = view.api.analyzeCleanup;
    view.api.analyzeCleanup = async value => { options = value; const result = await analyze(value); return { ...result, groups: ['shader', 'npm'].map(id => ({ id, count: 1, size: 10, allocated: 4096 })) }; };
    await check(view); assert.equal(options.minDays, 1);
    assert.ok(Array.from(options.ruleIds).includes('shader')); assert.ok(Array.from(options.ruleIds).includes('npm'));
    assert.equal(section(view, 'shaders').querySelector('input').checked, true);
    assert.equal(section(view, 'apps').querySelector('input').checked, true);
    toggle(view, 'shaders', false); toggle(view, 'system', false); toggle(view, 'recycle', false);
    view.allow(true); await clean(view); assert.deepEqual(Array.from(view.executions[0][1]), ['npm']);
});

test('a single confirmation executes cache, Windows and Recycle Bin work in order', async () => {
    const view = fixture(); await view.window.DiscRevealCleanup.refresh(); await check(view);
    const sequence = []; let modalCount = 0;
    view.window.DiscRevealModal.confirm = async () => { modalCount++; return true; };
    const execute = view.api.executeCleanup; view.api.executeCleanup = async (...args) => { sequence.push('cache'); return execute(...args); };
    view.api.executeWindowsCleanup = async (...args) => { sequence.push('system'); assert.equal(args[0], 8); assert.deepEqual(Array.from(args[1]), ['thumbnails']); assert.equal(args[2], false); return { completed: ['thumbnails'], failed: [], reportedFreed: 1000 }; };
    view.api.emptyCleanupRecycleBin = async drive => { sequence.push('recycle'); assert.equal(drive, 'C:\\'); return []; };
    await clean(view); assert.equal(modalCount, 1); assert.deepEqual(sequence, ['cache', 'system', 'recycle']);
    assert.equal(view.find(view.controls, 'cleanupCleanSize').disabled, true);
});

test('rollback remains a separate coarse area and requires its explicit acknowledgement', async () => {
    const view = fixture();
    view.api.analyzeWindowsCleanup = async () => ({ planId: 8, groups: [{ id: 'thumbnails', size: 1000, status: 'ready' }, { id: 'update', size: 2000, status: 'ready' }, { id: 'previous', size: 3000, status: 'ready' }] });
    await view.window.DiscRevealCleanup.refresh(); await check(view); let modal, request;
    view.window.DiscRevealModal.confirm = async value => { modal = value; return true; };
    view.api.executeWindowsCleanup = async (...args) => { request = args; return { completed: args[1], failed: [], reportedFreed: 0 }; };
    await clean(view); assert.equal(modal.acknowledgement, 'cleanupSystemRollbackAck'); assert.equal(request[2], true);
    await check(view); toggle(view, 'rollback', false); await clean(view);
    assert.equal(modal.acknowledgement, 'cleanupAcknowledge'); assert.deepEqual(Array.from(request[1]), ['thumbnails']); assert.equal(request[2], false);
});

test('admin-only Windows groups are not preset and administrator restart needs confirmation', async () => {
    const view = fixture(); await view.window.DiscRevealCleanup.refresh(); await check(view);
    assert.equal(section(view, 'rollback').querySelector('input').checked, false);
    assert.equal(section(view, 'rollback').querySelector('input').disabled, true);
    let restarts = 0; view.api.restartCleanupAsAdmin = async () => { restarts++; };
    await view.find(view.body, 'cleanupSystemAdmin').onclick(); assert.equal(restarts, 0);
    view.allow(true); await view.find(view.body, 'cleanupSystemAdmin').onclick(); assert.equal(restarts, 1);
});

test('cancelling a check keeps partial results unselected and the progress animation connected', async () => {
    const view = fixture(); await view.window.DiscRevealCleanup.refresh(); let complete, native = 0;
    view.api.analyzeCleanup = () => new Promise(resolve => { complete = resolve; });
    view.api.analyzeWindowsCleanup = async () => { native++; };
    const operation = check(view); await new Promise(resolve => setImmediate(resolve));
    const card = view.body.querySelector('.cleanup-progress'), fill = card.querySelector('.progress-bar-fill');
    view.progress({ planId: 9, checked: 100, groups: [] }); for (const tick of view.timers) tick();
    assert.equal(view.body.querySelector('.cleanup-progress'), card); assert.equal(card.querySelector('.progress-bar-fill'), fill);
    await view.find(view.body, 'cancel').onclick();
    complete({ planId: 9, groups: [{ id: 'temp', count: 1, allocated: 4096 }], cancelled: true }); await operation;
    assert.equal(native, 0); assert.equal(view.find(view.controls, 'cleanupCleanSize').disabled, true);
    assert.equal(view.timers.size, 0);
});

test('cancellation between operations prevents later Windows and Recycle Bin mutations', async () => {
    const view = fixture(); await view.window.DiscRevealCleanup.refresh(); await check(view); view.allow(true);
    let complete, native = 0; view.api.executeCleanup = () => new Promise(resolve => { complete = resolve; });
    view.api.executeWindowsCleanup = async () => { native++; };
    const pending = clean(view); await new Promise(resolve => setImmediate(resolve));
    await view.find(view.body, 'cancel').onclick();
    complete({ remaining: [], removed: 0, estimatedFreed: 0, skipped: 0, cancelled: true }); await pending;
    assert.equal(native, 0); assert.equal(view.emptyCalls.length, 0);
    assert.equal(view.body.querySelector('.cleanup-notice').textContent.includes('cleanupPartial'), true);
});

test('a failed Windows operation stops before emptying the Recycle Bin and reports partial results', async () => {
    const view = fixture(); await view.window.DiscRevealCleanup.refresh(); await check(view); view.allow(true);
    view.api.executeWindowsCleanup = async () => ({ completed: [], failed: [{ id: 'thumbnails', status: 'failed' }], reportedFreed: 0 });
    await clean(view); assert.equal(view.executions.length, 1); assert.equal(view.emptyCalls.length, 0);
    assert.equal(view.body.querySelector('.cleanup-notice').textContent.includes('cleanupPartial'), true);
});

test('unavailable Windows analysis does not hide cache results or pretend those system files are selected', async () => {
    const view = fixture(); view.api.analyzeWindowsCleanup = async () => { throw Error('Windows unavailable'); };
    await view.window.DiscRevealCleanup.refresh(); await check(view);
    assert.equal(section(view, 'browsers').querySelector('input').checked, true);
    assert.equal(section(view, 'system'), undefined);
    assert.equal(view.body.querySelector('.cleanup-notice').textContent.includes('cleanupSystemUnavailable'), true);
});

test('completion notices translate without losing the coarse selection', async () => {
    const view = fixture(); await view.window.DiscRevealCleanup.refresh(); await check(view);
    toggle(view, 'windows', false); view.language('en:'); await view.window.DiscRevealCleanup.refresh();
    assert.equal(section(view, 'windows').querySelector('input').checked, false);
    assert.equal(section(view, 'browsers').querySelector('input').checked, true);
    assert.equal(view.body.querySelector('.cleanup-notice').textContent, 'en:cleanupSimpleChecked');
});

test('a new analysis invalidates a pending deletion confirmation', async () => {
    const view = fixture(); await view.window.DiscRevealCleanup.refresh(); await check(view);
    let confirm; view.window.DiscRevealModal.confirm = () => new Promise(resolve => { confirm = resolve; });
    const pending = clean(view);
    view.api.analyzeCleanup = async () => ({ planId: 99, groups: [] });
    await check(view); confirm(true); await pending; assert.equal(view.executions.length, 0); assert.equal(view.emptyCalls.length, 0);
});

test('Recycle Bin contents are explicitly disclosed and a declined confirmation changes nothing', async () => {
    const view = fixture(); await view.window.DiscRevealCleanup.refresh(); await check(view); let modal;
    view.window.DiscRevealModal.confirm = async value => { modal = value; return false; };
    await clean(view); assert.ok(modal.messageHtml.includes('cleanupSimpleBinWarning'));
    assert.equal(modal.danger, true); assert.equal(view.executions.length, 0); assert.equal(view.emptyCalls.length, 0);
});

test('initial loading and analysis have one live clock which is disposed at completion', async () => {
    const view = fixture(); let load;
    view.api.cleanupRules = () => new Promise(resolve => { load = resolve; });
    const pending = view.window.DiscRevealCleanup.refresh();
    assert.ok(view.body.querySelector('.cleanup-progress').querySelector('.pulse-live'));
    assert.equal(view.body.querySelector('.operation-progress-cancel').hidden, true); assert.equal(view.timers.size, 1);
    const rules = [{ id: 'temp', category: 'windows', available: true, minDays: 7 }]; load(rules); await pending;
    view.api.cleanupRules = async () => rules; assert.equal(view.timers.size, 0);
    await check(view); assert.equal(view.timers.size, 0); assert.equal(view.body.querySelector('.cleanup-progress'), null);
});

test('each initial cleanup API failure ends loading and offers a working retry', async () => {
    for (const key of ['cleanupRules', 'cleanupRecycleBins']) {
        const view = fixture(), original = view.api[key];
        view.api[key] = async () => { throw Error('unavailable'); };
        await view.window.DiscRevealCleanup.refresh();
        assert.equal(view.timers.size, 0); assert.equal(view.body.querySelector('.cleanup-progress'), null);
        assert.equal(view.errors.length, 1); assert.equal(view.find(view.controls, 'retry').disabled, false);
        view.api[key] = original; await view.find(view.controls, 'retry').onclick();
        assert.equal(view.find(view.controls, 'cleanupCheckEverything').disabled, false);
        assert.equal(view.body.querySelector('.cleanup-notice'), null);
    }
});

test('keyboard selection and disclosure focus survive coarse area rebuilds', async () => {
    const view = fixture(); await view.window.DiscRevealCleanup.refresh(); await check(view);
    const before = section(view, 'browsers').querySelector('input'); before.focus();
    before.checked = false; before.onchange();
    const after = section(view, 'browsers').querySelector('input');
    assert.notEqual(after, before); assert.equal(before.isConnected, false);
    assert.equal(view.document.activeElement, after); assert.equal(after.checked, false);
    const summary = section(view, 'browsers').querySelector('summary'); summary.focus();
    await view.window.DiscRevealCleanup.refresh();
    assert.equal(view.document.activeElement, section(view, 'browsers').querySelector('summary'));
});

test('Windows.old effects are visible before selection and repeated in the destructive review', async () => {
    const view = fixture();
    view.api.analyzeWindowsCleanup = async () => ({ planId: 8, groups: [{ id: 'previous', status: 'ready', size: 1000 }] });
    await view.window.DiscRevealCleanup.refresh(); await check(view);
    assert.ok(section(view, 'rollback').querySelectorAll('p').some(p => p.textContent === 'cleanupSystemEffect_previous'));
    let review; view.window.DiscRevealModal.confirm = async value => { review = value; return false; };
    await clean(view); assert.ok(review.messageHtml.includes('cleanupSystemEffect_previous')); assert.equal(view.executions.length, 0);
});
