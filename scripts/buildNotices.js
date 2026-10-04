/** Generates the Windows dependency index from the locked Cargo graph; full texts use cargo-about. */
const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const root = path.join(__dirname, '..');
const metadata = JSON.parse(execFileSync('cargo', [
    'metadata', '--locked', '--offline', '--format-version', '1', '--filter-platform', 'x86_64-pc-windows-msvc',
], { cwd: path.join(root, 'src-tauri'), encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 }));
const included = new Set(metadata.resolve.nodes.map(node => node.id));
const crates = metadata.packages.filter(crate => included.has(crate.id) && crate.name !== 'discreveal')
    .sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version));
if (crates.some(crate => !crate.license)) throw new Error('A dependency has no declared license; review it before distributing.');
const filename = path.join(root, 'THIRD-PARTY-NOTICES.md');
const previous = fs.readFileSync(filename, 'utf8');
const start = previous.indexOf('| Crate | Version | Licence |');
const end = previous.indexOf('\n## Bundled files', start);
if (start < 0 || end < 0) throw new Error('Third-party notice table markers are missing.');
const table = '| Crate | Version | Licence |\n|---|---|---|\n' +
    crates.map(crate => `| ${crate.name} | ${crate.version} | ${crate.license} |`).join('\n') + '\n';
fs.writeFileSync(filename, previous.slice(0, start) + table + previous.slice(end), 'utf8');
console.log(`Updated Windows license index: ${crates.length} crates`);
