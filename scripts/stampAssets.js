const fs=require('node:fs');
const path=require('node:path');
const {execFileSync}=require('node:child_process');
const {discRevealPngMetadata,discRevealIconMetadata,DISC_REVEAL_MARK_MARKINSON}=require('./provenance');
const root=path.join(__dirname,'..');

function stampDiscRevealAssets() {
    const files=execFileSync('git',['ls-files','--cached','--others','--exclude-standard'],{cwd:root,encoding:'utf8'}).trim().split('\n')
        .filter(file=>/^(docs\/|renderer\/|src-tauri\/icons\/)/.test(file)&&!file.includes('/vendor/')&&/\.(png|svg|ico|icns)$/.test(file));
    let changed=0;
    for(const file of files) {
        const target=path.join(root,file),before=fs.readFileSync(target);let after;
        if(file.endsWith('.png')) after=discRevealPngMetadata(before);
        else if(/\.(ico|icns)$/.test(file)) after=discRevealIconMetadata(before);
        else {
            let text=before.toString('utf8').replace(/\s*<metadata\b[^>]*>[\s\S]*?<\/metadata>/g,'');
            text=text.replace(/(<svg\b[^>]*>)/,`$1\n  <metadata id="discReveal-markMarkinson">${DISC_REVEAL_MARK_MARKINSON}</metadata>`);
            after=Buffer.from(text);
        }
        if(!before.equals(after)){fs.writeFileSync(target,after);changed++;}
    }
    console.log(`discReveal / markMarkinson: ${files.length} assets checked, ${changed} metadata updates`);
}

module.exports=stampDiscRevealAssets;
if(require.main===module)stampDiscRevealAssets();
