# Size vs size on disk: Understand the difference

markMarkinson · 2026-10-04 · 3 min read

A file can contain 100 MB of data yet occupy less space on your drive—or more. That does not necessarily mean anything is wrong. File size and occupied space answer different questions. Knowing the difference helps when you want to free up storage.

File sizes are useful for a quick overview. For a closer look at occupied file space, enable the additional measurement in DiscReveal.

## Two sizes, two questions

File size describes how much data an application receives when it reads a file. Occupied space is the drive space allocated to store it. Storage blocks and filesystem features can make these values differ.

Many small files can occupy more space than their combined file sizes suggest. Compressed files can require less. A file-size scan is therefore a useful way to find large files, but it is not a full accounting of your drive.

## Compression and sparse files

NTFS can store file contents in compressed form while applications read the uncompressed data. A file containing 100 MB may therefore occupy less drive space. The amount depends on its contents.

Sparse files leave certain empty data regions without allocated storage blocks. A file can appear large while requiring much less space. Virtual drives and specialist applications are examples of where you may encounter them.

Sources: [Microsoft Learn: File compression](https://learn.microsoft.com/en-us/windows/win32/fileio/file-compression-and-decompression) · [Microsoft Learn: Sparse files](https://learn.microsoft.com/en-us/windows/win32/fileio/sparse-files)

## Hard links: different names, one file

A hard link gives the same file another name on the same drive. Unlike two independent copies, these names share the data. Adding every displayed file size counts the same contents more than once.

Removing one name does not release that data while further hard links remain. A large-looking group is therefore not automatically a large cleanup opportunity. DiscReveal counts shared file space once in its additional measurement.

Sources: [Microsoft Learn: Hard links](https://learn.microsoft.com/en-us/windows/win32/fileio/hard-links-and-junctions)

## Choose the right measurement in DiscReveal

The measurement covers allocated file data. Filesystem metadata, snapshots and backups are outside this total. Changing files and missing access permissions can also cause differences from the drive’s storage display.

1. **Find large files quickly:** Choose Scan, select your drive and click “Start scan”. The default view shows file sizes and updates while scanning.
2. **Take a closer look at occupied space:** Open Filter under Scan and enable “Measure occupied space”. Then start the scan. It accounts for compression, sparse files, additional data streams and hard links, and takes longer.
3. **Check incomplete values:** Unavailable measurements are identified. The size tooltip also shows the file size. After deleting files, scan again to update occupied-space accounting.

## What does this mean for duplicate search?

DiscReveal compares the full contents before confirming a duplicate. Potential savings use measured occupied space, rather than simply multiplying file size by the number of matches.

One original stays in each group. Copies with further hard links contribute no reclaimable space; unavailable measurements are identified and excluded. Moving files to the Recycle Bin keeps their data on the drive until you empty it.

## Related links

- [What is taking up space on my Windows drive?](https://discreveal.com/guides/what-is-taking-up-disk-space/)
- [How do we find duplicate files with different names?](https://discreveal.com/guides/how-we-find-duplicates/)
- [Files in the Recycle Bin: deleted or still there?](https://discreveal.com/guides/recycle-bin-files-really-deleted/)
- [All articles](https://discreveal.com/guides/)
- [DiscReveal](https://discreveal.com/)
- [Windows download](https://github.com/markmarkinson/discreveal/releases/latest/download/discreveal-portable.exe)
