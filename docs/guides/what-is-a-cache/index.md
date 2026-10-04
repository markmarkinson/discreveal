# What is a cache, and when should you clear it?

markMarkinson · 2026-10-02 · 2 min read

“Clear cache” sounds like an easy route to more space and a faster computer. A cache is actually meant to save work. Whether you should remove it depends on its purpose.

Clear a specific cache when you need space or are addressing a concrete problem. Constantly clearing it can bring back the work it was meant to avoid.

## A shortcut for repeated work

Imagine looking up the same term several times and keeping a note beside you. A cache similarly holds available results for later requests. Applications may cache previews, computed information or downloaded content. The exact contents depend on the cache.

## Memory caches and cache files are different

Windows also keeps read file data in memory. A later request can be served from the system file cache. An application’s cache files, however, occupy drive space. A faster second scan therefore does not prove that the app created a persistent cache.

Sources: [Microsoft Learn: File Caching](https://learn.microsoft.com/en-us/windows/win32/fileio/file-caching)

## When is clearing useful?

Use the application’s own controls where possible. Deleting everything in AppData is not cache cleanup. Settings, profiles and your own work may live there too.

- A particular cache takes substantial space and you do not currently need its stored content.
- The application’s help recommends clearing it for a specific display or refresh problem.
- You want to remove cached content from a shared device. Review history and account data separately; these are not necessarily the same storage.

## What happens afterwards?

Anything still needed has to be loaded or computed again. Think of a large photo list with no existing thumbnails: building it the first time can take work again. That alone does not indicate a new fault.

A useful question is whether you are trying to free space, fix a problem or simply make a number smaller. The first two goals usually provide a better reason for deliberate cleanup.

## DiscReveal’s optional cache

The duplicate finder can remember short content fingerprints and file metadata. Unchanged files can skip that preliminary check next time. Confirmed matches are still compared in full. A cache hit alone never triggers cleanup.

The cache is off by default. If enabled, it lives locally in AppData. The app shows its size and lets you clear it. Drive-scan results instead stay in memory only for the session.

## Related links

- [What is AppData, and what can you delete there?](https://discreveal.com/guides/what-is-appdata/)
- [What is Prefetch, and should you delete the folder?](https://discreveal.com/guides/what-is-prefetch/)
- [How do we find duplicate files with different names?](https://discreveal.com/guides/how-we-find-duplicates/)
- [All articles](https://discreveal.com/guides/)
- [DiscReveal](https://discreveal.com/)
- [Windows download](https://github.com/markmarkinson/discreveal/releases/latest/download/discreveal-portable.exe)
