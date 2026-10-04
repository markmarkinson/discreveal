const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const os=require('node:os');
const path=require('node:path');
const {promoteRelease,verifyRelease}=require('../scripts/releaseArtifacts');

test('failed promotion restores the previous complete set and preferences',()=>{
    const root=fs.mkdtempSync(path.join(os.tmpdir(),'discreveal-release-test-'));
    try {
        const staged=path.join(root,'stage'),dist=path.join(root,'dist');
        fs.mkdirSync(staged);fs.mkdirSync(dist);
        for(const name of ['discreveal-portable.exe','THIRD-PARTY-LICENSES.html'])fs.writeFileSync(path.join(staged,name),'old '+name);
        promoteRelease(staged,dist,'1.0.0');
        const previous=fs.readFileSync(path.join(dist,'release-manifest.json'));
        fs.writeFileSync(path.join(dist,'settings.ini'),'preferences');
        const metadata=path.join(root,'release.json');fs.writeFileSync(metadata,'old facts');
        for(const name of ['discreveal-portable.exe','THIRD-PARTY-LICENSES.html'])fs.writeFileSync(path.join(staged,name),'new '+name);
        for(const failAt of [0,1,2,3]) {
            assert.throws(()=>promoteRelease(staged,dist,'1.1.0',[[metadata,Buffer.from('new facts')]],index=>{
                if(index===failAt)throw Error('injected failure');
            }),/injected failure/);
            assert.deepEqual(fs.readFileSync(path.join(dist,'release-manifest.json')),previous);
            assert.equal(fs.readFileSync(metadata,'utf8'),'old facts');
            assert.equal(fs.readFileSync(path.join(dist,'settings.ini'),'utf8'),'preferences');
            assert.equal(verifyRelease(dist,'1.0.0').version,'1.0.0');
        }
    } finally {fs.rmSync(root,{recursive:true,force:true});}
});

test('interrupted or tampered releases cannot pass artifact verification',()=>{
    const root=fs.mkdtempSync(path.join(os.tmpdir(),'discreveal-release-test-'));
    try {
        const staged=path.join(root,'stage'),dist=path.join(root,'dist');fs.mkdirSync(staged);
        for(const name of ['discreveal-portable.exe','THIRD-PARTY-LICENSES.html'])fs.writeFileSync(path.join(staged,name),'asset');
        promoteRelease(staged,dist,'1.0.0');
        fs.writeFileSync(path.join(dist,'.release-pending'),'interrupted');
        assert.throws(()=>verifyRelease(dist,'1.0.0'),/incomplete/);
        assert.throws(()=>promoteRelease(staged,dist,'1.0.0'),/interrupted/);
        fs.unlinkSync(path.join(dist,'.release-pending'));
        fs.writeFileSync(path.join(dist,'discreveal-portable.exe'),'different');
        assert.throws(()=>verifyRelease(dist,'1.0.0'),/does not match/);
    } finally {fs.rmSync(root,{recursive:true,force:true});}
});
