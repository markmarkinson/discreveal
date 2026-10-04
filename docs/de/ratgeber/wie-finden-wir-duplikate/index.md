# Wie finden wir doppelte Dateien – auch mit anderem Namen?

markMarkinson · 2026-10-02 · 3 Min. Lesezeit

„Urlaub.jpg“ und „Urlaub-Kopie.jpg“ können dasselbe Foto enthalten. Zwei Dateien mit exakt demselben Namen können dagegen völlig verschieden sein. Deshalb sucht DiscReveal nach identischem Inhalt.

Ein Treffer bedeutet: Die Dateiinhalte wurden vollständig verglichen. Ob eine Kopie an ihrem Ort trotzdem gebraucht wird, entscheidest du.

## Erst aussortieren, dann gründlich vergleichen

1. **Größe vergleichen:** Dateien mit unterschiedlicher Länge können nicht byteweise identisch sein. DiscReveal bildet zuerst Gruppen gleicher Größe; Einzeldateien ohne passenden Partner brauchen keine Inhaltsprüfung.
2. **Kleine Stellen prüfen:** Der Finder liest kleine Ausschnitte am Anfang, in der Mitte und am Ende. Unterschiedliche Kennungen sortieren Kandidaten aus. Übereinstimmende Ausschnitte allein ergeben noch keinen bestätigten Treffer.
3. **Den kompletten Inhalt bestätigen:** Übrig gebliebene Kandidaten werden vollständig byteweise verglichen. Bei vielen unterschiedlichen Inhalten mit gleichen Stichproben helfen zusätzliche vollständige Inhaltskennungen, unnötige Vergleiche zu vermeiden. Auch dann bleibt der Bytevergleich entscheidend.

## Warum nicht einfach den Namen benutzen?

Ein Dateiname beschreibt, wie eine Datei heißt, nicht was sie enthält. Das Umbenennen einer kopierten PDF ändert ihren Inhalt nicht. Umgekehrt kann eine neu exportierte PDF denselben Namen wie die vorige Version haben.

DiscReveal sucht exakte Kopien, keine nur ähnlich aussehenden Fotos. Ein Bild in anderer Auflösung oder ein neu komprimiertes Video kann visuell ähnlich sein und trotzdem andere Dateibytes haben. Solche Varianten werden nicht als identischer Inhalt zusammengeführt.

## Was macht die Suche schneller?

Dateien unter 10 MB sind standardmäßig ausgeschlossen. Das konzentriert die Suche auf größere Kopien. Für kleine Fotos oder Dokumente kannst du diesen Ausschluss bewusst deaktivieren. Zusätzliche Größen- und Dateitypfilter verringern die Kandidaten weiter.

Wie viele Dateien gleichzeitig gelesen werden, richtet sich nach dem Laufwerkstyp. Der optionale Cache merkt sich Vorprüfungen unveränderter Dateien. Große Gruppen können bereits gelesene Inhalte kurz im RAM wiederverwenden. Keine dieser Abkürzungen ersetzt die Bestätigung identischer Inhalte.

## Ein Original bleibt – aber der Pfad ist wichtig

In jeder Gruppe bleibt die älteste bekannte Kopie als Original erhalten. Vor dem Bereinigen wird die ausgewählte Kopie erneut mit diesem Original verglichen. Geänderte oder nicht mehr passende Dateien werden übersprungen.

Eine Sicherung ist absichtlich doppelt vorhanden. Auch ein Projekt kann einen bestimmten Pfad benötigen. Prüfe deshalb die Orte, bevor du auswählst. Mehrere Hardlink-Namen derselben physischen Datei zählen nicht als zusätzliche freisetzbare Kopien.

## Was bedeutet „potenziell freigebbar“?

Die Anzeige summiert den gemessenen belegten Speicher überzähliger Kopien. Pro Gruppe bleibt ein Original erhalten. Komprimierte Dateien und Sparse-Dateien werden entsprechend ihrer tatsächlichen Belegung berücksichtigt. Weitere Hardlinks derselben Datei tragen keinen freigebbaren Speicher bei; nicht verfügbare Messungen werden kenntlich gemacht und nicht zur Summe addiert.

Die Duplikatbereinigung verschiebt ausgewählte Kopien in den Papierkorb. Solange sie dort liegen, wird ihr Platz nicht automatisch frei. Prüfe die Auswahl lieber bewusst, als den größten angezeigten Betrag zum Löschziel zu machen.

## Weitere Links

- [Doppelte Dateien entfernen – ein Original behalten](https://discreveal.com/de/ratgeber/doppelte-dateien-entfernen/)
- [Was ist ein Datei-Hashwert – und was sagt er über Sicherheit?](https://discreveal.com/de/ratgeber/datei-hashwert-pruefen/)
- [Was ist ein Cache – und wann lohnt sich das Leeren?](https://discreveal.com/de/ratgeber/was-ist-ein-cache/)
- [Alle Artikel](https://discreveal.com/de/ratgeber/)
- [DiscReveal](https://discreveal.com/de/)
- [Windows-Download](https://github.com/markmarkinson/discreveal/releases/latest/download/discreveal-portable.exe)
