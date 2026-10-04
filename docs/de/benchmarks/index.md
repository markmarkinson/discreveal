# DiscReveal im Vergleich: Laufwerksscan und Duplikatsuche

Zwei Aufgaben, zwei Vergleichstests. Hier siehst du die gemessenen Zeiten, die verwendeten Programme und die Bedingungen der Messungen.

## Laufwerksscan

Rund 200.000 Dateien, 14,95 GB, fünf Methoden im selben Ordner. DiscReveal zeigt die fertigen Ergebnisse nach 1,19 Sekunden — inklusive Scan, Datenübertragung und Darstellung der Oberfläche.

| Methode | Median |
|---|---:|
| DiscReveal | 1.19 s |
| robocopy /L | 1.81 s |
| Node.js recursive walk | 6.95 s |
| cmd dir /s | 8.40 s |
| PowerShell 7.6.5 | 11.64 s |

Intel Core i7-6700K, Samsung 860 EVO SATA-SSD, aufgewärmter Windows-Dateicache, Median aus je fünf Durchläufen. DiscReveal: veröffentlichte Windows-App, bereits geöffnet in der Listenansicht, gemessen vom echten Klick bis zur vollständig dargestellten Ergebnisliste. Befehle: vom Prozessstart bis zum Abschluss, ohne Dateilisten auf dem Bildschirm auszugeben. Die tatsächliche Zeit hängt von deinem Laufwerk und deinen Dateien ab.

Der Test vergleicht Windows-Befehle und ein Skript mit DiscReveal, keine anderen grafischen Speicheranalyse-Programme. Er misst keine Duplikatsuche.

[Rohdaten des Laufwerksscans](https://discreveal.com/benchmarks/scan-0.2.3.json)

## Duplikatsuche

### Duplikatsuche

DiscReveal findet identische Dateiinhalte und behält beim Bereinigen ein Original. Hier vergleichen wir die Suchzeiten mit Czkawka.

In drei von vier gezielten Tests ist unser Suchkern schneller als Czkawka 12.0.2. Im besonders schwierigen vierten Fall liegt Czkawka vorn. Beide finden genau die erwarteten Duplikate.

**Suchkern / Kommandozeile · ohne grafische Oberfläche · warmer Windows-Dateicache.**

| Testfall | DiscReveal | Czkawka 12.0.2 |
|---|---:|---:|
| Gleicher Anfang, anderer Inhalt | 71 ms | 193 ms |
| Unterschiede außerhalb der Stichproben | 392 ms | 354 ms |
| Viele identische Kopien | 77 ms | 192 ms |
| Kleine Dateien ausgeschlossen | 15 ms | 88 ms |

Intel Core i7-6700K, SATA-HDD, fünf Durchläufe je Programm nach einem Aufwärmlauf, Median. Gemessen wurde die Zeit vom Prozessstart bis zum fertigen Ergebnisexport. Beide App-Caches waren aus; der Windows-Dateicache war bereits gefüllt. DiscReveal wurde als isolierter Suchkern mit vollständigem Bytevergleich gemessen, Czkawka 12.0.2 als Kommandozeilenprogramm mit BLAKE3 und Standard-Threadzahl. Die Oberfläche war bei beiden nicht enthalten.

Die Testdateien wurden bewusst für unterschiedliche Prüffälle erstellt. Das sind keine typischen persönlichen Dokumente. Die Werte hängen von Dateien, Laufwerk und Cache ab und versprechen keine allgemeine Beschleunigung. Bei kaltem Dateicache oder in der vollständigen App können andere Zeiten entstehen.

[Alle Messwerte und der genaue Testaufbau](https://discreveal.com/benchmarks/duplicates-0.2.5.json).
