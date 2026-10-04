# Was ist ein Datei-Hashwert – und was sagt er über Sicherheit?

markMarkinson · 2026-10-02 · 2 Min. Lesezeit

Neben einem Download steht oft eine lange Folge aus Zahlen und Buchstaben. Das ist kein Passwort. Ein Datei-Hashwert ist eine berechnete Kennung des Inhalts, mit der du zwei Dateien vergleichen kannst.

Eine passende Prüfsumme kann bestätigen, dass dein Download zur vertrauenswürdigen Referenz passt. Sie ist kein Virenscan und kein allgemeines Gütesiegel.

## Was wird dabei verglichen?

Ein Verfahren wie SHA-256 berechnet aus den Dateibytes einen Wert fester Länge. Ein anderer Dateiname ändert diesen Inhaltswert nicht; veränderte Inhalte ergeben normalerweise einen anderen Wert. SHA-256 wird in PowerShell von Get-FileHash unterstützt.

Quellen: [Microsoft Learn: Get-FileHash](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.utility/get-filehash)

## Ein Beispiel: der Download einer App

Du lädst discreveal-portable.exe über den offiziellen Download und möchtest sicherstellen, dass sie vollständig angekommen ist. In den Release-Details steht die Prüfsumme des angebotenen Builds. Vergleiche sie mit dem Wert deiner gespeicherten EXE, nicht mit dem eines älteren Releases.

Eine Datei aus einer anderen Version darf einen anderen Wert haben. Auch ein selbst erstellter Build kann abweichen. Der Unterschied beantwortet zuerst die Frage „Ist es dieselbe Datei?“; warum sie anders ist, muss separat geklärt werden.

## So prüfst du den Wert in PowerShell

Öffne PowerShell im Ordner der heruntergeladenen Datei. Der folgende Befehl liest die Datei und verändert sie nicht. Vergleiche die Ausgabe in der Spalte Hash mit der offiziell veröffentlichten SHA-256-Prüfsumme.

```powershell
Get-FileHash -LiteralPath .\discreveal-portable.exe -Algorithm SHA256
```

Quellen: [Microsoft Learn: Get-FileHash](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.utility/get-filehash)

## Was ein übereinstimmender Wert nicht beweist

Die Referenz muss aus einer vertrauenswürdigen Quelle stammen. Liefert eine fremde Website sowohl eine manipulierte Datei als auch ihren neu berechneten Wert, passen beide trotzdem zusammen. Aus einem passenden Wert allein folgt deshalb keine vertrauenswürdige Herkunft.

Eine Prüfsumme bewertet zudem nicht, was ein Programm tut. Sie prüft weder seine Berechtigungen noch sein Verhalten. Bei einer unbekannten Anwendung gehören Herkunft, Sicherheitsprüfung und die Entscheidung, sie auszuführen, weiterhin dazu.

## Wie DiscReveal dir den Vergleich abnimmt

Das Update-Symbol neben dem Appnamen öffnet eine Seite in deiner App-Sprache. Der Link enthält Version, Sprache und – sofern lesbar – die Inhaltskennung der laufenden EXE. Die Seite vergleicht diese Angaben mit dem aktuellen offiziellen Release und hebt abweichende Stellen hervor.

Deine gescannten Dateien und ihre Pfade sind nicht Teil dieses Links. Die Prüfung startet nur durch deinen Klick; Updates werden nicht automatisch installiert. Die kurzen Kennungen der Duplikatsuche haben einen anderen Zweck und ersetzen diesen Download-Vergleich nicht.

## Weitere Links

- [Wie finden wir doppelte Dateien – auch mit anderem Namen?](https://discreveal.com/de/ratgeber/wie-finden-wir-duplikate/)
- [Was ist Prefetch – und sollte man den Ordner löschen?](https://discreveal.com/de/ratgeber/was-ist-prefetch/)
- [Sicheres Löschen: Was funktioniert auf HDD und SSD?](https://discreveal.com/de/ratgeber/sicheres-loeschen-hdd-ssd/)
- [Alle Artikel](https://discreveal.com/de/ratgeber/)
- [DiscReveal](https://discreveal.com/de/)
- [Windows-Download](https://github.com/markmarkinson/discreveal/releases/latest/download/discreveal-portable.exe)
