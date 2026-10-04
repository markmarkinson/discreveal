/** discReveal by markMarkinson: release facts shared by HTML and text guides. */
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const root = path.join(__dirname, '..');
const source = path.join(root, 'docs-src', 'release.json');
const artifact = path.join(root, 'dist', 'discreveal-portable.exe');
const version = () => JSON.parse(fs.readFileSync(path.join(root,'package.json'))).version;
const digest = bytes => crypto.createHash('sha256').update(bytes).digest('hex');

function writeReleaseMetadata() {
    const bytes = fs.readFileSync(artifact);
    const facts = {version:version(), file:'discreveal-portable.exe', bytes:bytes.length, sha256:digest(bytes)};
    fs.writeFileSync(source, JSON.stringify(facts,null,2)+'\n');
    return facts;
}

function readReleaseMetadata() {
    const facts = JSON.parse(fs.readFileSync(source));
    if (facts.version !== version() || facts.file !== 'discreveal-portable.exe' ||
        !Number.isSafeInteger(facts.bytes) || facts.bytes <= 0 || !/^[a-f0-9]{64}$/.test(facts.sha256)) {
        throw Error('Release facts are missing or stale. Run npm run dist:portable before site:build.');
    }
    if (fs.existsSync(artifact)) {
        const bytes = fs.readFileSync(artifact);
        if (bytes.length !== facts.bytes || digest(bytes) !== facts.sha256) {
            throw Error('Release facts do not match dist/discreveal-portable.exe. Rebuild the portable release.');
        }
    }
    return facts;
}

module.exports = {writeReleaseMetadata, readReleaseMetadata};
