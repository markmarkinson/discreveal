const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const vm = require('node:vm');
const root = path.join(__dirname,'..');
const read = file => fs.readFileSync(path.join(root,file),'utf8');
const release = JSON.parse(read('docs-src/release.json'));
const digest = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const escaped = text => text.replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;');

test('search metadata and release references agree with the shipped version and actual image dimensions', () => {
    for (const lang of ['en','de']) {
        const route = lang==='de' ? 'de/' : '';
        const html = read('docs/'+route+'index.html');
        const nodes = [...html.matchAll(/<script type="application\/ld\+json">(.*?)<\/script>/g)].map(([,text])=>JSON.parse(text));
        const app = nodes.find(node=>node['@type']==='SoftwareApplication');
        const page = nodes.find(node=>node['@type']==='WebPage');
        const site = nodes.find(node=>node['@type']==='WebSite');
        assert.equal(app.softwareVersion, undefined);
        assert.equal(JSON.parse(read('docs/release.json')).version, release.version);
        assert.equal(app.fileSize,release.bytes+' bytes');
        assert.equal(app.aggregateRating,undefined);
        assert.equal(page.mainEntity['@id'],app['@id']);
        assert.equal(page.isPartOf['@id'],site['@id']);
        assert.equal(page.inLanguage,lang);
        assert.match(html,new RegExp(`<link rel="canonical" href="https://discreveal.com/${route}"`));
        assert.ok(html.includes(`type="text/markdown" href="https://discreveal.com/${route}index.md"`));
        assert.ok(html.includes('rel="describedby" type="text/plain" href="https://discreveal.com/llms.txt"'));
        const image = fs.readFileSync(path.join(root,`docs/images/list-view${lang==='de'?'-de':''}.png`));
        assert.ok(html.includes(`property="og:image:width" content="${image.readUInt32BE(16)}"`));
        assert.ok(html.includes(`property="og:image:height" content="${image.readUInt32BE(20)}"`));
        assert.ok(html.includes(`<strong>DiscReveal Portable</strong>`));
        const description = /<meta name="description" content="([^"]+)"/.exec(html)[1];
        assert.ok(description.length<=180);
    }
    assert.deepEqual(JSON.parse(read('docs/release.json')).sha256,release.sha256);
    if (fs.existsSync(path.join(root,'dist/discreveal-portable.exe'))) {
        assert.equal(digest(fs.readFileSync(path.join(root,'dist/discreveal-portable.exe'))),release.sha256);
    }
});

test('FAQs are the same visible answers in HTML, structured data and both product guides', () => {
    for (const route of ['', 'de/']) {
        const html = read('docs/'+route+'index.html');
        const faq = [...html.matchAll(/<script type="application\/ld\+json">(.*?)<\/script>/g)]
            .map(([,value])=>JSON.parse(value)).find(value=>value['@type']==='FAQPage');
        assert.equal(faq.mainEntity.length,15);
        const guide = read('docs/'+route+'index.md');
        for (const question of faq.mainEntity) {
            assert.ok(html.includes('<span>'+escaped(question.name)+'</span>'),question.name);
            assert.ok(html.includes('<p>'+escaped(question.acceptedAnswer.text)+'</p>'),question.name);
            assert.ok(guide.includes('### '+question.name+'\n\n'+question.acceptedAnswer.text));
        }
        if (route) {
            assert.ok(html.includes('aria-label="Vorheriger Screenshot"'));
            assert.ok(html.includes('alt="DiscReveal-Listenansicht'));
        }
    }
});

test('AI reference links resolve to generated files and preserve current release and measured benchmark versions', () => {
    const index = read('docs/llms.txt');
    assert.ok(index.startsWith('# DiscReveal\n\n> '));
    for (const section of index.split(/^## /m).slice(1)) {
        const lines = section.split('\n').slice(1).filter(line=>line.trim());
        assert.ok(lines.every(line=>/^- \[[^\]]+\]\(https:\/\//.test(line)));
    }
    for (const file of ['llms.txt','llms-full.txt','index.md','de/index.md']) {
        const text = read('docs/'+file);
        for (const [,href] of text.matchAll(/\]\((https:\/\/discreveal\.com\/[^)]+)\)/g)) {
            const url = new URL(href);
            const filename = url.pathname.endsWith('/') ? url.pathname+'index.html' : url.pathname;
            assert.ok(fs.existsSync(path.join(root,'docs',filename)),href);
        }
    }
    for (const file of ['index.md','de/index.md']) {
        const text = read('docs/'+file);
        assert.ok(!text.includes(`Version: ${release.version}`));
        assert.ok(text.includes(release.sha256));
        const benchmarkPath=(file.startsWith('de/')?'de/':'')+'benchmarks/';
        assert.ok(text.includes('discreveal.com/'+benchmarkPath));
        assert.ok(read('docs/'+benchmarkPath+'index.md').includes('DiscReveal'));
        assert.ok(text.includes('10 MB'));
    }
    const sitemap = read('docs/sitemap.xml');
    for (const entry of sitemap.matchAll(/<url>([\s\S]*?)<\/url>/g)) {
        const location = /<loc>([^<]+)<\/loc>/.exec(entry[1])[1];
        const lastmod = /<lastmod>([^<]+)<\/lastmod>/.exec(entry[1])?.[1];
        const article = require('../docs-src/guides').find(a=>['en','de'].some(l=>location===`https://discreveal.com/${a[l].slug}/`));
        assert.equal(lastmod, article ? article.updated || article.published : undefined, location);
    }
    assert.ok(read('docs/404.html').includes('name="robots" content="noindex"'));
});

test('release metadata rejects a stale version or executable hash instead of advertising them', () => {
    const bytes = Buffer.from('test executable');
    let facts = {version:'0.2.4',file:'discreveal-portable.exe',bytes:bytes.length,sha256:digest(bytes)};
    const context = {module:{exports:{}},__dirname:path.join(root,'scripts'),require:name=>{
        if (name==='node:fs') return {existsSync:()=>true,readFileSync:file=>
            file.endsWith('package.json') ? JSON.stringify({version:'0.2.4'}) :
            file.endsWith('release.json') ? JSON.stringify(facts) : bytes};
        return require(name);
    }};
    vm.runInNewContext(read('scripts/releaseMetadata.js'),context);
    assert.equal(context.module.exports.readReleaseMetadata().sha256,digest(bytes));
    facts.version='0.2.3';
    assert.throws(()=>context.module.exports.readReleaseMetadata(),/stale/);
    facts.version='0.2.4';facts.sha256='0'.repeat(64);
    assert.throws(()=>context.module.exports.readReleaseMetadata(),/do not match/);
});

test('website API failure preserves the static version and file size', async () => {
    let ready;
    const version = {textContent:'v0.2.4 · 5.1 MB'};
    const document = {documentElement:{lang:'en'},querySelector:()=>null,
        querySelectorAll:selector=>selector==='.js-download-link'?[{}]:[],
        getElementById:id=>id==='download-version'?version:null,addEventListener:(_,handler)=>{ready=handler;}};
    vm.runInNewContext(read('docs/app.js'),{document,fetch:async()=>{throw Error('offline');}});
    ready();
    await new Promise(resolve=>setImmediate(resolve));
    assert.equal(version.textContent,'v0.2.4 · 5.1 MB');
});
