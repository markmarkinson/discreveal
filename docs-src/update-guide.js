const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');
const {discRevealSiteDictionaries: copy} = vm.runInNewContext(fs.readFileSync(path.join(__dirname,'i18n.js'),'utf8')+'\n;({discRevealSiteDictionaries});');
const article = {id:'update-check', category:'organization', tags:['discreveal'], kind:'guide', published:'2026-10-02', updated:'2026-10-04', related:['file-hash','disk-space','duplicate-files']};
for (const lang of ['de','en']) {
    const de=lang==='de', dict=copy[lang];
    article[lang]={
        slug:de?'de/ratgeber/discreveal-version-pruefen':'guides/check-discreveal-version',
        title:de?'DiscReveal: Deine Version mit einem Klick prüfen':'DiscReveal: Check your version with one click',
        description:de?'So prüfst du direkt aus DiscReveal, ob ein Update verfügbar ist und deine App zum offiziellen Download passt. Einfach erklärt, Schritt für Schritt.':'Check for a DiscReveal update and see whether your app matches the official download. A simple, step-by-step guide.',
        intro:dict.updateIntro,
        result:de?'Ein Klick auf das Update-Symbol genügt. Die Prüfung öffnet sich im Browser; du entscheidest anschließend, ob du eine neue Version herunterladen möchtest.':'Click the update icon to open the check in your browser. You decide whether to download a new version afterwards.',
        sections:[
            {title:de?'So öffnest du die Prüfung':'How to open the check',steps:[1,2,3].map(i=>[dict[`updateStep${i}Title`],dict[`updateStep${i}Body`]])},
            {title:de?'Was bedeutet das Ergebnis?':'What does the result mean?',bullets:de?[
                'Deine App ist aktuell: Ihr Stand entspricht dem aktuellen offiziellen Download.',
                'Eine neue Version ist verfügbar: Du kannst den aktuellen offiziellen Download öffnen.',
                'Die Programmdatei unterscheidet sich: Die Seite zeigt beide Fingerabdrücke untereinander und hebt abweichende Zeichen farbig hervor. Ein aktueller App-Stand allein bestätigt noch nicht, dass auch die Datei identisch ist.',
                'Der Fingerabdruck fehlt: Die Version kann verglichen werden, die Programmdatei selbst jedoch nicht.'
            ]:[
                'Your app is current: Its build matches the current official download.',
                'A new version is available: You can open the current official download.',
                'The program file differs: The page shows both fingerprints on separate lines and highlights differing characters. A current app build alone does not establish that the files are identical.',
                'The fingerprint is missing: The version can still be compared, but the program file itself cannot.'
            ],paragraphs:[dict.updateDifferenceBody]},
            {title:de?'Was wird dabei übertragen?':'What information is shared?',paragraphs:[dict.updateLinkBody,de?'Die Prüfung startet erst, wenn du das Symbol anklickst. Der Link öffnet discreveal.com in deinem Browser.':'The check starts only when you click the icon. The link opens discreveal.com in your browser.']},
            {title:de?'Wie aktualisiere ich DiscReveal?':'How do I update DiscReveal?',paragraphs:[dict.updateManualBody,de?'Lade die neue discreveal-portable.exe von der offiziellen Website herunter. Schließe die laufende App und ersetze die bisherige EXE durch den neuen Download. Für DiscReveal selbst ist keine Installation nötig.':'Download the new discreveal-portable.exe from the official website. Close the running app and replace your existing EXE with the new download. DiscReveal itself needs no installation.']}
        ]
    };
}
module.exports=[article];
