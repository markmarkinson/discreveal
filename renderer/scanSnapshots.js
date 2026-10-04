/** Reuses completed drive-scan subtrees between live snapshots. Session only. */
(function () {
    'use strict';

    function createSnapshotCache() {
        const completed = new Map();
        return {
            clear() { completed.clear(); },
            merge(tree, reusedPaths = []) {
                const reused = new Set(reusedPaths);
                tree.children = (tree.children || []).map((child) => {
                    if (reused.has(child.path)) {
                        // A missing reference must remain visibly incomplete.
                        // The full done event independently restores all nodes.
                        return completed.get(child.path) || { ...child, inProgress: true };
                    }
                    if (child.isDir && !child.inProgress && child.path) {
                        completed.set(child.path, child);
                    }
                    return child;
                });
                return tree;
            },
        };
    }

    window.DiscRevealSnapshots = { createSnapshotCache };
})();
