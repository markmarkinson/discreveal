const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');
const script = fs.readFileSync(path.join(__dirname, '../docs/404.html'), 'utf8').match(/<script>([\s\S]*?)<\/script>/)[1];

class Element {
    constructor(tag = 'div') { this.tagName = tag.toUpperCase(); this.children = []; this.hidden = true; }
    appendChild(child) { this.children.push(child); }
    append(...children) { this.children.push(...children); }
    replaceChildren(...children) { this.children = children; }
}

async function loadPage(pathname, hash, downloadUrl, metadata = {}) {
    const elements = new Map();
    const document = {documentElement:{}, createElement:tag=>new Element(tag), getElementById:id=>{
        if (!elements.has(id)) elements.set(id, new Element());
        return elements.get(id);
    }};
    vm.runInNewContext(script, {document,location:{pathname},AbortController,setTimeout,clearTimeout,fetch:async(url)=>({ok:true,json:async()=>url==='/release.json'?{version:'0.2.0',sha256:hash,...metadata}:({
        tag_name:'current',html_url:'https://github.com/markmarkinson/discreveal/releases/tag/v0.2.0',
        assets:[{name:'discreveal-portable.exe',digest:`sha256:${hash}`,browser_download_url:downloadUrl}]
    })})});
    await new Promise(resolve=>setImmediate(resolve));
    return {document,elements};
}

test('update page uses app language and aligns both hashes with exact mismatch positions', async()=>{
    const official = 'a'.repeat(64), local = 'a'.repeat(17) + 'b' + 'a'.repeat(46);
    for (const [prefix,language,title] of [['/de','de','Version stimmt, aber diese Datei nicht'],['','en','Version matches, but this file doesn’t']]) {
        const {document,elements} = await loadPage(`${prefix}/v-0.2.0/${local}`,official);
        assert.equal(document.documentElement.lang,language);
        assert.equal(elements.get('title').textContent,title);
        const rows=elements.get('hash-comparison').children.slice(0,2);
        for (const [index,value] of [[0,local],[1,official]]) {
            const characters=rows[index].children[1].children;
            assert.equal(characters.map(character=>character.textContent).join(''),value);
            assert.deepEqual(characters.flatMap((character,position)=>character.tagName==='MARK'?[position]:[]),[17]);
        }
    }
});

test('matching or invalid digests do not produce a misleading mismatch panel',async()=>{
    const hash='a'.repeat(64);
    for (const official of [hash,'not-a-hash']) {
        const {elements}=await loadPage(`/de/v-0.2.0/${hash}`,official);
        assert.equal(elements.get('hash-comparison'),undefined);
    }
});

test('current version distinguishes matching files from missing fingerprints', async()=>{
    const hash='a'.repeat(64);
    for (const prefix of ['', '/de']) {
        for (const [suffix,official,matched] of [[hash,hash,true],[hash,'invalid',false],['',hash,false]]) {
            const {elements}=await loadPage(`${prefix}/v-0.2.0/${suffix}`,official);
            const status=elements.get('body').children[0].textContent;
            assert.match(status,matched ? /matches|stimmt mit/ : /not checked|nicht geprüft/);
        }
    }
});

test('a hanging release request times out with a usable release link',async()=>{
    const elements=new Map();
    const document={documentElement:{},createElement:tag=>new Element(tag),getElementById:id=>{
        if(!elements.has(id)) elements.set(id,new Element()); return elements.get(id);
    }};
    let expire;
    vm.runInNewContext(script,{document,location:{pathname:'/de/v-0.2.0'},AbortController,
        setTimeout:callback=>{expire=callback;return 1;},clearTimeout:()=>{},
        fetch:(_url,{signal})=>new Promise((_resolve,reject)=>signal.addEventListener('abort',()=>reject(Error('timeout'))))});
    assert.equal(elements.get('action').children[0].href,'https://github.com/markmarkinson/discreveal/releases/latest');
    expire(); await new Promise(resolve=>setImmediate(resolve));
    assert.match(elements.get('title').textContent,/Konnte nicht/);
    assert.equal(elements.get('action').children.length,1);
});

test('an older version shows the update and labels its hash against the latest release',async()=>{
    const {elements}=await loadPage(`/de/v-0.1.0/${'a'.repeat(64)}`,'b'.repeat(64));
    assert.equal(elements.get('title').textContent,'Update verfügbar');
    assert.equal(elements.get('hash-comparison').children[1].children[0].textContent,'Offizieller Download · SHA-256');
});

test('update and changed-file buttons download the EXE directly, with a stable fallback',async()=>{
    const direct='https://github.com/markmarkinson/discreveal/releases/latest/download/discreveal-portable.exe';
    const asset='https://github.com/markmarkinson/discreveal/releases/download/v0.2.0/discreveal-portable.exe';
    for(const version of ['0.1.0','0.2.0']) {
        for(const url of [undefined,asset]) {
            const {elements}=await loadPage(`/de/v-${version}/${'a'.repeat(64)}`,'b'.repeat(64),url);
            assert.equal(elements.get('action').children[0].href,url || direct);
        }
    }
});

test('update-page flags are inline and switching languages retains the version and fingerprint',async()=>{
    const html=fs.readFileSync(path.join(__dirname,'../docs/404.html'),'utf8');
    for(const flag of ['de','en'])assert.match(html,new RegExp(`<span id="flag-${flag}"[^>]*><svg `));
    const hash='a'.repeat(64);
    for(const suffix of ['/v-0.2.0','/v-0.2.0/'+hash]) {
        for(const [prefix,other,visible,label] of [['','/de','de','Auf Deutsch umschalten'],['/de','','en','Switch to English']]) {
            const {elements}=await loadPage(prefix+suffix,hash);
            assert.equal(elements.get('language-link').href,other+suffix);
            assert.equal(elements.get('language-link').ariaLabel,label);
            assert.equal(elements.get('flag-'+visible).hidden,false);
            assert.equal(elements.get('flag-'+(visible==='de'?'en':'de')).hidden,true);
        }
    }
});


test('third-party notices use inline language flags with reciprocal localized links',()=>{
    for(const lang of ['de','en']) {
        const other=lang==='de'?'en':'de';
        const html=fs.readFileSync(path.join(__dirname,'../docs',lang==='de'?'de/third-party-notices/index.html':'third-party-notices/index.html'),'utf8');
        const link=html.match(/<a class="language-btn"[^>]*>[\s\S]*?<\/a>/)[0];
        assert.ok(link.includes(`href="/${other==='de'?'de/':''}third-party-notices/"`));
        assert.ok(link.includes(`lang="${other}"`));
        assert.match(link,/<svg[ >]/);
        assert.ok(!link.includes('<img'));
        assert.ok(!link.includes('undefined'));
    }
});

test('stable release metadata must match the actual official asset before confirming an update',async()=>{
    for(const metadata of [{version:'invalid'},{sha256:'b'.repeat(64)}]){
        const {elements}=await loadPage('/de/v-0.2.0/'+ 'a'.repeat(64),'a'.repeat(64),undefined,metadata);
        assert.equal(elements.get('title').textContent,'Konnte nicht automatisch geprüft werden');
        assert.equal(elements.get('action').children[0].href,'https://github.com/markmarkinson/discreveal/releases/latest');
    }
});

test('update results keep build numbers out of visible explanations',async()=>{
    for(const requested of ['0.1.0','0.2.0','9.9.9']){
        const {elements}=await loadPage('/de/v-'+requested+'/'+ 'a'.repeat(64),'b'.repeat(64));
        assert.doesNotMatch(elements.get('body').innerHTML,/\b\d+\.\d+\.\d+\b/);
    }
});
