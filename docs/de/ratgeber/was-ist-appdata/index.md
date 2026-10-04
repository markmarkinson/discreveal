# Was ist AppData – und was darf man dort löschen?

markMarkinson · 2026-10-02 · 2 Min. Lesezeit

Unter deinem Windows-Benutzerordner liegt AppData. Der Name klingt technisch, doch hier liegen oft Dinge, die du täglich nutzt: Einstellungen, lokale Daten und Zwischenspeicher deiner Programme.

AppData ist kein reiner Müllordner. Prüfe das betreffende Programm und seine Aufräumfunktionen, statt AppData komplett zu leeren.

## Wo liegt AppData?

Der übliche Ort ist C:\Users\DEIN-NAME\AppData. Windows unterscheidet Local, Roaming und LocalLow. Mit Windows-Taste + R und %LOCALAPPDATA% öffnest du Local; %APPDATA% führt normalerweise zu Roaming. Zum Nachsehen musst du diese Ordner nicht verändern.

Quellen: [Microsoft Learn: Windows known folders](https://learn.microsoft.com/en-us/windows/win32/shell/knownfolderid)

## Local, Roaming und LocalLow

Die tatsächlichen Inhalte bestimmt das Programm. Aus dem Ordnernamen kannst du deshalb nicht ableiten, ob ein bestimmtes Unterverzeichnis entbehrlich ist.

- Local: Daten des jeweiligen Benutzerkontos auf diesem PC, beispielsweise lokale Zwischenspeicher.
- Roaming: ein Bereich für benutzerbezogene Anwendungsdaten. Der Name allein bedeutet nicht, dass dein privater PC diese Daten in eine Cloud hochlädt.
- LocalLow: ein eigener Windows-Bereich für Anwendungen mit niedrigerer Integrität; er ist kein „weniger wichtiger“ Papierkorb.

Quellen: [Microsoft Learn: Windows known folders](https://learn.microsoft.com/en-us/windows/win32/shell/knownfolderid) · [Microsoft Press: Windows Internals – Integrity Levels (PDF)](https://download.microsoft.com/download/1/4/0/14045a9e-c978-47d1-954b-92b9fd877995/97807356648739_samplechapters.pdf)

## Warum wächst der Ordner?

Denk an zwei unterschiedliche Fälle: Ein Programm speichert Vorschauen, die es erneut erzeugen kann. Ein anderes hält hier ein Profil oder noch nicht anderweitig gesicherte Arbeitsdaten. Beide können groß sein; nur der erste Fall ist wahrscheinlich ein Cache. Eine Größenanzeige entscheidet diesen Unterschied nicht.

Notiere beim Prüfen den Programmnamen, den Unterordner und seine Größe. Kläre über die Programmhilfe, was darin gespeichert wird. Wenn du ein Programm nicht mehr nutzt, beginne mit seiner Deinstallation und prüfe verbliebene Daten anschließend gezielt.

## Ein sinnvoller Weg zum Aufräumen

1. **Große Unterordner finden:** DiscReveal zeigt dir, welcher Teil von AppData den Platz belegt. Ein geschützter oder unzugänglicher Bereich kann unvollständig erfasst sein.
2. **Im Programm nachsehen:** Suche nach Einstellungen wie Cache leeren, Speicher verwalten oder Offline-Inhalte entfernen. So kann das Programm seine Daten selbst konsistent verwalten.
3. **Wichtige Daten sichern:** Prüfe Profile und lokale Arbeitsdaten, bevor du etwas entfernst. Ein Cache ist keine Sicherung; umgekehrt ist nicht jede Anwendungsdatei ein Cache.

## Was legt DiscReveal dort ab?

Nur mit deinem Opt-in speichert der Duplikatfinder seinen Cache unter %LOCALAPPDATA%\DiscReveal\Cache. Er enthält Dateipfade, Größen, Änderungszeiten und kurze Inhaltskennungen, keine Kopien deiner Dateien. Du kannst ihn in der App leeren. Die Oberfläche nutzt außerdem ein lokales WebView2-Profil; die portablen Einstellungen liegen in discreveal_data neben der EXE.

## Weitere Links

- [Was ist ein Cache – und wann lohnt sich das Leeren?](https://discreveal.com/de/ratgeber/was-ist-ein-cache/)
- [Was belegt meine Festplatte unter Windows?](https://discreveal.com/de/ratgeber/was-belegt-meine-festplatte/)
- [Was ist Prefetch – und sollte man den Ordner löschen?](https://discreveal.com/de/ratgeber/was-ist-prefetch/)
- [Alle Artikel](https://discreveal.com/de/ratgeber/)
- [DiscReveal](https://discreveal.com/de/)
- [Windows-Download](https://github.com/markmarkinson/discreveal/releases/latest/download/discreveal-portable.exe)
