/**
 * Builds `src-tauri/icons/icon.ico` with every size rendered natively from
 * the SVG sources, instead of scaling one bitmap down. Windows picks the
 * entry closest to the size it needs, so each common size (including the
 * ones used at 125 % and 150 % display scaling) gets its own sharp bitmap.
 *
 * Small sizes use a simplified, bolder design (`icon-small.svg`) that stays
 * readable at 16 to 32 pixels. Rendering uses headless Microsoft Edge, which
 * is present on every machine that can build the app (WebView2 dependency).
 *
 * Usage: node scripts/buildIcon.js
 */
const { execFileSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { pathToFileURL } = require('node:url');

const projectRoot = path.join(__dirname, '..');
const iconsDir = path.join(projectRoot, 'src-tauri', 'icons');

const FULL_DESIGN = path.join(projectRoot, 'renderer', 'icon.svg');
const SMALL_DESIGN = path.join(iconsDir, 'icon-small.svg');
const SMALL_DESIGN_MAX_SIZE = 32;
const SIZES = [16, 20, 24, 30, 32, 36, 40, 48, 60, 64, 72, 80, 96, 128, 256];
// Rendered once more, only for the standalone icon.png, at the size `npx tauri icon` used.
const STANDALONE_PNG_SIZE = 512;

const { discRevealPngMetadata } = require('./provenance');

const EDGE_CANDIDATES = ['ProgramFiles(x86)', 'ProgramFiles']
    .map((variable) => process.env[variable])
    .filter(Boolean)
    .map((folder) => path.join(folder, 'Microsoft', 'Edge', 'Application', 'msedge.exe'));

function findEdge() {
    const edge = EDGE_CANDIDATES.find((candidate) => fs.existsSync(candidate));
    if (!edge) throw new Error('Microsoft Edge was not found; it is required to render the icon.');
    return edge;
}

/** Renders `svgPath` as a transparent PNG of exactly `size` x `size` pixels. */
function renderPng(edge, svgPath, size, workDir) {
    const svg = fs.readFileSync(svgPath, 'utf-8').replace(/width="\d+" height="\d+"/, `width="${size}" height="${size}"`);
    const htmlPath = path.join(workDir, `icon-${size}.html`);
    const pngPath = path.join(workDir, `icon-${size}.png`);
    fs.writeFileSync(
        htmlPath,
        `<!doctype html><meta charset="utf-8"><style>html,body{margin:0;background:transparent;overflow:hidden}svg{display:block}</style>${svg}`
    );
    execFileSync(edge, [
        '--headless',
        '--disable-gpu',
        '--hide-scrollbars',
        '--default-background-color=00000000',
        `--window-size=${size},${size}`,
        `--screenshot=${pngPath}`,
        pathToFileURL(htmlPath).href,
    ], { stdio: 'ignore' });
    return discRevealPngMetadata(fs.readFileSync(pngPath));
}

/** Packs PNG images into an ICO container (PNG-compressed entries). */
function buildIco(images) {
    const headerSize = 6 + 16 * images.length;
    const header = Buffer.alloc(headerSize);
    header.writeUInt16LE(0, 0);
    header.writeUInt16LE(1, 2);
    header.writeUInt16LE(images.length, 4);

    let offset = headerSize;
    images.forEach(({ size, data }, index) => {
        const entry = 6 + 16 * index;
        header.writeUInt8(size === 256 ? 0 : size, entry);
        header.writeUInt8(size === 256 ? 0 : size, entry + 1);
        header.writeUInt8(0, entry + 2);
        header.writeUInt8(0, entry + 3);
        header.writeUInt16LE(1, entry + 4);
        header.writeUInt16LE(32, entry + 6);
        header.writeUInt32LE(data.length, entry + 8);
        header.writeUInt32LE(offset, entry + 12);
        offset += data.length;
    });
    return Buffer.concat([header, ...images.map((image) => image.data)]);
}

const edge = findEdge();
const workDir = fs.mkdtempSync(path.join(os.tmpdir(), 'discreveal-icon-'));
try {
    const images = SIZES.map((size) => ({
        size,
        data: renderPng(edge, size <= SMALL_DESIGN_MAX_SIZE ? SMALL_DESIGN : FULL_DESIGN, size, workDir),
    }));
    const icoTarget = path.join(iconsDir, 'icon.ico');
    fs.writeFileSync(icoTarget, buildIco(images));
    console.log(`Wrote ${icoTarget} with sizes ${SIZES.join(', ')}`);

    const pngTarget = path.join(iconsDir, 'icon.png');
    fs.writeFileSync(pngTarget, renderPng(edge, FULL_DESIGN, STANDALONE_PNG_SIZE, workDir));
    console.log(`Wrote ${pngTarget} at ${STANDALONE_PNG_SIZE}x${STANDALONE_PNG_SIZE}`);
} finally {
    fs.rmSync(workDir, { recursive: true, force: true });
}
