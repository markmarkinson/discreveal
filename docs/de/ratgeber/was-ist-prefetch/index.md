# Was ist Prefetch – und sollte man den Ordner löschen?

markMarkinson · 2026-10-02 · 2 Min. Lesezeit

Im Windows-Ordner taucht „Prefetch“ auf, gefüllt mit Dateien, deren Namen nach Programmen aussehen. Das wirkt wie ein weiterer Ordner, den man einfach leeren könnte. Seine Aufgabe ist aber, wiederkehrende Starts vorzubereiten.

Prefetch ist eine Starthilfe von Windows. Behandle den Ordner nicht wie deine Downloads und lösche ihn nicht routinemäßig, um den PC zu beschleunigen.

## Was steckt hinter Prefetch?

Windows berücksichtigt frühere Lesezugriffe, um Daten für Starts vorausschauend bereitzustellen. Prefetch-Dateien liegen üblicherweise unter C:\Windows\Prefetch. Sie enthalten Informationen über Programmstarts und beteiligte Dateien; sie sind keine zweite Installation des jeweiligen Programms.

Quellen: [Microsoft Learn: Improving System Startup Performance](https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/improving-system-startup-performance) · [Microsoft Incident Response: Prefetch (PDF)](https://cdn-dynmedia-1.microsoft.com/is/content/microsoftcorp/microsoft/final/en-us/microsoft-brand/documents/IR-Guidebook-Final.pdf)

## Bringt Löschen mehr Geschwindigkeit?

Unsere praktische Empfehlung: Ein Ordner, der wiederkehrende Arbeit erleichtern soll, gehört nicht in eine automatische „alles weg“-Routine. Entfernst du solche Informationen, fehlen sie zunächst für den nächsten Start. Daraus lässt sich kein dauerhafter Geschwindigkeitsgewinn ableiten.

Wenn das Laufwerk voll ist, miss zuerst die Größen. Vergleiche Prefetch mit deinen Videos, Downloads und alten Sicherungen. Eine kleine Windows-Hilfsdatei ist meist kein guter Tausch gegen den Aufwand einer Systemänderung. Bei konkreten Startproblemen lohnt sich die Diagnose des betroffenen Programms.

## Ist der Wert im Dateinamen eine Sicherheitsprüfung?

Der kurze Wert im Namen einer Prefetch-Datei bezieht sich auf den Programmpfad und teilweise auf Startparameter. Er ist keine SHA-256-Prüfsumme des heruntergeladenen Programms. Ein vertrauter Name allein beweist außerdem nicht, dass eine Datei harmlos ist.

Quellen: [Microsoft Incident Response: Prefetch (PDF)](https://cdn-dynmedia-1.microsoft.com/is/content/microsoftcorp/microsoft/final/en-us/microsoft-brand/documents/IR-Guidebook-Final.pdf)

## So findest du sinnvollere Aufräumziele

1. **Größen vergleichen:** Scanne dein Laufwerk mit DiscReveal und folge den größten Ordnern. Die Ansicht zeigt Größen, ohne daraus automatisch eine Löschentscheidung zu machen.
2. **Eigene Dateien zuerst prüfen:** Beginne mit großen Dateien, deren Zweck du kennst. Alte Videoexporte oder bewusst aufbewahrte Archive sind leichter zu beurteilen als Windows-interne Startdaten.
3. **Windows selbst aufräumen lassen:** Für temporäre Windows-Dateien ist die Speicherverwaltung der passende Ausgangspunkt. Lies die ausgewählten Kategorien, bevor du eine Bereinigung bestätigst.

Quellen: [Microsoft Support: Storage Sense](https://support.microsoft.com/en-us/windows/experience/storage-filemanagement/manage-drive-space-with-storage-sense)

## Weitere Links

- [Was ist ein Cache – und wann lohnt sich das Leeren?](https://discreveal.com/de/ratgeber/was-ist-ein-cache/)
- [Was belegt meine Festplatte unter Windows?](https://discreveal.com/de/ratgeber/was-belegt-meine-festplatte/)
- [Was ist ein Datei-Hashwert – und was sagt er über Sicherheit?](https://discreveal.com/de/ratgeber/datei-hashwert-pruefen/)
- [Alle Artikel](https://discreveal.com/de/ratgeber/)
- [DiscReveal](https://discreveal.com/de/)
- [Windows-Download](https://github.com/markmarkinson/discreveal/releases/latest/download/discreveal-portable.exe)
