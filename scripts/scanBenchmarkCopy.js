/** All numerical scan claims come from one measured dataset, independently of release version. */
const data = require('../docs/benchmarks/scan-0.2.3.json');
function benchmarkCopy(lang) {
    const de = lang === 'de';
    const number = (value, digits = 2) => value.toLocaleString(de ? 'de-DE' : 'en-US', {minimumFractionDigits:digits, maximumFractionDigits:digits});
    const time = id => data.tools.find(tool => tool.id === id).medianMs;
    const files = number(Math.round(data.files / 1000) * 1000, 0);
    const gb = number(data.bytes / 1e9);
    const seconds = number(time('discreveal') / 1000);
    return {
        benchBody: de
            ? `Rund ${files} Dateien, ${gb} GB, fünf Methoden im selben Ordner. DiscReveal zeigt die fertigen Ergebnisse nach ${seconds} Sekunden — inklusive Scan, Datenübertragung und Darstellung der Oberfläche.`
            : `Around ${files} files, ${gb} GB, five methods on the same folder. DiscReveal displays the finished results in ${seconds} seconds — including scanning, data transfer and interface rendering.`,
        homeBenchmarkBody: de
            ? `Rund ${files} Dateien, ${gb} GB: DiscReveal zeigt die fertigen Scanergebnisse in ${seconds} Sekunden. So schneidet die App im selben Ordner gegenüber Windows-Befehlen und einem Skript ab.`
            : `Around ${files} files, ${gb} GB: DiscReveal displays the finished scan results in ${seconds} seconds. Here is how the app compares with Windows commands and a script on the same folder.`,
        bench1Num: seconds + ' s',
        bench2Num: number(time('robocopy') / time('discreveal'), 1) + '×',
        bench3Num: number(time('powershell') / time('discreveal'), 1) + '×',
        benchmarkAlt: (de ? 'Scanvergleich: ' : 'Scan comparison: ') + data.tools.map(tool => `${tool.id === 'discreveal' ? 'DiscReveal' : tool.name} ${number(tool.medianMs/1000)} s`).join(', '),
    };
}
module.exports = benchmarkCopy;
