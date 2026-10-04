/** Localized, static article overview; no client-side filtering needed for discovery. */
const fs = require('node:fs');
const path = require('node:path');
const root = path.join(__dirname,'..');
const base = 'https://discreveal.com';
const reports = require('../docs-src/benchmark-reports');
const escape = value => String(value).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;');

module.exports = function buildKnowledgeHub({flags,articles,route,hubRoute,card,categories,category,digest}) {
    const allEntries=[...articles,...reports];
    const reportCard=(a,lang)=>`<a class="guide-card" href="${route(a,lang)}" data-tags="discreveal"><span class="section-kicker">${escape(categories[a.category][lang])}</span><h3>${escape(a[lang].title)}</h3><p>${escape(a[lang].description)}</p><span class="guide-read">${lang==='de'?'Vergleich ansehen':'View comparison'} <span aria-hidden="true">→</span></span></a>`;
    for (const lang of ['en','de']) for (const tagged of [false,true]) {
        const entries=tagged?allEntries.filter(a=>(a.tags||[]).includes('discreveal')):allEntries;
        const activeCategories=Object.keys(categories).filter(key=>entries.some(a=>category(a)===key));
        const pageRoute=l=>hubRoute(l)+(tagged?'discreveal/':'');
        const de = lang==='de', other=de?'en':'de', home=de?'/de/':'/', url=base+pageRoute(lang);
        const title=tagged?(de?'Wissen zu DiscReveal: Anleitungen und Benchmarks':'DiscReveal knowledge: Guides and benchmarks'):de?'Wissen & Ratgeber für deinen Windows-Alltag':'Knowledge & guides for everyday Windows use';
        const description=tagged?(de?'Alle DiscReveal-Anleitungen und Benchmarks: Speicherplatz analysieren, Duplikate prüfen, Updates finden und Suchzeiten vergleichen.':'All DiscReveal guides and benchmarks: analyze disk space, review duplicates, check for updates and compare scan times.'):de?'AppData, Cache, Prefetch und Datei-Prüfsummen verständlich erklärt. Praktische Ratgeber zu Speicherplatz, Duplikaten und sicherem Löschen.':'Understand AppData, cache, Prefetch and file checksums. Practical guides to disk space, duplicate files and safer cleanup on Windows.';
        const intro=tagged?(de?'Hier findest du alle Artikel mit dem Tag DiscReveal – von den ersten Schritten in der App bis zu unseren Vergleichstests. Die Artikel behalten ihre jeweilige Themenkategorie.':'Find every article tagged DiscReveal, from getting started with the app to our performance comparisons. Each article keeps its topic category.'):de?'Was belegt deinen Speicher? Welche Dateien braucht Windows? Und wann ist etwas wirklich gelöscht? Hier findest du verständliche Erklärungen und praktische Schritte – zum Nachlesen, bevor du aufräumst.':'What takes up disk space? Which files does Windows need? And when is something really deleted? Find clear explanations and practical steps to read before you clean up.';
        const topics=de?{benchmarks:'Laufwerksscan und Duplikatsuche im Vergleich mit anderen Werkzeugen.',windows:'Ordner und Begriffe, die dir auf deinem PC begegnen.',organization:'Platzfresser erkennen und überzählige Kopien bewusst prüfen.',security:'Verstehen, was Prüfsummen und Löschverfahren leisten – und wo ihre Grenzen liegen.'}:{benchmarks:'Drive scans and duplicate searches compared with other tools.',windows:'Understand the folders and terms you encounter on your PC.',organization:'Find space hogs and review extra copies deliberately.',security:'Learn what checksums and deletion methods can do, and where their limits lie.'};
        const collection={'@context':'https://schema.org','@type':'CollectionPage',url,name:title,description,inLanguage:lang,
            isPartOf:{'@id':base+'/#website'},mainEntity:{'@type':'ItemList',numberOfItems:entries.length,itemListElement:entries.map((a,i)=>({'@type':'ListItem',position:i+1,name:a[lang].title,url:base+route(a,lang)}))}};
        const breadcrumb={'@context':'https://schema.org','@type':'BreadcrumbList',itemListElement:[{'@type':'ListItem',position:1,name:'DiscReveal',item:base+home},{'@type':'ListItem',position:2,name:title,item:url}]};
        const groups=activeCategories.map(key=>`<section id="${key}"><div class="section-head"><h2>${escape(categories[key][lang])}</h2><p>${escape(topics[key])}</p></div><div class="guide-cards">${entries.filter(a=>category(a)===key).map(a=>reports.includes(a)?reportCard(a,lang):card(a,lang)).join('')}</div></section>`).join('\n');
        const html=`<!DOCTYPE html>
<html lang="${lang}" data-disc-reveal="discReveal" data-mark-markinson="markMarkinson"><head>
<meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="color-scheme" content="dark">
<title>${escape(title)} · DiscReveal</title><meta name="description" content="${escape(description)}"><meta name="robots" content="index,follow,max-image-preview:large">
<link rel="canonical" href="${url}"><link rel="alternate" hreflang="en" href="${base+pageRoute('en')}"><link rel="alternate" hreflang="de" href="${base+pageRoute('de')}"><link rel="alternate" hreflang="x-default" href="${base+pageRoute('en')}">
<link rel="alternate" type="text/markdown" href="${url}index.md"><link rel="describedby" type="text/plain" href="${base}/llms.txt">
<meta property="og:type" content="website"><meta property="og:site_name" content="DiscReveal"><meta property="og:title" content="${escape(title)}"><meta property="og:description" content="${escape(description)}"><meta property="og:url" content="${url}"><meta property="og:locale" content="${de?'de_DE':'en_US'}"><meta property="og:locale:alternate" content="${de?'en_US':'de_DE'}"><meta property="og:image" content="${base}/icon.png"><meta property="og:image:alt" content="DiscReveal">
<meta name="twitter:card" content="summary"><meta name="twitter:title" content="${escape(title)}"><meta name="twitter:description" content="${escape(description)}"><meta name="twitter:image" content="${base}/icon.png"><meta name="twitter:image:alt" content="DiscReveal">
<link rel="icon" href="/favicon.svg"><link rel="stylesheet" href="/style.css?v=${digest('style.css')}"><link rel="stylesheet" href="/guides.css?v=${digest('guides.css')}">
${[collection,breadcrumb].map(s=>`<script type="application/ld+json">${JSON.stringify(s).replaceAll('<','\\u003c')}</script>`).join('\n')}
</head><body><nav class="site-nav"><div class="site-nav-inner"><a class="brand" href="${home}"><img src="/icon.png" width="28" height="28" alt=""><span>DiscReveal</span></a><div class="nav-right"><a class="language-btn" href="${pageRoute(other)}" lang="${other}" aria-label="${de?'Switch to English':'Auf Deutsch umschalten'}">${flags[other]}</a><a class="pill-btn pill-btn-primary" href="https://github.com/markmarkinson/discreveal/releases/latest/download/discreveal-portable.exe">${de?'Für Windows herunterladen':'Download for Windows'}</a></div></div></nav>
<main class="wrap guide-main knowledge-hub"><header class="guide-header"><a class="guide-back" href="${home}">← ${de?'Zur Startseite':'Back to home'}</a><p class="section-kicker">${de?'Wissen & Ratgeber':'Learn & explore'}</p><h1>${escape(title)}</h1><p class="guide-intro">${escape(intro)}</p></header>
<nav class="knowledge-topics" aria-label="${de?'Themen':'Topics'}">${activeCategories.map(key=>`<a class="pill-btn pill-btn-ghost" href="#${key}">${escape(categories[key][lang])}</a>`).join('')}</nav>
<nav class="knowledge-topics knowledge-tags" aria-label="Tags"><span>Tags</span><a class="pill-btn pill-btn-ghost" href="${hubRoute(lang)}discreveal/" ${tagged?'aria-current="page"':''}>DiscReveal</a>${tagged?`<a class="pill-btn pill-btn-ghost" href="${hubRoute(lang)}">${de?'Alle Artikel':'All articles'}</a>`:''}</nav>
${groups}</main><footer class="wrap"><div class="footer-row"><div class="footer-links"><a href="${home}">${de?'Zur Startseite':'Back to home'}</a><a href="${home}third-party-notices/">${de?'Drittanbieter & Lizenzen':'Third-party notices'}</a><a href="https://github.com/markmarkinson/discreveal">GitHub</a></div><p class="footer-note">DiscReveal · ${de?'Quellcode öffentlich einsehbar':'Source code publicly available'}</p></div></footer></body></html>`;
        const markdown=`# ${title}\n\n${intro}\n\n`+activeCategories.map(key=>`## ${categories[key][lang]}\n\n${topics[key]}\n\n`+entries.filter(a=>category(a)===key).map(a=>`- [${a[lang].title}](${base+route(a,lang)}): ${a[lang].description}`).join('\n')).join('\n\n')+`\n\n[DiscReveal](${base+home})\n`;
        const directory=path.join(root,'docs',pageRoute(lang));
        fs.mkdirSync(directory,{recursive:true});
        fs.writeFileSync(path.join(directory,'index.html'),html);
        fs.writeFileSync(path.join(directory,'index.md'),markdown);
        console.log(`Wrote ${pageRoute(lang)} overview and Markdown`);
    }
};

module.exports.reports = reports;
