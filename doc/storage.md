# Storage

`STORAGE` is the directory the server serves. `CACHE` holds files derived from it. Nothing under `CACHE` is a user file.

## `GET /storage/ls`

`GET /storage/ls?path=/a/b/c&limit=30&cursor=…` returns one page of that directory.

```json
{
  "items": [],
  "next_cursor": null,
  "total": 0
}
```

`path` is inside `STORAGE`. It defaults to the root. `limit` defaults to 30 and must be from 1 to 100. `total` counts every listable name in the directory, not just this page. `next_cursor` is `null` on the last page. Pass it back as `cursor` to get the next page.

Each item is either a directory or a file. `kind` is `dir` or `file`.

```json
{ "kind": "dir", "name": "photos", "modified_at": 1710000000000, "created_at": null }
```

```json
{
  "kind": "file",
  "name": "img.jpg",
  "modified_at": 1710000000000,
  "created_at": 1710000000000,
  "size": 12345,
  "mime": "image/jpeg",
  "image": { "width": 4032, "height": 3024, "hdr": "none", "animated": false },
  "has_preview": true
}
```

`modified_at` and `created_at` are Unix milliseconds. `created_at` is `null` when the filesystem has no birth time. `size` is bytes. `image` is present only when the file's headers were read as an image: upright pixel size, how it carries HDR (`none`, `unknown`, `gain_map`, `pq`, `hlg`), and whether it has frames. `has_preview` is true when that image data is present and a preview bundle for this version of the file is already on disk, so asking for a thumbnail will not wait on a render.

### What gets listed

The directory is read once. Symlinks, special files, names that are not UTF-8, and the OS metadata files `.DS_Store`, `.apdisk`, and `Thumbs.db` are dropped. Directories come first, then files. Within a kind, names are ordered case-insensitively, and an exact name breaks a tie (`a` before `A`).

Only the names that fall on this page are `stat`ed. The rest of the directory stays names. A name that disappears between the read and the `stat` is skipped.

The cursor is the kind and name of the last item on the page, so the next page starts after it. The cursor does not have to point at a name that still exists.

### Where `mime` and `image` come from

A known non-image extension is typed from the extension table. The file is not opened. An `.npe` file is `naspresso/npe`. When a file is sniffed, a header of `NSPE` is that same type.

An image extension is probed from its headers. That probe supplies both `mime` and `image`. If the headers cannot be read, the entry falls back to the extension and `image` is omitted.

A file with no known extension is `application/octet-stream` when it is empty or larger than 512 KiB. Otherwise the first 4 KiB are sniffed: magic bytes win, and a file that looks like text becomes `text/plain`.

Image probes and those sniffs are remembered on the file as the extended attribute `user.naspresso.file`. A later listing reads the attribute before opening. It trusts the record only when the record's version is supported and the time it was captured is at least the file's modification time, so a save replaces a stale record. The attribute is written after the JSON has been returned. A listing does not wait on that write. Files that were not opened are not written.

The captured time is the clock time of the read, not the file's own modification time. Writing the attribute changes the file's ctime only, so it does not invalidate this record or the preview fingerprint below.

## Preview bundles

A preview is an AVIF whose longest side is at most 128, 256, or 512 pixels. Smaller images are not enlarged. `GET /media/preview/256/a/b/c.jpg` serves one of those three.

All three sizes of one source file live in one bundle. The cache mirrors `STORAGE` under `CACHE/preview`, with `.preview` appended to the file name: `STORAGE/a/b/c.jpg` is cached as `CACHE/preview/a/b/c.jpg.preview`.

The bundle is built largest first. Each smaller size is resized from the previous one, so the source is decoded once. The file is replaced by writing a temporary file beside it and renaming it into place, so a crash cannot leave half a bundle.

```text
"NSPM" | version: u8 | count: u8 | source fingerprint: 16 ASCII bytes
count × (longest side: u16 LE | length: u32 LE)
the AVIFs, back to back, in that same order
```

Version is 1. The fingerprint is a hash of the source path, its modification time, and its birth time. A bundle whose fingerprint does not match the file open in front of us is ignored and rendered again.

`has_preview` does not open the bundle. It `stat`s the bundle file and checks that the bundle's modification time is at least the source's. That is a cheaper answer for a directory page. The preview request itself still checks the fingerprint, so a bundle that merely looks new is not sent.

Rendering is also coalesced: two requests for the same source and fingerprint share one render, and only a few renders run at once.
