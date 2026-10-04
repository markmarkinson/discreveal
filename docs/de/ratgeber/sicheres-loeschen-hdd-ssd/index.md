# Sicheres Löschen: Was funktioniert auf HDD und SSD?

markMarkinson · 2026-10-02 · 3 Min. Lesezeit

Eine vertrauliche Datei soll nicht mehr auffindbar sein. Dafür reicht es nicht, dass ihr Name aus einem Ordner verschwindet. Entscheidend sind das Speichermedium, weitere Kopien und das Ziel deiner Bereinigung.

Das Überschreiben einer Datei ist keine garantierte Bereinigung eines ganzen Geräts. Besonders auf SSDs lässt sich daraus keine vollständige Unwiederherstellbarkeit versprechen.

## Entfernen und Überschreiben sind verschiedene Schritte

Beim normalen Löschen wird nicht grundsätzlich der gesamte alte Inhalt überschrieben. Programme für sicheres Löschen versuchen zusätzlich, die erreichbaren Dateidaten durch andere Daten zu ersetzen. Das kann die Wiederherstellung dieser Daten erschweren; es entfernt nicht automatisch Kopien an anderen Orten.

Quellen: [Microsoft Learn: SDelete](https://learn.microsoft.com/en-us/sysinternals/downloads/sdelete)

## Was bedeutet das auf einer HDD?

Eine HDD speichert Daten magnetisch auf rotierenden Scheiben. Das Überschreiben erreichbarer Speicherbereiche ist hier ein mögliches Bereinigungsverfahren. Das Überschreiben einer einzelnen Datei erfasst jedoch nicht automatisch frühere Kopien, Sicherungen oder Bereiche außerhalb dieser Datei. Für die Weitergabe eines Laufwerks solltest du deshalb den gesamten Datenträger betrachten.

Quellen: [NIST SP 800-88 Rev. 2: Media Sanitization](https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-88r2.pdf)

## Warum SSDs anders behandelt werden müssen

Flash-Speicher verteilt Schreibvorgänge intern und kann zusätzliche, nicht direkt adressierbare Bereiche haben. Ein Überschreiben über den normalen Dateipfad erreicht deshalb nicht zuverlässig sämtliche früheren physischen Daten. Eine einzelne überschreibende App kann das nicht zu einer garantierten vollständigen Löschung machen.

Quellen: [NIST SP 800-88 Rev. 2: Media Sanitization](https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-88r2.pdf)

## Was DiscReveal konkret macht

Der Modus „Sicher löschen“ überschreibt die zugänglichen Inhalte ausgewählter Dateien mit Zufallsdaten und entfernt sie anschließend. Er ist getrennt vom Papierkorbmodus und nicht die Standardaktion der Duplikatbereinigung.

Hardlinks werden beim sicheren Überschreiben verweigert, weil mehrere Namen auf dieselben Daten verweisen können. Reparse-Verknüpfungen, komprimierte, verschlüsselte und Sparse-NTFS-Dateien sowie Dateien mit zusätzlichen Datenströmen sind ebenfalls ausgeschlossen. Bei diesen NTFS-Eigenschaften kann normales Überschreiben alte physische Daten auch auf HDDs zurücklassen. Der Vorgang beseitigt keine früheren Backups, Cloud-Versionen oder jede mögliche Metadaten- und Dateisystemspur. Er ist kein Verfahren zur zertifizierten Datenträgervernichtung.

## Wenn du einen PC weitergibst

Sichere zunächst die Daten, die du behalten möchtest, und prüfe die Sicherung. Für die Abgabe eines Geräts ist eine Bereinigung des ganzen Datenträgers das passendere Ziel als das einzelne Löschen sichtbarer Dokumente.

Microsoft bietet beim Zurücksetzen mit „Alles entfernen“ eine Option zur Datenbereinigung an. Sie erschwert die Wiederherstellung, richtet sich aber an Privatnutzer und erfüllt laut Microsoft keine staatlichen oder industriellen Löschstandards. Für besonders sensible Geräte brauchst du ein zum Medium passendes, überprüfbares Verfahren.

Quellen: [Microsoft Support: Reset your PC](https://support.microsoft.com/en-us/windows/experience/backup-recovery/reset-your-pc)

## Welche Frage solltest du zuerst beantworten?

„Sicher“ beschreibt immer ein Ziel und seine Grenzen. Ein größerer Aufwand allein ist kein Beweis, dass jede frühere Kopie verschwunden ist.

- Möchtest du Platz gewinnen? Dafür ist dauerhaftes Überschreiben in vielen Alltagssituationen nicht nötig.
- Möchtest du ein versehentliches Entfernen rückgängig machen können? Dann ist der Papierkorb die passendere erste Wahl.
- Gibst du das ganze Gerät ab? Betrachte sämtliche Speicherorte und nutze ein Verfahren für den kompletten Datenträger.

## Weitere Links

- [Dateien im Papierkorb: gelöscht oder noch da?](https://discreveal.com/de/ratgeber/papierkorb-dateien-wirklich-geloescht/)
- [Doppelte Dateien entfernen – ein Original behalten](https://discreveal.com/de/ratgeber/doppelte-dateien-entfernen/)
- [Was ist ein Datei-Hashwert – und was sagt er über Sicherheit?](https://discreveal.com/de/ratgeber/datei-hashwert-pruefen/)
- [Alle Artikel](https://discreveal.com/de/ratgeber/)
- [DiscReveal](https://discreveal.com/de/)
- [Windows-Download](https://github.com/markmarkinson/discreveal/releases/latest/download/discreveal-portable.exe)
