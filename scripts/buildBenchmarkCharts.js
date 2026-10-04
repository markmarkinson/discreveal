const fs=require('node:fs');
const path=require('node:path');
const data=require('../docs/benchmarks/scan-0.2.3.json');
const root=path.join(__dirname,'..');
const xml=s=>String(s).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;');
function buildBenchmarkCharts(){
    const tools=[...data.tools].sort((a,b)=>a.medianMs-b.medianMs);
    const max=Math.max(...tools.map(t=>t.medianMs));
    for(const lang of ['en','de']){
        const de=lang==='de';const number=n=>n.toLocaleString(de?'de-DE':'en-US');
        const title=de?'Scanzeit im direkten Vergleich':'Scan time on the same folder';
        const subtitle=(de?'rund ':'about ')+number(Math.round(data.files/10000)*10000)+' '+(de?'Dateien':'files')+' · '+(data.bytes/1e9).toFixed(2).replace('.',de?',':'.')+' GB · '+(de?'Median aus '+data.sampleCount+' Durchläufen · kürzer ist besser':'median of '+data.sampleCount+' runs · lower is better');
        const rows=tools.map((tool,i)=>{const y=78+i*48;const width=tool.medianMs/max*490;const app=tool.id==='discreveal';const time=(tool.medianMs/1000).toFixed(2).replace('.',de?',':'.');return `<text x="177" y="${y+18}" fill="${app?'#f5f5f5':'#cfcfcf'}" font-size="13" ${app?'font-weight="700" ':''}text-anchor="end">${xml(tool.chartName)}</text>\n  <rect x="190" y="${y}" width="${width.toFixed(1)}" height="26" rx="4" fill="${app?'#e50914':'#4a4040'}"/>\n  <text x="${(200+width).toFixed(1)}" y="${y+18}" fill="#f5f5f5" font-size="13" font-weight="700">${time} s</text>`;}).join('\n  ');
        const footer=de?'DiscReveal: vom Klick bis zur fertigen Ergebnisliste, inklusive Oberfläche.':'DiscReveal: from click to finished results, including the interface.';
        const svg=`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 780 360" font-family="Segoe UI, Arial, sans-serif" role="img" aria-label="${xml(title)}">\n  <metadata id="discReveal-markMarkinson">discReveal by markMarkinson</metadata>\n  <rect width="780" height="360" rx="12" fill="#120e0d"/>\n  <text x="24" y="34" fill="#f5f5f5" font-size="17" font-weight="700">${title}</text>\n  <text x="24" y="54" fill="#a3a3a1" font-size="12.5">${subtitle}</text>\n  <line x1="190" y1="70" x2="190" y2="303" stroke="#362929"/>\n  ${rows}\n  <text x="24" y="326" fill="#a3a3a1" font-size="11.5">${footer}</text>\n  <text x="24" y="346" fill="#a3a3a1" font-size="11.5">${de?'Alle Methoden: gleicher Ordner, Windows-Dateicache aufgewärmt, keine Dateiinhalte gelesen.':'All methods: same folder, warm Windows file cache, no file contents read.'}</text>\n</svg>\n`;
        fs.writeFileSync(path.join(root,'docs/images/chart-scan-speed'+(de?'-de':'')+'.svg'),svg);
    }
}
module.exports=buildBenchmarkCharts;
if(require.main===module)buildBenchmarkCharts();
