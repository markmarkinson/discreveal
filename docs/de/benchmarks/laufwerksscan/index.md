# Laufwerksscan: DiscReveal im Vergleich

DiscReveal, robocopy, PowerShell und weitere Methoden im selben Ordner. Zeiten inklusive fertiger App-Oberfläche, Testaufbau und Rohdaten.

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
