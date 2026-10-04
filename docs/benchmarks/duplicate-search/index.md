# Duplicate search: DiscReveal and Czkawka compared

Four targeted comparisons using identical files. Search times, correct matches and the limits of measurements without the interface.

## Duplicate search

### Duplicate search

DiscReveal finds identical file contents and retains an original during cleanup. Here we compare search times with Czkawka.

Our search core is faster than Czkawka 12.0.2 in three of four targeted tests. Czkawka leads in the particularly difficult fourth case. Both find exactly the expected duplicates.

**Search core / command line · no graphical interfaces · warm Windows file cache.**

| Test case | DiscReveal | Czkawka 12.0.2 |
|---|---:|---:|
| Same beginning, different content | 71 ms | 193 ms |
| Differences outside the samples | 392 ms | 354 ms |
| Many identical copies | 77 ms | 192 ms |
| Small files excluded | 15 ms | 88 ms |

Intel Core i7-6700K, SATA HDD, five runs per program after a warm-up, median. Time was measured from process start to completed result export. Both app caches were disabled; the Windows file cache was already warm. DiscReveal was measured as an isolated search core with full byte comparison; Czkawka 12.0.2 as its command-line program with BLAKE3 and the default thread count. Neither graphical interface was included.

The test files were deliberately created for different checking scenarios. They are not a typical collection of personal documents. Results depend on files, drive and cache and do not promise a general speedup. Cold-cache runs or the complete app may take different amounts of time.

[All measurements and the full test setup](https://discreveal.com/benchmarks/duplicates-0.2.5.json).
