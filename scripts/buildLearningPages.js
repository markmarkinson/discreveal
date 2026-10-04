/** Static, crawlable articles and matching Markdown, without browser JavaScript. */
const fs = require('node:fs');
const path = require('node:path');
const articles = require('../docs-src/guides');
const root = path.join(__dirname, '..');
const base = 'https://discreveal.com';
const download = 'https://github.com/markmarkinson/discreveal/releases/latest/download/discreveal-portable.exe';
const escape = value => String(value).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;');
const route = (article,lang) => '/' + article[lang].slug + '/';
const hubRoute = lang => lang === 'de' ? '/de/ratgeber/' : '/guides/';
const categories = {benchmarks:{de:'Benchmarks',en:'Benchmarks'},windows:{de:'Windows verstehen',en:'Understanding Windows'},organization:{de:'Ordnung & Speicherplatz',en:'Organization & disk space'},security:{de:'Sicherheit',en:'File safety'}};
const category = article => article.category || 'organization';
const readingTime = text => Math.max(1,Math.ceil([text.intro,text.result,...text.sections.flatMap(s=>[s.title,...(s.paragraphs||[]),...(s.bullets||[]),...(s.steps||[]).flat()])].join(' ').split(/\s+/).length/200));
const digest = file => require('node:crypto').createHash('sha256').update(fs.readFileSync(path.join(root,'docs',file))).digest('hex').slice(0,12);
const card = (article,lang) => `<a class="guide-card" href="${route(article,lang)}"${(article.tags||[]).includes('discreveal')?' data-tags="discreveal"':''}><span class="section-kicker">${escape(categories[category(article)][lang])}</span><h3>${escape(article[lang].title)}</h3><p>${escape(article[lang].description)}</p><span class="guide-read">${lang==='de'?'Artikel lesen':'Read the article'} <span aria-hidden="true">→</span><span class="guide-card-time">${readingTime(article[lang])} ${lang==='de'?'Min.':'min'}</span></span></a>`;

function formatPublishedDate(value, lang) {
    if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) throw Error('Invalid article date');
    return new Intl.DateTimeFormat(lang==='de'?'de-DE':'en-US',{year:'numeric',month:'long',day:'numeric',timeZone:'UTC'}).format(new Date(value+'T12:00:00Z'));
}

function homeCards(lang) {
    const de = lang === 'de';
    const featured=['disk-space','duplicate-files'].map(id=>articles.find(a=>a.id===id));
    return `<section id="guides" class="wrap"><div class="section-head"><p class="section-kicker">${de?'Wissen & Ratgeber':'Learn & explore'}</p><h2>${de?'Dateien verstehen. Bewusst aufräumen.':'Understand your files. Clean up with care.'}</h2><p>${de?'So findest du Platzfresser und entfernst überzählige Kopien mit DiscReveal. Zwei Anleitungen zeigen dir die Schritte direkt in der App.':'Find space hogs and remove extra copies with DiscReveal. These two guides walk you through the steps in the app.'}</p></div><div class="guide-cards">${featured.map(a=>card(a,lang)).join('')}</div><p class="knowledge-all"><a class="pill-btn pill-btn-ghost" href="${hubRoute(lang)}">${de?'Alle Artikel entdecken':'Explore all articles'} <span aria-hidden="true">→</span></a></p></section>`;
}

function generate({flags}) {
    for (const article of articles) for (const lang of ['en','de']) {
        const de = lang === 'de', text = article[lang], url = base+route(article,lang);
        const home = de ? '/de/' : '/', other = de ? 'en' : 'de';
        const imagePath = article.image ? `/images/${article.image}${de ? '-de' : ''}.png` : '/icon.png';
        const image = fs.readFileSync(path.join(root,'docs',imagePath));
        const width = image.readUInt32BE(16), height = image.readUInt32BE(20);
        const paragraphs = section => (section.paragraphs || []).map(p=>`<p>${escape(p)}</p>`).join('');
        const steps = section => section.steps ? `<ol class="guide-steps">${section.steps.map(([title,body])=>`<li><h3>${escape(title)}</h3><p>${escape(body)}</p></li>`).join('')}</ol>` : '';
        const bullets = section => section.bullets ? `<ul>${section.bullets.map(b=>`<li>${escape(b)}</li>`).join('')}</ul>` : '';
        const screenshot = article.image ? `<figure class="guide-shot"><a href="${imagePath}" aria-label="${de ? 'Screenshot in voller Größe öffnen' : 'Open full-size screenshot'}"><img src="${imagePath}" width="${width}" height="${height}" alt="${escape(text.alt||'DiscReveal')}" loading="lazy"></a><figcaption>${escape(text.caption)}</figcaption></figure>` : '';
        const content = text.sections.map((section,i)=>`<section id="step-${i+1}"><h2>${escape(section.title)}</h2>${paragraphs(section)}${steps(section)}${bullets(section)}${section.steps ? screenshot : ''}${section.code?`<pre class="guide-code"><code>${escape(section.code)}</code></pre>`:''}${section.sources?`<p class="guide-sources">${de?'Quellen':'Sources'}: ${section.sources.map(([name,url])=>`<a href="${escape(url)}">${escape(name)}</a>`).join(' · ')}</p>`:''}</section>`).join('\n');
        const relatedList = (article.related || articles.filter(a=>a.id!==article.id && category(a)===category(article)).map(a=>a.id)).map(id=>articles.find(a=>a.id===id)).filter(Boolean).slice(0,3);
        const breadcrumb = {'@context':'https://schema.org','@type':'BreadcrumbList',itemListElement:[
            {'@type':'ListItem',position:1,name:'DiscReveal',item:base+home},
            {'@type':'ListItem',position:2,name:de?'Wissen & Ratgeber':'Knowledge & guides',item:base+hubRoute(lang)},
            {'@type':'ListItem',position:3,name:text.title,item:url},
        ]};
        const schema = {'@context':'https://schema.org','@type':'Article','@id':url+'#article',headline:text.title,
            description:text.description,inLanguage:lang,mainEntityOfPage:url,image:base+imagePath,
            author:{'@type':'Person',name:'markMarkinson',url:'https://github.com/markmarkinson'},
            publisher:{'@type':'Organization',name:'DiscReveal',url:base+'/'},
            datePublished:article.published||'2026-10-02',dateModified:article.updated||article.published||'2026-10-02',articleSection:categories[category(article)][lang],keywords:article.tags||[],
            citation:[...new Set(text.sections.flatMap(s=>(s.sources||[]).map(([,url])=>url)))],
            about:article.kind==='knowledge'?{'@type':'Thing',name:text.title}:{'@type':'SoftwareApplication','@id':base+'/#software',name:'DiscReveal',operatingSystem:'Windows 10, Windows 11',applicationCategory:'UtilitiesApplication'}};
        const html = `<!DOCTYPE html>
<html lang="${lang}" data-disc-reveal="discReveal" data-mark-markinson="markMarkinson"><head>
<meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="color-scheme" content="dark">
<title>${escape(text.title)} · DiscReveal</title><meta name="description" content="${escape(text.description)}"><meta name="robots" content="index,follow,max-image-preview:large">
<link rel="canonical" href="${url}"><link rel="alternate" hreflang="en" href="${base+route(article,'en')}"><link rel="alternate" hreflang="de" href="${base+route(article,'de')}"><link rel="alternate" hreflang="x-default" href="${base+route(article,'en')}">
<link rel="alternate" type="text/markdown" href="${url}index.md"><link rel="describedby" type="text/plain" href="${base}/llms.txt">
<meta property="og:type" content="article"><meta property="og:site_name" content="DiscReveal"><meta property="og:title" content="${escape(text.title)}"><meta property="og:description" content="${escape(text.description)}"><meta property="og:url" content="${url}"><meta property="og:locale" content="${de?'de_DE':'en_US'}"><meta property="og:locale:alternate" content="${de?'en_US':'de_DE'}"><meta property="og:image" content="${base+imagePath}"><meta property="og:image:width" content="${width}"><meta property="og:image:height" content="${height}"><meta property="og:image:alt" content="${escape(text.alt||'DiscReveal')}">
<meta name="twitter:card" content="${article.image?'summary_large_image':'summary'}"><meta name="twitter:title" content="${escape(text.title)}"><meta name="twitter:description" content="${escape(text.description)}"><meta name="twitter:image" content="${base+imagePath}"><meta name="twitter:image:alt" content="${escape(text.alt||'DiscReveal')}">
<link rel="icon" href="/favicon.svg"><link rel="stylesheet" href="/style.css?v=${digest('style.css')}"><link rel="stylesheet" href="/guides.css?v=${digest('guides.css')}">
${[schema,breadcrumb].map(value=>`<script type="application/ld+json">${JSON.stringify(value).replaceAll('<','\\u003c')}</script>`).join('\n')}
</head><body><nav class="site-nav"><div class="site-nav-inner"><a class="brand" href="${home}"><img src="/icon.png" width="28" height="28" alt=""><span>DiscReveal</span></a><div class="nav-right"><a class="language-btn" href="${route(article,other)}" lang="${other}" aria-label="${de?'Switch to English':'Auf Deutsch umschalten'}">${flags[other]}</a><a class="pill-btn pill-btn-primary" href="${download}">${de?'Für Windows herunterladen':'Download for Windows'}</a></div></div></nav>
<main class="wrap guide-main"><article><header class="guide-header"><a class="guide-back" href="${hubRoute(lang)}">← ${de?'Alle Artikel':'All articles'}</a><p class="section-kicker">${escape(categories[category(article)][lang])}</p><h1>${escape(text.title)}</h1><p class="guide-meta">markMarkinson · <time datetime="${article.published||'2026-10-02'}">${formatPublishedDate(article.published||'2026-10-02',lang)}</time> · ${readingTime(text)} ${de?'Min. Lesezeit':'min read'}</p>${(article.tags||[]).includes('discreveal')?`<p class="guide-tags"><a class="pill-btn pill-btn-ghost" href="${hubRoute(lang)}discreveal/">DiscReveal</a></p>`:''}<p class="guide-intro">${escape(text.intro)}</p><p class="guide-result">${escape(text.result)}</p></header>
<nav class="guide-contents" aria-label="${de?'Inhalt':'Contents'}"><h2>${de?'In dieser Anleitung':'In this guide'}</h2><ol>${text.sections.map((s,i)=>`<li><a href="#step-${i+1}">${escape(s.title)}</a></li>`).join('')}</ol></nav>
${content}<aside class="guide-next"><h2>${de?'Das könnte dir auch helfen':'You may also find this useful'}</h2><ul>${relatedList.map(a=>`<li><a href="${route(a,lang)}">${escape(a[lang].title)} →</a></li>`).join('')}</ul></aside>
<div class="guide-cta"><h2>${de?'Schau nach, wo dein Speicher bleibt':'See where your disk space goes'}</h2><p>${de?'Kostenlos für Windows 10/11 · Ohne DiscReveal-Installation · Kein Benutzerkonto':'Free for Windows 10/11 · No DiscReveal installation · No account'}</p><a class="pill-btn pill-btn-primary" href="${download}">${de?'DiscReveal herunterladen':'Download DiscReveal'}</a></div></article></main>
<footer class="wrap"><div class="footer-row"><div class="footer-links"><a href="${home}">${de?'Zur Startseite':'Back to home'}</a><a href="${hubRoute(lang)}">${de?'Alle Artikel':'All articles'}</a><a href="${home}third-party-notices/">${de?'Drittanbieter & Lizenzen':'Third-party notices'}</a><a href="https://github.com/markmarkinson/discreveal">GitHub</a></div><p class="footer-note">DiscReveal · ${de?'Quellcode öffentlich einsehbar':'Source code publicly available'}</p></div></footer></body></html>`;
        const markdown = `# ${text.title}\n\nmarkMarkinson · ${article.published||'2026-10-02'} · ${readingTime(text)} ${de?'Min. Lesezeit':'min read'}\n\n${text.intro}\n\n${text.result}\n\n` + text.sections.map(section=>[
            `## ${section.title}`,...(section.paragraphs||[]),
            section.steps?section.steps.map(([title,body],i)=>`${i+1}. **${title}:** ${body}`).join('\n'):'',
            section.steps&&article.image?`![${text.alt}](${base+imagePath})\n\n${text.caption}`:'',
            section.bullets?section.bullets.map(value=>'- '+value).join('\n'):'',
            section.code?'```powershell\n'+section.code+'\n```':'',
            section.sources?`${de?'Quellen':'Sources'}: `+section.sources.map(([name,url])=>`[${name}](${url})`).join(' · '):'',
        ].filter(Boolean).join('\n\n')).join('\n\n') + `\n\n## ${de?'Weitere Links':'Related links'}\n\n${relatedList.map(a=>`- [${a[lang].title}](${base+route(a,lang)})`).join('\n')}\n- [${de?'Alle Artikel':'All articles'}](${base+hubRoute(lang)})\n- [DiscReveal](${base+home})\n- [${de?'Windows-Download':'Windows download'}](${download})\n`;
        const directory = path.join(root,'docs',text.slug);
        fs.mkdirSync(directory,{recursive:true});
        fs.writeFileSync(path.join(directory,'index.html'),html);
        fs.writeFileSync(path.join(directory,'index.md'),markdown);
        console.log(`Wrote ${text.slug}/index.html and index.md`);
    }
    require('./buildKnowledgeHub')({flags,articles,route,hubRoute,card,categories,category,digest});
}
module.exports = {formatPublishedDate,generate,homeCards,articles,route,hubRoute,digest};
