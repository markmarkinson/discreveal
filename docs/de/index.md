# DiscReveal – Produktinformationen

> Finde große Ordner, doppelte Dateien und entbehrliche Zwischenspeicher auf deiner Festplatte oder SSD. DiscReveal zeigt schon beim Scannen, wo dein Speicherplatz bleibt – kostenlos und ohne Installation.

[Offizielle Website](https://discreveal.com/de/)

## Auf einen Blick

- Windows 10 oder 11, 64-Bit (x64).
- Kostenlos, Quellcode öffentlich einsehbar. DiscReveal Source-Available License 1.0. Autor: markMarkinson.
- Portable EXE ohne DiscReveal-Installation. Größe: 5,4 MB (5.361.664 Bytes).
- Microsoft Edge WebView2 muss vorhanden sein. Die App bietet bei Bedarf einen Installationslink.
- Deutsch und Englisch. Lokale Festplatten, SSDs und USB-Laufwerke mit Laufwerksbuchstaben.
- Keine unterstützten macOS- oder Linux-Downloads; Netzlaufwerke werden nicht zur Auswahl angeboten.

## Erste Schritte

1. [discreveal-portable.exe herunterladen](https://github.com/markmarkinson/discreveal/releases/latest/download/discreveal-portable.exe) und öffnen.
2. Ein Laufwerk auswählen und „Scan starten“ anklicken. Ordnergrößen erscheinen während des Scans.
3. Große Ordner durchsehen, Dateien suchen oder die Diagramme öffnen. Löschen ist nach Ende oder Abbruch des Scans möglich.
4. Für doppelte Dateien „Duplikate“ im oberen Menü wählen. Ein vorheriger Laufwerksscan ist nicht nötig.

## Funktionen

### Live-Ergebnisse

Sieh Ordnergrößen schon während des Scans und öffne die Ergebnisse direkt. Für einen genaueren Blick misst DiscReveal den belegten Dateispeicher und berücksichtigt Komprimierung und gemeinsam genutzte Dateien. Du wählst zwischen schnellem Überblick und genauerer Messung.

### Große Dateien schnell finden

Beginne mit den größten Ordnern in der Liste, stöbere in der Symbolansicht oder suche im gesamten gescannten Laufwerk nach einem Namen. Größenbalken und Prozentwerte zeigen, wo sich das Aufräumen am meisten lohnt.

### Aufräumen mit Kontrolle

Du wählst aus, was entfernt wird und wie. DiscReveal fragt vor dem Löschen nach und weist bei Windows-Inhalten und ausgewählten sensiblen Wurzel- und Profilpfaden zusätzlich auf das Risiko hin.

### Duplikate finden

Finde identische Dateien auch mit anderem Namen und sieh, wie viel belegten Speicher überzählige Kopien freigeben könnten. Filtere die laufenden Ergebnisse und wähle geeignete Kopien aus. Ein Original bleibt; vor dem Verschieben in den Papierkorb werden die Kopien erneut geprüft.

### Deinen Speicher besser verstehen

Das Diagrammsymbol neben dem Ordnerbaum zeigt die Belegung nach Dateityp, Größe und Alter sowie die größten einzelnen Dateien. So siehst du, wo sich ein genauerer Blick lohnt.

### Deutsch und Englisch

Nutze DiscReveal auf Deutsch oder Englisch. Menüs, Erklärungen und Dialoge wechseln sofort. Auch die Seite zur Versionsprüfung übernimmt deine Spracheinstellung.

### Windows aufräumen

Prüfe temporäre Dateien, Browser- und App-Zwischenspeicher, Windows-Reste und den Papierkorb. Sieh dir die Bereiche an, wähle ab, was bleiben soll, und bestätige die Bereinigung. Für Systembereiche können Administratorrechte nötig sein.

- **Papierkorb:** Wiederherstellbar, genau wie im Explorer
- **Endgültig löschen:** Überspringt den Papierkorb
- **Sicher löschen:** Überschreibt zugängliche Dateiinhalte zuerst mit Zufallsdaten; verweigert sich bei Hardlinks

## Datenschutz & Kontrolle

- **Nur im gescannten Bereich:** Dateiaktionen werden gegen den gescannten Ordner geprüft. Der Ausgangsordner selbst und Pfade außerhalb dieses Bereichs sind ausgeschlossen.
- **Zusätzlicher Schutz für sensible Ordner:** Für Windows-Inhalte sowie ausgewählte Systemwurzeln und Benutzerprofilpfade ist eine zusätzliche Bestätigung mit Risikoerklärung nötig. Dieser zusätzliche Schutz gilt nicht für jeden Unterordner.
- **Programme bleiben geschlossen:** Programme, Skripte und Verknüpfungen werden im Explorer angezeigt, statt ausgeführt zu werden.
- **Lokal scannen, ohne Konto:** Scans laufen auf deinem Computer, ohne Telemetrie und ohne Konto. Die App-Oberfläche blockiert externe Anfragen. Nur wenn du auf das Update-Symbol klickst, öffnet sich die Prüfung in deinem Browser.
- **Deine Daten bleiben bei dir:** Scanergebnisse bleiben im Arbeitsspeicher. Deine Einstellungen werden lokal gespeichert. Der Zwischenspeicher für schnellere Duplikatsuchen ist freiwillig, standardmäßig aus und jederzeit löschbar. Die Oberfläche nutzt außerdem ein lokales WebView2-Profil. Gescannte Dateien und Pfade werden nicht hochgeladen.
- **Quellcode einsehbar, Funktionen geprüft:** Der Quellcode ist öffentlich. Automatische Tests prüfen Scans, Dateiaktionen, die Duplikatbereinigung und die Versionsprüfung.

## Benchmarks

[Vergleichstests für Laufwerksscan und Duplikatsuche](https://discreveal.com/de/benchmarks/)

## Updates

Klicke in DiscReveal auf das Update-Symbol neben dem Appnamen. Die Prüfung öffnet sich in deinem Browser, passend zu deiner Spracheinstellung.

[So prüfst du deine Version](https://discreveal.com/de/ratgeber/discreveal-version-pruefen/)

SHA-256 des offiziellen Downloads:

`e51292adc84530e4326a7f63f13086f09b938bc58931cf0f72d1c6c24d8b2049`

## Häufig gestellte Fragen

### Wie schützt DiscReveal meine Dateien?

DiscReveal scannt lokal und entfernt keine Dateien ohne Bestätigung. Bei sensiblen Ordnern ist eine zusätzliche Zustimmung nötig. Lade die App von der offiziellen Website oder dem GitHub-Release herunter; jedes Release nennt einen Fingerabdruck zum Prüfen der Programmdatei.

### Räumt DiscReveal automatisch auf, oder entscheide ich, was gelöscht wird?

Du entscheidest, was entfernt wird. Der Laufwerksscan zeigt dir, wo dein Speicherplatz bleibt. In der Duplikatsuche markiert „Alles Geeignete markieren“ persönliche Dokumente und Medien, wobei pro Gruppe ein Original erhalten bleibt. Du prüfst die Auswahl und bestätigst die Bereinigung; Programm- und Systemdateien sind von diesen Sammelvorschlägen ausgeschlossen.

### Was kann ich wiederherstellen, wenn ich versehentlich etwas lösche?

Hängt vom Modus ab. Papierkorb-Löschungen sind wiederherstellbar, genau wie im Explorer. Endgültig und sicher löschen sind es nicht — sicher löschen überschreibt die zugänglichen Dateiinhalte zuerst. Bei SSDs können ältere Kopien in intern verwalteten Speicherblöcken verbleiben.

### Kann ich Ordner vom Scan ausschließen?

Ja — füge beliebige Ordner zur Ausschlussliste hinzu, oder verlass dich auf die Standardwerte. DiscReveal überspringt von Haus aus die üblichen Spiele-Bibliotheksordner (Steam, Epic, GOG, Riot, EA, Ubisoft, Xbox), die selten geprüft werden müssen.

### Kann ich den Laufwerksscan und die Duplikatsuche filtern?

Ja. Der Laufwerksscan bietet Ordnerausschlüsse und Größengrenzen. Die Duplikatsuche hat eigene Mindest- und Höchstgrößen sowie ein- oder ausgeschlossene Dateitypen. Treffer kannst du zusätzlich live nach Name oder Pfad, Größe, Dateiendung und gleichen oder unterschiedlichen Dateinamen filtern.

### Kann ich ein USB- oder externes Laufwerk scannen?

Ja, jedes lokale Laufwerk mit Laufwerksbuchstaben — auch USB. Der Scanner passt die Anzahl paralleler Threads an den Laufwerkstyp an (weniger auf USB und rotierenden Platten, mehr auf NVMe/SSD), damit er schnell bleibt, ohne das Laufwerk zu überlasten.

### Was ist der Unterschied zur Größenanzeige im Explorer?

Die Ordnereigenschaften im Explorer messen den ausgewählten Ordner. DiscReveal zeigt dir das Laufwerk im Überblick – mit Ordnergrößen, durchsuchbarem Ordnerbaum, Diagrammen und einer eigenen Duplikatsuche.

### Ist DiscReveal wirklich kostenlos?

Ja — kostenlos, ohne Werbung, Telemetrie oder Konto. Die offizielle App darfst du privat und intern im Unternehmen kostenlos nutzen. Der Quellcode ist öffentlich einsehbar. Änderungen und kommerzieller Weitervertrieb benötigen eine Erlaubnis; die Einzelheiten stehen in der Lizenz.

### Ist DiscReveal portabel?

Ja. Lade die EXE herunter und starte sie, ohne DiscReveal zu installieren. Das geht auch vom USB-Stick. Nimm discreveal_data mit, wenn du deine Einstellungen behalten möchtest. WebView2 muss vorhanden sein; DiscReveal richtet keinen Hintergrunddienst ein.

### Bleibt beim Bereinigen von Duplikaten ein Original erhalten?

Ja. In jeder Duplikatgruppe bleibt ein Original erhalten. Vor dem Verschieben in den Papierkorb werden die ausgewählten Kopien erneut mit diesem Original verglichen. Geänderte oder fehlende Dateien und Hardlinks werden übersprungen. Die mögliche Ersparnis basiert auf gemessenem belegtem Speicher; fehlende Messungen werden kenntlich gemacht und nicht addiert. Speicherplatz wird nach dem Leeren des Papierkorbs frei.

### Warum überspringt die Duplikatsuche Dateien unter 10 MB?

Die Vorgabe konzentriert sich auf größere Dateien, bei denen eine entfernte Kopie meist mehr Platz freigibt. Schalte „Kleine Dateien ausschließen“ ab, um alle Größen einzubeziehen, oder wähle eine eigene Grenze. Ordner müssen weiterhin durchsucht werden; die Zeitersparnis hängt von deinen Dateien ab.

### Speichert oder überträgt DiscReveal meine Scanergebnisse?

Scanergebnisse bleiben im Arbeitsspeicher und werden nicht hochgeladen. Einstellungen werden lokal gespeichert. Der Zwischenspeicher für Duplikatsuchen ist standardmäßig aus, braucht deine Zustimmung und lässt sich leeren. WebView2 speichert ein lokales Oberflächenprofil. Der Update-Link enthält nur App-Version, Sprache und, falls verfügbar, den Fingerabdruck der Programmdatei.

### Was kann die Systembereinigung entfernen?

Wähle Bereinigen → Alles prüfen. DiscReveal prüft unterstützte temporäre Dateien, Browser-, App-, Paket- und Grafik-Caches, Fehlerberichte, Windows-Systemreste und den Papierkorb. Die Ergebnisse sind in groben Bereichen vorausgewählt. Wähle ab, was du behalten möchtest, und bestätige eine gemeinsame Bereinigung. Laufende Apps sowie geschützte und zu junge Dateien werden ausgelassen. Windows bereinigt seine Systemreste selbst; geschützte Bereiche benötigen Administratorrechte. Frühere Windows-Versionen und Papierkorb sind eigene abwählbare Bereiche. Ihre Entfernung ist endgültig und kann Wiederherstellungs- oder Rückkehrmöglichkeiten entfernen. Caches werden bei Bedarf neu aufgebaut. Vor deiner Bestätigung wird nichts gelöscht.

### Warum unterscheiden sich Dateigröße und belegter Speicher?

Die Dateigröße beschreibt, wie viele Daten eine Datei enthält. Der belegte Speicher kann etwa durch Komprimierung, reservierte Blöcke oder mehrere Namen derselben Datei abweichen. Für den schnellen Überblick zeigt DiscReveal Dateigrößen. Aktiviere unter Scan → Filter „Belegten Speicher messen“ für eine genauere Messung. Sie dauert länger und zählt gemeinsam genutzte Dateien nur einmal. Dateisystem-Metadaten, Snapshots und Backups sind nicht enthalten.

### Was bedeutet ein unvollständiger Scan?

Ein Teil der Daten konnte nicht vollständig geprüft werden, etwa weil du den Scan gestoppt hast, Zugriffsrechte fehlen oder eine Datei sich verändert hat. Die sichtbaren Ergebnisse helfen dir weiterhin, decken aber nicht alles ab. Beachte den Hinweis und scanne bei Bedarf erneut. Eine fehlende Messung wird kenntlich gemacht und nicht als null belegter Speicher ausgegeben.

## Offizielle Links

- [Windows-Download](https://github.com/markmarkinson/discreveal/releases/latest/download/discreveal-portable.exe)
- [Aktuelles Release](https://github.com/markmarkinson/discreveal/releases/latest)
- [Öffentlicher Quellcode](https://github.com/markmarkinson/discreveal)
- [DiscReveal Source-Available License 1.0](https://github.com/markmarkinson/discreveal/blob/main/LICENSE)
- [Drittanbieter & Lizenzen](https://discreveal.com/de/third-party-notices/)

## Wissen & Ratgeber

[Alle Artikel](https://discreveal.com/de/ratgeber/)

- [Was belegt meine Festplatte unter Windows?](https://discreveal.com/de/ratgeber/was-belegt-meine-festplatte/)
- [Doppelte Dateien entfernen – ein Original behalten](https://discreveal.com/de/ratgeber/doppelte-dateien-entfernen/)
- [Was ist Prefetch – und sollte man den Ordner löschen?](https://discreveal.com/de/ratgeber/was-ist-prefetch/)
- [Was ist AppData – und was darf man dort löschen?](https://discreveal.com/de/ratgeber/was-ist-appdata/)
- [Was ist ein Cache – und wann lohnt sich das Leeren?](https://discreveal.com/de/ratgeber/was-ist-ein-cache/)
- [Was ist ein Datei-Hashwert – und was sagt er über Sicherheit?](https://discreveal.com/de/ratgeber/datei-hashwert-pruefen/)
- [Wie finden wir doppelte Dateien – auch mit anderem Namen?](https://discreveal.com/de/ratgeber/wie-finden-wir-duplikate/)
- [Sicheres Löschen: Was funktioniert auf HDD und SSD?](https://discreveal.com/de/ratgeber/sicheres-loeschen-hdd-ssd/)
- [Dateien im Papierkorb: gelöscht oder noch da?](https://discreveal.com/de/ratgeber/papierkorb-dateien-wirklich-geloescht/)
- [DiscReveal: Deine Version mit einem Klick prüfen](https://discreveal.com/de/ratgeber/discreveal-version-pruefen/)
- [Größe vs. Größe auf Datenträger: den Unterschied verstehen](https://discreveal.com/de/ratgeber/groesse-und-groesse-auf-datentraeger/)
- [Windows bereinigen: Speicherplatz gezielt freigeben](https://discreveal.com/de/ratgeber/windows-bereinigen-speicherplatz-freigeben/)
