# Drive scans: DiscReveal compared

DiscReveal, robocopy, PowerShell and other methods on the same folder. Times including the finished app interface, test setup and raw data.

## Drive scan

Around 200,000 files, 14.95 GB, five methods on the same folder. DiscReveal displays the finished results in 1.19 seconds — including scanning, data transfer and interface rendering.

| Method | Median |
|---|---:|
| DiscReveal | 1.19 s |
| robocopy /L | 1.81 s |
| Node.js recursive walk | 6.95 s |
| cmd dir /s | 8.40 s |
| PowerShell 7.6.5 | 11.64 s |

Intel Core i7-6700K, Samsung 860 EVO SATA SSD, warm Windows file cache, median of five runs per method. DiscReveal: the published Windows app, already open in list view, measured from a real click to the fully displayed final results. Commands: process start to completion, without printing file lists to the screen. Actual times depend on your drive and files.

This test compares Windows commands and a script with DiscReveal, not other graphical disk analyzers. It does not measure duplicate search.

[Drive scan raw data](https://discreveal.com/benchmarks/scan-0.2.3.json)
