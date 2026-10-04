const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');

function cache() {
    const context = { window: {} };
    vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../renderer/scanSnapshots.js'), 'utf8'), context);
    return context.window.DiscRevealSnapshots.createSnapshotCache();
}

test('live snapshots reuse complete subtrees while other folder sizes grow', () => {
    const snapshots = cache();
    const completed = { path: 'C:\\A', isDir: true, size: 13, children: [{ path: 'C:\\A\\x', size: 13 }] };
    snapshots.merge({ children: [completed, { path: 'C:\\B', isDir: true, inProgress: true, size: 2 }] });
    const tree = snapshots.merge({ size: 22, children: [
        { path: 'C:\\B', isDir: true, inProgress: true, size: 9 },
        { path: 'C:\\A', isDir: true, size: 13, children: [] },
    ] }, ['C:\\A']);
    assert.equal(tree.children[1], completed);
    assert.equal(tree.children[0].size, 9);
    assert.equal(tree.children.reduce((total, child) => total + child.size, 0), tree.size);
    assert.equal(tree.children[1].children[0].path, 'C:\\A\\x');
});

test('folded-out subtrees can return, and starting another scan drops their references', () => {
    const snapshots = cache();
    const completed = { path: 'C:\\A', isDir: true, size: 13, children: [{ size: 13 }] };
    snapshots.merge({ children: [completed] });
    snapshots.merge({ children: [{ path: '', overflowCount: 2, size: 24 }] });
    assert.equal(snapshots.merge({ children: [{ path: 'C:\\A', isDir: true, size: 13 }] }, ['C:\\A']).children[0], completed);
    snapshots.clear();
    const unknown = snapshots.merge({ children: [{ path: 'C:\\A', isDir: true, size: 13 }] }, ['C:\\A']).children[0];
    assert.equal(unknown.inProgress, true);
    assert.equal(unknown.size, 13);
    assert.notEqual(unknown, completed);
});

test('newly completed folders replace progress stubs without caching unfinished folders', () => {
    const snapshots = cache();
    snapshots.merge({ children: [{ path: 'C:\\A', isDir: true, inProgress: true, size: 2 }] });
    assert.equal(snapshots.merge({ children: [{ path: 'C:\\A', isDir: true, size: 2 }] }, ['C:\\A']).children[0].inProgress, true);
    const completed = { path: 'C:\\A', isDir: true, size: 3, children: [{ size: 3 }] };
    snapshots.merge({ children: [completed] });
    assert.equal(snapshots.merge({ children: [{ path: 'C:\\A', isDir: true, size: 3 }] }, ['C:\\A']).children[0], completed);
});
