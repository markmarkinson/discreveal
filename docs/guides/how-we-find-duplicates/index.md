# How do we find duplicate files with different names?

markMarkinson · 2026-10-02 · 3 min read

Holiday.jpg and Holiday-copy.jpg may contain the same photo. Two files with exactly the same name can instead be entirely different. DiscReveal therefore looks for identical content.

A confirmed match means the full file contents were compared. You decide whether a copy is still needed in its particular location.

## Rule out differences, then compare thoroughly

1. **Compare sizes:** Files with different lengths cannot be byte-for-byte identical. DiscReveal first groups equal sizes; files without a size partner need no content check.
2. **Check small samples:** The finder reads small samples from the beginning, middle and end. Different fingerprints rule out candidates. Matching samples alone do not establish a confirmed duplicate.
3. **Confirm the complete contents:** Remaining candidates are compared byte for byte in full. If many different contents share a sample, additional complete-content fingerprints reduce unnecessary comparisons. The byte comparison remains decisive.

## Why not just use filenames?

A filename describes what a file is called, not what it contains. Renaming a copied PDF does not change its contents. A newly exported PDF can also reuse the previous version’s name.

DiscReveal finds exact copies, not merely similar-looking photos. A resized picture or recompressed video can look similar while containing different bytes. Those variants are not grouped as identical content.

## What makes the search faster?

Files below 10 MB are excluded by default to focus on larger copies. You can deliberately disable this limit for small photos or documents. Further size and file-type filters reduce the candidates.

Reader concurrency follows the drive type. The optional cache remembers preliminary checks for unchanged files. Large groups can briefly reuse already-read content in memory. None of these shortcuts replaces confirmation of matching contents.

## An original stays, but its location matters

Each group retains the oldest known copy as an original. Cleanup compares each selected copy with that original again. Changed or no longer matching files are skipped.

A backup is intentionally a duplicate. A project may require a specific path too. Check locations before selecting files. Multiple hard-link names for one physical file do not count as extra reclaimable copies.

## What does “potential savings” mean?

The total uses measured occupied space for extra copies, leaving one original per group. Compressed and sparse files contribute their actual allocation. Files with further hard links contribute no reclaimable space; unavailable measurements are identified and excluded from the total.

Duplicate cleanup moves selected copies to the Recycle Bin. Space does not automatically become free while they remain there. Review the selection deliberately rather than making the largest displayed number your deletion target.

## Related links

- [Remove duplicate files while keeping an original](https://discreveal.com/guides/remove-duplicate-files/)
- [What is a file hash, and what does it tell you about safety?](https://discreveal.com/guides/check-file-hash/)
- [What is a cache, and when should you clear it?](https://discreveal.com/guides/what-is-a-cache/)
- [All articles](https://discreveal.com/guides/)
- [DiscReveal](https://discreveal.com/)
- [Windows download](https://github.com/markmarkinson/discreveal/releases/latest/download/discreveal-portable.exe)
