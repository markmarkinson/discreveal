/**
 * Builds the release binary and copies it to `dist/discreveal-portable.exe`.
 *
 * The release executable embeds the whole frontend and needs no files beside
 * it, so it is the portable build as-is; WebView2 ships with Windows 10/11.
 */
const { execFileSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const projectRoot = path.join(__dirname, '..');
const readFile = (...segments) => fs.readFileSync(path.join(projectRoot, ...segments), 'utf-8');

const { version } = JSON.parse(readFile('package.json'));
const tauriVersion = JSON.parse(readFile('src-tauri', 'tauri.conf.json')).version;
const cargoVersion = /^version\s*=\s*"([^"]+)"/m.exec(readFile('src-tauri', 'Cargo.toml'))?.[1];

// The three manifests must agree, otherwise the exe metadata and the file name diverge.
if (version !== tauriVersion || version !== cargoVersion) {
    console.error(`Version mismatch: package.json ${version}, tauri.conf.json ${tauriVersion}, Cargo.toml ${cargoVersion}`);
    process.exit(1);
}

require('./stampAssets')();

console.log('Checking release quality gates...');
for (const args of [
    ['fmt', '--manifest-path', 'src-tauri/Cargo.toml', '--', '--check'],
    ['test', '--manifest-path', 'src-tauri/Cargo.toml', '--locked'],
    ['clippy', '--manifest-path', 'src-tauri/Cargo.toml', '--all-targets', '--locked', '--', '-D', 'warnings'],
]) execFileSync('cargo', args, { cwd: projectRoot, stdio: 'inherit' });

console.log('Building release binary (LTO enabled, this takes a few minutes)...');
execFileSync('npm', ['run', 'tauri:build'], { cwd: projectRoot, stdio: 'inherit', shell: true });

const targetDir = process.env.CARGO_TARGET_DIR
    ? path.resolve(projectRoot, 'src-tauri', process.env.CARGO_TARGET_DIR)
    : path.join(projectRoot, 'src-tauri', 'target');
const builtExecutable = path.join(targetDir, 'release', 'discreveal.exe');
if (!fs.existsSync(builtExecutable)) {
    console.error(`Build output not found: ${builtExecutable}`);
    process.exit(1);
}

const distDir = path.join(projectRoot, 'dist');
const stagingRoot = path.join(projectRoot, '.internal');
fs.mkdirSync(stagingRoot, { recursive: true });
const staged = fs.mkdtempSync(path.join(stagingRoot, 'release-stage-'));

const portableExecutable = path.join(staged, 'discreveal-portable.exe');
fs.copyFileSync(builtExecutable, portableExecutable);

const sizeMb = (fs.statSync(portableExecutable).size / 1024 / 1024).toFixed(2);
console.log(`Portable executable staged (${sizeMb} MB)`);

// Regenerated fresh from Cargo.lock on every release (never committed, like the exe itself):
// the full copyright + license text of every bundled crate, required by MIT/Apache-2.0/BSD/...
// to travel with a binary distribution. THIRD-PARTY-NOTICES.md in the repo is only the short
// crate-to-license-id table; this is the complete text those licenses actually require.
execFileSync(process.execPath, [path.join(projectRoot, 'scripts', 'buildNotices.js')], { cwd: projectRoot, stdio: 'inherit' });
console.log('Generating third-party license texts (cargo-about)...');
const licenseReport = path.join(staged, 'THIRD-PARTY-LICENSES.html');
try {
    execFileSync('cargo', ['about', 'generate', 'about.hbs', '-o', licenseReport], {
        cwd: path.join(projectRoot, 'src-tauri'),
        stdio: 'inherit',
        shell: true,
    });
} catch (error) {
    console.error(
        'cargo-about failed or is not installed. Install it with:\n' +
        '  cargo install cargo-about --features cli\n' +
        `Original error: ${error.message}`
    );
    process.exit(1);
}
console.log(`Third-party license texts ready: ${licenseReport}`);
// The application's custom terms are separate from dependency license grants.
const escapeLicense = text => text.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
const appLicense = `<section id="discreveal-license"><h2>DiscReveal Source-Available License 1.0</h2><pre class="license-text">${escapeLicense(readFile('LICENSE'))}</pre></section>`;
fs.writeFileSync(licenseReport, fs.readFileSync(licenseReport, 'utf8').replace('</main>', `${appLicense}</main>`));
// Keep the website's full notices tied to the same report shipped with the app.
const manifest = require('./releaseArtifacts').manifestFor(staged, version);
const executable = manifest.assets['discreveal-portable.exe'];
const facts = {version, file:'discreveal-portable.exe', bytes:executable.bytes, sha256:executable.sha256};
require('./releaseArtifacts').promoteRelease(staged, distDir, version, [
    [path.join(projectRoot, 'docs-src', 'third-party-licenses.html'), fs.readFileSync(licenseReport)],
    [path.join(projectRoot, 'docs-src', 'release.json'), Buffer.from(JSON.stringify(facts,null,2)+'\n')],
], index => {
    if (index !== 4) return;
    execFileSync(process.execPath, [path.join(projectRoot, 'scripts', 'buildSeoPages.js')], {cwd:projectRoot, stdio:'inherit'});
    execFileSync('npm', ['test'], {cwd:projectRoot, stdio:'inherit', shell:true});
});
// Only this audit-owned staging directory is removed; portable preferences remain untouched.
if (!path.resolve(staged).startsWith(path.resolve(stagingRoot)+path.sep)) throw Error('Staging directory escaped internal root');
fs.rmSync(staged, {recursive:true});
console.log(`Complete portable release ready: ${path.join(distDir, 'discreveal-portable.exe')}`);
