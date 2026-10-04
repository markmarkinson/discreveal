const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const root = path.join(__dirname,'../docs');
const articles = require('../docs-src/guides');
const reports = require('../scripts/buildKnowledgeHub').reports;
const read = file => fs.readFileSync(path.join(root,file),'utf8');

test('published learning pages have reciprocal language links, discoverable references and working local links', () => {
    const sitemap = read('sitemap.xml'), index = read('llms.txt');
    for (const article of articles) for (const lang of ['en','de']) {
        const route = article[lang].slug+'/', html = read(route+'index.html');
        const canonical = 'https://discreveal.com/'+route;
        assert.equal((html.match(/<h1\b/g)||[]).length,1);
        assert.ok(html.includes(`<html lang="${lang}"`));
        assert.ok(html.includes(`rel="canonical" href="${canonical}"`));
        assert.ok(html.includes('content="index,follow,max-image-preview:large"'));
        for (const other of ['en','de']) assert.ok(html.includes(`hreflang="${other}" href="https://discreveal.com/${article[other].slug}/"`));
        const schema = [...html.matchAll(/<script type="application\/ld\+json">(.*?)<\/script>/g)].map(([,value])=>JSON.parse(value));
        assert.equal(schema[0].mainEntityOfPage,canonical);
        assert.equal(schema[0].inLanguage,lang);
        assert.equal(schema[0].datePublished,article.published||'2026-10-02');
        assert.equal(schema[0].dateModified,article.updated||article.published||'2026-10-02');
        assert.equal(schema[1].itemListElement.at(-1).item,canonical);
        assert.ok(sitemap.includes(`<loc>${canonical}</loc>`));
        assert.ok(index.includes(canonical+'index.md'));
        assert.ok(read(lang==='de'?'de/ratgeber/index.html':'guides/index.html').includes(`href="/${route}"`));
        if (article.image) assert.ok(read(route+'index.md').includes(article[lang].caption));
        assert.ok(!html.includes('undefined'));
        for (const [,href] of html.matchAll(/(?:href|src)="([^"]+)"/g)) {
            const url = new URL(href,canonical);
            if (url.hostname!=='discreveal.com') continue;
            const file = url.pathname.endsWith('/') ? url.pathname+'index.html' : url.pathname;
            assert.ok(fs.existsSync(path.join(root,file)),href);
            if (url.hash && file.endsWith('.html')) assert.ok(read(file).includes(`id="${url.hash.slice(1)}"`),href);
        }
    }
});

test('knowledge hubs list all articles with localized categories and search metadata', () => {
    for (const lang of ['en','de']) {
        const route=lang==='de'?'de/ratgeber/':'guides/', html=read(route+'index.html');
        const schema=[...html.matchAll(/<script type="application\/ld\+json">(.*?)<\/script>/g)].map(([,value])=>JSON.parse(value));
        assert.equal(schema[0]['@type'],'CollectionPage');
        assert.equal(schema[0].inLanguage,lang);
        assert.equal(schema[0].mainEntity.numberOfItems,articles.length+reports.length);
        assert.equal(new Set(schema[0].mainEntity.itemListElement.map(a=>a.url)).size,articles.length+reports.length);
        assert.ok(read('sitemap.xml').includes(`<loc>https://discreveal.com/${route}</loc>`));
        assert.ok(read(lang==='de'?'de/index.html':'index.html').includes(`href="/${route}"`));
        assert.ok(read('llms.txt').includes('https://discreveal.com/'+route));
        for (const id of ['benchmarks','windows','organization','security']) assert.ok(html.includes(`id="${id}"`));
        for (const article of articles) assert.ok(read(route+'index.md').includes(article[lang].title));
    }
});

test('knowledge articles retain primary references, read-only checksum examples and erasure limits', () => {
    const knowledge=articles.filter(a=>a.kind==='knowledge');
    assert.equal(knowledge.length,9);
    for (const article of knowledge) for (const lang of ['en','de']) {
        const html=read(article[lang].slug+'/index.html'), markdown=read(article[lang].slug+'/index.md');
        assert.ok(html.includes(`<time datetime="${article.published}">`));
        for (const id of article.related) assert.ok(articles.some(a=>a.id===id),id);
        for (const section of article[lang].sections) {
            for (const [,url] of section.sources||[]) {
                assert.ok(html.includes(`href="${url}"`));
                assert.ok(markdown.includes(url));
                assert.match(new URL(url).hostname,/(^|\.)(microsoft\.com|nist\.gov)$/);
            }
            if (section.code) {
                assert.equal(section.code,'Get-FileHash -LiteralPath .\\discreveal-portable.exe -Algorithm SHA256');
                assert.ok(markdown.includes('```powershell\n'+section.code));
            }
        }
    }
    assert.ok(read('de/ratgeber/sicheres-loeschen-hdd-ssd/index.html').includes('keine vollständige Unwiederherstellbarkeit'));
    assert.ok(read('guides/secure-deletion-hdd-ssd/index.html').includes('does not meet government or industry'));
    assert.ok(read('de/ratgeber/papierkorb-dateien-wirklich-geloescht/index.html').includes('keine Garantie gegen Datenrettung'));
});


test('update instructions live in a localized article and comparison reports are discoverable in knowledge', () => {
    const article=articles.find(a=>a.id==='update-check');
    for (const lang of ['de','en']) {
        const home=read(lang==='de'?'de/index.html':'index.html');
        const hub=read(lang==='de'?'de/ratgeber/index.html':'guides/index.html');
        assert.ok(!home.includes('class="update-steps"'));
        assert.ok(!home.includes('class="faq-item update-explanation"'));
        assert.ok(home.includes(`id="updates" class="update-jump" href="/${article[lang].slug}/"`));
        assert.equal((home.match(/class="guide-card"/g)||[]).length,2);
        const html=read(article[lang].slug+'/index.html'), md=read(article[lang].slug+'/index.md');
        for (const section of article[lang].sections) {
            assert.ok(html.includes(section.title));
            assert.ok(md.includes(section.title));
        }
        for (const report of reports) {
            assert.ok(hub.includes(`href="/${report[lang].slug}/"`));
            assert.ok(read((lang==='de'?'de/ratgeber/':'guides/')+'index.md').includes(report[lang].title));
        }
    }
});


test('DiscReveal is an additional tag and benchmark articles keep separate categories and language routes',()=>{
    for(const lang of ['de','en']) {
        const prefix=lang==='de'?'de/ratgeber/':'guides/';
        const tag=read(prefix+'discreveal/index.html'), hub=read(prefix+'index.html');
        assert.ok(hub.includes(`href="/${prefix}discreveal/"`));
        assert.ok(tag.includes('aria-current="page"'));
        assert.ok(tag.includes('id="benchmarks"'));
        assert.ok(tag.includes('id="organization"'));
        for(const id of ['disk-space','duplicate-files','how-duplicates']) {
            const a=articles.find(a=>a.id===id);
            assert.ok(a.tags.includes('discreveal'));
            assert.equal(a.category||'organization','organization');
            assert.ok(tag.includes(`href="/${a[lang].slug}/"`));
            assert.ok(read(a[lang].slug+'/index.html').includes(`href="/${prefix}discreveal/"`));
        }
        assert.ok(!tag.includes('href="/'+articles.find(a=>a.id==='prefetch')[lang].slug+'/"'));
        assert.equal(reports.length,2);
        for(const report of reports) {
            assert.equal(report.category,'benchmarks');
            const html=read(report[lang].slug+'/index.html');
            assert.ok(hub.includes(`href="/${report[lang].slug}/"`));
            assert.ok(html.includes(`id="${report.id}"`));
            assert.ok(!html.includes(`id="${reports.find(r=>r!==report).id}"`));
            assert.ok(read('sitemap.xml').includes(`https://discreveal.com/${report[lang].slug}/`));
            assert.ok(read('llms.txt').includes(`https://discreveal.com/${report[lang].slug}/index.md`));
            for(const other of ['de','en']) assert.ok(html.includes(`hreflang="${other}" href="https://discreveal.com/${report[other].slug}/"`));
        }
    }
});
