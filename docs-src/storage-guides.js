/** Current product guides, written around user questions rather than release changes. */
const refs = {
    links: ['Microsoft Learn: Hard links', 'https://learn.microsoft.com/en-us/windows/win32/fileio/hard-links-and-junctions'],
    compression: ['Microsoft Learn: File compression', 'https://learn.microsoft.com/en-us/windows/win32/fileio/file-compression-and-decompression'],
    sparse: ['Microsoft Learn: Sparse files', 'https://learn.microsoft.com/en-us/windows/win32/fileio/sparse-files'],
    cleanup: ['Microsoft Support: Free up drive space', 'https://support.microsoft.com/en-us/windows/experience/storage-filemanagement/free-up-drive-space-in-windows'],
};
module.exports = [
    {
        id: 'occupied-space', category: 'organization', tags: ['discreveal'], kind: 'knowledge', published: '2026-10-04',
        related: ['disk-space', 'how-duplicates', 'recycle-bin'],
        de: {
            slug: 'de/ratgeber/groesse-und-groesse-auf-datentraeger',
            title: 'Größe vs. Größe auf Datenträger: den Unterschied verstehen',
            description: 'Dateigröße und belegter Speicher einfach erklärt: Komprimierung, Sparse-Dateien und Hardlinks. So wählst du die passende Messung in DiscReveal.',
            intro: 'Eine Datei ist 100 MB groß, belegt auf dem Laufwerk aber weniger – oder mehr. Das ist nicht automatisch ein Fehler. Dateigröße und belegter Speicher beantworten unterschiedliche Fragen. Wenn du Platz freigeben möchtest, lohnt es sich, den Unterschied zu kennen.',
            result: 'Für den schnellen Überblick reichen Dateigrößen. Für eine genauere Einschätzung des belegten Dateispeichers kannst du in DiscReveal die zusätzliche Messung einschalten.',
            sections: [
                {title: 'Zwei Größen, zwei Fragen', paragraphs: ['Die Dateigröße sagt dir, wie viele Daten eine Anwendung beim Lesen der Datei erhält. Der belegte Speicher beschreibt den dafür zugeordneten Platz auf dem Laufwerk. Dieser kann durch Speicherblöcke und Dateisystemfunktionen abweichen.', 'Viele kleine Dateien können mehr Platz belegen, als ihre Dateigrößen zusammen ergeben. Umgekehrt können komprimierte Dateien weniger benötigen. Ein Scan mit Dateigrößen ist deshalb ein guter Weg zu großen Dateien, aber keine vollständige Abrechnung des Laufwerks.']},
                {title: 'Komprimierung und Sparse-Dateien', paragraphs: ['NTFS kann Dateiinhalt komprimiert speichern, während Programme weiterhin die unkomprimierten Daten lesen. Eine Datei mit 100 MB Inhalt kann dadurch weniger Platz belegen. Wie viel, hängt vom Inhalt ab.', 'Sparse-Dateien lassen bestimmte leere Datenbereiche ohne zugeordnete Speicherblöcke aus. Die Datei kann groß erscheinen und trotzdem deutlich weniger Platz benötigen. Das begegnet dir etwa bei virtuellen Laufwerken oder spezialisierten Anwendungen.'], sources: [refs.compression, refs.sparse]},
                {title: 'Hardlinks: mehrere Namen, dieselbe Datei', paragraphs: ['Ein Hardlink ist ein weiterer Name für dieselbe Datei auf demselben Laufwerk. Anders als zwei unabhängige Kopien teilen diese Namen die Daten. Addierst du jede angezeigte Dateigröße, zählst du denselben Inhalt mehrfach.', 'Das Entfernen eines Namens gibt diese Daten nicht frei, solange weitere Hardlinks bestehen. Deshalb ist eine scheinbar große Gruppe nicht automatisch ein großes Aufräumziel. DiscReveal zählt gemeinsam genutzten Dateispeicher in der zusätzlichen Messung einmal.'], sources: [refs.links]},
                {title: 'Die passende Messung in DiscReveal wählen', steps: [
                    ['Schnell die größten Dateien finden', 'Wähle Scan, dein Laufwerk und „Scan starten“. Die Standardansicht zeigt Dateigrößen und aktualisiert sich während der Suche.'],
                    ['Belegten Speicher genauer ansehen', 'Öffne unter Scan die Filter und aktiviere „Belegten Speicher messen“. Starte anschließend den Scan. Die Messung berücksichtigt Komprimierung, Sparse-Dateien, zusätzliche Datenströme und Hardlinks; sie dauert länger.'],
                    ['Unvollständige Werte beachten', 'Nicht verfügbare Messungen werden kenntlich gemacht. Der Hinweis an einer Größe zeigt zusätzlich die Dateigröße. Nach dem Löschen scannst du erneut, damit die Zuordnung des belegten Speichers aktualisiert wird.'],
                ], paragraphs: ['Die Messung umfasst belegte Dateidaten. Dateisystem-Metadaten, Snapshots und Backups sind nicht Teil dieser Summe. Auch während des Scans veränderte Dateien und fehlende Zugriffsrechte können Unterschiede zur Laufwerksanzeige verursachen.']},
                {title: 'Was heißt das für die Duplikatsuche?', paragraphs: ['DiscReveal prüft den vollständigen Inhalt, bevor es eine Kopie als Duplikat bestätigt. Die angezeigte mögliche Ersparnis basiert auf gemessenem belegtem Speicher – nicht einfach auf Dateigröße mal Trefferzahl.', 'Ein Original bleibt pro Gruppe erhalten. Kopien mit weiteren Hardlinks tragen keinen freigebbaren Speicher zur Summe bei; fehlende Messungen werden ausgelassen und kenntlich gemacht. Beim Verschieben in den Papierkorb bleiben die Daten zunächst auf dem Laufwerk. Platz wird nach dessen Leeren frei.']},
            ],
        },
        en: {
            slug: 'guides/size-vs-size-on-disk',
            title: 'Size vs size on disk: Understand the difference',
            description: 'Understand file size and occupied space: compression, sparse files and hard links. Choose the right storage measurement in DiscReveal.',
            intro: 'A file can contain 100 MB of data yet occupy less space on your drive—or more. That does not necessarily mean anything is wrong. File size and occupied space answer different questions. Knowing the difference helps when you want to free up storage.',
            result: 'File sizes are useful for a quick overview. For a closer look at occupied file space, enable the additional measurement in DiscReveal.',
            sections: [
                {title: 'Two sizes, two questions', paragraphs: ['File size describes how much data an application receives when it reads a file. Occupied space is the drive space allocated to store it. Storage blocks and filesystem features can make these values differ.', 'Many small files can occupy more space than their combined file sizes suggest. Compressed files can require less. A file-size scan is therefore a useful way to find large files, but it is not a full accounting of your drive.']},
                {title: 'Compression and sparse files', paragraphs: ['NTFS can store file contents in compressed form while applications read the uncompressed data. A file containing 100 MB may therefore occupy less drive space. The amount depends on its contents.', 'Sparse files leave certain empty data regions without allocated storage blocks. A file can appear large while requiring much less space. Virtual drives and specialist applications are examples of where you may encounter them.'], sources: [refs.compression, refs.sparse]},
                {title: 'Hard links: different names, one file', paragraphs: ['A hard link gives the same file another name on the same drive. Unlike two independent copies, these names share the data. Adding every displayed file size counts the same contents more than once.', 'Removing one name does not release that data while further hard links remain. A large-looking group is therefore not automatically a large cleanup opportunity. DiscReveal counts shared file space once in its additional measurement.'], sources: [refs.links]},
                {title: 'Choose the right measurement in DiscReveal', steps: [
                    ['Find large files quickly', 'Choose Scan, select your drive and click “Start scan”. The default view shows file sizes and updates while scanning.'],
                    ['Take a closer look at occupied space', 'Open Filter under Scan and enable “Measure occupied space”. Then start the scan. It accounts for compression, sparse files, additional data streams and hard links, and takes longer.'],
                    ['Check incomplete values', 'Unavailable measurements are identified. The size tooltip also shows the file size. After deleting files, scan again to update occupied-space accounting.'],
                ], paragraphs: ['The measurement covers allocated file data. Filesystem metadata, snapshots and backups are outside this total. Changing files and missing access permissions can also cause differences from the drive’s storage display.']},
                {title: 'What does this mean for duplicate search?', paragraphs: ['DiscReveal compares the full contents before confirming a duplicate. Potential savings use measured occupied space, rather than simply multiplying file size by the number of matches.', 'One original stays in each group. Copies with further hard links contribute no reclaimable space; unavailable measurements are identified and excluded. Moving files to the Recycle Bin keeps their data on the drive until you empty it.']},
            ],
        },
    },
    {
        id: 'windows-cleanup', category: 'organization', tags: ['discreveal'], kind: 'knowledge', published: '2026-10-04', image: 'cleanup',
        related: ['disk-space', 'cache', 'recycle-bin'],
        de: {
            slug: 'de/ratgeber/windows-bereinigen-speicherplatz-freigeben',
            title: 'Windows bereinigen: Speicherplatz gezielt freigeben',
            description: 'Temporäre Dateien, Browser-Caches und Windows-Reste gezielt aufräumen. So prüfst du die Bereiche in DiscReveal und entscheidest, was bleiben soll.',
            intro: 'Downloads, Zwischenspeicher und Installationsreste wachsen oft unbemerkt. Zum Aufräumen musst du nicht jeden versteckten Ordner kennen. DiscReveal prüft unterstützte Bereiche und fasst die Ergebnisse verständlich zusammen. Du entscheidest anschließend, was entfernt wird.',
            result: 'Mit einer gemeinsamen Prüfung findest du Aufräumziele, wählst ganze Bereiche ab und bestätigst erst dann die Bereinigung.',
            alt: 'DiscReveal-Bereinigung mit groben Bereichen und einer Speicherübersicht', caption: 'Die Bereinigung fasst Ergebnisse nach Bereichen zusammen. Die Aufnahme zeigt Beispieldaten.',
            sections: [
                {title: 'Erst erkennen, wo der Platz bleibt', paragraphs: ['Ist das Laufwerk voll, beginne mit dem Scan. Große Videos, Archive oder alte Downloads können mehr Platz beanspruchen als ein kleiner Cache. Die Bereinigung ergänzt diesen Überblick: Sie prüft bekannte Aufräumbereiche, statt aus einer großen Datei automatisch eine Löschentscheidung zu machen.']},
                {title: 'In DiscReveal aufräumen', steps: [
                    ['Bereiche prüfen', 'Wähle „Bereinigen“ im oberen Menü und klicke auf „Alles prüfen“. Die Fortschrittsanzeige hält dich über die Prüfung auf dem Laufenden.'],
                    ['Die Auswahl ansehen', 'Prüfe die gefundenen Größen. Die Ergebnisse sind in groben Bereichen vorausgewählt. Öffne einen Bereich, wenn du genauer wissen möchtest, was enthalten ist, oder wähle ihn vollständig ab.'],
                    ['Entfernen bewusst bestätigen', 'Starte die Bereinigung erst, wenn die Auswahl für dich passt. DiscReveal fragt nach deiner Bestätigung. Geöffnete Apps, geschützte Dateien und zu junge Dateien werden ausgelassen.'],
                ]},
                {title: 'Welche Bereiche werden geprüft?', bullets: ['Temporäre Dateien: ältere Arbeitsdateien aus unterstützten Temp-Bereichen.', 'Browser: unterstützte Zwischenspeicher, ohne bewusst Anmeldungen und Verlauf zu entfernen.', 'App-Zwischenspeicher: unterstützte App- und Paket-Caches; Einstellungen und Projekte bleiben.', 'Spiel- und Grafik-Caches: wiederherstellbare Shader- und Grafikdaten.', 'Alte Fehlerberichte: unterstützte Diagnose- und Protokollreste.', 'Windows aufräumen: von Windows geprüfte System- und Installationsreste sowie Vorschaubilder.', 'Papierkorb: bereits gelöschte Dateien endgültig entfernen.'], paragraphs: ['Was verfügbar ist, hängt von den installierten Programmen, Zugriffsrechten und deinem System ab. Nicht jeder Cache auf dem Rechner gehört zum unterstützten Katalog. Ein leerer Bereich bedeutet deshalb nicht, dass die App alle denkbaren Programmdaten überprüft hat.']},
                {title: 'Bei Windows-Resten und Papierkorb genauer hinsehen', paragraphs: ['Geschützte Windows-Bereiche können Administratorrechte benötigen. DiscReveal lässt Windows seine Systemreste über die vorgesehenen Bereinigungsfunktionen prüfen und entfernen.', 'Eine frühere Windows-Installation kann die Rückkehr zur vorherigen Version ermöglichen. Entfernst du diese Daten, entfällt diese Möglichkeit. Beim Leeren des Papierkorbs entfällt das einfache Wiederherstellen. Beide Bereiche kannst du abwählen, wenn du sie behalten möchtest.'], sources: [refs.cleanup]},
                {title: 'Weniger Ballast heißt nicht automatisch ein schnellerer PC', paragraphs: ['Bereinigen schafft Platz für deine Dateien und Windows-Arbeit. Es ist kein pauschales Versprechen für schnellere Programmstarts oder mehr Bilder pro Sekunde.', 'Caches können nach der Bereinigung wieder wachsen, weil Programme sie bei Bedarf neu erstellen. Räume deshalb nach Bedarf auf und schau bei ständig vollem Speicher nach der Ursache. Den gesamten AppData-Ordner oder unbekannte Windows-Dateien solltest du nicht wahllos löschen.']},
            ],
        },
        en: {
            slug: 'guides/windows-cleanup-free-disk-space',
            title: 'Windows cleanup: Free up disk space with control',
            description: 'Review temporary files, browser caches and Windows leftovers. Use DiscReveal to check cleanup areas and decide what you want to keep.',
            intro: 'Downloads, caches and installation leftovers can grow unnoticed. You do not have to understand every hidden folder to clean up. DiscReveal checks supported areas and groups the results clearly. You then decide what to remove.',
            result: 'One check finds cleanup opportunities, lets you deselect whole areas and asks for confirmation before removing anything.',
            alt: 'DiscReveal cleanup with broad areas and a storage summary', caption: 'Cleanup groups results by area. The screenshot uses example data.',
            sections: [
                {title: 'First find where the space goes', paragraphs: ['If your drive is full, start with Scan. Large videos, archives or old downloads can take up more space than a small cache. Cleanup complements that overview: it checks known cleanup areas instead of treating every large file as something to delete.']},
                {title: 'Clean up with DiscReveal', steps: [
                    ['Check the areas', 'Choose “Cleanup” in the top menu and click “Check everything”. The progress display keeps you informed while it checks.'],
                    ['Review the selection', 'Check the sizes found. Results are preselected in broad areas. Open an area to see what it includes, or deselect the whole area.'],
                    ['Confirm what to remove', 'Start cleanup only when you are happy with the selection. DiscReveal asks for confirmation. Running apps, protected files and files that are too recent are skipped.'],
                ]},
                {title: 'Which areas are checked?', bullets: ['Temporary files: older working files in supported temporary locations.', 'Browsers: supported caches, without deliberately removing logins and browsing history.', 'App caches: supported application and package caches; settings and projects stay.', 'Game and graphics caches: shader and graphics data that can be rebuilt.', 'Old error reports: supported diagnostic and log leftovers.', 'Windows cleanup: system and installation leftovers checked by Windows, plus thumbnails.', 'Recycle Bin: permanently remove files already placed in the bin.'], paragraphs: ['Availability depends on your installed applications, access permissions and system. The supported catalog does not include every cache on your computer. An empty area therefore does not mean every possible application file was examined.']},
                {title: 'Take a closer look at Windows leftovers and the Recycle Bin', paragraphs: ['Protected Windows areas can require administrator rights. DiscReveal uses Windows’ intended cleanup functions to check and remove system leftovers.', 'A previous Windows installation may let you return to the earlier version. Removing those files removes that option. Emptying the Recycle Bin removes its straightforward Restore option. You can deselect either area if you want to keep it.'], sources: [refs.cleanup]},
                {title: 'Less clutter does not automatically mean a faster PC', paragraphs: ['Cleanup makes room for your files and for Windows to work. It is not a blanket promise of faster application launches or higher game frame rates.', 'Caches can grow again as applications rebuild them. Clean up when needed, and investigate the cause if storage repeatedly fills up. Avoid deleting all of AppData or unfamiliar Windows files at random.']},
            ],
        },
    },
];
