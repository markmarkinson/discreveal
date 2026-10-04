const test=require('node:test');
const assert=require('node:assert/strict');
const crypto=require('node:crypto');
const {synchronize}=require('../scripts/syncLive');
const {verifyPublished}=require('../scripts/verifyPublished');
const version='1.2.3', tag='current';
const files={'discreveal-portable.exe':Buffer.from('executable'),'THIRD-PARTY-LICENSES.html':Buffer.from('licenses')};
const manifest={version,assets:Object.fromEntries(Object.entries(files).map(([name,bytes])=>[name,{bytes:bytes.length,sha256:crypto.createHash('sha256').update(bytes).digest('hex')}]))};
const facts={version,file:'discreveal-portable.exe',...manifest.assets['discreveal-portable.exe']};
function simulation() {
    let state=null, failing=null, commits=0, verifies=0;
    const calls=[];
    const refs=new Map([['refs/heads/main','old-main'],['refs/tags/v1.0.0','old-tag']]);
    const releases=new Set(['v1.0.0','v1.1.0',tag]);
    const options={version,manifest,facts,log(){},readState:()=>state,writeState:value=>{state=structuredClone(value);},verify:()=>{verifies++;if(failing==='verify')throw Error('injected verify');},run(name,args){
        const text=[name,...args].join(' ');calls.push(text);
        if(failing==='push'&&name==='git'&&args[0]==='push')throw Error('injected push');
        if(failing==='edit'&&name==='gh'&&args[0]==='release'&&args[1]==='edit')throw Error('injected edit');
        if(failing==='delete'&&name==='gh'&&args[0]==='release'&&args[1]==='delete'){releases.delete(args[2]);failing=null;throw Error('ambiguous delete');}
        if(name==='git') {
            if(args[0]==='status'||args[0]==='cat-file'||args[0]==='fetch')return '';
            if(args[0]==='remote')return args[2]==='live'?'https://github.com/markmarkinson/discreveal.git':'https://github.com/private/dev.git';
            if(args[0]==='rev-parse')return 'source-tree';
            if(args[0]==='commit-tree'){commits++;return 'snapshot-'+commits;}
            if(args[0]==='var')return 'Test <test@example.com> 1 +0000';
            if(args[0]==='mktag')return 'tag-object-'+commits;
            if(args[0]==='ls-remote')return [...refs].map(([ref,sha])=>sha+'\t'+ref).join('\n');
            if(args[0]==='push'){for(const arg of args){if(arg.startsWith(':refs/'))refs.delete(arg.slice(1));else if(/^[^:]+:refs\//.test(arg)){const [sha,ref]=arg.split(':');refs.set(ref,sha);}}return '';}
        }
        if(name==='gh') {
            if(args[0]==='api' && args.includes('POST'))return JSON.stringify({status:'queued'});
            if(args[0]==='api')return JSON.stringify([[...releases].map(tag_name=>({tag_name}))]);
            if(args[1]==='view')return JSON.stringify({tagName:tag,isDraft:false,assets:Object.entries(manifest.assets).map(([name,asset])=>({name,size:asset.bytes,digest:'sha256:'+asset.sha256}))});
            if(args[1]==='delete'){releases.delete(args[2]);return '';}
            if(args[1]==='edit')return '';
        }
        throw Error('Unexpected command: '+text);
    }};
    return {options,calls,refs,releases,get state(){return state;},get commits(){return commits;},get verifies(){return verifies;},fail(value){failing=value;}};
}
for(const failure of ['push','edit','verify'])test('publication '+failure+' failure retains old releases and resumes the same snapshot',()=>{
    const sim=simulation();sim.fail(failure);
    assert.throws(()=>synchronize(sim.options),/injected/);
    assert.ok(sim.releases.has('v1.0.0'));assert.ok(sim.refs.has('refs/tags/v1.0.0'));
    assert.notEqual(sim.state.phase,'complete');assert.match(sim.state.error,/injected/);
    sim.fail(null);synchronize(sim.options);
    assert.equal(sim.commits,1);assert.equal(sim.state.phase,'complete');assert.deepEqual([...sim.releases],[tag]);
    assert.deepEqual([...sim.refs.keys()],['refs/heads/main','refs/tags/'+tag]);
    const before=sim.verifies;synchronize(sim.options);assert.equal(sim.commits,1);assert.equal(sim.verifies,before+1);
});
test('ambiguous deletion resumes from current remote releases and verifies again',()=>{
    const sim=simulation();sim.fail('delete');assert.throws(()=>synchronize(sim.options),/ambiguous/);
    assert.equal(sim.state.phase,'public-downloads-verified');
    synchronize(sim.options);assert.equal(sim.commits,1);assert.equal(sim.verifies,2);assert.deepEqual([...sim.releases],[tag]);
});
test('dry run creates no state and mutates no remote refs or releases',()=>{
    const sim=simulation();synchronize({...sim.options,dryRun:true});assert.equal(sim.state,null);
    assert.equal(sim.commits,0);assert.equal(sim.verifies,0);assert.ok(sim.calls.every(line=>!line.includes(' push ')&&!line.includes(' delete ')&&!line.includes(' edit ')&&!line.includes(' --method POST ')));
});
function publicNetwork(problem) {
    return async(url,options)=>{
        assert.ok(options.signal);assert.equal(options.cache,'no-store');
        let value;
        if(url.endsWith('/release.json'))value=problem==='stale-site'?{...facts,version:'1.0.0'}:facts;
        else if(url.endsWith('/releases/latest'))value={tag_name:tag,assets:Object.entries(manifest.assets).map(([name,asset])=>({name,digest:'sha256:'+asset.sha256,size:asset.bytes,browser_download_url:'https://github.com/markmarkinson/discreveal/releases/download/'+(problem==='wrong-tag'?'v1.0.0':tag)+'/'+name}))};
        else if(url.includes('/download/'))value=problem==='tampered'?Buffer.from('tampered'):files[url.split('/').pop()];
        else value=problem==='stale-page'?'<strong>v1.0.0</strong>':`data-release-sha256="${facts.sha256}" /releases/latest/download/discreveal-portable.exe`;
        return {ok:problem!=='http-error',status:503,json:async()=>value,text:async()=>value,arrayBuffer:async()=>value};
    };
}
test('public verification downloads and checks both complete artifacts and localized pages',async()=>{
    assert.equal((await verifyPublished(facts,publicNetwork(),manifest)).actualDownloadsVerified,true);
});
for(const problem of ['stale-site','wrong-tag','tampered','stale-page','http-error'])test('public verification rejects '+problem,async()=>{
    await assert.rejects(()=>verifyPublished(facts,publicNetwork(problem),manifest));
});
test('public verification rejects missing manifest assets and network failures',async()=>{
    await assert.rejects(()=>verifyPublished(facts,publicNetwork(),{version,assets:{}}),/incomplete/);
    await assert.rejects(()=>verifyPublished(facts,async()=>{throw Error('network offline');},manifest),/network offline/);
});
test('a concurrent source change after public verification keeps older releases',()=>{
    const sim=simulation();
    sim.options.verify=()=>sim.refs.set('refs/heads/main','another-publisher');
    assert.throws(()=>synchronize(sim.options),/source changed after verification/);
    assert.ok(sim.releases.has('v1.0.0'));assert.ok(sim.releases.has('v1.1.0'));
    assert.equal(sim.state.phase,'public-downloads-verified');assert.notEqual(sim.state.phase,'complete');
});

test('publication requests Pages for a new snapshot and skips rebuilding an unchanged completed snapshot',()=>{
    const sim=simulation();synchronize(sim.options);
    const pages=()=>sim.calls.filter(line=>line.includes(' --method POST ')&&line.endsWith('/pages/builds')).length;
    assert.equal(pages(),1);
    synchronize(sim.options);assert.equal(pages(),1);
});
