# Platforms

The server is developed and run on macOS and Linux. Those are the two systems the extended-attribute cache, the safe file open, and read-ahead control are implemented for. Nothing else is claimed to work.

FreeBSD and Windows are a small amount of our own code away. The image decoders have no operating-system checks. This has not been cross-compiled, so the lists below are from the `cfg` gates and from the build scripts of the C libraries.

## Media crate

Decoding and encoding are portable Rust plus four native libraries: `libde265` (HEVC), `dav1d` (AVIF decode), `turbojpeg` (libjpeg-turbo), and `rav1e` (AVIF encode). JPEG, PNG, GIF, WebP, JPEG XL, and the header parsers are plain Rust.

The only platform code is opening a file to probe its headers (`crates/media/src/probe.rs`). The same open exists for the text sniff in `server/src/storage/mime.rs`.

- **FreeBSD** already gets `O_NOFOLLOW` and `O_NONBLOCK`, so a symlink is not followed and a FIFO cannot block the read. Turning read-ahead off is a no-op. FreeBSD has `posix_fadvise`, and the Linux call in those two files can be reused.
- **Windows** compiles that probe, but the open flags are omitted. A symlink is followed, a named pipe can block the read, and read-ahead stays on. The replacements are `FILE_FLAG_OPEN_REPARSE_POINT` and `FILE_FLAG_RANDOM_ACCESS` on the same two opens.

What can stop a build is producing the C libraries:

| Library | FreeBSD | Windows |
|---|---|---|
| `libde265` | pkg-config lookup of a system `libde265`. Install the package, or enable that crate's embedded build. | The crate installs it through vcpkg. Needs MSVC and a long first build. |
| `dav1d` | A system library, or a Meson build cloned during `cargo build` (Git, Meson, Ninja). | The same Meson path. |
| `turbojpeg` | CMake build of libjpeg-turbo. `require-simd` needs NASM. | The same. |
| `rav1e` | Rust, plus NASM for the `asm` feature. | The same. |

## FreeBSD

Unix code is already shared: the open flags above, and SIGTERM in `main.rs`. `xattr.rs` compiles, then its get and set return `-1`, so a listing never hits the cache and every store fails.

Replace those two stubs with `extattr_get_link` and `extattr_set_link` in the user namespace. The attribute name is `naspresso.file`. The `user.` prefix on `user.naspresso.file` is Linux's namespace, not part of the name. A missing attribute is `ENOATTR`. ZFS and UFS already store user attributes. This needs a FreeBSD machine to confirm the errno.

Optionally call `posix_fadvise` from the probe and the sniff, as on Linux.

## Windows

`xattr.rs` does not compile. `c_path` uses the Unix `OsStrExt` with no `cfg`, so the Windows stubs under it are never reached.

The cache belongs in an NTFS alternate data stream, `file.jpg:naspresso.file`. It is not a directory entry and it is not counted in the file's size. Writing a stream updates LastWriteTime. The record is trusted only when its captured time is at least the file's modification time, so the write has to save that time and put it back, or the next listing misses forever.

The probe and the sniff need the two open flags from the media section. Shutdown is already fine: SIGTERM is Unix-only, and Windows waits on Ctrl-C.

The long part is the first build: MSVC, CMake, NASM, vcpkg for `libde265`, and Meson for `dav1d`.
