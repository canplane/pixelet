# The pixel pass's footprint

**2026-10-26.** `cast_cs` is as large a module as `miss_cs` was before
[the miss from a list](./20261026-the-miss-from-a-list.md): every lane pays the
walk's variables before its first read, though almost every pixel only takes the
visible sample and tests its entry (`cast::entry`). This weighs what that
footprint costs the `pixel` span before splitting the pass.

## How

One measuring build of main de2f73c26, not kept, choosing the pixels' kernel
through the eye's spare word: `cast_cs` as built; the walk stubbed out
(`cast_stub_cs`: a pixel without an entered visible sample writes `ray::MISS`
and returns, nothing listed); and `cast_cs` counting into spare words of its
marks' buffer, read back. Fixed path, dpr 2, 4112 × 2226 drawn (9.15 Mpx),
`resolution` fixed, shadows off, AO corner, spans read a frame each.

## Numbers

`pixel` p50 / p95 / max, ms, the loads in the order run:

| Load | As built         | Walk stubbed    |
| ---- | ---------------- | --------------- |
| 1    | 2.2 / 2.4 / 3.6  | 1.2 / 1.4 / 1.5 |
| 2    | 2.2 / 5.4 / 26.0 | 1.2 / 2.8 / 3.5 |

The counting load, 120 frames read back: 99.9997% of pixels take the visible
sample; walking, p50 26 / p95 146 / max 697 pixels; marked, p50 0 / p95 15 / max
162; 8×8 groups holding a walker, p50 26 / p95 45 / max 82.

cast.gen.wgsl declares 1111 function-scope variables at the top of its one
function, before the visibility buffer's `textureLoad`; the stub's module
declares 45.

## Not taken

About 1.0 ms of the 2.2 ms at p50 is the footprint, paid by lanes that never
walk. A take-only kernel listing its walkers for a reading walk off an indirect
dispatch would win that at most, out of about 20 ms of the cast's spans, for a
new kernel, a list sized by the pixels and its buffers. Not built.

The 9–18 ms `pixel` p95 seen earlier did not reproduce on de2f73c26: its two
loads as built gave 2.4–5.4 ms, the high one from a few frames while tiles
loaded. The counting load, `cast_cs` with an atomic or more a pixel, gave 2.3 /
2.4.
