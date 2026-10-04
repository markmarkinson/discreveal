# Secure deletion: what works on HDDs and SSDs?

markMarkinson · 2026-10-02 · 3 min read

You want a confidential file to become inaccessible. Its name disappearing from a folder is not enough to establish that. The storage medium, other copies and your cleanup goal matter.

Overwriting one file does not guarantee that an entire device is sanitized. On SSDs in particular, it cannot promise complete irrecoverability.

## Removing and overwriting are separate operations

Ordinary deletion does not inherently overwrite all previous contents. Secure-delete utilities additionally try to replace accessible file data with other data. That can hinder recovery of those contents; it does not automatically remove copies elsewhere.

Sources: [Microsoft Learn: SDelete](https://learn.microsoft.com/en-us/sysinternals/downloads/sdelete)

## What does that mean on an HDD?

An HDD stores data magnetically on rotating platters. Overwriting accessible storage areas is one possible sanitization method. Overwriting a single file, however, does not automatically cover earlier copies, backups or areas outside that file. When handing over a drive, consider the entire storage medium.

Sources: [NIST SP 800-88 Rev. 2: Media Sanitization](https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-88r2.pdf)

## Why SSDs need a different approach

Flash storage manages writes internally and may contain additional areas that normal software cannot directly address. Overwriting through a file path therefore cannot reliably reach every earlier physical copy. A single overwriting app cannot turn that into guaranteed complete erasure.

Sources: [NIST SP 800-88 Rev. 2: Media Sanitization](https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-88r2.pdf)

## What DiscReveal actually does

Secure delete overwrites accessible contents of selected files with random data and then removes them. It is separate from the Recycle Bin mode and is not the default action for duplicate cleanup.

Secure overwrite refuses hard links because several names can refer to the same data. Reparse links, compressed, encrypted and sparse NTFS files, and files with additional data streams are also excluded. Ordinary overwriting can leave previous physical data for these NTFS file types even on HDDs. It does not eliminate earlier backups, cloud versions or every possible metadata and filesystem trace. It is not a certified media-destruction procedure.

## When giving a PC to someone else

First back up what you want to keep and verify that backup. When handing over a device, sanitizing the whole storage medium is a more appropriate goal than individually removing visible documents.

Microsoft offers a data-cleaning option when resetting with Remove everything. It makes recovery harder, but is intended for consumers and does not meet government or industry erasure standards. Highly sensitive devices require an appropriate, verifiable method for the medium involved.

Sources: [Microsoft Support: Reset your PC](https://support.microsoft.com/en-us/windows/experience/backup-recovery/reset-your-pc)

## Start with the goal

“Secure” always involves a goal and its limits. More effort alone does not prove that every earlier copy has disappeared.

- Do you want to free space? Overwriting is unnecessary in many everyday cleanup situations.
- Do you want to recover an accidental removal? The Recycle Bin is a more appropriate first choice.
- Are you handing over the whole device? Consider every storage location and use a procedure for the complete medium.

## Related links

- [Files in the Recycle Bin: deleted or still there?](https://discreveal.com/guides/recycle-bin-files-really-deleted/)
- [Remove duplicate files while keeping an original](https://discreveal.com/guides/remove-duplicate-files/)
- [What is a file hash, and what does it tell you about safety?](https://discreveal.com/guides/check-file-hash/)
- [All articles](https://discreveal.com/guides/)
- [DiscReveal](https://discreveal.com/)
- [Windows download](https://github.com/markmarkinson/discreveal/releases/latest/download/discreveal-portable.exe)
