# What is AppData, and what can you delete there?

markMarkinson · 2026-10-02 · 2 min read

Your Windows user folder contains AppData. The name sounds technical, but this is where programs often keep everyday settings, local data and cached information.

AppData is not a folder of disposable files. Check the application and its cleanup options rather than emptying the whole directory.

## Where is AppData?

The usual location is C:\Users\YOUR-NAME\AppData. Windows separates Local, Roaming and LocalLow. Press Windows + R and enter %LOCALAPPDATA% to open Local; %APPDATA% normally opens Roaming. You can inspect these folders without changing them.

Sources: [Microsoft Learn: Windows known folders](https://learn.microsoft.com/en-us/windows/win32/shell/knownfolderid)

## Local, Roaming and LocalLow

The application decides what it stores. The directory name cannot tell you whether a particular subfolder is disposable.

- Local: data for your user account on this PC, such as local caches.
- Roaming: a location for user-specific application data. The name alone does not mean your home PC uploads it to a cloud service.
- LocalLow: a separate Windows location for lower-integrity applications; it is not a bin for less important files.

Sources: [Microsoft Learn: Windows known folders](https://learn.microsoft.com/en-us/windows/win32/shell/knownfolderid) · [Microsoft Press: Windows Internals – Integrity Levels (PDF)](https://download.microsoft.com/download/1/4/0/14045a9e-c978-47d1-954b-92b9fd877995/97807356648739_samplechapters.pdf)

## Why does it grow?

Consider two different cases: one application stores previews it can rebuild, while another keeps a profile or work not backed up elsewhere. Both can be large. Only the first is likely to be a cache; a size display cannot make that distinction.

Note the application name, subfolder and size. Consult its documentation to understand the contents. If you no longer use the application, start by uninstalling it and then review any data left behind.

## A practical cleanup approach

1. **Find the large subfolders:** DiscReveal shows which parts of AppData occupy space. Protected or inaccessible areas may not be fully counted.
2. **Check the application:** Look for options such as Clear cache, Manage storage or Remove offline content. The application can manage its own data consistently.
3. **Protect important data:** Review profiles and local work before removing anything. A cache is not a backup; equally, not every application file is a cache.

## What does DiscReveal store there?

Only with your opt-in does the duplicate finder keep a cache in %LOCALAPPDATA%\DiscReveal\Cache. It contains file paths, sizes, modification times and short content fingerprints, not copies of your files. You can clear it in the app. The interface also uses a local WebView2 profile; portable settings live in discreveal_data next to the EXE.

## Related links

- [What is a cache, and when should you clear it?](https://discreveal.com/guides/what-is-a-cache/)
- [What is taking up space on my Windows drive?](https://discreveal.com/guides/what-is-taking-up-disk-space/)
- [What is Prefetch, and should you delete the folder?](https://discreveal.com/guides/what-is-prefetch/)
- [All articles](https://discreveal.com/guides/)
- [DiscReveal](https://discreveal.com/)
- [Windows download](https://github.com/markmarkinson/discreveal/releases/latest/download/discreveal-portable.exe)
