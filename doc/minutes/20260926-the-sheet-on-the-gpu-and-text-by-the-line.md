# The sheet on the GPU, and text by the line

**2026-09-26** — found while giving the device's kernel what the page's
browser had been doing for it, decided in the conversation. It settles how
the billboard sheet is drawn on every target, and what a platform is asked
for when there is text.

## What happened

The page drew three things on the CPU and copied them to the GPU. The kernel
recorded 2D paint calls — paths, images, text — into a stream, and the shell
ran them on an OffscreenCanvas and uploaded the result: the Moon's image into
its texture, a tile of the texture pack into each material layer, and every
badge and bubble into the billboard sheet. Widths came back by a separate
round trip (`MEASURE`), a character at a time.

A device has no canvas. Carrying the stream there meant a 2D rasterizer in
the kernel — tiny-skia, a shaper and fonts shipped with the app — or a
second implementation of every paint call per platform.

## What the paint was for

Read call by call, none of it needed a 2D canvas:

- **The Moon** is one image copied whole into a layer: a copy, not a drawing.
- **A material layer** is one tile of the pack's sheet, scaled to the layer:
  a copy of a texture region.
- **A badge** is round rectangles, circles, rings, rectangles and a bubble's
  tail, and text. The shapes are what a shader draws as signed distances.

So the stream is gone, on the page as well. An image is uploaded as the
texture it already was and copied on the GPU (`COPY_TEXTURE_TO_TEXTURE`). A
sheet entry is drawn by one pass into a texture of its size and copied into
its rectangle (`pxl-render gpu/sheet.rs`, `frame/shader/sheet.wgsl`): its shapes
as signed distances, its text as pixels the platform drew. The page's shell
lost the paint player, its scratch canvas and the width round trip.

## Text, by the line

What is left for a platform is text, and the question was the unit.

**A glyph at a time (considered first, not taken).** The kernel would lay
glyphs out of an atlas, asking the platform only for one glyph's mask and
advance. It reuses glyphs across names, but a glyph at a time cannot kern,
cannot join Arabic, cannot reorder bidirectional runs, and cannot form an
emoji sequence: shaping happens between glyphs, and the unit leaves it out.

**Shaping in the kernel (not taken).** rustybuzz and unicode-bidi with font
files the kernel holds — how Chrome and Skia do it. It needs the fonts'
bytes and a fallback chain of its own: Overture names places in every
script, and Noto CJK alone is more than ten megabytes. Mapbox and MapLibre
draw ideographs with the platform's local fonts for the same reason.

**The platform shapes glyph runs, the kernel caches glyphs (not taken).**
Correct and reusable, but a page's canvas gives no glyph IDs, so the page
would be left the one target without it.

**A line at a time (taken).** The kernel asks for a line of text in a weight,
a size and a color, and the platform draws the whole line: `fillText` on a
page, CoreText's `CTLine` on iOS. Kerning, ligatures, joining, bidi, fallback
and emoji are the platform engine's, the quality a page's labels already
had, on every target alike. A text too wide for its box is broken into lines
by the shell, measuring with the same platform (`ts/client/src/abi/text.ts`
`line_ends`), and the kernel asks for each line's pixels
(`pxl-kernel sys/io/text.rs`, host.json's `TEXT_LAYOUT` and `TEXT_RASTER`).

> Since moved: the lines are broken in the client's `abi/`, which answers
> the kernel's own calls (`ts/client/src/abi/text.ts` `line_ends`), not
> by the shell, which is the UI alone.

The cost is reuse: "Gangnam Station" and "Gangnam-gu Office" share no
pixels. A line is rasterized once, when a sheet entry is drawn, and what is
kept is the sheet; the expensive part before was the whole badge rasterized
on the CPU and uploaded, which this removes. If a profile ever says glyph
reuse matters, the third option can be added on the devices without changing
the boundary's shape.

## Where it stands

Built on both targets. Images decode where the device is: the browser's
`createImageBitmap` on a page, the `png` and `image-webp` crates on a device
(xpute-gpu `image.rs`). The sheet pass and the platform's lines draw every
badge and bubble on the page, the simulator and an iPhone.
