/** Shared, measured duplicate-search content for HTML and Markdown. */
const data = require('../docs/benchmarks/duplicates-0.2.5.json');
const copy = {
    de: {
        kicker: 'Duplikatsuche', title: 'Duplikatsuche im Vergleich',
        body: 'DiscReveal findet identische Dateiinhalte und behält beim Bereinigen ein Original. Hier vergleichen wir die Suchzeiten mit Czkawka.',
        comparison: 'Direkter Vergleich mit Czkawka',
        intro: 'In drei von vier gezielten Tests ist unser Suchkern schneller als Czkawka 12.0.2. Im besonders schwierigen vierten Fall liegt Czkawka vorn. Beide finden genau die erwarteten Duplikate.',
        scope: 'Suchkern / Kommandozeile · ohne grafische Oberfläche · warmer Windows-Dateicache',
        labels: ['Gleicher Anfang, anderer Inhalt', 'Unterschiede außerhalb der Stichproben', 'Viele identische Kopien', 'Kleine Dateien ausgeschlossen'],
        descriptions: ['512 Dateien à 512 KiB · 64 Duplikatgruppen', '60 Dateien à 8 MiB · 20 Duplikatgruppen', '12 identische Dateien à 16 MiB', '6.000 kleine Dateien + 16 große Einzeldateien'],
        methodTitle: 'Was wurde gemessen?',
        method: 'Intel Core i7-6700K, SATA-HDD, fünf Durchläufe je Programm nach einem Aufwärmlauf, Median. Gemessen wurde die Zeit vom Prozessstart bis zum fertigen Ergebnisexport. Beide App-Caches waren aus; der Windows-Dateicache war bereits gefüllt. DiscReveal wurde als isolierter Suchkern mit vollständigem Bytevergleich gemessen, Czkawka 12.0.2 als Kommandozeilenprogramm mit BLAKE3 und Standard-Threadzahl. Die Oberfläche war bei beiden nicht enthalten.',
        limits: 'Die Testdateien wurden bewusst für unterschiedliche Prüffälle erstellt. Das sind keine typischen persönlichen Dokumente. Die Werte hängen von Dateien, Laufwerk und Cache ab und versprechen keine allgemeine Beschleunigung. Bei kaltem Dateicache oder in der vollständigen App können andere Zeiten entstehen.',
        raw: 'Alle Messwerte und der genaue Testaufbau', lower: 'Kürzere Balken sind besser. Zeiten in Millisekunden.',
    },
    en: {
        kicker: 'Duplicate search', title: 'Duplicate search compared',
        body: 'DiscReveal finds identical file contents and retains an original during cleanup. Here we compare search times with Czkawka.',
        comparison: 'A direct comparison with Czkawka',
        intro: 'Our search core is faster than Czkawka 12.0.2 in three of four targeted tests. Czkawka leads in the particularly difficult fourth case. Both find exactly the expected duplicates.',
        scope: 'Search core / command line · no graphical interfaces · warm Windows file cache',
        labels: ['Same beginning, different content', 'Differences outside the samples', 'Many identical copies', 'Small files excluded'],
        descriptions: ['512 files of 512 KiB · 64 duplicate groups', '60 files of 8 MiB · 20 duplicate groups', '12 identical files of 16 MiB', '6,000 small files + 16 large unique files'],
        methodTitle: 'What did we measure?',
        method: 'Intel Core i7-6700K, SATA HDD, five runs per program after a warm-up, median. Time was measured from process start to completed result export. Both app caches were disabled; the Windows file cache was already warm. DiscReveal was measured as an isolated search core with full byte comparison; Czkawka 12.0.2 as its command-line program with BLAKE3 and the default thread count. Neither graphical interface was included.',
        limits: 'The test files were deliberately created for different checking scenarios. They are not a typical collection of personal documents. Results depend on files, drive and cache and do not promise a general speedup. Cold-cache runs or the complete app may take different amounts of time.',
        raw: 'All measurements and the full test setup', lower: 'Shorter bars are better. Times in milliseconds.',
    },
};
const escape = text => String(text).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('"','&quot;');
const number = (n,lang) => n.toLocaleString(lang==='de'?'de-DE':'en-US',{maximumFractionDigits:0});
function section(lang) {
    const c=copy[lang];
    const panels=data.workloads.map((work,i)=>{
        const max=Math.max(...Object.values(work.medians));
        const bars=[['discReveal','DiscReveal'],['czkawka','Czkawka 12.0.2']].map(([key,label])=>`<div class="dupe-bench-row"><span>${label}</span><div class="dupe-bench-track"><span class="dupe-bench-bar ${key==='discReveal'?'dupe-bench-app':''}" style="width:${(100*work.medians[key]/max).toFixed(2)}%"></span></div><strong>${number(work.medians[key],lang)} ms</strong></div>`).join('');
        return `<article class="dupe-bench-panel"><h4>${escape(c.labels[i])}</h4><p>${escape(c.descriptions[i])}</p>${bars}</article>`;
    }).join('');
    return `<section id="duplicate-benchmark" class="wrap"><div class="section-head"><p class="section-kicker">${escape(c.kicker)}</p><h2>${escape(c.title)}</h2><p>${escape(c.body)}</p></div><div class="dupe-bench-head"><h3>${escape(c.comparison)}</h3><p>${escape(c.intro)}</p><p class="dupe-bench-scope">${escape(c.scope)}</p></div><div class="dupe-bench-panels">${panels}</div><p class="dupe-bench-note">${escape(c.lower)}</p><details class="faq-item dupe-bench-method"><summary><span>${escape(c.methodTitle)}</span><span class="faq-plus" aria-hidden="true"><svg width="12" height="12" viewBox="0 0 24 24" fill="none"><path d="M12 5v14M5 12h14" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/></svg></span></summary><div class="faq-answer"><p>${escape(c.method)}</p><p>${escape(c.limits)}</p><p><a href="/benchmarks/duplicates-0.2.5.json">${escape(c.raw)}</a></p></div></details></section>`;
}
function markdown(lang,base='https://discreveal.com') {
    const c=copy[lang];
    return `### ${c.kicker}\n\n${c.body}\n\n${c.intro}\n\n**${c.scope}.**\n\n| ${lang==='de'?'Testfall':'Test case'} | DiscReveal | Czkawka 12.0.2 |\n|---|---:|---:|\n${data.workloads.map((work,i)=>`| ${c.labels[i]} | ${number(work.medians.discReveal,lang)} ms | ${number(work.medians.czkawka,lang)} ms |`).join('\n')}\n\n${c.method}\n\n${c.limits}\n\n[${c.raw}](${base}/benchmarks/duplicates-0.2.5.json).`;
}
module.exports={section,markdown,copy,data};
