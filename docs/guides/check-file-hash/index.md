# What is a file hash, and what does it tell you about safety?

markMarkinson · 2026-10-02 · 2 min read

Downloads often have a long string of letters and numbers beside them. It is not a password. A file hash is a value calculated from the contents, useful for comparing files.

A matching checksum can confirm that a download matches a trusted reference. It is not a virus scan or a general seal of approval.

## What does a hash compare?

An algorithm such as SHA-256 calculates a fixed-length value from file bytes. Renaming the file does not change that content value; changing its contents normally produces a different value. PowerShell supports SHA-256 through Get-FileHash.

Sources: [Microsoft Learn: Get-FileHash](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.utility/get-filehash)

## Example: downloading an application

You download discreveal-portable.exe from the official download and want to check that it arrived intact. The release details list the checksum of the offered build. Compare it with the value of your saved EXE, not a value from an older release.

A different application version can legitimately have a different value. A build you compile yourself may also differ. The comparison first answers “Is this the same file?”; the reason for a difference needs a separate explanation.

## Check the value in PowerShell

Open PowerShell in the downloaded file’s folder. The following command reads the file without changing it. Compare the Hash column with the officially published SHA-256 checksum.

```powershell
Get-FileHash -LiteralPath .\discreveal-portable.exe -Algorithm SHA256
```

Sources: [Microsoft Learn: Get-FileHash](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.utility/get-filehash)

## What a matching value cannot prove

The reference must come from a trusted source. If an unfamiliar website provides both a modified file and a newly calculated checksum, the two still match. A matching value alone therefore does not establish trustworthy origin.

A checksum also does not assess what a program does. It does not inspect permissions or behaviour. For an unfamiliar application, origin, security checks and the decision to run it still matter.

## How DiscReveal handles the comparison

The update icon beside the version opens a page in your app language. Its link includes the version, language and, when readable, the running EXE’s content fingerprint. The page compares these with the current official release and highlights differences.

Your scanned files and paths are not included in this link. The check starts only when you click; updates are never installed automatically. The short fingerprints used by duplicate search have another purpose and do not replace download verification.

## Related links

- [How do we find duplicate files with different names?](https://discreveal.com/guides/how-we-find-duplicates/)
- [What is Prefetch, and should you delete the folder?](https://discreveal.com/guides/what-is-prefetch/)
- [Secure deletion: what works on HDDs and SSDs?](https://discreveal.com/guides/secure-deletion-hdd-ssd/)
- [All articles](https://discreveal.com/guides/)
- [DiscReveal](https://discreveal.com/)
- [Windows download](https://github.com/markmarkinson/discreveal/releases/latest/download/discreveal-portable.exe)
