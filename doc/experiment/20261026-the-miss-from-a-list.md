# The miss from a list

**2026-10-26.** `miss` was about 20 ms of every cast frame on the fixed path
(p95 21–25 ms), dispatched a thread a drawn pixel, each but the marked
returning at once. This finds what it costs and dispatches it over the
marked pixels alone.

## How

Fixed path, dpr 2, 2056 × 1113 (about 9.2 Mpx), `resolution` fixed, shadows
off, AO corner, the dev build. Branch `experiment/20261026-miss-compact`.
Before building, three measuring variants of main (21f72517b), none kept:
the walk stubbed out (`miss_cs` returns once it finds a mark, the mark
cleared); `cast_cs` counting the marked pixels and the 8 × 8 groups holding
one, read back into spare profiler lanes; and `sample_cs` with `faced` never
asked (for the next topic).

## What it costs

| Build                         | `miss` p50 / p95, ms         |
| ----------------------------- | ---------------------------- |
| main, two loads               | 20.0 / 21.6, 20.0 / 21.2     |
| the walk stubbed, two loads   | 0.1 / 0.2, 0.1 / 0.3         |

Marked pixels a cast frame, read a frame or two late: p50 5, p95 41k, max
99.6k; groups holding one p50 3, p95 4.4k, max 6.6k. The bursts come while
tiles load and the eye zooms; most frames mark none to fifty.

`miss` costs 20 ms in frames that mark none, so its cost is not the walks: it
is launching 9.2M threads of a module whose walk declares 1751 function-scope
variables before its first `textureLoad` (miss.gen.wgsl), which every lane
pays before it returns.

## The list

`cast_cs` lists each pixel it marks (pxl-spec-core `observed::MARKS_AT`,
room for 2^20), taking its place off a count in a small buffer of its own
(`ray::MISS_ARGS_WORDS`), which it binds as its eighth storage buffer and
raises the walk's dispatch in, 64 workgroups a row of 4096 listed. The count
is copied into the pyramid's buffer after `cast_cs` (`observed::MARKS_COUNT`),
and `miss_list_cs` walks each listed pixel as `miss_cs` did (`missed`), 64
lanes a workgroup, dispatched indirect. A pixel past the list's room has
`cast_cs` raise the second dispatch, `miss_cs` over every pixel as before,
which runs after the list's and finds the listed ones no longer marked; it
is none otherwise.

The list lives in the pyramid's buffer because `miss_cs` already binds eight
storage buffers, the WebGPU default, and this adapter reports
`maxStorageBuffersPerShaderStage` 8 (ts/app/web/.dev-console.log), so no
device raise is there to take; frame/shader/floor.rs holds every layout to
the default. `cast_cs` binds the pyramid read-write to list into it, by
plain writes: its count, the one atomic, is in the other buffer, so no word
of the pyramid is typed atomic in the reading walk (cast.gen.wgsl binds it
`var<storage, read_write>` of plain `u32`).

## Numbers

| Build                         | `miss` p50 / p95 | cast's spans p50 / p95 | interval p50 / p95 / max |
| ----------------------------- | ---------------- | ---------------------- | ------------------------ |
| main                          | 20.0 / 24.7      | 43.3 / 64.0            | 42 / 89 / 1375           |
| the list, load 1              | 0.3 / 0.4        |                        | — / 77 / 2355            |
| the list, load 2              | 0.3 / 0.7        | 19.1 / 33.0            | 22 / 66 / 1568           |

The cast's spans are `cast` through `miss` summed a frame, compute spans
all bar `sraster`. Each interval max is the path's first-zoom stall, which
main has too. Main's two earlier loads: interval p95 91 and 86.

## Exact

A check build, a measuring build's and not kept, walked each listed pixel
twice and counted those whose four words differed the second time, into two
words past the list's count: 171,309 listed over 89 read
frames, at most 95,096 in one (within the room, so the whole-screen walk
never ran), every one checked, none differing. The walk off a list is the
walk `miss_cs` made, from the same start through the same query; which
thread runs it, and what the pyramid holds when it does, does not change its
words. A still frame after load on main and on the list's build: 136 of
1.2M screenshot pixels differ, all in the actor's animation and the
profiler's FPS label.

Not tried: resuming a listed pixel from where the reading walk stopped
rather than its cell's start. With `miss` under a millisecond it has
nothing left to win.

## Taken

The list is the cast now: `cast_cs` lists the pixels it marks and counts
them in `ray::MISS_ARGS_WORDS`, the count is copied to
`observed::MARKS_COUNT`, `miss_list_cs` walks the list off an indirect
dispatch, and `miss_cs` over every pixel runs only where the list's 2^20 are
full, its dispatch otherwise none. The check is not kept, nor are the
measuring variants above.

The list sits in the observed pyramid's buffer, not a buffer of its own,
because the walk that writes the pyramid already binds eight storage
buffers, WebGPU's default and what this adapter reports, and every layout is
held to the default (frame/shader/floor.rs). The reading walk binds the
pyramid read-write only to list into it by plain writes; its one atomic, the
count, is in the dispatch's own buffer, so its reads of the pyramid stay
plain (cast.gen.wgsl, `array<u32>`).
