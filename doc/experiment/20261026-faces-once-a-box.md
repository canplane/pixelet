# Faces once a box

**2026-10-26.** `sample_cs` asked what lies about the face each sample
stopped on (`World::faced`, up to twelve blocks of the world's query) for all
of about 0.9M samples, and `cast_cs` reads that word only off a box's own
sample, the one its visible sample names: about 50k. This asks it once a box.

## How

Fixed path, dpr 2, `resolution` fixed, shadows off, AO corner, the dev
build. Branch `experiment/20261026-faces-once`, on main at de2f73c26 (the
miss from a list). To size it first, a variant of 21f72517b with `faced`
never asked in `sample_cs` (none kept).

`boxes_cs` raises a dispatch's workgroups as it writes boxes, three words
past the draw's args (pxl-spec-core `raster::ARGS_FACES`, `FACES_LANES` 64
boxes a workgroup). `faces_cs` (pxl-shader raster.rs), a thread a box off
that indirect dispatch, in a pass of its own after the raster's (span
`faces`), takes the box's sample, casts that sample's ray as `sample_cs`
did (`eye_ray` off `Grid::sample`), rebuilds its stop from the node, block
and axis the sample left, asks `faced` with no looks bound (it reads a
block's value alone, not the ground's looks) and leaves the word where
`cast_cs` reads it (`beam::SAMPLE_FACES`). A box whose sample lacks met no
face, 0, as before. It binds eight storage buffers: the world's five, the
samples, the boxes and the args, read-only beside its indirect use.

## Numbers

| Build                                 | `sample` p50 / p95 | `faces` p50 / p95 | both p50 / p95 | interval p50 / p95 |
| ------------------------------------- | ------------------ | ----------------- | -------------- | ------------------ |
| 21f72517b, 3 loads, 2056 × 1113       | 11.9–14.1 / 21.9–25.6 |                |                | 42 / 86–91         |
| `faced` never asked, 2 loads, × 1113  | 5.8 / 16.6, 4.3 / 9.0 |                |                | — / 72, 86         |
| de2f73c26, 2056 × 1169                | 11.2 / 24.3        |                   |                | 25 / 67            |
| faces once a box, load 1, × 1169      | 4.1 / 12.2         | 0.5 / 1.8         | 4.8 / 14.2     | 21 / 68            |
| faces once a box, load 2, × 1169      | 3.9 / 8.5          | 0.7 / 1.8         | 4.8 / 9.6      | 17 / 47            |

In ms, over the cast frames whose times were read. The cast's spans summed
a frame: 21.7 / 56.6 on de2f73c26, 13.2 / 35.4 and 12.3 / 24.7 here.

## Exact

A check build, a measuring build's and not kept, had `sample_cs` ask as
before and `faces_cs` compare its word with the sample's instead of writing
it, counting both into two words past the boxes: 1,277,494 boxes over 91
casting frames, at most 53,716 in one, none differing. The word is the
world's query's answer about the same block from the same face, asked once
instead of once a sample.

## What it showed

- `faced` was most of `sample`: about 7–10 ms at p50 of the 11–14.
- Asked once a box it costs 0.5–0.7 ms at p50 and 1.8 at p95, so the two
  passes together take a third of what `sample` did alone.

## Alternated

With the check's code taken out (`boxes` bound read-only), main
(de2f73c26) and this build alternated load by load, while other agents'
pages rendered on 5173 and 5175, so the intervals are theirs as much as
ours and only the spans are compared. Every load, in order:

| Load | Build | Size        | `sample` p50 / p95 | `faces` p50 / p95 | cast frames timed |
| ---- | ----- | ----------- | ------------------ | ----------------- | ----------------- |
| 1    | faces | 2056 × 1169 | 4.3 / 9.3          | 0.8 / 2.6         | 112               |
| 2    | main  | 2056 × 1169 | 23.8 / 28.7        |                   | 6                 |
| 3    | faces | 2056 × 1169 | 4.3 / 9.2          | 0.5 / 2.4         | 69                |
| 4    | main  | 2056 × 1169 | 10.9 / 15.5        |                   | 48                |
| 5    | faces | 2056 × 1113 | 4.0 / 7.3          | 0.6 / 1.4         | 116               |
| 6    | main  | 2056 × 1113 | 15.3 / 22.3        |                   | 62                |
| 7    | faces | 2056 × 1113 | 4.1 / 4.7          | 0.5 / 1.4         | 26                |

The window's height moved between loads with the browser's own chrome; each
pair is at one size. Load 2 timed six cast frames and is no measure. Main's
`sample` runs 11–15 ms at p50 against 4.0–4.3 here, the dispatch over the
boxes 0.5–0.8 more.

## Taken

What lies about a face is asked once a box now: `boxes_cs` raises the
dispatch's workgroups past the draw's args (`raster::ARGS_FACES`), and
`faces_cs` asks `faced` for each box's sample in a pass of its own, timed in
span `faces`, before the pixels read it; `sample_cs` asks it no more. The
check and the measuring variants are not kept. A sample whose box another
drew keeps no word of its faces: `cast_cs` reads only a box's own sample's,
and `boxes_cs` compares samples on the words before it (`alike`).
