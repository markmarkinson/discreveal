/** Builds localized notices pages from the release report and component index. */
const fs = require('node:fs');
const path = require('node:path');
const root = path.join(__dirname, '..');
const escape = value => String(value).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

module.exports = function buildNoticesPages({flags}) {
    const read = file => fs.readFileSync(path.join(root, file), 'utf8');
    const rows = [...read('THIRD-PARTY-NOTICES.md').matchAll(/^\| ([^|]+) \| ([^|]+) \| ([^|]+) \|$/gm)]
        .map(match => match.slice(1).map(value=>value.trim()))
        .filter(row => row[0] !== 'Crate' && row[0] !== 'Component');
    const crates = rows.filter(row=>!row[2].includes('`'));
    const report = read('docs-src/third-party-licenses.html');
    const licenses = [...report.matchAll(/<h3 id="[^"]+">([^<]+)<\/h3>\s*<h4>Used by:<\/h4>\s*<ul class="license-used-by">([\s\S]*?)<\/ul>\s*<pre class="license-text">([\s\S]*?)<\/pre>/g)];
    if (!crates.length || !licenses.length) throw Error('Missing component index or release license texts');
    const font = read('renderer/fonts/LICENSE.txt');
    const tslib = read('renderer/vendor/tauri-api/external/tslib/tslib.es6.js').split('/* global')[0].replace(/^\/\*\*+\s*|\*+ \/\s*$/g,'').trim();
    for (const lang of ['en', 'de']) {
        const de = lang === 'de';
        const home = de ? '/de/' : '/';
        const route = de ? '/de/third-party-notices/' : '/third-party-notices/';
        const title = de ? 'Drittanbieter & Lizenzen' : 'Third-party components & licenses';
        const back = de ? 'Zur Startseite' : 'Back to home';
        const intro = de ? 'DiscReveal nutzt Open-Source-Komponenten. Hier findest du die verwendeten Bibliotheken, ihre Versionen und die zugehörigen Lizenztexte.' : 'DiscReveal uses open-source components. Find the libraries, their versions and their license texts here.';
        const component = de ? 'Komponente' : 'Component';
        const license = de ? 'Lizenz' : 'License';
        const used = de ? 'Verwendet von' : 'Used by';
        const disclosure = (name, body) => `<details class="faq-item notices-license"><summary><span>${name}</span><span class="faq-plus" aria-hidden="true"><svg width="12" height="12" viewBox="0 0 24 24" fill="none"><path d="M12 5v14M5 12h14" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/></svg></span></summary><div class="notices-license-body">${body}</div></details>`;
        const groups = new Map();
        for (const match of licenses) {
            if (!groups.has(match[1])) groups.set(match[1], []);
            groups.get(match[1]).push(`<div class="notices-license-entry"><p>${used}</p><ul>${match[2]}</ul><pre>${match[3]}</pre></div>`);
        }
        const texts = [...groups].map(([name, entries])=>disclosure(name, entries.join('\n'))).join('\n');
        const html = `<!DOCTYPE html>
<html lang="${lang}" data-disc-reveal="discReveal" data-mark-markinson="markMarkinson"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="color-scheme" content="dark">
<title>${title} · DiscReveal</title><meta name="description" content="${intro}"><link rel="canonical" href="https://discreveal.com${route}">
<link rel="alternate" hreflang="en" href="https://discreveal.com/third-party-notices/"><link rel="alternate" hreflang="de" href="https://discreveal.com/de/third-party-notices/">
<link rel="icon" href="/favicon.svg"><link rel="stylesheet" href="/style.css"><link rel="stylesheet" href="/notices.css"></head><body>
<nav class="site-nav"><div class="site-nav-inner"><a class="brand" href="${home}"><img src="/icon.png" width="28" height="28" alt=""><span>DiscReveal</span></a><div class="nav-right"><a class="language-btn" href="${de ? '/third-party-notices/' : '/de/third-party-notices/'}" lang="${de ? 'en' : 'de'}" aria-label="${de ? 'Switch to English' : 'Auf Deutsch umschalten'}">${flags[de ? 'en' : 'de']}</a><a class="pill-btn pill-btn-ghost" href="${home}">${back}</a></div></div></nav>
<main class="wrap notices-main"><header class="notices-header"><span class="badge">DiscReveal Portable</span><h1>${title}</h1><p>${intro}</p></header>
<section aria-labelledby="app-license-title"><h2 id="app-license-title">${de ? 'DiscReveal: kostenlos, Quellcode öffentlich einsehbar' : 'DiscReveal: free, with publicly available source code'}</h2><p>${de ? 'Die unveränderte App darfst du privat und intern im Unternehmen kostenlos nutzen. Änderungen und kommerzieller Weitervertrieb benötigen eine schriftliche Erlaubnis. Die eigenen Lizenzen der folgenden Komponenten bleiben bestehen.' : 'Use the unmodified app free of charge personally or internally at work. Modifications and commercial redistribution require written permission. The components below retain their own licenses.'}</p>${disclosure('DiscReveal Source-Available License 1.0', `<pre>${escape(read('LICENSE'))}</pre>`)}</section>
<section aria-labelledby="components-title"><div class="notices-section-heading"><h2 id="components-title">${de ? 'Verwendete Komponenten' : 'Components used'}</h2><span class="notices-count" role="status" data-total="${crates.length}">${crates.length}</span></div>
<label class="notices-search"><span>${de ? 'Bibliothek, Version oder Lizenz suchen' : 'Search library, version or license'}</span><input id="notices-search" type="search" placeholder="${de ? 'Zum Beispiel: tauri, MIT …' : 'For example: tauri, MIT …'}"></label>
<div class="notices-table-wrap"><table><thead><tr><th>${component}</th><th>Version</th><th>${license}</th></tr></thead><tbody>${crates.map(row=>`<tr>${row.map(value=>`<td>${escape(value)}</td>`).join('')}</tr>`).join('\n')}</tbody></table></div><p id="notices-empty" hidden>${de ? 'Keine passenden Komponenten gefunden.' : 'No matching components found.'}</p></section>
<section aria-labelledby="bundled-title"><h2 id="bundled-title">${de ? 'Schriftarten & eingebundener Code' : 'Fonts & bundled code'}</h2>
${disclosure('Plus Jakarta Sans · JetBrains Mono <small>SIL OFL 1.1</small>',`<pre>${escape(font)}</pre>`)}
${disclosure('@tauri-apps/api <small>Apache-2.0 OR MIT</small>',`<p>Copyright 2019–2024 Tauri Programme within The Commons Conservancy</p><p>${de ? 'Lizenztexte: Apache License 2.0 und MIT, siehe unten.' : 'License texts: Apache License 2.0 and MIT, listed below.'}</p>`)}
${disclosure('tslib <small>0BSD</small>',`<pre>${escape(tslib)}</pre>`)}</section>
<section aria-labelledby="licenses-title"><h2 id="licenses-title">${de ? 'Vollständige Lizenztexte' : 'Full license texts'}</h2><p class="notices-muted">${de ? 'Nach Lizenz und den jeweils zugehörigen Komponenten gruppiert. Die Originaltexte bleiben unverändert.' : 'Grouped by license and the components it covers. Original license texts are preserved.'}</p>${texts}</section></main>
<footer class="wrap"><div class="footer-row"><a href="${home}">${back}</a><p class="footer-note">DiscReveal · ${de?'Quellcode öffentlich einsehbar':'Source code publicly available'}</p></div></footer><script src="/notices.js"></script></body></html>`;
        const directory = path.join(root, 'docs', route);
        fs.mkdirSync(directory, {recursive:true});
        fs.writeFileSync(path.join(directory,'index.html'), html);
        console.log(`Wrote ${path.relative(root, directory)}/index.html (${crates.length} components, ${licenses.length} license notices)`);
    }
};
