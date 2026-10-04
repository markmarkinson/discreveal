/** Generate readable product references from the same facts and copy as the site. */
const fs = require('node:fs');
const path = require('node:path');
const root = path.join(__dirname,'..');
const base = 'https://discreveal.com';
const repo = 'https://github.com/markmarkinson/discreveal';
const download = repo + '/releases/latest/download/discreveal-portable.exe';
const plain = text => text.replace(/<[^>]+>/g,'');

module.exports = function buildProductGuides({dictionaries,release,faqEntries}) {
    const data = JSON.parse(fs.readFileSync(path.join(root,'docs/benchmarks/scan-0.2.3.json')));
    const guides = {};
    for (const lang of ['en','de']) {
        const de = lang === 'de', dict = dictionaries[lang], locale = de ? 'de-DE' : 'en-US';
        const size = (release.bytes / 1e6).toLocaleString(locale,{minimumFractionDigits:1,maximumFractionDigits:1});
        const link = de ? base+'/de/' : base+'/';
        const facts = de ? [
            'Windows 10 oder 11, 64-Bit (x64).',
            `Kostenlos, Quellcode öffentlich einsehbar. DiscReveal Source-Available License 1.0. Autor: markMarkinson.`,
            `Portable EXE ohne DiscReveal-Installation. Größe: ${size} MB (${release.bytes.toLocaleString(locale)} Bytes).`,
            'Microsoft Edge WebView2 muss vorhanden sein. Die App bietet bei Bedarf einen Installationslink.',
            'Deutsch und Englisch. Lokale Festplatten, SSDs und USB-Laufwerke mit Laufwerksbuchstaben.',
            'Keine unterstützten macOS- oder Linux-Downloads; Netzlaufwerke werden nicht zur Auswahl angeboten.',
        ] : [
            'Windows 10 or 11, 64-bit (x64).',
            'Free, with publicly available source code. DiscReveal Source-Available License 1.0. Author: markMarkinson.',
            `Portable executable; no DiscReveal installation. Size: ${size} MB (${release.bytes.toLocaleString(locale)} bytes).`,
            'Microsoft Edge WebView2 is required. The app offers an installation link if it is missing.',
            'English and German. Local hard drives, SSDs and USB drives with drive letters.',
            'No supported macOS or Linux downloads; network drives are not offered in the drive picker.',
        ];
        const features = Array.from({length:7},(_,i)=>`### ${plain(dict[`f${i+1}Title`])}\n\n${plain(dict[`f${i+1}Body`])}`).join('\n\n');
        const modes = [1,2,3].map(n=>`- **${dict[`f3Mode${n}`]}:** ${dict[`f3Behavior${n}`]}`).join('\n');
        const steps = de ? [
            `1. [discreveal-portable.exe herunterladen](${download}) und öffnen.`,
            '2. Ein Laufwerk auswählen und „Scan starten“ anklicken. Ordnergrößen erscheinen während des Scans.',
            '3. Große Ordner durchsehen, Dateien suchen oder die Diagramme öffnen. Löschen ist nach Ende oder Abbruch des Scans möglich.',
            '4. Für doppelte Dateien „Duplikate“ im oberen Menü wählen. Ein vorheriger Laufwerksscan ist nicht nötig.',
        ] : [
            `1. [Download discreveal-portable.exe](${download}) and open it.`,
            '2. Choose a drive and click “Start scan”. Folder sizes appear while scanning.',
            '3. Browse large folders, search for files or open the charts. Deletion is available after the scan completes or is stopped.',
            '4. For duplicate files, choose “Duplicates” in the top menu. A previous drive scan is not required.',
        ];
        const privacy = Array.from({length:6},(_,i)=>`- **${plain(dict[`secTitle${i+1}`])}:** ${plain(dict[`secBody${i+1}`])}`).join('\n');
        const updates = `${dict.updateStep1Body}\n\n[${dict.updateJump.replace(' →','')}](${base+require('./buildLearningPages').route(require('../docs-src/guides').find(a=>a.id==='update-check'),lang)})\n\n${de?'SHA-256 des offiziellen Downloads':'Official download SHA-256'}:\n\n\`${release.sha256}\``;
        const faq = faqEntries(dict).map(({question,answer})=>`### ${plain(question)}\n\n${plain(answer)}`).join('\n\n');
        const text = `# DiscReveal – ${de?'Produktinformationen':'Product guide'}\n\n> ${dict.heroSub}\n\n[${de?'Offizielle Website':'Official website'}](${link})\n\n## ${de?'Auf einen Blick':'At a glance'}\n\n${facts.map(value=>'- '+value).join('\n')}\n\n## ${de?'Erste Schritte':'Getting started'}\n\n${steps.join('\n')}\n\n## ${dict.featuresKicker}\n\n${features}\n\n${modes}\n\n## ${dict.securityKicker}\n\n${privacy}\n\n## ${dict.benchKicker}\n\n[${de?'Vergleichstests für Laufwerksscan und Duplikatsuche':'Drive scan and duplicate-search comparisons'}](${base+require('./buildBenchmarkPages').route(lang)})\n\n## ${dict.updateKicker}\n\n${updates}\n\n## ${dict.faqTitle}\n\n${faq}\n\n## ${de?'Offizielle Links':'Official links'}\n\n- [${de?'Windows-Download':'Windows download'}](${download})\n- [${de?'Aktuelles Release':'Latest release'}](${repo}/releases/latest)\n- [${de?'Öffentlicher Quellcode':'Public source code'}](${repo})\n- [DiscReveal Source-Available License 1.0](${repo}/blob/main/LICENSE)\n- [${dict.footerNotices}](${base}/${de?'de/':''}third-party-notices/)\n`;
        const learningLinks = require('../docs-src/guides').map(article => `- [${article[lang].title}](${base}/${article[lang].slug}/)`).join('\n');
        guides[lang] = text + `\n## ${de?'Wissen & Ratgeber':'Knowledge & guides'}\n\n[${de?'Alle Artikel':'All articles'}](${base+require('./buildLearningPages').hubRoute(lang)})\n\n${learningLinks}\n`;
        fs.writeFileSync(path.join(root,'docs',de?'de/index.md':'index.md'),guides[lang]);
    }
    const llms = `# DiscReveal\n\n> Free, portable disk space analyzer, duplicate finder and system cleanup tool for Windows 10/11 x64. Shows folder sizes during scanning and retains an original when cleaning up duplicate copies. Available in English and German.\n\nOfficial product by markMarkinson. Current portable download: ${release.bytes} bytes. License: DiscReveal Source-Available License 1.0. Requires Microsoft Edge WebView2. Scans locally without an account or telemetry. Optional duplicate-search cache is off by default; local settings and a WebView2 profile are saved. The app is built with Rust, Tauri and JavaScript and currently ships for Windows only.\n\nThe published benchmarks describe the tested builds and conditions; they are not measurements of every later build. Test conditions, raw data and the limits of the comparison are documented below.\n\n## Product documentation\n\n- [English product guide](${base}/index.md): Features, requirements, duplicate cleanup, privacy, benchmark methodology and update checks.\n- [Deutsche Produktinformationen](${base}/de/index.md): Funktionen, Voraussetzungen, Duplikatbereinigung, Datenschutz, Messwerte und Versionsprüfung.\n- [Current release facts](${base}/release.json): Version, exact executable size, SHA-256 and official download link.\n\n## Official website and downloads\n\n- [English website](${base}/): Live demo, screenshots, features and download.\n- [Deutsche Website](${base}/de/): Vorschau, Screenshots, Funktionen und Download.\n- [Windows executable](${download}): Official portable discreveal-portable.exe.\n- [Latest GitHub release](${repo}/releases/latest): Current release notes, executable and license download.\n- [Public source code](${repo}): Source, English and German README, license and build instructions.\n\n## Evidence and licensing\n\n- [Complete-app benchmark data](${base}/benchmarks/scan-0.2.3.json): Five runs per method on the same folder; the measured build, warm Windows file cache, interface rendering included. Command comparisons, not comparisons against other graphical disk analyzers.\n- [DiscReveal Source-Available License 1.0 license](${repo}/blob/main/LICENSE): Product license.\n- [Third-party notices](${base}/third-party-notices/): Included components and original license texts.\n\n## Optional\n\n- [Combined English and German reference](${base}/llms-full.txt): Both product guides in one file.\n- [Deutsche Lizenzhinweise](${base}/de/third-party-notices/): Verwendete Komponenten und Lizenztexte.\n`;
    const learningIndex = [...require('../docs-src/guides'),...require('../docs-src/benchmark-reports')].flatMap(article => ['en','de'].map(lang => `- [${article[lang].title}](${base}/${article[lang].slug}/index.md): ${article[lang].description}`)).join('\n');
    fs.writeFileSync(path.join(root,'docs/llms.txt'),llms.replace('## Evidence and licensing',`## Knowledge & guides\n\n- [English knowledge hub](${base}/guides/): All articles organized by topic.\n- [DiscReveal guides and comparisons](${base}/guides/discreveal/): Articles tagged DiscReveal.\n- [Deutscher Wissensbereich](${base}/de/ratgeber/): Alle Artikel nach Themen.\n- [Wissen zu DiscReveal](${base}/de/ratgeber/discreveal/): Artikel mit dem Tag DiscReveal.\n\n${learningIndex}\n\n## Evidence and licensing\n\n- [English benchmark report](${base}/benchmarks/): Full drive scan and duplicate-search comparisons.\n- [Deutsche Vergleichstests](${base}/de/benchmarks/): Ergebnisse und Testaufbau.\n- [Duplicate-search core comparison](${base}/benchmarks/duplicates-0.2.5.json): DiscReveal core vs Czkawka 12.0.2 CLI, five warm-cache runs, app caches off, no graphical interfaces. Targeted synthetic workloads; both return the exact expected groups.`));
    fs.writeFileSync(path.join(root,'docs/llms-full.txt'),guides.en+'\n---\n\n'+guides.de);
    fs.writeFileSync(path.join(root,'docs/release.json'),JSON.stringify({...release,
        product:'DiscReveal',author:'markMarkinson',platform:'Windows 10/11 x64',license:'DiscReveal Source-Available License 1.0',
        downloadUrl:download,releaseUrl:repo+'/releases/latest'},null,2)+'\n');
    console.log('Wrote product guides, llms.txt, llms-full.txt and release.json');
};
