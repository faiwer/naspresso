# media

Understanding and transforming media files: what a file is, its size and HDR
kind from its headers alone, and AVIF previews of images.

## Public API

- `sniff_mime(head, path)`: the type from the first `SNIFF_LEN` bytes, else by
  extension.
- `probe_file(path) -> Probe`: the MIME type and, for an image, an
  `ImageMetadata`: upright size, `Hdr` kind and whether it's animated. Never
  fails: what can't be read is typed by extension. Only files named like images
  are opened, as regular files (no symlinks or FIFOs), with read-ahead off.
- `preview::render(path, quality, speed) -> Rendered`: AVIFs of the image at
  128, 256 and 512 px on the longest side, never upscaled. Blocking and
  CPU-bound.

## Probing

Probes read headers and skip bodies, so listing a folder of 50 MB photos reads a
few KB per file. Most formats have their own walker in `<format>/probe.rs`:

- JPEG: size from the frame header; orientation from EXIF; a gain map from XMP
  (`hdrgm:Version`, Apple's `HDRGainMapVersion`) or the ISO 21496-1 APP2 block.
- PNG: IHDR, `cICP` (PQ, HLG), `eXIf`, `acTL`.
- HEIF (AVIF, HEIC): the `meta` box, read whole. A `tmap` item or Apple's
  `hdrgainmap` auxiliary image means a gain map; an `nclx` with PQ or HLG means
  those. Sequences come from the `ftyp` brands.
- JPEG XL: the image header and the `jhgm` box. A packed JPEG's gain map is
  inside the packed data, so those report `Hdr::Unknown`.
- GIF, WebP, BMP, TIFF: size, orientation and animation; HDR isn't detected.

## Previews

Every preview is an AVIF, since Chrome is the only client. Each kind of image
keeps its kind of HDR:

- SDR: 8-bit pixels in the source's color space. An RGB ICC profile is
  embedded as is; gray and CMYK profiles count as sRGB, since decoders hand
  out RGB for those.
- PQ and HLG: one 16-bit BT.2100 PQ image. HLG and narrower primaries (BT.709,
  Display P3) are converted to it (`pq.rs`), at the largest preview size
  rather than the full image.
- Gain map: an ISO 21496-1 AVIF. The SDR base is the primary item, so any
  reader shows it, and a `tmap` item derives the HDR rendition from the base
  and a hidden gain map (`heif/avif.rs`). The gain map is scaled in proportion
  to the base and follows its orientation.

All of them are coded as 10-bit full-range 4:4:4 AV1 (`av1/encode.rs`). Alpha
is a separate AV1 item, at higher quality than the color.
Animated images give their first frame.

Each size is resized from the one above it (`fast_image_resize`), and all the
encodes run at once: one small rav1e encode keeps only a core or two busy.

### Formats

| Format   | SDR | PQ / HLG | Gain map                                |
| -------- | --- | -------- | --------------------------------------- |
| JPEG     | yes | —        | Ultra HDR, ISO 21496-1                  |
| JPEG XL  | yes | yes      | `jhgm`; a packed JPEG's, as for JPEG    |
| AVIF     | yes | yes      | `tmap`                                  |
| HEIC     | yes | yes      | `tmap` (iOS 18+), Apple's (iOS 14.1–17) |
| PNG      | yes | no       | —                                       |
| WebP     | yes | —        | —                                       |
| GIF, BMP | yes | —        | —                                       |
| TIFF     | yes | —        | —                                       |

### Decoders

- JPEG: libjpeg-turbo (`turbojpeg`), with DCT scaling to decode near the
  largest preview size instead of in full.
- AVIF: dav1d. HEIC: libde265. Both go through our own HEIF reader
  (`heif/items.rs`, `heif/decode.rs`): grids of tiles, decoded in parallel and
  cropped to the output size, alpha, rotation and mirroring, color from `nclx`
  or ICC. Planes become RGB with `yuvutils-rs` (`yuv.rs`).
- JPEG XL: `jxl-oxide`.
- PNG, WebP, GIF, BMP, TIFF: the `image` crate.

An HEVC image without color boxes is treated as sRGB with BT.601 full-range
YCbCr, as macOS's ImageIO decodes it.

### Apple's older gain maps

iPhones from iOS 14.1 to 17 store the gain map as an auxiliary image
(`urn:com:apple:photo:2020:aux:hdrgainmap`) and the headroom only in the EXIF
maker note, tags 0x21 and 0x30 (`heif/apple.rs`). The map's gains apply to
linear light, `HDR = SDR × (1 + (headroom − 1) × g)`, where `g` is the map value
through the sRGB curve. The map is rewritten as ISO gains so it fits the same
`tmap` output.

## System libraries

- `dav1d` and `libde265`, linked dynamically. Install the newest `libde265` you
  can: Debian lists several CVEs against 1.1.1, and it parses untrusted files.
- libjpeg-turbo is built from source by `turbojpeg` with CMake. On x86, its SIMD
  and rav1e's assembly need `nasm`.

## Gaps

- PNG HDR (`cICP` PQ or HLG) and SDR `cICP` aren't decoded with their colors;
  HDR PNGs fail with `HdrUnsupported`.
- Apple's older gain-map JPEGs (`HDRGainMapVersion` only) are detected, not
  previewed.
- Gain maps whose alternate image has its own color space
  (`use_base_color_space = false`) aren't supported.
- HEIF: `clap` crops are ignored, premultiplied alpha is treated as straight,
  and matrices other than identity (4:4:4 only), BT.601, BT.709, BT.2020,
  SMPTE 240 and FCC are rejected.
- RAW formats and video poster frames aren't supported.
