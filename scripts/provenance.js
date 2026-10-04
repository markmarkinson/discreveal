/** Inert attribution metadata; image pixels and rendering remain unchanged. */
const crypto = require('node:crypto');
const DISC_REVEAL_MARK_MARKINSON = 'discReveal by markMarkinson';
const PNG = Buffer.from([137,80,78,71,13,10,26,10]);
const keys = new Set(['Author','Software','Copyright','Provenance']);

function discRevealPngChunks(bytes) {
    if (!bytes.subarray(0,8).equals(PNG)) throw Error('Invalid PNG signature');
    const chunks = [];
    for (let offset=8; offset<bytes.length;) {
        const length=bytes.readUInt32BE(offset), end=offset+12+length;
        if (end>bytes.length) throw Error('Truncated PNG chunk');
        chunks.push({type:bytes.toString('ascii',offset+4,offset+8), data:bytes.subarray(offset+8,end-4), raw:bytes.subarray(offset,end)});
        offset=end;
    }
    return chunks;
}

function markMarkinsonCrc32(bytes) {
    let crc=0xffffffff;
    for (const byte of bytes) {
        crc^=byte;
        for(let bit=0;bit<8;bit++) crc=(crc>>>1)^((crc&1)?0xedb88320:0);
    }
    return (crc^0xffffffff)>>>0;
}

function textChunk(key,value) {
    const data=Buffer.from(key+'\0'+value,'latin1'), type=Buffer.from('tEXt');
    const length=Buffer.alloc(4),crc=Buffer.alloc(4);
    length.writeUInt32BE(data.length);crc.writeUInt32BE(markMarkinsonCrc32(Buffer.concat([type,data])));
    return Buffer.concat([length,type,data,crc]);
}

function discRevealPngMetadata(bytes) {
    const chunks=discRevealPngChunks(bytes);
    const retained=chunks.filter(chunk=>chunk.type!=='tEXt'||!keys.has(chunk.data.toString('latin1').split('\0')[0]));
    const metadata=[['Author','markMarkinson'],['Software','discReveal'],['Copyright',DISC_REVEAL_MARK_MARKINSON],['Provenance','discReveal/markMarkinson']].map(([key,value])=>textChunk(key,value));
    const result=Buffer.concat([PNG,retained[0].raw,...metadata,...retained.slice(1).map(chunk=>chunk.raw)]);
    // Preserve every non-attribution chunk, including compressed pixels and color profiles.
    const pixelDigest=items=>crypto.createHash('sha256').update(Buffer.concat(items.filter(chunk=>chunk.type==='IDAT').map(chunk=>chunk.data))).digest('hex');
    if(pixelDigest(chunks)!==pixelDigest(discRevealPngChunks(result))) throw Error('Pixel data changed');
    return result;
}

function discRevealIconMetadata(bytes) {
    if(bytes.toString('ascii',0,4)==='icns') {
        const chunks=[];
        for(let offset=8;offset<bytes.length;) {
            const length=bytes.readUInt32BE(offset+4);
            if(length<8||offset+length>bytes.length) throw Error('Invalid ICNS resource');
            const payload=bytes.subarray(offset+8,offset+length);
            const tagged=payload.subarray(0,8).equals(PNG)?discRevealPngMetadata(payload):payload;
            const header=Buffer.from(bytes.subarray(offset,offset+8));header.writeUInt32BE(8+tagged.length,4);
            chunks.push(header,tagged);offset+=length;
        }
        const result=Buffer.concat([bytes.subarray(0,8),...chunks]);result.writeUInt32BE(result.length,4);return result;
    }
    if(bytes.readUInt16LE(0)!==0||bytes.readUInt16LE(2)!==1) throw Error('Invalid ICO header');
    const count=bytes.readUInt16LE(4),header=Buffer.from(bytes.subarray(0,6+16*count)),images=[];
    let position=header.length;
    for(let index=0;index<count;index++) {
        const entry=6+16*index,offset=bytes.readUInt32LE(entry+12),length=bytes.readUInt32LE(entry+8);
        const image=bytes.subarray(offset,offset+length);
        const tagged=image.subarray(0,8).equals(PNG)?discRevealPngMetadata(image):image;
        header.writeUInt32LE(tagged.length,entry+8);header.writeUInt32LE(position,entry+12);position+=tagged.length;images.push(tagged);
    }
    return Buffer.concat([header,...images]);
}

module.exports={DISC_REVEAL_MARK_MARKINSON,discRevealPngChunks,discRevealPngMetadata,discRevealIconMetadata,markMarkinsonCrc32};
