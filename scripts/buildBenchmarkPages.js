/** Full measurement reports live separately from the product landing page. */
const fs=require('node:fs');
const path=require('node:path');
const root=path.join(__dirname,'..'), base='https://discreveal.com';
const escape=s=>String(s).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;');
const reports=require('../docs-src/benchmark-reports');
const route=(lang,id)=>id?'/'+reports.find(r=>r.id===id)[lang].slug+'/':lang==='de'?'/de/benchmarks/':'/benchmarks/';
const duplicate=require('./duplicateBenchmark');

function homeSection(lang) {
    const de=lang==='de', copy=require('./scanBenchmarkCopy')(lang);
    const scan=require('../docs/benchmarks/scan-0.2.3.json');
    const time=id=>scan.tools.find(t=>t.id===id).medianMs;
    const number=(n,digits)=>n.toLocaleString(de?'de-DE':'en-US',{minimumFractionDigits:digits,maximumFractionDigits:digits});
    const stats=[
        [number(time('discreveal')/1000,2)+' s',de?'bis zur fertigen Ergebnisliste, inklusive Oberfläche':'to the finished results, including the interface'],
        [number(time('robocopy')/time('discreveal'),1)+'×',de?'schneller als robocopy /L in diesem Test':'faster than robocopy /L in this test'],
        [number(time('powershell')/time('discreveal'),1)+'×',de?'schneller als PowerShell in diesem Test':'faster than PowerShell in this test'],
    ];
    return `<section id="benchmarks" class="wrap"><div class="section-head"><p class="section-kicker">Benchmarks</p><h2>${de?'Weniger warten. Mehr Überblick.':'Less waiting. More insight.'}</h2><p>${escape(copy.homeBenchmarkBody)}</p></div><div class="bench-grid"><div class="bench-chart"><img src="/images/chart-scan-speed${de?'-de':''}.svg" alt="${de?'Scanvergleich mit DiscReveal, robocopy, Node.js, cmd und PowerShell':'Scan comparison with DiscReveal, robocopy, Node.js, cmd and PowerShell'}" loading="lazy"></div><div class="bench-stats">${stats.map(([value,label])=>`<div><div class="bench-stat-num">${value}</div><div class="bench-stat-label">${label}</div></div>`).join('')}</div></div><div class="knowledge-topics"><a class="pill-btn pill-btn-ghost" href="${route(lang,'drive-scan')}">${de?'Laufwerksscan im Detail':'Drive scan in detail'} →</a><a class="pill-btn pill-btn-ghost" href="${route(lang,'duplicate-benchmark')}">${de?'Duplikatsuche im Detail':'Duplicate search in detail'} →</a></div></section>`;
}

function generate({flags,dictionaries}) {
    const {digest}=require('./buildLearningPages');
    const scan=require('../docs/benchmarks/scan-0.2.3.json');
    for(const lang of ['en','de']) for(const id of [null,...reports.map(r=>r.id)]) {
        const de=lang==='de', other=de?'en':'de', dict=dictionaries[lang], home=de?'/de/':'/', url=base+route(lang,id);
        const report=reports.find(r=>r.id===id);
        const title=report?report[lang].title:de?'DiscReveal im Vergleich: Laufwerksscan und Duplikatsuche':'DiscReveal benchmarks: drive scan and duplicate search';
        const intro=report?report[lang].description:de?'Zwei Aufgaben, zwei Vergleichstests. Hier siehst du die gemessenen Zeiten, die verwendeten Programme und die Bedingungen der Messungen.':'Two tasks, two comparison tests. See measured times, the programs compared and the conditions behind each result.';
        const description=report?report[lang].description:de?'Vergleichstests für DiscReveal: Laufwerksscan mit fertiger Oberfläche und Duplikatsuche gegen Czkawka. Messwerte, Testaufbau und Rohdaten.':'DiscReveal performance comparisons: a drive scan including the finished interface, and duplicate search against Czkawka. Results, methods and raw data.';
        const drive=`<section id="drive-scan"><div class="section-head"><p class="section-kicker">${de?'Laufwerksscan':'Drive scan'}</p><h2>${escape(dict.benchTitle)}</h2><p>${escape(dict.benchBody)}</p></div><div class="bench-grid"><div class="bench-chart"><img src="/images/chart-scan-speed${de?'-de':''}.svg" alt="${escape(dict.benchmarkAlt)}" loading="lazy"></div><div class="bench-stats">${[1,2,3].map(n=>`<div><div class="bench-stat-num">${escape(dict[`bench${n}Num`])}</div><div class="bench-stat-label">${escape(dict[`bench${n}Label`])}</div></div>`).join('')}</div></div><h3>${de?'Testaufbau und Einordnung':'Method and scope'}</h3><p>${escape(dict.benchMethod)}</p><p>${de?'Dieser Test vergleicht den Laufwerksscan mit Windows-Befehlen und einem Skript, nicht mit anderen grafischen Speicheranalyse-Programmen. Er misst keine Duplikatsuche.':'This test compares drive scanning with Windows commands and a script, not other graphical disk analyzers. It does not measure duplicate search.'}</p><p><a href="/benchmarks/scan-0.2.3.json">${de?'Alle Messwerte und der genaue Testaufbau':'All measurements and the full test setup'}</a></p></section>`;
        const schema={'@context':'https://schema.org','@type':id?'Article':'WebPage',...(id?{headline:title,articleSection:'Benchmarks',keywords:['discreveal'],author:{'@type':'Person',name:'markMarkinson',url:'https://github.com/markmarkinson'},datePublished:'2026-10-02',mainEntityOfPage:url,image:base+'/icon.png',publisher:{'@type':'Organization',name:'DiscReveal',url:base+'/'}}:{}),url,name:title,description,inLanguage:lang,isPartOf:{'@id':base+'/#website'}};
        const html=`<!DOCTYPE html>
<html lang="${lang}" data-disc-reveal="discReveal" data-mark-markinson="markMarkinson"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="color-scheme" content="dark"><title>${escape(title)}</title><meta name="description" content="${escape(description)}"><meta name="robots" content="index,follow,max-image-preview:large">
<link rel="canonical" href="${url}">${['en','de','x-default'].map(l=>`<link rel="alternate" hreflang="${l}" href="${base+route(l==='x-default'?'en':l,id)}">`).join('')}<link rel="alternate" type="text/markdown" href="${url}index.md"><link rel="describedby" type="text/plain" href="${base}/llms.txt">
<meta property="og:type" content="${id?'article':'website'}"><meta property="og:site_name" content="DiscReveal"><meta property="og:title" content="${escape(title)}"><meta property="og:description" content="${escape(description)}"><meta property="og:url" content="${url}"><meta property="og:locale" content="${de?'de_DE':'en_US'}"><meta property="og:image" content="${base}/icon.png"><meta name="twitter:card" content="summary"><meta name="twitter:title" content="${escape(title)}"><meta name="twitter:description" content="${escape(description)}"><meta name="twitter:image" content="${base}/icon.png">
<link rel="icon" href="/favicon.svg"><link rel="stylesheet" href="/style.css?v=${digest('style.css')}"><link rel="stylesheet" href="/guides.css?v=${digest('guides.css')}"><script type="application/ld+json">${JSON.stringify(schema)}</script></head><body>
<nav class="site-nav"><div class="site-nav-inner"><a class="brand" href="${home}"><img src="/icon.png" width="28" height="28" alt=""><span>DiscReveal</span></a><div class="nav-right"><a class="language-btn" href="${route(other,id)}" aria-label="${de?'Switch to English':'Auf Deutsch umschalten'}">${flags[other]}</a><a class="pill-btn pill-btn-primary" href="https://github.com/markmarkinson/discreveal/releases/latest/download/discreveal-portable.exe">${de?'Für Windows herunterladen':'Download for Windows'}</a></div></div></nav>
<main class="wrap guide-main knowledge-hub benchmark-report"><header class="guide-header"><a class="guide-back" href="${de?'/de/ratgeber/':'/guides/'}">← ${de?'Wissen & Ratgeber':'Knowledge & guides'}</a><p class="section-kicker">Benchmarks</p><h1>${escape(title)}</h1>${id?`<p class="guide-tags"><a class="pill-btn pill-btn-ghost" href="${de?'/de/ratgeber/':'/guides/'}discreveal/">DiscReveal</a></p>`:''}<p class="guide-intro">${escape(intro)}</p></header><nav class="knowledge-topics" aria-label="${de?'Vergleichstests':'Comparison tests'}"><a class="pill-btn pill-btn-ghost" href="${route(lang,'drive-scan')}">${de?'Laufwerksscan':'Drive scan'}</a><a class="pill-btn pill-btn-ghost" href="${route(lang,'duplicate-benchmark')}">${de?'Duplikatsuche':'Duplicate search'}</a></nav>${id==='duplicate-benchmark'?'':drive}${id==='drive-scan'?'':duplicate.section(lang).replace('class="wrap"','class="benchmark-details"')}</main>
<footer class="wrap"><div class="footer-row"><div class="footer-links"><a href="${home}">${de?'Zur Startseite':'Back to home'}</a><a href="${de?'/de/ratgeber/':'/guides/'}">${de?'Wissen & Ratgeber':'Knowledge & guides'}</a><a href="${home}third-party-notices/">${de?'Drittanbieter & Lizenzen':'Third-party notices'}</a></div><p class="footer-note">DiscReveal · ${de?'Quellcode öffentlich einsehbar':'Source code publicly available'}</p></div></footer></body></html>`;
        const rows=scan.tools.map(t=>`| ${t.id === 'discreveal' ? 'DiscReveal' : t.name} | ${(t.medianMs/1000).toFixed(2)} s |`).join('\n');
        let markdown=`# ${title}\n\n${intro}\n\n## ${de?'Laufwerksscan':'Drive scan'}\n\n${dict.benchBody}\n\n| ${de?'Methode':'Method'} | Median |\n|---|---:|\n${rows}\n\n${dict.benchMethod}\n\n${de?'Der Test vergleicht Windows-Befehle und ein Skript mit DiscReveal, keine anderen grafischen Speicheranalyse-Programme. Er misst keine Duplikatsuche.':'This test compares Windows commands and a script with DiscReveal, not other graphical disk analyzers. It does not measure duplicate search.'}\n\n[${de?'Rohdaten des Laufwerksscans':'Drive scan raw data'}](${base}/benchmarks/scan-0.2.3.json)\n\n## ${de?'Duplikatsuche':'Duplicate search'}\n\n${duplicate.markdown(lang)}\n`;
        if(id) {
            const split=markdown.indexOf(`\n\n## ${de?'Duplikatsuche':'Duplicate search'}`);
            if(id==='drive-scan') markdown=markdown.slice(0,split)+'\n';
            else markdown=`# ${title}\n\n${intro}`+markdown.slice(split);
        }
        const directory=path.join(root,'docs',route(lang,id));
        fs.mkdirSync(directory,{recursive:true});
        fs.writeFileSync(path.join(directory,'index.html'),html);
        fs.writeFileSync(path.join(directory,'index.md'),markdown);
    }
}
module.exports={generate,homeSection,route,reports};
