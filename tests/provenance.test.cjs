const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {discRevealPngChunks,discRevealPngMetadata,discRevealIconMetadata,markMarkinsonCrc32}=require('../scripts/provenance');
const read=file=>fs.readFileSync(path.join(__dirname,'..',file));

function unchangedPixels(before,after) {
    const payload=bytes=>discRevealPngChunks(bytes).filter(chunk=>chunk.type!=='tEXt').map(chunk=>chunk.raw);
    assert.deepEqual(payload(after),payload(before));
    for(const chunk of discRevealPngChunks(after)) {
        assert.equal(chunk.raw.readUInt32BE(chunk.raw.length-4),markMarkinsonCrc32(chunk.raw.subarray(4,-4)));
    }
}

test('PNG attribution is idempotent and preserves pixel/color chunks with valid CRCs',()=>{
    const before=read('docs/images/list-view.png'),after=discRevealPngMetadata(before);
    unchangedPixels(before,after);
    assert.deepEqual(discRevealPngMetadata(after),after);
    const tags=discRevealPngChunks(after).filter(chunk=>chunk.type==='tEXt').map(chunk=>chunk.data.toString('latin1'));
    assert.ok(tags.includes('Author\0markMarkinson'));
    assert.ok(tags.includes('Software\0discReveal'));
    assert.equal(tags.filter(tag=>tag.startsWith('Author\0')).length,1);
});

test('ICO metadata preserves every image and keeps resource offsets and sizes valid',()=>{
    const before=read('src-tauri/icons/icon.ico'),after=discRevealIconMetadata(before);
    const count=before.readUInt16LE(4);assert.equal(after.readUInt16LE(4),count);
    let end=6+16*count;
    for(let index=0;index<count;index++) {
        const entry=6+16*index;
        assert.deepEqual(after.subarray(entry,entry+8),before.subarray(entry,entry+8));
        assert.equal(after.readUInt32LE(entry+12),end);
        const image=bytes=>{const offset=bytes.readUInt32LE(entry+12);return bytes.subarray(offset,offset+bytes.readUInt32LE(entry+8));};
        unchangedPixels(image(before),image(after));end+=after.readUInt32LE(entry+8);
    }
    assert.equal(end,after.length);assert.deepEqual(discRevealIconMetadata(after),after);
});

test('ICNS metadata preserves PNG pixels and legacy bitmap resources',()=>{
    const before=read('src-tauri/icons/icon.icns'),after=discRevealIconMetadata(before);
    const entries=bytes=>{const list=[];for(let p=8;p<bytes.length;){const n=bytes.readUInt32BE(p+4);list.push({type:bytes.toString('ascii',p,p+4),body:bytes.subarray(p+8,p+n)});p+=n;}return list;};
    const original=entries(before),updated=entries(after);assert.equal(original.length,updated.length);
    assert.equal(after.readUInt32BE(4),after.length);
    updated.forEach((entry,index)=>{
        assert.equal(entry.type,original[index].type);
        if(entry.body[0]===137)unchangedPixels(original[index].body,entry.body);
        else assert.deepEqual(entry.body,original[index].body);
    });
    assert.deepEqual(discRevealIconMetadata(after),after);
});

test('invalid image containers are rejected instead of rewritten',()=>{
    assert.throws(()=>discRevealPngMetadata(Buffer.from('not an image')));
    assert.throws(()=>discRevealIconMetadata(Buffer.alloc(16)));
});
