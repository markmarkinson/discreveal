/** Read-only, end-to-end publication check. Metadata alone is not a completed deployment. */
const crypto=require('node:crypto');
const fs=require('node:fs');
const path=require('node:path');
async function verifyPublished(facts, request=fetch, expectedManifest) {
    async function read(url,type='json') {
        const response=await request(url,{headers:{Accept:'application/vnd.github+json'},signal:AbortSignal.timeout(15000),cache:'no-store'});
        if(!response.ok)throw Error(`${url}: HTTP ${response.status}`);
        return type==='json'?response.json():type==='text'?response.text():response.arrayBuffer();
    }
    const site=await read('https://discreveal.com/release.json');
    const release=await read('https://api.github.com/repos/markmarkinson/discreveal/releases/latest');
    if(site.version!==facts.version || release.tag_name!=='current')throw Error('Website and latest release have not reached the expected version');
    const manifest=expectedManifest || JSON.parse(fs.readFileSync(path.join(__dirname,'../dist/release-manifest.json')));
    if(manifest.version!==facts.version || !manifest.assets?.['discreveal-portable.exe'] || !manifest.assets?.['THIRD-PARTY-LICENSES.html'])throw Error('Expected release manifest is incomplete or stale');
    for(const [name,expected] of Object.entries(manifest.assets)) {
        const asset=(release.assets||[]).find(item=>item.name===name);
        const url=asset?.browser_download_url;
        if(!url || !url.includes('/download/current/') || !/^https:\/\/github\.com\/markmarkinson\/discreveal\/releases\/download\/[^/]+\/(discreveal-portable\.exe|THIRD-PARTY-LICENSES\.html)$/.test(url))throw Error(`Missing official asset: ${name}`);
        if(asset.digest!==`sha256:${expected.sha256}` || asset.size!==expected.bytes)throw Error(`GitHub asset mismatch: ${name}`);
        const bytes=Buffer.from(await read(url,'bytes'));
        if(bytes.length!==expected.bytes || crypto.createHash('sha256').update(bytes).digest('hex')!==expected.sha256)throw Error(`Actual download mismatch: ${name}`);
    }
    if(site.sha256!==facts.sha256 || site.bytes!==facts.bytes)throw Error('Website executable facts differ');
    for(const suffix of ['','de/']) {
        const html=await read('https://discreveal.com/'+suffix,'text');
        if(!html.includes(`data-release-sha256="${facts.sha256}"`) || !html.includes('/releases/latest/download/discreveal-portable.exe'))throw Error(`Published ${suffix||'English'} page is stale`);
    }
    return {version:facts.version,verifiedAt:new Date().toISOString(),actualDownloadsVerified:true};
}
module.exports={verifyPublished};
if(require.main===module) {
    (async()=>{
        const facts=require('./releaseMetadata').readReleaseMetadata();
        const attempts=process.argv.includes('--wait')?24:1;
        let last;
        for(let n=0;n<attempts;n++) {
            try {console.log(JSON.stringify(await verifyPublished(facts)));return;}
            catch(error){last=error;if(n+1<attempts)await new Promise(resolve=>setTimeout(resolve,5000));}
        }
        throw last;
    })().catch(error=>{console.error(error.message);process.exitCode=1;});
}
