/**
 * Exposes the backend to the UI scripts as `window.discreveal`.
 *
 * This is the only file that talks to Tauri directly. All other scripts use
 * this narrow API, which mirrors the commands registered in `src-tauri`.
 */
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { listen } from '@tauri-apps/api/event';

const appWindow = getCurrentWindow();

/** Subscribes to a backend event; the callback receives the payload only. */
const subscribe = (eventName) => (callback) => listen(eventName, (event) => callback(event.payload));

window.discreveal = Object.freeze({
    minimizeWindow: () => appWindow.minimize(),
    toggleMaximizeWindow: () => appWindow.toggleMaximize(),
    closeWindow: () => appWindow.close(),

    getVersion: () => invoke('plugin:app|version'),
    listDrives: () => invoke('list_drives'),
    loadSettings: () => invoke('load_settings'),
    saveSettings: (settings) => invoke('save_settings', { settings }),
    pickFolder: (title) => invoke('pick_folder', { title }),
    getFileIcons: (paths) => invoke('get_file_icons', { paths }),
    openPath: (targetPath, scope) => invoke('open_path', { targetPath, scope }),
    checkForUpdates: (language) => invoke('check_for_updates', { language }),
    getProtection: (targetPath, scope) => invoke('get_protection', { targetPath, scope }),
    deleteItem: (targetPath, mode, acknowledgeProtected, scope) =>
        invoke('delete_item', { targetPath, mode, acknowledgeProtected, scope }),
    deleteDuplicate: (targetPath, keeperPath, expectedSize, mode, acknowledgeProtected, scope) =>
        invoke('delete_duplicate', { targetPath, keeperPath, expectedSize, mode, acknowledgeProtected, scope }),

    startScan: (options) => invoke('start_scan', { options }),
    cancelScan: () => invoke('cancel_scan'),
    expandFolder: (targetPath, scope, operationId, offset, measureAllocated = false) => invoke('expand_folder', { targetPath, scope, operationId, options: { offset, measureAllocated } }),
    cancelExpandFolder: (operationId) => invoke('cancel_expand_folder', { operationId }),
    onScanProgress: subscribe('scan:progress'),
    onScanPartial: subscribe('scan:partial'),
    onScanDone: subscribe('scan:done'),
    onScanError: subscribe('scan:error'),
    onOperationStatus: subscribe('operation:status'),
    operationStatuses: () => invoke('get_operation_statuses'),

    findDuplicates: (scanId, roots, excludes, useCache, options) => invoke('find_duplicates', { scanId, roots, excludes, useCache, options }),
    cancelDuplicates: () => invoke('cancel_duplicates'),
    cancelDuplicateCleanup: () => invoke('cancel_duplicate_cleanup'),
    onDuplicatesProgress: subscribe('duplicates:progress'),
    onDuplicatesGroup: subscribe('duplicates:group'),
    onDuplicatesDone: subscribe('duplicates:done'),
    clearHashCache: () => invoke('clear_hash_cache'),
    hashCacheStats: () => invoke('hash_cache_stats'),
    cleanupRules: () => invoke('cleanup_rules'),
    analyzeCleanup: (options) => invoke('analyze_cleanup', { options }),
    executeCleanup: (planId, groupIds, excludedIds) => invoke('execute_cleanup', { planId, groupIds, excludedIds, acknowledged: true }),
    cancelCleanup: () => invoke('cancel_cleanup'),
    cleanupItems: (planId, groupId, offset) => invoke('cleanup_items', { planId, groupId, offset }),
    openCleanupSettings: () => invoke('open_cleanup_settings'),
    runWindowsCleanup: (drive) => invoke('run_windows_cleanup', { drive }),
    analyzeWindowsCleanup: (drive = null) => invoke('analyze_windows_cleanup', { drive }),
    executeWindowsCleanup: (planId, groupIds, rollbackAcknowledged) => invoke('execute_windows_cleanup', { planId, groupIds, acknowledged: true, rollbackAcknowledged }),
    restartCleanupAsAdmin: () => invoke('restart_cleanup_as_admin', { acknowledged: true }),
    onWindowsCleanupProgress: subscribe('cleanup:system-progress'),
    pickCleanupProject: (title) => invoke('pick_cleanup_project', { title }),
    onCleanupProgress: subscribe('cleanup:progress'),
    cleanupRecycleBins: () => invoke('cleanup_recycle_bins'),
    emptyCleanupRecycleBin: (drive) => invoke('empty_cleanup_recycle_bin', { drive, acknowledged: true }),
});
