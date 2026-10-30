# Where decor grows

**2026-10-27.** Panning brought `generate` spikes of 6–12 ms on frames that fill
squares. This splits them, and records what the reshaping read off the code
fixed, so the open part is not rediscovered.

## How

The working tree of the day, R3 (each candidate at its own record, grown again
only where a box meets what it reads) and R2 (decor asking the world through
the query's support). `GO_TO` 127.06026, 37.28820, zoomed out to about 1679,
polar not set; six left-drag pans of 800 px over about 13 s; dpr 2, 2056 ×
1169, `resolution` fixed. Grow over the frames with a GPU read. The variants
were compile-time builds, each reverted.

## Numbers

| Build                                    | p95 ms | max  | fill frames p50 |
| ---------------------------------------- | ------ | ---- | --------------- |
| R3                                       | 11.1   | 23.9 | 1.7             |
| R3 + R2                                  | 13.8   | 25.4 | 1.77            |
| footing only (no asks, no claim)         | 14.8   | 23.4 | 1.44            |
| no footing (every stood candidate grows) | 12.4   | 20.8 | 3.47            |

Load to load the same build differs by about 3 ms at p95. With each dispatch
in a span of its own, for one build: `decor_cs` p50 5.05, p95 18.7; mark p95
1.25; count, args and place p95 0.20 or under.

## What it showed

- **The spike is `decor_cs`'s alone.** Marking, counting, laying out and
  placing are each under 1.3 ms at p95.
- **Neither the footing nor the asks make it.** Removing both leaves it.
  The coordinating session's runs the same morning, at the same pose with
  key pans:
  - `generated` compiled out of `decor_cs`, `retained` kept: the module 1.2 MB →
    62 KB, p95 0.2–0.9 ms;
  - `generated` compiled in and skipped at run time, `retained` run: 7–8 ms;
  - returning before `retained`, the module still whole: 0.3 ms;
  - split into a stand kernel and a grow kernel: 6.5–6.7 ms.

  The first two run the same lanes and differ by the module's size alone, so
  the kernel's size and the occupancy it leaves, not the work its lanes do,
  is the likely cost; the third shows the lanes cheap where they return
  early in it. Not settled.
- **Serializing a patch's world reads made a load frame 117 ms** (pixelet-d6,
  a thread a patch growing all its candidates). The contest stays a patch's
  (`decor_thin_cs`), the growth a record's (`decor_cs`).

Fixed off the code, not the numbers: the footing one support in the query;
every block asked off the footing's line, on both sides; a square's margin
grown again, not its 3 × 3; a patch's stream and contest drawn once, tested
equal to `retained` over the catalog (pxl-spec-core scatter.test.rs); one
footing a candidate on the CPU.

Timing stopped here on the user's word: what is plainly redundant is fixed by
reading, and the whole is timed once, against 9/26 (backlog P1).
