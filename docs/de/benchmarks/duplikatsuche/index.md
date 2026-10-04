# Duplikatsuche: DiscReveal und Czkawka im Vergleich

Vier gezielte Vergleichstests mit identischen Dateien. Suchzeiten, richtige Treffer und Grenzen der Messung ohne Oberfläche.

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
