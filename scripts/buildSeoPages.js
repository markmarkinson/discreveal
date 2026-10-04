/**
 * Generates the two served pages of the DiscReveal website — docs/index.html
 * (English) and docs/de/index.html (German) — from docs-src/site.template.html
 * and the language dictionaries in docs-src/i18n.js.
 *
 * Why a build step for a "no framework" static site: the previous single-URL
 * page left every piece of text as an empty `data-i18n` element until app.js
 * ran client-side. That's invisible to crawlers that don't execute
 * JavaScript (most AI answer-engine bots today) and slower to index even for
 * ones that do — Google's own docs describe JS-rendered content as a second,
 * delayed indexing pass. It also meant there was exactly one URL for two
 * languages, which rules out hreflang entirely (Google's guidance is
 * explicit: distinct URLs per language, not a client-side toggle on one).
 *
 * This script bakes the real text into static HTML once per language, each
 * at its own URL, with the matching <title>, meta description, canonical,
 * hreflang alternates, Open Graph/Twitter tags and JSON-LD (SoftwareApplication
 * + FAQPage, generated from the same FAQ content that's actually on the page,
 * matching the visible page content. Markup alone does not guarantee rich results).
 *
 * docs-src/i18n.js and site.template.html are the source of truth; nothing
 * under docs/ should be hand-edited except style.css, app.js, and the
 * shared assets (images/, fonts/, favicon.svg, icon.png).
 *
 * Usage: node scripts/buildSeoPages.js
 */
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const projectRoot = path.join(__dirname, '..');
const srcDir = path.join(projectRoot, 'docs-src');
const docsDir = path.join(projectRoot, 'docs');

const SITE_BASE = 'https://discreveal.com';
const release = require('./releaseMetadata').readReleaseMetadata();

// Every screenshot that exists in both a plain (English) and a "-de" (German) version —
// used to localize every <img src="images/NAME.png"> in the template per output language,
// except the one deliberately marked class="lang-cross-shot" (see below).
const LOCALIZED_SHOTS = ['list-view', 'live-scan', 'icon-view', 'search', 'protected-dialog', 'duplicates', 'charts', 'cleanup'];

const { version } = JSON.parse(fs.readFileSync(path.join(projectRoot, 'package.json'), 'utf-8'));

/** Loads the browser-facing dictionaries file (plain `const` declarations, no exports) into a sandbox. */
function loadDictionaries() {
    const source = fs.readFileSync(path.join(srcDir, 'i18n.js'), 'utf-8');
    // Top-level `const` bindings don't become properties of the vm context object (same as
    // `const` never creating a `window.x` in a real browser), so the sandbox itself can't see
    // them after the fact. Appending an expression that references them in the *same* script
    // works, because vm.runInContext returns the value of its last evaluated expression.
    const result = vm.runInNewContext(`${source}\n;({ discRevealSiteDictionaries, discRevealFlags });`, {});
    for (const lang of ['en','de']) Object.assign(result.discRevealSiteDictionaries[lang], require('./scanBenchmarkCopy')(lang));
    return { dictionaries: result.discRevealSiteDictionaries, flags: result.discRevealFlags };
}

function escapeHtml(value) {
    return String(value).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function escapeAttr(value) {
    return escapeHtml(value).replace(/"/g, '&quot;');
}

/** Strips the (usually inline) tags a translated string carries, for use as plain-text JSON-LD content. */
function stripTags(value) {
    return String(value).replace(/<[^>]+>/g, '');
}

const LANGUAGES = {
    en: {
        htmlLang: 'en',
        outDir: docsDir,
        assetPrefix: '',
        otherLangPath: 'de/',
        otherLangKey: 'de',
        otherLangLabel: 'Auf Deutsch umschalten',
        ogLocale: 'en_US',
        title: 'DiscReveal – Disk Space, Duplicates & Windows Cleanup',
        description:
            'Find large files, remove duplicate copies and clean up Windows with DiscReveal. Live results, occupied-space measurement and control over what stays. Free and portable.',
    },
    de: {
        htmlLang: 'de',
        outDir: path.join(docsDir, 'de'),
        assetPrefix: '../',
        otherLangPath: '../',
        otherLangKey: 'en',
        otherLangLabel: 'Switch to English',
        ogLocale: 'de_DE',
        title: 'DiscReveal – Speicherplatz, Duplikate & Windows bereinigen',
        description:
            'Finde große Dateien, entferne Duplikate und bereinige Windows mit DiscReveal. Live-Ergebnisse, belegten Speicher messen und selbst entscheiden. Kostenlos und portabel.',
    },
};

function faqEntries(dict) {
    const keys = Object.keys(dict).filter(key => /^faqQ\d+$/.test(key)).sort((a,b)=>Number(a.slice(4))-Number(b.slice(4)));
    return keys.map((key,index) => {
        if (key !== `faqQ${index+1}` || !dict[`faqA${index+1}`]) throw Error('FAQ questions and answers must be complete and consecutive');
        return {question:dict[key], answer:dict[`faqA${index+1}`]};
    });
}

function buildJsonLd(lang, config, dict) {
    const canonical = `${SITE_BASE}/${lang === 'en' ? '' : 'de/'}`;
    const ogImagePath = `images/list-view${lang === 'de' ? '-de' : ''}.png`;

    const softwareApplication = {
        '@context': 'https://schema.org',
        '@type': 'SoftwareApplication',
        '@id': `${SITE_BASE}/#software`,
        name: 'DiscReveal',
        operatingSystem: 'Windows 10, Windows 11',
        applicationCategory: 'UtilitiesApplication',
        description: config.description,
        inLanguage: ['en','de'],
        url: canonical,
        downloadUrl: 'https://github.com/markmarkinson/discreveal/releases/latest/download/discreveal-portable.exe',
        fileSize: `${release.bytes} bytes`,
        softwareRequirements: dict.reqRuntimeVal,
        processorRequirements: 'x64',
        isAccessibleForFree: true,
        image: `${SITE_BASE}/icon.png`,
        sameAs: 'https://github.com/markmarkinson/discreveal',
        license: 'https://github.com/markmarkinson/discreveal/blob/main/LICENSE',
        offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' },
        author: { '@type': 'Person', name: 'markMarkinson', url: 'https://github.com/markmarkinson' },
        screenshot: `${SITE_BASE}/${ogImagePath}`,
        featureList: [dict.f1Title, dict.f2Title, dict.f3Title, dict.f4Title, dict.f5Title, dict.f6Title, dict.f7Title].map(stripTags),
    };

    const faqPage = {
        '@context': 'https://schema.org',
        '@type': 'FAQPage',
        '@id': `${canonical}#faq`,
        mainEntity: faqEntries(dict).map(({question,answer}) => {
            return {
                '@type': 'Question',
                name: stripTags(question),
                acceptedAnswer: { '@type': 'Answer', text: stripTags(answer) },
            };
        }),
    };

    const website = {'@context':'https://schema.org', '@type':'WebSite', '@id':`${SITE_BASE}/#website`,
        name:'DiscReveal', url:`${SITE_BASE}/`, inLanguage:['en','de']};
    const webpage = {'@context':'https://schema.org', '@type':'WebPage', '@id':canonical+'#webpage',
        url:canonical, name:config.title, description:config.description, inLanguage:lang,
        isPartOf:{'@id':website['@id']}, mainEntity:{'@id':softwareApplication['@id']}};
    return [softwareApplication, faqPage, website, webpage];
}

function buildHead(lang, config) {
    const canonical = `${SITE_BASE}/${lang === 'en' ? '' : 'de/'}`;
    const altEn = `${SITE_BASE}/`;
    const altDe = `${SITE_BASE}/de/`;
    const ogImage = `${SITE_BASE}/images/list-view${lang === 'de' ? '-de' : ''}.png`;
    const structuredData = buildJsonLd(lang, config, config.dict);
    const png = fs.readFileSync(path.join(docsDir, 'images', `list-view${lang === 'de' ? '-de' : ''}.png`));
    const imageWidth = png.readUInt32BE(16), imageHeight = png.readUInt32BE(20);
    const imageAlt = config.dict.shot1Alt;

    return `
    <title>${escapeHtml(config.title)}</title>
    <meta name="description" content="${escapeAttr(config.description)}" />
    <meta name="robots" content="index,follow,max-image-preview:large" />
    <link rel="canonical" href="${canonical}" />
    <link rel="alternate" type="text/markdown" href="${canonical}index.md" />
    <link rel="describedby" type="text/plain" href="${SITE_BASE}/llms.txt" />
    <link rel="alternate" hreflang="en" href="${altEn}" />
    <link rel="alternate" hreflang="de" href="${altDe}" />
    <link rel="alternate" hreflang="x-default" href="${altEn}" />
    <meta property="og:type" content="website" />
    <meta property="og:site_name" content="DiscReveal" />
    <meta property="og:title" content="${escapeAttr(config.title)}" />
    <meta property="og:description" content="${escapeAttr(config.description)}" />
    <meta property="og:url" content="${canonical}" />
    <meta property="og:locale" content="${config.ogLocale}" />
    <meta property="og:locale:alternate" content="${lang === 'en' ? 'de_DE' : 'en_US'}" />
    <meta property="og:image" content="${ogImage}" />
    <meta property="og:image:width" content="${imageWidth}" />
    <meta property="og:image:height" content="${imageHeight}" />
    <meta property="og:image:alt" content="${escapeAttr(imageAlt)}" />
    <meta name="twitter:card" content="summary_large_image" />
    <meta name="twitter:title" content="${escapeAttr(config.title)}" />
    <meta name="twitter:description" content="${escapeAttr(config.description)}" />
    <meta name="twitter:image" content="${ogImage}" />
    <meta name="twitter:image:alt" content="${escapeAttr(imageAlt)}" />
    <link rel="preload" href="${config.assetPrefix}fonts/plus-jakarta-sans-latin.woff2" as="font" type="font/woff2" crossorigin="anonymous" />
    <link rel="preload" href="${config.assetPrefix}fonts/jetbrains-mono-latin.woff2" as="font" type="font/woff2" crossorigin="anonymous" />
    ${structuredData.map(value=>`<script type="application/ld+json">${JSON.stringify(value).replace(/</g,'\\u003c')}</script>`).join('\n    ')}`;
}

function generate() {
    require('./buildBenchmarkCharts')();
    const { dictionaries, flags } = loadDictionaries();
    const template = fs.readFileSync(path.join(srcDir, 'site.template.html'), 'utf-8');

    for (const [lang, config] of Object.entries(LANGUAGES)) {
        const dict = dictionaries[lang];
        if (!dict) throw new Error(`No dictionary for language "${lang}"`);
        const size = (release.bytes/1e6).toLocaleString(lang === 'de' ? 'de-DE' : 'en-US', {minimumFractionDigits:1,maximumFractionDigits:1});
        dict.statSize = `≈ ${size} MB`;
        dict.reqSizeVal = `${size} MB · .exe`;
        config.dict = dict;

        let html = template;
        html = html.replace('<!--BENCHMARK_OVERVIEW-->', require('./buildBenchmarkPages').homeSection(lang));
        html = html.replace('<!--LEARNING_GUIDES-->', require('./buildLearningPages').homeCards(lang));
        html = html.replaceAll('href="#updates"', `href="${require('./buildLearningPages').route(require('./buildLearningPages').articles.find(a=>a.id==='update-check'),lang)}"`);
        html = html.replaceAll('href="#guides"', `href="${require('./buildLearningPages').hubRoute(lang)}"`);
        html = html.replace('<!--FAQ_ITEMS-->', faqEntries(dict).map(({question,answer}) =>
            `<details class="faq-item"><summary><span>${escapeHtml(question)}</span><span class="faq-plus" aria-hidden="true"><svg width="12" height="12" viewBox="0 0 24 24" fill="none"><path d="M12 5v14M5 12h14" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/></svg></span></summary><div class="faq-answer"><p>${escapeHtml(answer)}</p></div></details>`).join('\n'));
        html = html.replace('<p class="download-version" id="download-version"></p>',
            `<p class="download-version" id="download-version" data-release-sha256="${release.sha256}"><strong>DiscReveal Portable</strong> · ${size} MB</p>`);
        const imageKeys = {'list-view':'shot1Alt','icon-view':'shot2Alt','live-scan':'shot3Alt','search':'shot4Alt','protected-dialog':'shot5Alt','duplicates':'shot6Alt','charts':'shot7Alt','cleanup':'shot8Alt','chart-scan-speed':'benchmarkAlt'};
        html = html.replace(/<img\b[^>]*>/g, tag => {
            const match = /src="images\/([\w-]+)\.(png|svg)"/.exec(tag);
            const key = imageKeys[match?.[1]];
            if (key) tag = tag.replace(/alt="[^"]*"/,`alt="${escapeAttr(dict[key])}"`);
            if (match?.[2] === 'png') {
                const image = fs.readFileSync(path.join(docsDir,'images',match[1]+'.png'));
                tag = tag.replace(/width="\d+"/,`width="${image.readUInt32BE(16)}"`)
                    .replace(/height="\d+"/,`height="${image.readUInt32BE(20)}"`);
            }
            return tag;
        });
        for (const [label,key] of [['Previous','lightboxPrevious'],['Next','lightboxNext'],['Close','lightboxClose']]) {
            html = html.replace(`aria-label="${label}"`,`aria-label="${escapeAttr(dict[key])}"`);
        }

        // data-i18n-html="key": raw innerHTML (values intentionally carry markup, e.g. <em>).
        html = html.replace(/data-i18n-html="(\w+)"([^>]*)>([^<]*)</g, (match, key, attrs) => {
            if (dict[key] === undefined) throw new Error(`Missing ${lang}.${key}`);
            return `data-i18n-html="${key}"${attrs}>${dict[key]}<`;
        });

        // data-i18n="key": HTML-escaped text content.
        html = html.replace(/data-i18n="(\w+)"([^>]*)>([^<]*)</g, (match, key, attrs) => {
            if (dict[key] === undefined) throw new Error(`Missing ${lang}.${key}`);
            return `data-i18n="${key}"${attrs}>${escapeHtml(dict[key])}<`;
        });

        // Screenshot captions baked in as data-label so app.js needs no dictionary at runtime.
        for (let i = 1; i <= 8; i++) {
            const label = dict[`shot${i}Label`];
            html = html.replace(
                new RegExp(`data-index="${i - 1}">`),
                `data-index="${i - 1}" data-label="${escapeAttr(label)}">`
            );
        }

        // Every screenshot is localized to the output language, e.g. images/list-view.png ->
        // images/list-view-de.png for the German page — except the one image marked
        // class="lang-cross-shot", which deliberately shows the OTHER language (it's the
        // screenshot proving the language switch itself works) and so gets the opposite
        // treatment. Lock it first so the generic loop below skips it.
        html = html.replace(/(<img class="lang-cross-shot" src="images\/[\w-]+\.png)"/, '$1#LOCKED"');
        if (lang === 'de') {
            html = html.replace('images/chart-scan-speed.svg', 'images/chart-scan-speed-de.svg');
            for (const name of LOCALIZED_SHOTS) {
                html = html.split(`images/${name}.png"`).join(`images/${name}-de.png"`);
            }
        }
        html = html.replace('.png#LOCKED"', lang === 'de' ? '.png"' : '-de.png"');

        html = html.replace('<html lang="en"', `<html lang="${config.htmlLang}"`);

        // Language-switch control: was a JS-driven button, now a real link (no client-side
        // re-render needed — each language already has its own fully rendered URL).
        html = html.replace(
            '<button type="button" id="site-lang-btn" class="language-btn"></button>',
            `<a href="${config.otherLangPath}" class="language-btn" aria-label="${escapeAttr(config.otherLangLabel)}" title="${escapeAttr(config.otherLangLabel)}">${flags[config.otherLangKey]}</a>`
        );

        // The German page lives one directory deeper; its asset references need ../.
        if (config.assetPrefix) {
            html = html
                .replace(/href="style\.css"/, `href="${config.assetPrefix}style.css"`)
                .replace(/href="guides\.css"/, `href="${config.assetPrefix}guides.css"`)
                .replace(/src="app\.js"/, `src="${config.assetPrefix}app.js"`)
                .replace(/href="favicon\.svg"/, `href="${config.assetPrefix}favicon.svg"`)
                .replace(/(href|src)="icon\.png"/g, `$1="${config.assetPrefix}icon.png"`)
                .replace(/(href|src)="images\//g, `$1="${config.assetPrefix}images/`);
        }

        html = html.replace('<!--SEO_HEAD-->', buildHead(lang, config));
        // A changed stylesheet must also reach returning visitors with a cached asset.
        const styleDigest = require('node:crypto').createHash('sha256')
            .update(fs.readFileSync(path.join(docsDir, 'style.css'))).digest('hex').slice(0, 12);
        html = html.replace(/href="((?:\.\.\/)?style\.css)"/, `href="$1?v=${styleDigest}"`);
        html = html.replace(/href="((?:\.\.\/)?guides\.css)"/, `href="$1?v=${require('./buildLearningPages').digest('guides.css')}"`);

        fs.mkdirSync(config.outDir, { recursive: true });
        const outPath = path.join(config.outDir, 'index.html');
        fs.writeFileSync(outPath, html);
        console.log(`Wrote ${path.relative(projectRoot, outPath)}`);
    }

    require('./buildNoticesPage')({flags});
    require('./buildLearningPages').generate({flags});
    require('./buildBenchmarkPages').generate({flags,dictionaries});
    require('./buildProductGuides')({dictionaries, release, faqEntries});
    writeSitemap();
}

/** Only canonical HTML pages; no invented lastmod dates on unchanged rebuilds. */
function writeSitemap() {
    const urls = [
        { loc: `${SITE_BASE}/` },
        { loc: `${SITE_BASE}/de/` },
        { loc: `${SITE_BASE}/third-party-notices/`, notices: true },
        { loc: `${SITE_BASE}/de/third-party-notices/`, notices: true },
        { loc: `${SITE_BASE}/guides/`, hub: true },
        { loc: `${SITE_BASE}/de/ratgeber/`, hub: true },
        { loc: `${SITE_BASE}/benchmarks/`, benchmark: true },
        { loc: `${SITE_BASE}/de/benchmarks/`, benchmark: true },
        ...require('./buildKnowledgeHub').reports.flatMap(article => ['en','de'].map(lang => ({loc:`${SITE_BASE}/${article[lang].slug}/`,article}))),
        ...['en','de'].map(lang=>({loc:`${SITE_BASE}${require('./buildLearningPages').hubRoute(lang)}discreveal/`,article:{en:{slug:'guides/discreveal'},de:{slug:'de/ratgeber/discreveal'}}})),
        ...require('./buildLearningPages').articles.flatMap(article => ['en','de'].map(lang => ({
            loc: `${SITE_BASE}/${article[lang].slug}/`, article,
        }))),
    ];
    const alternates = `
    <xhtml:link rel="alternate" hreflang="en" href="${SITE_BASE}/" />
    <xhtml:link rel="alternate" hreflang="de" href="${SITE_BASE}/de/" />
    <xhtml:link rel="alternate" hreflang="x-default" href="${SITE_BASE}/" />`;

    const body = urls
        .map(
            (u) => `  <url>
    <loc>${u.loc}</loc>${u.article?.updated || u.article?.published ? `\n    <lastmod>${u.article.updated || u.article.published}</lastmod>` : ''}${u.article ? `
    <xhtml:link rel="alternate" hreflang="en" href="${SITE_BASE}/${u.article.en.slug}/" />
    <xhtml:link rel="alternate" hreflang="de" href="${SITE_BASE}/${u.article.de.slug}/" />
    <xhtml:link rel="alternate" hreflang="x-default" href="${SITE_BASE}/${u.article.en.slug}/" />` : u.benchmark ? alternates.replaceAll(`${SITE_BASE}/de/`, `${SITE_BASE}/de/benchmarks/`).replaceAll(`href="${SITE_BASE}/"`, `href="${SITE_BASE}/benchmarks/"`) : u.hub ? alternates.replaceAll(`${SITE_BASE}/de/`, `${SITE_BASE}/de/ratgeber/`).replaceAll(`href="${SITE_BASE}/"`, `href="${SITE_BASE}/guides/"`) : u.notices ? alternates.replaceAll(`${SITE_BASE}/de/`, `${SITE_BASE}/de/third-party-notices/`).replaceAll(`href="${SITE_BASE}/"`, `href="${SITE_BASE}/third-party-notices/"`) : alternates}
  </url>`
        )
        .join('\n');

    const xml = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">
${body}
</urlset>
`;
    const outPath = path.join(docsDir, 'sitemap.xml');
    fs.writeFileSync(outPath, xml);
    console.log(`Wrote ${path.relative(projectRoot, outPath)}`);
}

generate();
