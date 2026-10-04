/** Promote a completely generated set; never present interrupted writes as a ready release. */
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const names = ['discreveal-portable.exe', 'THIRD-PARTY-LICENSES.html'];
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');

function manifestFor(directory, version) {
    return {version, assets: Object.fromEntries(names.map(name => {
        const bytes = fs.readFileSync(path.join(directory, name));
        if (!bytes.length) throw Error(`Empty release asset: ${name}`);
        return [name, {bytes: bytes.length, sha256: hash(bytes)}];
    }))};
}

function verifyRelease(directory, version) {
    if (fs.existsSync(path.join(directory, '.release-pending'))) throw Error('Release promotion is incomplete; rebuild before publishing');
    const expected = JSON.parse(fs.readFileSync(path.join(directory, 'release-manifest.json')));
    const actual = manifestFor(directory, version);
    if (JSON.stringify(expected) !== JSON.stringify(actual)) throw Error('Release manifest does not match the complete artifact set');
    return actual;
}

function promoteRelease(staged, destination, version, sourceFiles = [], afterWrite = () => {}) {
    const manifest = manifestFor(staged, version);
    fs.mkdirSync(destination, {recursive: true});
    const marker = path.join(destination, '.release-pending');
    if (fs.existsSync(marker)) throw Error('An earlier promotion was interrupted; inspect .release-pending before retrying');
    const writes = names.map(name => [path.join(destination, name), fs.readFileSync(path.join(staged, name))]);
    writes.push(...sourceFiles, [path.join(destination, 'release-manifest.json'), Buffer.from(JSON.stringify(manifest, null, 2) + '\n')]);
    const before = writes.map(([file]) => [file, fs.existsSync(file) ? fs.readFileSync(file) : null]);
    fs.writeFileSync(marker, JSON.stringify({version,started:new Date().toISOString()}));
    try {
        writes.forEach(([file, bytes], index) => { fs.writeFileSync(file, bytes); afterWrite(index); });
        fs.unlinkSync(marker);
        verifyRelease(destination, version);
        return manifest;
    } catch (error) {
        let rollbackError;
        for (const [file, bytes] of before) {
            try { if (bytes === null) { if (fs.existsSync(file)) fs.unlinkSync(file); } else fs.writeFileSync(file, bytes); }
            catch (failure) { rollbackError = failure; }
        }
        if (!rollbackError) {
            if (fs.existsSync(marker)) fs.unlinkSync(marker);
        } else {
            if (!fs.existsSync(marker)) fs.writeFileSync(marker, JSON.stringify({version,rollbackFailed:true}));
            error.message += `; rollback incomplete: ${rollbackError.message}; pending marker retained`;
        }
        throw error;
    }
}
module.exports = {manifestFor, verifyRelease, promoteRelease};
