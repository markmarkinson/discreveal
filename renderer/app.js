/**
 * Application controller: wires the topbar, scan lifecycle, navigation,
 * search, deletion and the language switch to the backend API
 * (`window.discreveal`) and the view modules.
 */
const markMarkinsonI18n = window.DiscRevealI18n;
const { t, describeError, formatNumber } = markMarkinsonI18n;
const { formatBytes, escapeHtml, plainText, makeActivatable } = window.DiscRevealUtil;
const { renderList } = window.DiscRevealList;
const { renderGrid } = window.DiscRevealGrid;
const { renderCharts } = window.DiscRevealCharts;
const { renderDrivePicker, renderProgress, renderDuplicates, renderLiveGroups, oldestFile, totalUsedBytes, filterGroups, suggestedPaths, extensions } = window.DiscRevealDuplicates;
const { renderTree } = window.DiscRevealTree;
const { searchTree, renderSearchResults, dirnameOf } = window.DiscRevealSearch;

const SEARCH_DEBOUNCE_MS = 200;
/** Progress never reaches 100 % on its own: excluded or unreadable data means a scan can end below the drive's used space. */
const MAX_SCAN_PROGRESS_PERCENT = 97;
const UNKNOWN_TOTAL_PROGRESS_PERCENT = 8;

/** Flag shown on the language switch: the flag of the language it switches to. */
const FLAGS = {
    de: `<svg viewBox="0 0 5 3" preserveAspectRatio="xMidYMid slice" aria-hidden="true"><rect width="5" height="3" fill="#ffce00"/><rect width="5" height="2" fill="#dd0000"/><rect width="5" height="1" fill="#000"/></svg>`,
    en: `<svg viewBox="0 0 60 30" preserveAspectRatio="xMidYMid slice" aria-hidden="true"><clipPath id="flag-en-frame"><path d="M0,0 v30 h60 v-30 z"/></clipPath><clipPath id="flag-en-diagonals"><path d="M30,15 h30 v15 z v15 h-30 z h-30 v-15 z v-15 h30 z"/></clipPath><g clip-path="url(#flag-en-frame)"><path d="M0,0 v30 h60 v-30 z" fill="#012169"/><path d="M0,0 L60,30 M60,0 L0,30" stroke="#fff" stroke-width="6"/><path d="M0,0 L60,30 M60,0 L0,30" clip-path="url(#flag-en-diagonals)" stroke="#c8102e" stroke-width="4"/><path d="M30,0 v30 M0,15 h60" stroke="#fff" stroke-width="10"/><path d="M30,0 v30 M0,15 h60" stroke="#c8102e" stroke-width="6"/></g></svg>`,
};

const state = {
    settings: { language: navigator.language.startsWith('de') ? 'de' : 'en', lastDrive: '', excludes: [], minFolderSizeMB: 0, maxFolderSizeGB: 0, minFolderSizeUnit: 'MB', maxFolderSizeUnit: 'GB', viewMode: 'list', cacheHashesEnabled: false },
    initialized: false,
    scanCancelling: false,
    activeFeature: 'scan',
    duplicatesOpened: false,
    drives: [],
    /** Root of the current scan result, or `null` before the first result. */
    fullTree: null,
    /** Nodes from the root down to the folder currently shown. */
    zoomStack: [],
    scanning: false,
    /** Counter for scan ids; events of any other scan are ignored. */
    scanSeq: 0,
    /** Id of the scan whose events are accepted, or `null` when none is. */
    activeScanId: null,
    resultScanId: null,
    scanDiagnostics: null,
    activeScanSettingsKey: null,
    expandedPaths: new Set(),
    /** Drive of the current scan result; source of the capacity and the root badge. */
    scannedDrive: null,
    /** Which step of the duplicate finder is shown: pick drives, live search, or results. */
    duplicatesPhase: 'pick',
    /** Drive paths checked in the duplicate-finder drive picker. Kept across reopens for convenience. */
    duplicateSelectedDrives: new Set(),
    /** Most recent `duplicates:progress` payload, or `null` before the first one arrives. */
    duplicatesProgress: null,
    /** Combined used size of the selected drives — the walk phase's progress-bar denominator. */
    duplicatesTotalBytes: 0,
    /** `Date.now()` when the current duplicate search started, for the elapsed-time display. */
    duplicatesStartedAt: null,
    /** Counter for duplicate-search ids; events of any other search are ignored. */
    duplicatesScanSeq: 0,
    /** Id of the duplicate search whose events are accepted, or `null` when none is running. */
    activeDuplicatesScanId: null,
    duplicatesResultId: null,
    /** Most recent `find_duplicates` result, shown by the duplicates view. */
    duplicateGroups: [],
    duplicateGroupIndex: new Map(),
    duplicatePendingGroups: new Map(),
    duplicateScanFilter: {min:'10', minEnabled:true, minUnit:'MB', max:'', maxUnit:'GB', typeMode:'all', types:''},
    duplicateViewFilter: {name:'', min:'', minUnit:'MB', max:'', maxUnit:'GB', types:'', category:'all'},
    duplicateVisibleLimit: 50,
    duplicatesCleaning: false,
    duplicatesCleanupNotice: '',
    /** Paths checked for deletion in the duplicates view. */
    duplicateSelection: new Set(),
    /** `hash_cache_stats` result for the picker's cache line, or `null` while still loading. */
    duplicatesCacheStats: null,
    /** True from the moment "Abbrechen" is clicked until `duplicates:done` actually arrives. */
    duplicatesCancelling: false,
    /** Whether the most recent completed search (`duplicateGroups`) was stopped early by the user. */
    duplicatesWereCancelled: false,
};

/**
 * Completed scan results kept in memory for the rest of this session, so
 * switching back to an already-scanned drive is instant instead of a full
 * rescan — cleared automatically on app restart, never written to disk.
 * Keyed by drive path; each entry also remembers the exclude/filter settings
 * it was scanned with, since those change the shape of the tree. A cached
 * tree is a live reference to the same object `state.fullTree` pointed at
 * when the scan completed, so a later delete (which mutates that tree in
 * place, see removeItemFromTree) keeps the cache correct automatically,
 * without any extra invalidation step.
 */
const scanCache = new Map();
const scanSnapshots = window.DiscRevealSnapshots.createSnapshotCache();

/** Fingerprint of the settings that change what a scan actually finds. */
function scanSettingsKey() {
    return JSON.stringify({
        excludes: [...state.settings.excludes].sort(),
        minSizeMB: state.settings.minFolderSizeMB,
        maxSizeGB: state.settings.maxFolderSizeGB,
        measureAllocated: Boolean(state.settings.measureAllocated),
    });
}

/** Backend-registered identities travel with every result action, including cached results. */
function scanScope() { return { kind: 'scan', id: state.resultScanId }; }
function duplicatesScope() { return { kind: 'duplicates', id: state.duplicatesResultId }; }

/** Windows paths use one identity for UI lookup while retaining their original filesystem form. */
function pathKey(path) { return path.replace(/^\\\\\?\\/, '').replace(/\\+$/, '').toLowerCase(); }

const byId = (id) => document.getElementById(id);

const el = {
    scanControls: byId('scan-controls'),
    duplicateControls: byId('duplicate-controls'),
    cleanupControls: byId('cleanup-controls'),
    cleanupView: byId('cleanup-view'),
    cleanupBody: byId('cleanup-body'),
    driveSelect: byId('drive-select'),
    btnScan: byId('btn-scan'),
    btnCancel: byId('btn-cancel'),
    btnLanguage: byId('btn-language'),
    btnMin: byId('btn-min'),
    btnMax: byId('btn-max'),
    btnClose: byId('btn-close'),
    appVersion: byId('app-version'),
    btnCheckUpdates: byId('btn-check-updates'),
    btnExcludeToggle: byId('btn-exclude-toggle'),
    excludeMenu: byId('exclude-menu'),
    excludeList: byId('exclude-list'),
    btnAddExcludeFolder: byId('btn-add-exclude-folder'),
    btnFilterToggle: byId('btn-filter-toggle'),
    filterMenu: byId('filter-menu'),
    minFolderSize: byId('min-folder-size'),
    measureAllocated: byId('measure-allocated'),
    minFolderSizeUnit: byId('min-folder-size-unit'),
    maxFolderSize: byId('max-folder-size'),
    maxFolderSizeUnit: byId('max-folder-size-unit'),
    breadcrumbRow: byId('breadcrumb-row'),
    breadcrumb: byId('breadcrumb'),
    viewToggle: byId('view-toggle'),
    searchInput: byId('search-input'),
    btnSearchClear: byId('btn-search-clear'),
    searchResults: byId('search-results'),
    treePanel: byId('tree-panel'),
    treeRoot: byId('tree-root'),
    emptyState: byId('empty-state'),
    fileList: byId('file-list'),
    gridView: byId('grid-view'),
    progressToast: byId('progress-toast'),
    progressText: byId('progress-text'),
    progressElapsed: byId('progress-elapsed'),
    progressCancel: byId('progress-cancel'),
    progressBarFill: byId('progress-bar-fill'),
    progressDetail: byId('progress-detail'),
    btnCharts: byId('btn-charts'),
    chartsView: byId('charts-view'),
    chartsBody: byId('charts-body'),
    btnChartsClose: byId('btn-charts-close'),
    btnDuplicates: byId('btn-duplicates'),
    btnDuplicatesEmpty: byId('btn-duplicates-empty'),
    duplicatesView: byId('duplicates-view'),
    duplicatesBody: byId('duplicates-body'),
    btnDuplicatesClose: byId('btn-duplicates-close'),
    btnDupRestart: byId('btn-dup-restart'),
    btnDupTrash: byId('btn-dup-trash'),
    btnDupPermanent: byId('btn-dup-permanent'),
    btnDupSecure: byId('btn-dup-secure'),
    errorBanner: byId('error-banner'),
    errorBannerText: byId('error-banner-text'),
    btnErrorDismiss: byId('btn-error-dismiss'),
};

/* ── Errors ───────────────────────────────────────── */

function showError(message) {
    el.errorBannerText.textContent = message;
    el.errorBanner.classList.remove('hidden');
}

el.btnErrorDismiss.onclick = () => el.errorBanner.classList.add('hidden');

/**
 * Runs `action`, showing any failure in the error banner. With `messageKey`
 * the message is that translation with the error as `{detail}`; without it
 * the translated error is shown as is.
 */
async function withErrorBanner(action, messageKey) {
    try {
        return await action();
    } catch (error) {
        showError(messageKey ? t(messageKey, { detail: describeError(error) }) : describeError(error));
        return undefined;
    }
}

// The native browser menu offers reload and inspection commands that have no
// place in the app. Text fields keep it for copy and paste.
document.addEventListener('contextmenu', (event) => {
    if (!(event.target instanceof HTMLInputElement)) event.preventDefault();
});

/* ── Window controls and dropdowns ────────────────── */

el.btnMin.onclick = () => window.discreveal.minimizeWindow();
el.btnMax.onclick = () => window.discreveal.toggleMaximizeWindow();
el.btnClose.onclick = () => window.discreveal.closeWindow();

// Only one dropdown is open at a time, as their panels can overlap.
const dropdowns = [
    { button: el.btnExcludeToggle, menu: el.excludeMenu },
    { button: el.btnFilterToggle, menu: el.filterMenu },
];

function closeDropdowns() {
    for (const { button, menu } of dropdowns) {
        menu.classList.remove('open');
        button.setAttribute('aria-expanded', 'false');
    }
}

for (const { button, menu } of dropdowns) {
    button.onclick = (event) => {
        event.stopPropagation();
        const opening = !menu.classList.contains('open');
        closeDropdowns();
        if (opening) {
            menu.classList.add('open');
            button.setAttribute('aria-expanded', 'true');
        }
    };
}

document.addEventListener('click', (event) => {
    for (const { button, menu } of dropdowns) {
        if (menu.classList.contains('open') && !menu.contains(event.target) && event.target !== button) {
            menu.classList.remove('open');
            button.setAttribute('aria-expanded', 'false');
        }
    }
});

/* ── Language ─────────────────────────────────────── */

/** Applies `language` everywhere: static texts, the switch and every dynamic view. */
function applyLanguage(language) {
    markMarkinsonI18n.setLanguage(language);
    state.settings.language = markMarkinsonI18n.getLanguage();

    // The switch shows the flag of the language it would change to.
    el.btnLanguage.innerHTML = FLAGS[markMarkinsonI18n.otherLanguage()];
    el.btnLanguage.title = t('languageSwitch');
    el.btnLanguage.setAttribute('aria-label', t('languageSwitch'));

    renderDriveOptions();
    renderExcludeList();
    window.DiscRevealCleanup?.refresh();
    if (isDuplicatesActive()) { el.duplicatesBody.replaceChildren(); renderDuplicatesView(); }
    if (isChartsActive() && state.fullTree) el.btnCharts.onclick();
    updateViewToggle();
    if (state.scanning) el.progressText.textContent = t('scanning');
    if (state.fullTree) {
        renderCurrentLevel();
        if (el.searchInput.value.trim()) runSearch();
    }
}

el.btnLanguage.onclick = () => {
    applyLanguage(markMarkinsonI18n.otherLanguage());
    persistSettings();
};

/** Opens the update-check page for this version in the default browser; the page itself shows the verdict. */
el.btnCheckUpdates.onclick = () => withErrorBanner(() => window.discreveal.checkForUpdates(markMarkinsonI18n.getLanguage()));

/* ── Settings ─────────────────────────────────────── */

let settingsErrorShown = false;

function persistSettings() {
    if (!state.initialized) return;
    state.settings.lastDrive = el.driveSelect.value;
    window.discreveal.saveSettings(state.settings).catch((error) => {
        // Shown once: a read-only location would otherwise repeat it on every change.
        if (settingsErrorShown) return;
        settingsErrorShown = true;
        showError(t('settingsSaveFailed', { detail: describeError(error) }));
    });
}

/** Parses a non-negative number from an input, treating anything else as 0. */
function readLimit(input) {
    return Math.max(0, Number(input.value) || 0);
}

/**
 * Backend size filters have a fixed storage unit each (min: MB, max: GB,
 * see ScanOptions/Settings in scan.rs and settings.rs) — but the user asked
 * to pick MB or GB per field, since "between 10 and 15 MB" was previously
 * impossible to express with max hardcoded to GB. The chosen unit is itself
 * a persisted setting (minFolderSizeUnit/maxFolderSizeUnit); switching it
 * only changes how the stored MB/GB value is displayed, never the value
 * itself — only typing a new number does that.
 */
const MB_PER_GB = 1024;

function convertSize(value, fromUnit, toUnit) {
    if (fromUnit === toUnit) return value;
    return fromUnit === 'GB' ? value * MB_PER_GB : value / MB_PER_GB;
}

/** Rounds away floating-point noise from unit conversion (e.g. 0.48828125). */
function formatSizeValue(value) {
    if (!value) return '';
    return String(Math.round(value * 10000) / 10000);
}

function setupSizeFilterField(input, unitSelect, valueKey, unitKey, baseUnit) {
    input.onchange = () => {
        state.settings[valueKey] = convertSize(readLimit(input), state.settings[unitKey], baseUnit);
        persistSettings();
    };
    unitSelect.onchange = () => {
        const displayValue = convertSize(state.settings[valueKey], baseUnit, unitSelect.value);
        state.settings[unitKey] = unitSelect.value;
        input.value = formatSizeValue(displayValue);
        persistSettings();
    };
}

setupSizeFilterField(el.minFolderSize, el.minFolderSizeUnit, 'minFolderSizeMB', 'minFolderSizeUnit', 'MB');
el.measureAllocated.onchange = () => {
    state.settings.measureAllocated = el.measureAllocated.checked;
    persistSettings();
};
setupSizeFilterField(el.maxFolderSize, el.maxFolderSizeUnit, 'maxFolderSizeGB', 'maxFolderSizeUnit', 'GB');

for (const button of el.viewToggle.querySelectorAll('.view-toggle-btn')) {
    button.onclick = () => {
        state.settings.viewMode = button.dataset.mode;
        persistSettings();
        updateViewToggle();
        renderCurrentLevel();
    };
}

function updateViewToggle() {
    for (const button of el.viewToggle.querySelectorAll('.view-toggle-btn')) {
        const active = button.dataset.mode === state.settings.viewMode;
        button.classList.toggle('active', active);
        button.setAttribute('aria-pressed', String(active));
    }
}

function renderExcludeList() {
    el.excludeList.replaceChildren(
        ...state.settings.excludes.map((exclude, index) => {
            const item = document.createElement('li');
            item.className = 'exclude-item';

            const label = document.createElement('span');
            label.textContent = exclude;
            label.title = exclude;

            const remove = document.createElement('button');
            remove.type = 'button';
            remove.textContent = '✕';
            remove.setAttribute('aria-label', t('removeExclude', { name: exclude }));
            remove.onclick = () => {
                state.settings.excludes.splice(index, 1);
                renderExcludeList();
                persistSettings();
            };

            item.append(label, remove);
            return item;
        })
    );
}

el.btnAddExcludeFolder.onclick = async () => {
    const folder = await withErrorBanner(() => window.discreveal.pickFolder(t('folderDialogTitle')), 'folderPickFailed');
    if (!folder || state.settings.excludes.includes(folder)) return;
    state.settings.excludes.push(folder);
    renderExcludeList();
    persistSettings();
};

/* ── Drives ───────────────────────────────────────── */

function renderDriveOptions() {
    const selectedPath = el.driveSelect.value;

    if (state.drives.length === 0) {
        const option = document.createElement('option');
        option.textContent = t('noDrives');
        option.disabled = true;
        option.selected = true;
        el.driveSelect.replaceChildren(option);
        el.driveSelect.disabled = true;
        return;
    }
    el.driveSelect.disabled = false;
    el.driveSelect.replaceChildren(
        ...state.drives.map((drive) => {
            const option = document.createElement('option');
            option.value = drive.path;
            option.textContent = `${drive.path} (${formatBytes(drive.totalSize)})`;
            return option;
        })
    );
    if (state.drives.some((drive) => drive.path === selectedPath)) el.driveSelect.value = selectedPath;
}

function selectedDrive() {
    return state.drives.find((drive) => drive.path === el.driveSelect.value);
}

/** Short "media · file system" description of a drive, e.g. `SSD · NTFS`. */
function driveDetails(drive) {
    if (!drive) return '';
    const details = [];
    if (drive.mediaType !== 'Unknown') details.push(drive.busType === 'NVMe' ? 'NVMe' : drive.mediaType);
    if (drive.fileSystem !== 'Unknown') details.push(drive.fileSystem);
    return details.join(' · ');
}

el.driveSelect.onchange = () => {
    persistSettings();
    if (state.scanning) return;
    loadFromCacheOrReset(selectedDrive());
};

/**
 * Shows the cached result for `drive` instantly if one matches the current
 * exclude/filter settings, otherwise clears the view back to empty — never
 * leaves a previously scanned drive's results on screen next to a different
 * drive in the selector. Does not scan; only `btnScan` does that, always
 * fresh, which is also what re-populates the cache.
 */
function loadFromCacheOrReset(drive) {
    resetResultViews();
    clearSearch();
    el.errorBanner.classList.add('hidden');

    const cached = drive && scanCache.get(drive.path);
    if (!cached || cached.settingsKey !== scanSettingsKey()) {
        el.emptyState.classList.remove('hidden');
        return;
    }
    state.scannedDrive = cached.drive;
    state.resultScanId = cached.scanId;
    adoptTree(cached.tree);
    renderCurrentLevel();
}

/* ── Scan lifecycle ───────────────────────────────── */

let scanProgressTimer = null, scanProgressStarted = 0;
function beginScanUi() {
    state.scanCancelling = false;
    el.btnCancel.disabled = false; el.progressCancel.disabled = false;
    scanProgressStarted = Date.now();
    if (scanProgressTimer !== null) clearInterval(scanProgressTimer);
    const updateElapsed = () => { el.progressElapsed.textContent = t('dupElapsed', { seconds: formatNumber(Math.floor((Date.now() - scanProgressStarted) / 1000)) }); };
    updateElapsed(); scanProgressTimer = setInterval(updateElapsed, 1000);
    el.progressCancel.onclick = () => el.btnCancel.onclick();
    state.scanning = true;
    updateBusyUi();
    el.btnScan.classList.add('hidden');
    el.btnCancel.classList.remove('hidden');
    el.progressToast.classList.remove('hidden');
    el.progressText.textContent = t('scanning');
    el.progressBarFill.style.width = '0%';
    el.progressBarFill.parentElement.removeAttribute('aria-valuenow');
    el.progressDetail.textContent = '';
}

function endScanUi() {
    clearInterval(scanProgressTimer); scanProgressTimer = null;
    scanSnapshots.clear();
    state.scanning = false;
    state.scanCancelling = false;
    el.btnCancel.disabled = false; el.progressCancel.disabled = false;
    updateBusyUi();
    el.btnScan.classList.remove('hidden');
    el.btnCancel.classList.add('hidden');
    el.progressToast.classList.add('hidden');
    // Without any result there is nothing else to look at.
    if (!state.fullTree && state.activeFeature === 'scan') el.emptyState.classList.remove('hidden');
}

/** Clears the result views so nothing stale stays clickable during a new scan. */
function resetResultViews() {
    scanSnapshots.clear();
    el.btnDuplicates.setAttribute('aria-pressed', 'false');
    state.resultScanId = null;
    state.scanDiagnostics = null; updateScanCoverage();
    state.fullTree = null;
    state.zoomStack = [];
    state.expandedPaths = new Set();
    for (const view of [el.fileList, el.gridView, el.searchResults, el.chartsBody, el.treeRoot, el.breadcrumb]) {
        view.replaceChildren();
    }
    for (const view of [el.fileList, el.gridView, el.chartsView, el.duplicatesView, el.treePanel, el.breadcrumbRow, el.emptyState]) {
        view.classList.add('hidden');
    }
}

el.btnScan.onclick = async () => {
    const drive = selectedDrive();
    if (!drive || window.DiscRevealCleanup.isBusy() || state.duplicatesCleaning) return;

    persistSettings();
    el.errorBanner.classList.add('hidden');

    resetResultViews();
    clearSearch();
    state.scannedDrive = drive;
    state.activeScanId = ++state.scanSeq;
    state.resultScanId = state.activeScanId;
    state.activeScanSettingsKey = scanSettingsKey();
    beginScanUi();

    try {
        await window.discreveal.startScan({
            scanId: state.activeScanId,
            rootPath: drive.path,
            excludes: state.settings.excludes,
            minSizeMB: state.settings.minFolderSizeMB,
            maxSizeGB: state.settings.maxFolderSizeGB,
            measureAllocated: Boolean(state.settings.measureAllocated),
            isSsd: drive.isSsd,
            busType: drive.busType,
        });
    } catch (error) {
        state.activeScanId = null;
        endScanUi();
        showError(t('scanStartFailed', { detail: describeError(error) }));
    }
};

el.btnCancel.onclick = cancelActiveScan;
async function cancelActiveScan() {
    if (!state.scanning || state.scanCancelling) return;
    const scanId = state.activeScanId;
    state.scanCancelling = true;
    el.btnCancel.disabled = true; el.progressCancel.disabled = true;
    el.progressText.textContent = t('dupCancelling');
    try {
        await window.discreveal.cancelScan();
        if (state.activeScanId !== scanId) return;
        state.activeScanId = null;
        endScanUi();
        if (state.fullTree) showError(t('scanCancelled'));
    } catch (error) {
        if (state.activeScanId !== scanId) return;
        state.scanCancelling = false;
        el.btnCancel.disabled = false; el.progressCancel.disabled = false;
        el.progressText.textContent = t('scanning');
        showError(t('cancelFailed', { detail: describeError(error) }));
    }
}

/** Makes `tree` the current result and reveals the result panels. */
function adoptTree(tree) {
    const isFirstResult = state.fullTree === null;
    const currentPath = currentNode()?.path;
    state.fullTree = tree;
    // Keep the folder the user navigated to, if it still exists in the new tree.
    state.zoomStack = (currentPath && findNodeChain(tree, currentPath)) || [tree];
    if (isFirstResult) {
        el.emptyState.classList.add('hidden');
        el.treePanel.classList.toggle('hidden', state.activeFeature !== 'scan');
        state.expandedPaths.add(tree.path);
    }
}

/** Whether an event belongs to the scan the UI is currently showing. */
const isCurrentScan = (scanId) => state.activeScanId !== null && scanId === state.activeScanId;

/** Terminal events from the common backend contract recover abandoned reads.
 * Feature done events carry results; late/stale lifecycle events never replace them. */
function handleOperationStatus({ kind, requestId, status }) {
    if (status !== 'abandoned' && status !== 'failed') return;
    if (kind === 'scan' && isCurrentScan(requestId)) {
        state.activeScanId = null;
        endScanUi();
        showError(t('operationStopped'));
    } else if (kind === 'duplicates' && state.activeDuplicatesScanId === requestId) {
        state.activeDuplicatesScanId = null;
        state.duplicatesPhase = 'results';
        state.duplicatesWereCancelled = true;
        if (duplicatesElapsedTimer) { clearInterval(duplicatesElapsedTimer); duplicatesElapsedTimer = null; }
        state.duplicatesCancelling = false;
        renderDuplicatesView();
        updateBusyUi();
        showError(t('operationStopped'));
    }
}
window.discreveal.onOperationStatus?.(handleOperationStatus);

window.discreveal.onScanProgress(({ scanId, scannedFiles, scannedBytes, currentPath }) => {
    if (!isCurrentScan(scanId) || !state.scanning) return;
    const usedSize = state.scannedDrive?.usedSize ?? 0;
    const percent =
        usedSize > 0
            ? Math.min(MAX_SCAN_PROGRESS_PERCENT, (scannedBytes / usedSize) * 100)
            : UNKNOWN_TOTAL_PROGRESS_PERCENT;
    el.progressBarFill.style.width = `${percent}%`;
    const track = el.progressBarFill.parentElement;
    track.setAttribute('aria-label', t('scanning'));
    if (usedSize > 0) track.setAttribute('aria-valuenow', String(percent));
    else track.removeAttribute('aria-valuenow');
    el.progressDetail.textContent = t('progressDetail', {
        files: formatNumber(scannedFiles),
        bytes: formatBytes(scannedBytes),
        path: currentPath,
    });
});

// Partial results arrive while the scan is still running.
window.discreveal.onScanPartial(({ scanId, tree, reusedPaths }) => {
    if (!isCurrentScan(scanId) || !state.scanning) return;
    adoptTree(scanSnapshots.merge(tree, reusedPaths));
    renderCurrentLevel();
    refreshActiveSearch(false);
});

window.discreveal.onScanDone(({ scanId, tree, diagnostics }) => {
    if (!isCurrentScan(scanId)) return;
    state.activeScanId = null;
    state.scanDiagnostics = diagnostics || null;
    adoptTree(tree);
    scanCache.set(state.scannedDrive.path, {
        tree: state.fullTree,
        drive: state.scannedDrive,
        settingsKey: state.activeScanSettingsKey,
        scanId,
        diagnostics: state.scanDiagnostics,
    });
    endScanUi();
    updateScanCoverage();
    renderCurrentLevel();
    refreshActiveSearch(true);
});

window.discreveal.onScanError((error) => {
    if (!isCurrentScan(error.scanId)) return;
    state.activeScanId = null;
    endScanUi();
    showError(describeError(error));
});

/* ── Navigation ───────────────────────────────────── */

function currentNode() {
    return state.zoomStack[state.zoomStack.length - 1];
}

function updateScanCoverage() {
    const notice = document.getElementById('scan-coverage');
    if (!notice) return;
    const diagnostics = state.scanDiagnostics;
    const stale = Boolean(state.fullTree?.storageNeedsRescan);
    notice.classList.toggle('hidden', state.activeFeature !== 'scan' || (!stale && diagnostics?.coverage !== 'partial'));
    notice.textContent = stale ? t('storageNeedsRescan') : diagnostics ? t('scanCoverage', { unreadable: formatNumber(diagnostics.unreadable || 0), changed: formatNumber(diagnostics.changed || 0), limited: formatNumber((diagnostics.depthLimited || 0) + (diagnostics.resourceLimited || 0)), excluded: formatNumber(diagnostics.excluded || 0) }) : '';
}

function isSearchActive() {
    return !el.searchResults.classList.contains('hidden');
}

function onItemClick(item) {
    if (item.isDir) {
        state.zoomStack.push(item);
        renderCurrentLevel();
    } else if (item.path) {
        withErrorBanner(() => window.discreveal.openPath(item.path, scanScope()));
    }
}

/** Folders currently being expanded, so a second click can't fire a duplicate request. */
const expandingOverflowFor = new Set();
let expandOperationSeq = 0;

/**
 * Re-reads `node`'s children on demand, replacing the folded view with every
 * entry the backend just re-read (unsorted-by-significance, unfolded — see
 * expand_folder). This replaces `node.children` outright rather than
 * splicing in place of the overflow entry: the fresh list already contains
 * everything, including the items that were already shown, so splicing
 * would duplicate them.
 */
function onExpandOverflow(node) {
    if (expandingOverflowFor.has(node.path)) return;
    expandingOverflowFor.add(node.path);
    const operationId = ++expandOperationSeq, tree = state.fullTree, scope = scanScope();
    // Re-reading a folder folded down to a handful of leftovers is instant, but a folder
    // with thousands of small entries (Windows\WinSxS, in the worst case observed: ~20s for
    // 31,000+ items, each individually tiny) is not — without this, a click on those looks
    // like it did nothing until the redraw finally lands.
    const tail = el.fileList.querySelector('.list-row.is-tail') || el.gridView.querySelector('.grid-cell.is-tail');
    if (tail) {
        tail.classList.add('is-loading');
        const label = tail.querySelector('.list-row-name, .grid-cell-name');
        if (label) label.textContent = t('expandingOverflow');
        const stop = document.createElement('button'); stop.type = 'button'; stop.className = 'pill-btn pill-btn-ghost'; stop.textContent = t('cancel');
        stop.onclick = event => { event.stopPropagation(); stop.disabled = true; withErrorBanner(() => window.discreveal.cancelExpandFolder(operationId)); };
        tail.appendChild(stop);
    }
    withErrorBanner(async () => {
        const offset = node.children?.find(child => !child.path)?.nextOffset;
        const children = await window.discreveal.expandFolder(node.path, scope, operationId, offset, state.fullTree?.logicalSize !== undefined);
        if (state.fullTree !== tree || state.resultScanId !== scope.id) return;
        const known = new Map((node.children || []).filter(child => child.path).map(child => [pathKey(child.path), child]));
        for (const child of children) if (child.path) {
            const previous = known.get(pathKey(child.path));
            if (previous?.isDir && child.isDir && !child.children?.length) { child.size = previous.size; child.children = previous.children; }
            known.set(pathKey(child.path), child);
        }
        // Listing pages are not a fresh recursive size scan: retain known folder totals.
        node.children = [...known.values()].sort((a,b) => b.size-a.size).concat(children.filter(child => !child.path));
        if (currentNode() === node) renderCurrentLevel();
    }).finally(() => {
        expandingOverflowFor.delete(node.path);
        if (tail?.isConnected && state.fullTree === tree) {
            tail.classList.remove('is-loading');
            if (currentNode() === node) renderCurrentLevel();
        }
    });
}

function renderCurrentLevel() {
    const sizeHeading = document.getElementById('scan-size-heading');
    if (sizeHeading) sizeHeading.title = t(state.fullTree?.logicalSize === undefined ? 'logicalSizeHint' : 'allocatedSizeHint');
    // Search results replace the folder view until the search is cleared.
    if (state.activeFeature !== 'scan' || !currentNode() || isSearchActive() || isChartsActive() || isDuplicatesActive()) return;

    const isGrid = state.settings.viewMode === 'grid';
    el.fileList.classList.toggle('hidden', isGrid);
    el.gridView.classList.toggle('hidden', !isGrid);

    // Live updates redraw every half second, so rows must not fade in again each time.
    const options = { animate: !state.scanning, onExpandOverflow };
    if (isGrid) {
        renderGrid(el.gridView, currentNode(), onItemClick, onItemContextMenu, options);
    } else {
        renderList(el.fileList, currentNode(), onItemClick, onItemContextMenu, options);
    }

    renderBreadcrumb();
    // Unfolds the tree panel to the current folder's ancestors only — replacing, not adding
    // to, expandedPaths. Every earlier folder visited this session was previously left
    // expanded forever (only an explicit tree-arrow collapse ever removed a path), so the
    // tree panel's full rebuild on every navigation (see tree.js's renderTree) grew more
    // expensive the longer a session ran, regardless of where you'd actually navigated to.
    state.expandedPaths = new Set(state.zoomStack.map((node) => node.path));
    renderTreePanel();
}

function renderTreePanel() {
    if (!state.fullTree) return;
    renderTree(el.treeRoot, state.fullTree, {
        expandedPaths: state.expandedPaths,
        activePath: currentNode()?.path ?? null,
        rootBadge: driveDetails(state.scannedDrive),
        onToggle: (path) => {
            if (!state.expandedPaths.delete(path)) state.expandedPaths.add(path);
            renderTreePanel();
        },
        onSelect: (path) => {
            const chain = findNodeChain(state.fullTree, path);
            if (!chain) return;
            state.zoomStack = chain;
            renderCurrentLevel();
        },
    });
}

function renderBreadcrumb() {
    el.breadcrumbRow.classList.remove('hidden');
    el.breadcrumb.replaceChildren(
        ...state.zoomStack.map((node, index) => {
            const crumb = document.createElement('div');
            crumb.className = `breadcrumb-item${index === state.zoomStack.length - 1 ? ' active' : ''}`;
            crumb.textContent = node.name;
            makeActivatable(crumb, () => {
                state.zoomStack = state.zoomStack.slice(0, index + 1);
                renderCurrentLevel();
            });
            return crumb;
        })
    );
}

/**
 * Finds the node at `targetPath` (file or folder, any depth) below `root`
 * and returns the chain of nodes from `root` down to it, or `null`.
 */
function findNodeChain(root, targetPath) {
    if (pathKey(root.path) === pathKey(targetPath)) return [root];
    for (const child of root.children || []) {
        // The overflow node has no path and is never a target.
        if (!child.path) continue;
        if (pathKey(child.path) === pathKey(targetPath)) return [root, child];
        if (child.isDir) {
            const rest = findNodeChain(child, targetPath);
            if (rest) return [root, ...rest];
        }
    }
    return null;
}

/* ── Search ───────────────────────────────────────── */

let searchDebounceTimer = null;
function refreshActiveSearch(immediate) {
    if (!el.searchInput.value.trim() || state.activeFeature !== 'scan' || !isSearchActive()) return;
    clearTimeout(searchDebounceTimer);
    if (immediate) runSearch(); else searchDebounceTimer = setTimeout(runSearch, SEARCH_DEBOUNCE_MS);
}

/** Shows or hides the search results in place of the folder view. */
function setSearchActive(active) {
    el.searchResults.classList.toggle('hidden', !active);
    el.breadcrumb.classList.toggle('hidden', active);
    el.viewToggle.classList.toggle('hidden', active);
    if (active) {
        el.fileList.classList.add('hidden');
        el.gridView.classList.add('hidden');
        setChartsActive(false);
        setDuplicatesActive(false);
        el.breadcrumbRow.classList.toggle('hidden', isChartsActive());
    }
}

function clearSearch() {
    clearTimeout(searchDebounceTimer);
    el.searchInput.value = '';
    el.btnSearchClear.classList.add('hidden');
    setSearchActive(false);
    renderCurrentLevel();
}

function runSearch() {
    const query = el.searchInput.value.trim();
    if (!query || !state.fullTree) {
        setSearchActive(false);
        renderCurrentLevel();
        return;
    }

    setSearchActive(true);
    renderSearchResults(el.searchResults, searchTree(state.fullTree, query), (item) => {
        // A hit opens its containing folder, so neighbours stay visible.
        const chain = findNodeChain(state.fullTree, item.isDir ? item.path : dirnameOf(item.path));
        if (chain) state.zoomStack = chain;
        clearSearch();
    });
}

el.searchInput.oninput = () => {
    el.btnSearchClear.classList.toggle('hidden', !el.searchInput.value);
    clearTimeout(searchDebounceTimer);
    searchDebounceTimer = setTimeout(runSearch, SEARCH_DEBOUNCE_MS);
};

el.searchInput.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') clearSearch();
});

el.btnSearchClear.onclick = () => {
    clearSearch();
    el.searchInput.focus();
};

/* ── Charts ───────────────────────────────────────── */

function isChartsActive() {
    return !el.chartsView.classList.contains('hidden');
}

/** Shows or hides the statistics view in place of the folder view. */
function setChartsActive(active) {
    el.chartsView.classList.toggle('hidden', !active);
    el.breadcrumbRow.classList.toggle('hidden', active);
    if (active) {
        el.fileList.classList.add('hidden');
        el.gridView.classList.add('hidden');
        setSearchActive(false);
        setDuplicatesActive(false);
        el.breadcrumbRow.classList.toggle('hidden', isChartsActive());
    }
}

el.btnCharts.onclick = () => {
    if (!state.fullTree) return;
    setChartsActive(true);
    renderCharts(el.chartsBody, state.fullTree, (file) => {
        // Same as opening a search hit: jump to the file's containing folder.
        const chain = findNodeChain(state.fullTree, dirnameOf(file.path));
        if (chain) state.zoomStack = chain;
        setChartsActive(false);
        renderCurrentLevel();
    });
};

el.btnChartsClose.onclick = () => {
    setChartsActive(false);
    renderCurrentLevel();
};

/* ── Deletion ─────────────────────────────────────── */

/** Translation keys of the confirmation dialog for each delete mode. */
const DELETE_MODES = {
    trash: { title: 'trashTitle', confirm: 'trashConfirm', warning: 'trashWarning', danger: false },
    permanent: { title: 'permanentTitle', confirm: 'permanentConfirm', warning: 'permanentWarning', danger: true },
    secure: { title: 'secureTitle', confirm: 'secureConfirm', warning: 'secureWarning', danger: true },
};

/** Translation keys of the warning shown for each kind of protected item. */
const PROTECTION_WARNINGS = {
    windows: 'protectedWindows',
    programs: 'protectedPrograms',
    profile: 'protectedProfile',
    systemData: 'protectedSystemData',
    systemFile: 'protectedSystemFile',
};

function onItemContextMenu(item, x, y) {
    window.DiscRevealContextMenu.show(x, y, [
        // Works for folders too (opens them in Explorer), not just files — previously the
        // only way to reach openPath was clicking all the way down to a file.
        { label: t('menuOpenPath'), onSelect: () => withErrorBanner(() => window.discreveal.openPath(item.path, scanScope())) },
        { divider: true },
        { label: t('menuTrash'), onSelect: () => deleteItem(item, 'trash') },
        { divider: true },
        { label: t('menuPermanent'), danger: true, onSelect: () => deleteItem(item, 'permanent') },
        { label: t('menuSecure'), danger: true, onSelect: () => deleteItem(item, 'secure') },
        { divider: true },
        { label: t('menuCopyPath'), onSelect: () => copyPath(item.path) },
    ]);
}

async function copyPath(path) {
    try {
        await navigator.clipboard.writeText(path);
    } catch {
        showError(t('copyPathFailed', { path }));
    }
}

/** Markup of the warning block shown for a protected item. */
function protectionWarningHtml(reason) {
    const text = t(PROTECTION_WARNINGS[reason] ?? 'protectedWindows');
    return (
        `<div class="modal-protected"><strong>${escapeHtml(t('protectedHeading'))}</strong>` +
        `<p>${escapeHtml(text)}</p></div>`
    );
}

/** Deletes `item` after confirmation. Returns whether it was actually deleted. */
async function deleteItem(item, mode, scope = scanScope(), duplicate = null) {
    if (state.scanning) {
        showError(t('deleteDuringScan'));
        return false;
    }
    const keys = DELETE_MODES[mode];

    // Protected system items get an explicit warning and an acknowledgement.
    let protection = null;
    try {
        protection = await window.discreveal.getProtection(item.path, scope);
    } catch (error) {
        showError(t('deleteFailed', { detail: describeError(error) }));
        return false;
    }

    const contents = item.isDir ? ` · ${t('includingContents')}` : '';
    const warning = t(keys.warning);
    const confirmed = await window.DiscRevealModal.confirm({
        title: t(keys.title),
        confirmLabel: t(keys.confirm),
        danger: keys.danger || protection !== null,
        acknowledgement: protection !== null ? t('protectedAcknowledge') : undefined,
        messageHtml:
            (protection !== null ? protectionWarningHtml(protection) : '') +
            `<p><strong>${escapeHtml(plainText(markMarkinsonI18n.nodeName(item)))}</strong> · ${formatBytes(item.size)}${contents}</p>` +
            `<p class="modal-path">${escapeHtml(plainText(item.path))}</p>` +
            (warning ? `<p class="modal-warning">${escapeHtml(warning)}</p>` : ''),
    });
    if (!confirmed) return false;

    try {
        if (duplicate) await window.discreveal.deleteDuplicate(item.path, duplicate.path, item.size, mode, protection !== null, scope);
        else await window.discreveal.deleteItem(item.path, mode, protection !== null, scope);
    } catch (error) {
        showError(t('deleteFailed', { detail: describeError(error) }));
        return false;
    }
    removeItemFromTree(item.path);
    renderCurrentLevel();
    return true;
}

/**
 * Removes a deleted item from the loaded tree and reduces the size of every
 * ancestor accordingly, so the view stays correct without a rescan. The
 * zoom stack references the same nodes, so it stays in sync.
 */
function removeItemFromTree(targetPath) {
    const trees = new Set([state.fullTree, ...[...scanCache.values()].map((entry) => entry.tree)]);
    for (const tree of trees) {
        const chain = tree && findNodeChain(tree, targetPath);
        if (!chain || chain.length < 2) continue;
        const target = chain.at(-1);
        const parent = chain.at(-2);
        parent.children = parent.children.filter((child) => pathKey(child.path) !== pathKey(targetPath));
        for (const ancestor of chain.slice(0, -1)) {
            ancestor.size = Math.max(0, ancestor.size - target.size);
            if (ancestor.logicalSize !== undefined) ancestor.logicalSize = Math.max(0, ancestor.logicalSize - (target.logicalSize ?? target.size));
            if (ancestor.logicalSize !== undefined) ancestor.allocationUnknown = true;
        }
        if (tree.logicalSize !== undefined) {
            tree.storageNeedsRescan = true;
            for (const [drive, entry] of scanCache) if (entry.tree === tree) scanCache.delete(drive);
        }
    }
    updateScanCoverage();
}

/* ── Duplicate finder ─────────────────────────────── */
//
// Its own workflow, independent of the main scan: pick drives, watch a live
// search run (progress events from the backend's own background thread,
// see duplicates.rs), then review and delete from the results. Phases:
// 'pick' -> 'scanning' -> 'results', tracked in state.duplicatesPhase.

let duplicatesElapsedTimer = null;

function isDuplicatesActive() {
    return !el.duplicatesView.classList.contains('hidden');
}

/** Shows or hides the duplicate finder in place of the folder view. */
function setDuplicatesActive(active) {
    // Changing workspace preserves searches and their result identities.
    el.duplicatesView.classList.toggle('hidden', !active);
    el.breadcrumbRow.classList.toggle('hidden', active || !state.fullTree);
    el.emptyState.classList.toggle('hidden', active || Boolean(state.fullTree));
    el.btnDuplicates.setAttribute('aria-pressed', String(active));
    if (active) {
        el.fileList.classList.add('hidden');
        el.gridView.classList.add('hidden');
        setSearchActive(false);
        setChartsActive(false);
        el.breadcrumbRow.classList.add('hidden');
    }
}

/** The header's delete-mode buttons and "new search" link only make sense once results are shown. */
function updateDuplicatesHeaderButtons() {
    const showResultActions = state.duplicatesPhase === 'results';
    document.getElementById('dup-more-actions').classList.toggle('hidden', !showResultActions || !state.duplicateSelection.size);
    for (const button of [el.btnDupRestart, el.btnDupTrash, el.btnDupPermanent, el.btnDupSecure]) {
        button.classList.toggle('hidden', !showResultActions);
        button.disabled = state.duplicatesCleaning || (button !== el.btnDupRestart && !state.duplicateSelection.size);
    }
    el.btnDuplicatesClose.disabled = state.duplicatesCleaning;
    el.btnDuplicates.disabled = state.duplicatesCleaning;
    el.btnDuplicatesEmpty.disabled = state.duplicatesCleaning;
    el.btnScan.disabled = state.duplicatesCleaning || state.scanning || !state.drives.length;
}

function stopDuplicatesTimers() {
    clearTimeout(duplicatesRenderTimer);
    duplicatesRenderTimer = null;
    clearInterval(duplicatesElapsedTimer);
    duplicatesElapsedTimer = null;
}

/** Reloads the cache-size line shown on the drive picker; fetched fresh whenever it might have changed. */
async function refreshDuplicatesCacheStats() {
    try {
        state.duplicatesCacheStats = await window.discreveal.hashCacheStats();
    } catch {
        state.duplicatesCacheStats = null;
    }
    if (state.duplicatesPhase === 'pick') renderDuplicatesView();
}

function renderDuplicatesView() {
    updateBusyUi();
    if (state.activeFeature !== 'duplicates') return;
    const restoreBodyFocus = window.DiscRevealUtil.preserveFocus?.(el.duplicatesBody);
    const parkedActions = el.duplicatesView.querySelector('.duplicates-view-actions');
    parkedActions.append(el.btnDupRestart, document.getElementById('dup-more-actions'));
    const focused = el.duplicateControls.contains(document.activeElement) ? document.activeElement : null;
    const selection = focused && typeof focused.selectionStart === 'number' ? [focused.selectionStart, focused.selectionEnd] : null;
    for (const node of [...el.duplicateControls.children]) el.duplicatesBody.appendChild(node);
    updateDuplicatesHeaderButtons();
    if (state.duplicatesPhase === 'pick') {
        renderDrivePicker(el.duplicatesBody, state.drives, state.duplicateSelectedDrives, {
            onToggleDrive: (path) => {
                if (!state.duplicateSelectedDrives.delete(path)) state.duplicateSelectedDrives.add(path);
                renderDuplicatesView();
            },
            onStart: startDuplicatesSearch,
            filter: state.duplicateScanFilter,
            onFilterChange: (key,value)=>{state.duplicateScanFilter[key]=value; if (key === 'minEnabled') renderDuplicatesView();},
            cacheEnabled: Boolean(state.settings.cacheHashesEnabled),
            cacheStats: state.duplicatesCacheStats,
            onToggleCache: () => {
                state.settings.cacheHashesEnabled = !state.settings.cacheHashesEnabled;
                persistSettings();
                renderDuplicatesView();
            },
            onClearCache: () => withErrorBanner(async () => {
                await window.discreveal.clearHashCache();
                await refreshDuplicatesCacheStats();
            }),
        });
    } else if (state.duplicatesPhase === 'scanning') {
        renderDuplicateProgress();
        renderLiveGroups(el.duplicatesBody, state.duplicateGroups, (path) => withErrorBanner(() => window.discreveal.openPath(path, duplicatesScope())), [...state.duplicatePendingGroups.values()], duplicateResultOptions());
        state.duplicatePendingGroups.clear();
    } else {
        renderDuplicates(el.duplicatesBody, state.duplicateGroups, state.duplicateSelection, {
            ...duplicateResultOptions(),
            cancelled: state.duplicatesWereCancelled,
            onToggleSelect: (path) => {
                const group=state.duplicateGroups.find(group=>group.files.some(file=>file.path===path));
                if (!group || oldestFile(group).path===path || state.duplicatesCleaning) return;
                if (!state.duplicateSelection.delete(path)) state.duplicateSelection.add(path);
                renderDuplicatesView();
            },
            onOpenPath: (path) => withErrorBanner(() => window.discreveal.openPath(path, duplicatesScope())),
            onKeepOnlyOldest: (group) => {
                const oldest = oldestFile(group);
                state.duplicateSelection.delete(oldest.path);
                const eligible = group.files.filter(file => file !== oldest && file.cleanupEligible);
                const clear = eligible.length > 0 && eligible.every(file => state.duplicateSelection.has(file.path));
                for (const file of eligible) {
                    if (clear) state.duplicateSelection.delete(file.path);
                    else state.duplicateSelection.add(file.path);
                }
                renderDuplicatesView();
            },
        });
    }
    let cleanupProgress = el.duplicatesBody.querySelector('.dup-cleanup-progress');
    if (state.duplicatesCleaning) {
        if (!cleanupProgress) {
            cleanupProgress = window.DiscRevealProgress.create('dup-cleanup-progress', cancelDuplicateCleanup);
            el.duplicatesBody.insertBefore(cleanupProgress, el.duplicatesBody.firstChild);
        }
        window.DiscRevealProgress.update(cleanupProgress, {
            status: state.duplicatesCleanupNotice,
            elapsed: t('dupElapsed', {seconds:formatNumber(Math.floor((Date.now()-state.duplicatesCleanupStartedAt)/1000))}),
            percentage: state.duplicatesCleanupTotal ? state.duplicatesCleanupDone/state.duplicatesCleanupTotal*100 : null,
            cancelVisible: true,
            cancelling: Boolean(state.duplicatesCleanupCancelling),
            hint: state.duplicatesCleanupCancelling ? t('dupCleanupStopHint') : '',
        });
    } else cleanupProgress?.remove();
    mountDuplicateControls();
    restoreBodyFocus?.();
    if (focused?.isConnected) {
        focused.focus({ preventScroll: true });
        if (selection) focused.setSelectionRange(...selection);
    }
}

function mountDuplicateControls() {
    const setup = el.duplicatesBody.querySelector('.dup-setup');
    const selector = setup ? '.dup-filter-disclosure, .dup-setup-footer' : '.dup-tools';
    for (const node of el.duplicatesBody.querySelectorAll(selector)) el.duplicateControls.appendChild(node);
    if (!setup) el.duplicateControls.append(el.btnDupRestart, document.getElementById('dup-more-actions'));
}

function isCurrentDuplicatesScan(scanId) {
    return state.activeDuplicatesScanId !== null && scanId === state.activeDuplicatesScanId;
}

async function startDuplicatesSearch() {
    if (state.duplicateSelectedDrives.size === 0 || window.DiscRevealCleanup.isBusy()) return;
    let options;
    try { options=duplicateScanOptions(state.duplicateScanFilter); }
    catch(error) { showError(describeError(error)); return; }

    const selectedDrives = state.drives.filter((drive) => state.duplicateSelectedDrives.has(drive.path));
    state.duplicatesPhase = 'scanning';
    state.duplicatesProgress = null;
    state.duplicatesTotalBytes = totalUsedBytes(selectedDrives);
    state.duplicatesStartedAt = Date.now();
    state.duplicatesCancelling = false;
    state.duplicatesWereCancelled = false;
    state.activeDuplicatesScanId = ++state.duplicatesScanSeq;
    state.duplicatesResultId = state.activeDuplicatesScanId;
    state.duplicateGroups = [];
    state.duplicateVisibleLimit = 50;
    state.duplicatesCleanupNotice = '';
    el.duplicatesBody.replaceChildren();
    state.duplicateGroupIndex.clear();
    state.duplicatePendingGroups.clear();
    state.duplicateSelection = new Set();
    renderDuplicatesView();

    stopDuplicatesTimers();
    duplicatesElapsedTimer = setInterval(() => {
        if (state.duplicatesPhase === 'scanning') renderDuplicateProgress();
    }, 1000);

    try {
        await window.discreveal.findDuplicates(
            state.activeDuplicatesScanId,
            [...state.duplicateSelectedDrives],
            state.settings.excludes,
            Boolean(state.settings.cacheHashesEnabled),
            options,
        );
    } catch (error) {
        stopDuplicatesTimers();
        state.activeDuplicatesScanId = null;
        state.duplicatesPhase = 'pick';
        renderDuplicatesView();
        showError(t('dupFailed', { detail: describeError(error) }));
    }
}

/**
 * Requests cancellation but stays on the progress view (showing "wird
 * abgebrochen…", cancel button disabled) until the backend's own
 * `duplicates:done` arrives — the search still finishes confirming whatever
 * it already found before it actually stops, and those groups (see
 * `onDuplicatesDone`) are what the results view then shows, not an empty
 * screen. Idempotent: a second click while already cancelling is a no-op.
 */
function cancelDuplicatesSearch() {
    if (state.duplicatesCancelling) return;
    state.duplicatesCancelling = true;
    renderDuplicateProgress();
    window.discreveal.cancelDuplicates().catch(error=>{state.duplicatesCancelling=false;renderDuplicateProgress();showError(describeError(error));});
}

function renderDuplicateProgress() {
    if (state.activeFeature !== 'duplicates') return;
    const elapsedSeconds=state.duplicatesStartedAt ? (Date.now()-state.duplicatesStartedAt)/1000 : 0;
    renderProgress(el.duplicatesBody,state.duplicatesProgress,state.duplicatesTotalBytes,elapsedSeconds,{onCancel:cancelDuplicatesSearch,cancelling:state.duplicatesCancelling});
}

/**
 * Detaches from any in-flight search without waiting to see its results —
 * used when the user navigates away from the duplicate finder entirely
 * (closing it or reopening it fresh), unlike `cancelDuplicatesSearch` above,
 * which stays to show partial results. A `duplicates:done` for the
 * abandoned scan id that arrives later is silently ignored by
 * `isCurrentDuplicatesScan`.
 */
function abandonDuplicatesScan() {
    if (state.duplicatesPhase === 'scanning') window.discreveal.cancelDuplicates();
    stopDuplicatesTimers();
    state.activeDuplicatesScanId = null;
    state.duplicatesCancelling = false;
}

window.discreveal.onDuplicatesProgress((payload) => {
    if (!isCurrentDuplicatesScan(payload.scanId)) return;
    state.duplicatesProgress = payload;
    if (state.duplicatesPhase === 'scanning') renderDuplicateProgress();
});

let duplicatesRenderTimer = null;
window.discreveal.onDuplicatesGroup(({ scanId, group }) => {
    if (!isCurrentDuplicatesScan(scanId)) return;
    const key = pathKey(group.files[0].path);
    const index = state.duplicateGroupIndex.get(key);
    if (index === undefined) {
        state.duplicateGroupIndex.set(key, state.duplicateGroups.length);
        state.duplicateGroups.push(group);
    }
    else state.duplicateGroups[index] = group;
    state.duplicatePendingGroups.set(key, group);
    // Batch a burst of small groups without rebuilding progress or every earlier card.
    if (duplicatesRenderTimer === null) duplicatesRenderTimer = setTimeout(() => {
        duplicatesRenderTimer = null;
        if (isDuplicatesActive() && state.duplicatesPhase === 'scanning') renderDuplicatesView();
    }, 250);
});

window.discreveal.onDuplicatesDone(({ scanId, groups, cancelled }) => {
    if (!isCurrentDuplicatesScan(scanId)) return;
    stopDuplicatesTimers();
    state.activeDuplicatesScanId = null;
    state.duplicatesCancelling = false;
    state.duplicateGroups = groups;
    state.duplicateSelection = new Set();
    state.duplicatesWereCancelled = Boolean(cancelled);
    state.duplicatesPhase = 'results';
    renderDuplicatesView();
});

function openDuplicateFinder() {
    if (state.duplicatesCleaning || isDuplicatesActive()) return;
    if (!state.duplicatesOpened) {
        abandonDuplicatesScan();
        state.duplicatesOpened = true;
        state.duplicatesPhase = 'pick';
    }
    setDuplicatesActive(true);
    renderDuplicatesView();
    refreshDuplicatesCacheStats();
}

el.btnDuplicates.onclick = () => activateFeature('duplicates');
el.btnDuplicatesEmpty.onclick = () => activateFeature('duplicates');

el.btnDupRestart.onclick = () => {
    state.duplicatesPhase = 'pick';
    renderDuplicatesView();
    refreshDuplicatesCacheStats();
};

el.btnDuplicatesClose.onclick = () => {
    activateFeature('scan');
};

function duplicateScanOptions(filter) {
    const min=filter.minEnabled === false ? 0 : Number(filter.min || 0), max=Number(filter.max || 0);
    const minBytes=Math.round(min*(filter.minUnit==='GB'?1024**3:1024**2));
    const maxBytes=Math.round(max*(filter.maxUnit==='GB'?1024**3:1024**2));
    if (!Number.isSafeInteger(minBytes) || !Number.isSafeInteger(maxBytes) || min<0 || max<0 || (maxBytes && minBytes>maxBytes)) throw {code:'invalidSizeLimits'};
    const types=extensions(filter.types || '');
    if (filter.typeMode==='include' && !types.length) throw {code:'duplicateNoTypes'};
    return {minBytes,maxBytes,typeMode:filter.typeMode,extensions:types};
}

function duplicateResultOptions() {
    return {
        filter:state.duplicateViewFilter, limit:state.duplicateVisibleLimit, busy:state.duplicatesCleaning, notice:state.duplicatesCleanupNotice,
        onFilterChange:(key,value)=>{state.duplicateViewFilter[key]=value;state.duplicateVisibleLimit=50;renderDuplicatesView();},
        onMore:()=>{state.duplicateVisibleLimit+=50;renderDuplicatesView();},
        onSelectAll:()=>{state.duplicateSelection=suggestedPaths(filterGroups(state.duplicateGroups,state.duplicateViewFilter));renderDuplicatesView();},
        onClearSelection:()=>{state.duplicateSelection.clear();renderDuplicatesView();},
        onClean:cleanSelectedDuplicates,
    };
}

/** Manual deletion still revalidates a retained original and never permits deleting the keeper. */
async function deleteSelectedDuplicates(mode) {
    if (state.duplicatesPhase !== 'results' || state.duplicatesCleaning) return;
    for (const path of [...state.duplicateSelection]) {
        const group = state.duplicateGroups.find((candidate) => candidate.files.some((file) => file.path === path));
        const file = group?.files.find((candidate) => candidate.path === path);
        if (!file) continue;
        const keeper=oldestFile(group);
        if (file===keeper) { state.duplicateSelection.delete(path); continue; }

        const deleted = await deleteItem({ path: file.path, name: file.name, size: group.size, isDir: false }, mode, duplicatesScope(), keeper);
        if (!deleted) continue;

        state.duplicateSelection.delete(path);
        group.files = group.files.filter((candidate) => candidate.path !== path);
        if (group.files.length < 2) {
            state.duplicateGroups = state.duplicateGroups.filter((candidate) => candidate !== group);
        }
    }
    renderDuplicatesView();
}

/** Conservative batch cleanup: one review, recycle bin, and a fresh backend byte comparison for each copy. */
async function cleanSelectedDuplicates() {
    if (state.duplicatesPhase!=='results' || state.duplicatesCleaning || state.scanning) return;
    const scope=duplicatesScope(), plan=[];
    for (const group of state.duplicateGroups) {
        const keeper=oldestFile(group);
        for (const file of group.files) if (file!==keeper && file.cleanupEligible && state.duplicateSelection.has(file.path)) plan.push({group,file,keeper});
    }
    if (!plan.length) { showError(t('dupNoSafeSelection')); return; }
    const size=plan.reduce((sum,item)=>sum+window.DiscRevealDuplicates.reclaimableBytes(item.file,item.group),0);
    const confirmed=await window.DiscRevealModal.confirm({title:t('dupClean'),confirmLabel:t('dupCleanConfirm'),danger:true,
        messageHtml:`<div class="dup-confirm-content"><p>${escapeHtml(t('dupCleanReview',{count:formatNumber(plan.length),size:formatBytes(size)}))}</p><p>${escapeHtml(t('dupCleanupPolicy'))}</p>`+
            `<ul>${plan.slice(0,12).map(({file,keeper})=>`<li>${escapeHtml(plainText(file.path))}<br><strong>${escapeHtml(t('dupOriginalBadge'))}:</strong> ${escapeHtml(plainText(keeper.path))}</li>`).join('')}</ul>`+
            (plan.length>12?`<p>${escapeHtml(t('dupMorePreview',{count:formatNumber(plan.length-12)}))}</p>`:'')+'</div>'});
    if (!confirmed || state.duplicatesCleaning || state.duplicatesResultId!==scope.id) return;
    state.duplicatesCleaning=true;
    state.duplicatesCleanupCancelling=false;
    updateBusyUi();
    state.duplicatesCleanupStartedAt=Date.now(); state.duplicatesCleanupTotal=plan.length;
    state.duplicatesCleanupDone=0;
    let removed=0, skipped=0, removedBytes=0, lastError='';
    try {
        for (const {group,file,keeper} of plan) {
            if (state.duplicatesCleanupCancelling) break;
            state.duplicatesCleanupDone=removed+skipped;
            state.duplicatesCleanupNotice=t('dupCleanupProgress',{done:formatNumber(removed+skipped),total:formatNumber(plan.length)});
            renderDuplicatesView();
            try {
                if (await window.discreveal.getProtection(file.path,scope)) { skipped++; continue; }
                if (state.duplicatesCleanupCancelling) break;
                await window.discreveal.deleteDuplicate(file.path,keeper.path,group.size,'trash',false,scope);
                removeItemFromTree(file.path); state.duplicateSelection.delete(file.path);
                group.files=group.files.filter(candidate=>candidate.path!==file.path);
                if (group.files.length<2) state.duplicateGroups=state.duplicateGroups.filter(candidate=>candidate!==group);
                removed++; removedBytes+=window.DiscRevealDuplicates.reclaimableBytes(file,group);
            } catch(error) {
                if (error?.code === 'duplicateCleanupCancelled') { state.duplicatesCleanupCancelling=true; break; }
                skipped++; lastError=describeError(error);
            }
        }
    } finally {
        state.duplicatesCleaning=false;
        state.duplicatesCleanupNotice=t('dupCleanupDone',{count:formatNumber(removed),size:formatBytes(removedBytes),skipped:formatNumber(skipped)})+(lastError?` · ${lastError}`:'')+(state.duplicatesCleanupCancelling ? ' · '+t('dupCleanupStopped',{count:formatNumber(plan.length-removed-skipped)}) : '');
        updateBusyUi();
        renderDuplicatesView(); renderCurrentLevel();
    }
}

async function cancelDuplicateCleanup() {
    if (!state.duplicatesCleaning || state.duplicatesCleanupCancelling) return;
    state.duplicatesCleanupCancelling=true; renderDuplicatesView();
    await withErrorBanner(() => window.discreveal.cancelDuplicateCleanup());
}

el.btnDupTrash.onclick = () => deleteSelectedDuplicates('trash');
el.btnDupPermanent.onclick = () => deleteSelectedDuplicates('permanent');
el.btnDupSecure.onclick = () => deleteSelectedDuplicates('secure');

/* ── Startup ──────────────────────────────────────── */

function activateFeature(feature) {
    if (!['scan', 'duplicates', 'cleanup'].includes(feature)) return;
    if (state.duplicatesCleaning || window.DiscRevealCleanup.isCleaning()) { showError(t('workInProgress')); return; }
    state.activeFeature = feature;
    updateScanCoverage();
    updateBusyUi();
    closeDropdowns();
    for (const name of ['scan', 'duplicates', 'cleanup']) {
        const button = byId('nav-' + name);
        button.classList.toggle('is-active', name === feature);
        button.setAttribute('aria-pressed', String(name === feature));
    }
    el.scanControls.classList.toggle('hidden', feature !== 'scan');
    el.duplicateControls.classList.toggle('hidden', feature !== 'duplicates');
    el.cleanupControls.classList.toggle('hidden', feature !== 'cleanup');
    el.cleanupView.classList.toggle('hidden', feature !== 'cleanup');
    el.treePanel.classList.toggle('hidden', feature !== 'scan' || !state.fullTree);
    el.progressToast.classList.toggle('hidden', feature !== 'scan' || !state.scanning);
    setDuplicatesActive(false);
    setChartsActive(false);
    setSearchActive(false);
    for (const view of [el.fileList, el.gridView, el.breadcrumbRow, el.emptyState]) view.classList.add('hidden');
    if (feature === 'duplicates') openDuplicateFinder();
    else if (feature === 'cleanup') window.DiscRevealCleanup.refresh();
    else {
        el.emptyState.classList.toggle('hidden', Boolean(state.fullTree));
        el.breadcrumbRow.classList.toggle('hidden', !state.fullTree);
        renderCurrentLevel();
    }
}
for (const name of ['scan', 'duplicates', 'cleanup']) byId('nav-' + name).onclick = () => activateFeature(name);
window.DiscRevealCleanup.init({
    controls: el.cleanupControls, body: el.cleanupBody,
    active: () => state.activeFeature === 'cleanup',
    otherBusy: () => state.scanning || state.duplicatesPhase === 'scanning' || state.duplicatesCleaning,
    onError: showError,
    onBusyChange: updateBusyUi,
    onRemoved: () => {
        scanCache.clear(); state.fullTree = null; state.zoomStack = [];
        state.duplicateGroups = []; state.duplicateSelection.clear();
        state.duplicatesOpened = false; state.duplicatesPhase = 'pick';
    },
});

async function init() {
    window.discreveal.getVersion().then(
        (version) => {
            el.appVersion.textContent = `v${version}`;
        },
        () => {}
    );

    try {
        [state.settings, state.drives] = await Promise.all([
            window.discreveal.loadSettings(),
            window.discreveal.listDrives(),
        ]);
    } catch (error) {
        applyLanguage(state.settings.language);
        document.body.classList.remove('i18n-pending');
        showError(t('initFailed', { detail: describeError(error) }));
        showInitRetry();
        return;
    }

    state.initialized = true;
    document.getElementById('init-retry')?.remove();

    applyLanguage(state.settings.language);
    document.body.classList.remove('i18n-pending');

    // The remembered drive may be gone; keep the default selection then.
    if (state.drives.some((drive) => drive.path === state.settings.lastDrive)) {
        el.driveSelect.value = state.settings.lastDrive;
    }
    el.btnScan.disabled = state.drives.length === 0;

    el.minFolderSizeUnit.value = state.settings.minFolderSizeUnit;
    el.measureAllocated.checked = Boolean(state.settings.measureAllocated);
    el.minFolderSize.value = formatSizeValue(convertSize(state.settings.minFolderSizeMB, 'MB', state.settings.minFolderSizeUnit));
    el.maxFolderSizeUnit.value = state.settings.maxFolderSizeUnit;
    el.maxFolderSize.value = formatSizeValue(convertSize(state.settings.maxFolderSizeGB, 'GB', state.settings.maxFolderSizeUnit));
}

function showInitRetry() {
    el.btnScan.disabled = true;
    let retry = document.getElementById('init-retry');
    if (!retry) {
        retry = document.createElement('button'); retry.id = 'init-retry'; retry.className = 'pill-btn pill-btn-ghost';
        el.errorBanner.appendChild(retry);
    }
    retry.textContent = t('retry');
    retry.onclick = async () => { retry.disabled = true; await init(); if (retry.isConnected) retry.disabled = false; };
}

function updateBusyUi() {
    const cleanup = window.DiscRevealCleanup;
    const cleaning = state.duplicatesCleaning || Boolean(cleanup?.isCleaning());
    const active = state.scanning ? 'scan' : state.duplicatesPhase === 'scanning' || state.duplicatesCleaning ? 'duplicates' : cleanup?.isBusy() ? 'cleanup' : null;
    for (const name of ['scan', 'duplicates', 'cleanup']) {
        const button = byId('nav-' + name);
        button.disabled = cleaning && name !== state.activeFeature;
        button.classList.toggle('is-running', name === active);
        button.title = name === active ? t('workInProgress') : '';
    }
    let status = document.getElementById('background-work');
    if (!status) {
        status = document.createElement('button'); status.id = 'background-work'; status.className = 'pill-btn pill-btn-ghost background-work';
        el.scanControls.parentElement.appendChild(status);
    }
    status.classList.toggle('hidden', !active || active === state.activeFeature);
    status.textContent = active ? t('backgroundWork', { feature: t({scan:'navScan',duplicates:'navDuplicates',cleanup:'navCleanup'}[active]) }) : '';
    status.onclick = () => active && activateFeature(active);
}

init();
