# Größe vs. Größe auf Datenträger: den Unterschied verstehen

markMarkinson · 2026-10-04 · 3 Min. Lesezeit

Eine Datei ist 100 MB groß, belegt auf dem Laufwerk aber weniger – oder mehr. Das ist nicht automatisch ein Fehler. Dateigröße und belegter Speicher beantworten unterschiedliche Fragen. Wenn du Platz freigeben möchtest, lohnt es sich, den Unterschied zu kennen.

Für den schnellen Überblick reichen Dateigrößen. Für eine genauere Einschätzung des belegten Dateispeichers kannst du in DiscReveal die zusätzliche Messung einschalten.

## Zwei Größen, zwei Fragen

Die Dateigröße sagt dir, wie viele Daten eine Anwendung beim Lesen der Datei erhält. Der belegte Speicher beschreibt den dafür zugeordneten Platz auf dem Laufwerk. Dieser kann durch Speicherblöcke und Dateisystemfunktionen abweichen.

Viele kleine Dateien können mehr Platz belegen, als ihre Dateigrößen zusammen ergeben. Umgekehrt können komprimierte Dateien weniger benötigen. Ein Scan mit Dateigrößen ist deshalb ein guter Weg zu großen Dateien, aber keine vollständige Abrechnung des Laufwerks.

## Komprimierung und Sparse-Dateien

NTFS kann Dateiinhalt komprimiert speichern, während Programme weiterhin die unkomprimierten Daten lesen. Eine Datei mit 100 MB Inhalt kann dadurch weniger Platz belegen. Wie viel, hängt vom Inhalt ab.

Sparse-Dateien lassen bestimmte leere Datenbereiche ohne zugeordnete Speicherblöcke aus. Die Datei kann groß erscheinen und trotzdem deutlich weniger Platz benötigen. Das begegnet dir etwa bei virtuellen Laufwerken oder spezialisierten Anwendungen.

Quellen: [Microsoft Learn: File compression](https://learn.microsoft.com/en-us/windows/win32/fileio/file-compression-and-decompression) · [Microsoft Learn: Sparse files](https://learn.microsoft.com/en-us/windows/win32/fileio/sparse-files)

## Hardlinks: mehrere Namen, dieselbe Datei

Ein Hardlink ist ein weiterer Name für dieselbe Datei auf demselben Laufwerk. Anders als zwei unabhängige Kopien teilen diese Namen die Daten. Addierst du jede angezeigte Dateigröße, zählst du denselben Inhalt mehrfach.

Das Entfernen eines Namens gibt diese Daten nicht frei, solange weitere Hardlinks bestehen. Deshalb ist eine scheinbar große Gruppe nicht automatisch ein großes Aufräumziel. DiscReveal zählt gemeinsam genutzten Dateispeicher in der zusätzlichen Messung einmal.

Quellen: [Microsoft Learn: Hard links](https://learn.microsoft.com/en-us/windows/win32/fileio/hard-links-and-junctions)

## Die passende Messung in DiscReveal wählen

Die Messung umfasst belegte Dateidaten. Dateisystem-Metadaten, Snapshots und Backups sind nicht Teil dieser Summe. Auch während des Scans veränderte Dateien und fehlende Zugriffsrechte können Unterschiede zur Laufwerksanzeige verursachen.

1. **Schnell die größten Dateien finden:** Wähle Scan, dein Laufwerk und „Scan starten“. Die Standardansicht zeigt Dateigrößen und aktualisiert sich während der Suche.
2. **Belegten Speicher genauer ansehen:** Öffne unter Scan die Filter und aktiviere „Belegten Speicher messen“. Starte anschließend den Scan. Die Messung berücksichtigt Komprimierung, Sparse-Dateien, zusätzliche Datenströme und Hardlinks; sie dauert länger.
3. **Unvollständige Werte beachten:** Nicht verfügbare Messungen werden kenntlich gemacht. Der Hinweis an einer Größe zeigt zusätzlich die Dateigröße. Nach dem Löschen scannst du erneut, damit die Zuordnung des belegten Speichers aktualisiert wird.

## Was heißt das für die Duplikatsuche?

DiscReveal prüft den vollständigen Inhalt, bevor es eine Kopie als Duplikat bestätigt. Die angezeigte mögliche Ersparnis basiert auf gemessenem belegtem Speicher – nicht einfach auf Dateigröße mal Trefferzahl.

Ein Original bleibt pro Gruppe erhalten. Kopien mit weiteren Hardlinks tragen keinen freigebbaren Speicher zur Summe bei; fehlende Messungen werden ausgelassen und kenntlich gemacht. Beim Verschieben in den Papierkorb bleiben die Daten zunächst auf dem Laufwerk. Platz wird nach dessen Leeren frei.

## Weitere Links

- [Was belegt meine Festplatte unter Windows?](https://discreveal.com/de/ratgeber/was-belegt-meine-festplatte/)
- [Wie finden wir doppelte Dateien – auch mit anderem Namen?](https://discreveal.com/de/ratgeber/wie-finden-wir-duplikate/)
- [Dateien im Papierkorb: gelöscht oder noch da?](https://discreveal.com/de/ratgeber/papierkorb-dateien-wirklich-geloescht/)
- [Alle Artikel](https://discreveal.com/de/ratgeber/)
- [DiscReveal](https://discreveal.com/de/)
- [Windows-Download](https://github.com/markmarkinson/discreveal/releases/latest/download/discreveal-portable.exe)
