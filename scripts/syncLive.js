/** Publish one root source commit. Old public versions remain until downloads and Pages verify. */
const {execFileSync}=require('node:child_process');
const fs=require('node:fs');
const path=require('node:path');
const crypto=require('node:crypto');
const root=path.join(__dirname,'..');
const repository='markmarkinson/discreveal', remote='live';
const command=(name,args,input)=>execFileSync(name,args,{cwd:root,encoding:'utf8',input,stdio:['pipe','pipe','pipe']}).trim();

// Inject only external boundaries: regression tests run without credentials or remote mutations.
function synchronize(options={}) {
    const run=options.run||command;
    const git=(...args)=>run('git',args), gh=(...args)=>run('gh',args);
    const stateFile=path.join(root,'.internal','live-sync-state.json');
    const readState=options.readState||(()=>fs.existsSync(stateFile)?JSON.parse(fs.readFileSync(stateFile,'utf8')):null);
    const writeState=options.writeState||(state=>{
        fs.mkdirSync(path.dirname(stateFile),{recursive:true});
        fs.writeFileSync(stateFile+'.tmp',JSON.stringify(state,null,2)+'\n');
        fs.renameSync(stateFile+'.tmp',stateFile);
    });
    const log=options.log||console.log;
    if(git('status','--porcelain'))throw Error('Commit all source changes before syncing live');
    const liveUrl=git('remote','get-url',remote);
    if(liveUrl!==`https://github.com/${repository}.git`)throw Error('Unexpected live repository');
    if(git('remote','get-url','origin')===liveUrl)throw Error('Development and live remotes must be separate');
    const version=options.version||JSON.parse(fs.readFileSync(path.join(root,'package.json'),'utf8')).version;
    if(!/^\d+\.\d+\.\d+$/.test(version))throw Error('Expected a stable semantic version');
    const manifest=options.manifest||require('./releaseArtifacts').verifyRelease(path.join(root,'dist'),version);
    const facts=options.facts||require('./releaseMetadata').readReleaseMetadata();
    if(facts.version!==version || manifest.version!==version)throw Error('Build and release facts differ');
    const tag='current', mainRef='refs/heads/main', tagRef=`refs/tags/${tag}`;
    const release=JSON.parse(gh('release','view',tag,'--repo',repository,'--json','assets,isDraft,tagName'));
    if(release.isDraft || release.tagName!==tag)throw Error('Publish the current release before syncing its source snapshot');
    for(const name of ['discreveal-portable.exe','THIRD-PARTY-LICENSES.html']) {
        const asset=release.assets.find(item=>item.name===name), expected=manifest.assets[name];
        if(!asset || !expected || asset.digest!==`sha256:${expected.sha256}` || asset.size!==expected.bytes)throw Error(`Release artifact does not match dist/${name}`);
    }
    const getRefs=()=>git('ls-remote','--heads','--tags',remote).split('\n').filter(Boolean).map(line=>line.split(/\s+/)).filter(([,ref])=>!ref.endsWith('^{}'));
    const refs=getRefs();
    if(!refs.some(([,ref])=>ref===mainRef))throw Error('Live main branch is missing');
    const getOldReleases=()=>JSON.parse(gh('api','--paginate','--slurp',`repos/${repository}/releases`)).flat().filter(item=>item.tag_name!==tag);
    const obsoleteReleases=getOldReleases();
    log(`Live ${tag}: one source commit, one tag, one release`);
    log(`Remove ${refs.filter(([,ref])=>ref!==mainRef&&ref!==tagRef).length} obsolete refs and ${obsoleteReleases.length} old releases after verification`);
    if(options.dryRun){log('Dry run: no Git refs, releases or remote history changed');return;}
    const tree=git('rev-parse','HEAD^{tree}');
    const artifactFingerprint=crypto.createHash('sha256').update(JSON.stringify(manifest)).digest('hex');
    const previous=readState();
    let state=previous?.version===version && previous.tree===tree && previous.artifactFingerprint===artifactFingerprint?previous:{version,tree,artifactFingerprint,phase:'validated'};
    const record=phase=>{state={...state,phase,error:null,updatedAt:new Date().toISOString()};writeState(state);};
    try {
        // Persist commit/tag before pushing: retries reuse these objects even after an ambiguous network failure.
        if(!state.snapshot || !state.tagObject) {
            const snapshot=run('git',['commit-tree',tree],'DiscReveal Portable\n');
            const identity=git('var','GIT_COMMITTER_IDENT');
            const tagObject=run('git',['mktag'],`object ${snapshot}\ntype commit\ntag ${tag}\ntagger ${identity}\n\nDiscReveal Portable\n`);
            state={...state,snapshot,tagObject};record('validated');
        }
        // Fail early if the persisted local objects were removed, rather than recording a fictitious successful resume.
        git('cat-file','-e',`${state.snapshot}^{commit}`);
        git('cat-file','-e',state.tagObject);
        const sourceMatches=refs.some(([sha,ref])=>ref===mainRef&&sha===state.snapshot) && refs.some(([sha,ref])=>ref===tagRef&&sha===state.tagObject);
        const requestPagesBuild=!sourceMatches || state.phase!=='complete';
        if(!sourceMatches) {
            const leases=[mainRef,tagRef].map(ref=>`--force-with-lease=${ref}:${refs.find(([,name])=>name===ref)?.[0]||''}`);
            git('push','--atomic',...leases,remote,`${state.snapshot}:${mainRef}`,`${state.tagObject}:${tagRef}`);
        }
        record('source-pushed');
        gh('release','edit',tag,'--repo',repository,'--target','main','--latest');
        record('latest-release-selected');
        if(requestPagesBuild)gh('api','--method','POST',`repos/${repository}/pages/builds`);
        (options.verify||(()=>execFileSync(process.execPath,[path.join(root,'scripts','verifyPublished.js'),'--wait'],{cwd:root,stdio:'inherit'})))();
        record('public-downloads-verified');
        // Re-read after verification so a partly completed deletion can resume without stale IDs.
        const currentRefs=getRefs();
        if(!currentRefs.some(([sha,ref])=>ref===mainRef&&sha===state.snapshot) || !currentRefs.some(([sha,ref])=>ref===tagRef&&sha===state.tagObject))throw Error('Live source changed after verification; old releases retained');
        for(const old of getOldReleases())gh('release','delete',old.tag_name,'--repo',repository,'--yes');
        const obsoleteRefs=currentRefs.filter(([,ref])=>ref!==mainRef&&ref!==tagRef);
        if(obsoleteRefs.length)git('push','--atomic',...obsoleteRefs.map(([sha,ref])=>`--force-with-lease=${ref}:${sha}`),remote,...obsoleteRefs.map(([,ref])=>`:${ref}`));
        git('fetch',remote);
        record('complete');
        log(`Live snapshot: ${state.snapshot}`);
        return state;
    } catch(error) {
        state={...state,error:error.stderr?.toString().trim()||error.message,updatedAt:new Date().toISOString()};
        writeState(state);
        throw error;
    }
}
module.exports={synchronize};
if(require.main===module){try{synchronize({dryRun:process.argv.includes('--dry-run')});}catch(error){console.error(error.stderr?.toString().trim()||error.message);process.exitCode=1;}}
