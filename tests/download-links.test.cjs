const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');
const direct = 'https://github.com/markmarkinson/discreveal/releases/latest/download/discreveal-portable.exe';
const read = p => fs.readFileSync(path.join(__dirname, '..', p), 'utf8');
async function load(fetch) {
    let ready; const links = [{href:direct},{href:direct},{href:direct}];
    const document = {documentElement:{lang:'en'},querySelector:()=>null,querySelectorAll:selector=>selector==='.js-download-link'?links:[],getElementById:()=>null,addEventListener:(_,fn)=>{ready=fn;}};
    vm.runInNewContext(read('docs/app.js'),{document,fetch}); ready();
    await new Promise(resolve=>setImmediate(resolve)); return links;
}
test('all static website and README download buttons work without JavaScript or GitHub API',()=>{
    for(const file of ['docs/index.html','docs/de/index.html']) {
        const html=read(file); const links=[...html.matchAll(/<a href="([^"]+)"[^>]*js-download-link/g)];
        assert.equal(links.length,3); for(const [,href] of links) assert.equal(href,direct);
        const metadata=JSON.parse(html.match(/<script type="application\/ld\+json">(.*?)<\/script>/)[1]);
        assert.equal(metadata.downloadUrl,direct);assert.equal(metadata.softwareVersion,undefined);
        assert.match(html,/href="\/(?:de\/)?benchmarks\/(?:drive-scan|laufwerksscan)\/"/);
    }
    for(const file of ['README.md','README.de.md'])assert.ok(read(file).includes(`<a href="${direct}"><img src="docs/images/readme-download-`));
});
test('download buttons keep working when GitHub API fails or no EXE is listed',async()=>{
    for(const fetch of [async()=>{throw Error('offline')},async()=>({ok:false}),async()=>({ok:true,json:async()=>({assets:[]})})]) {
        for(const link of await load(fetch))assert.equal(link.href,direct);
    }
});
test('release enhancement selects the portable app rather than an unrelated executable',async()=>{
    const url='https://github.com/markmarkinson/discreveal/releases/download/current/discreveal-portable.exe';
    const links=await load(async()=>({ok:true,json:async()=>({assets:[{name:'Other.exe',browser_download_url:'wrong'},{name:'discreveal-portable.exe',browser_download_url:url}]})}));
    for(const link of links)assert.equal(link.href,url);
});
