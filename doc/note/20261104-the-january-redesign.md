# The January redesign, as the user recalls it

**2026-11-04** — told by the user while looking back over the history, and checked
against it. Between `checkpoint/20260107-float` and `checkpoint/20260405-rpc` the
history holds one commit for about three months' work, and the records of the time
are notes salvaged later. What moved the redesign is written nowhere else.

## What the user recalls

- **By about January 7 a skeleton stood**, built quickly with AI's help. Its upper
  bound felt closed, so the structure had to be set properly.
- **So all of it was redesigned**, in a data-oriented style.
- **The compile was broken and the work went top-down** for about a month, then
  was joined back up.
- **The user wrote first and had AI supplement**, unlike later work, which is left
  almost wholly to AI.
- **The January 19 commit gathers the work up to early April.** It was amended over
  and over.

## What the history shows

`361c796e6`, "full platform rewrite", has an author date of 2026-01-19 and a commit
date of 2026-04-03, as repeated amends leave them. It changes 463 files, 32,604 lines
added and 22,800 removed, lock files and other non-source among them. No commit from
those months remains in this clone: none is unreachable, and the reflog begins at
2026-10-26.

Source files (`.ts`, `.tsx`, `.js`, `.mjs` and shaders) at the two checkpoints:

| | January 7 | April 5 |
| --- | --- | --- |
| Files | 183 | 284 |
| Same path | 40 | 40 |
| Same path and bytes | 1 | 1 |

The one is `apps/web/tailwind.config.js`. The January build's core was
`packages/core`; April's splits into `packages/xpute` and `packages/pxl`.

April's lines also found in January, by area:

| Area | Lines | Also in January |
| --- | --- | --- |
| `apps/web/src/shared`, mostly UI | 1,284 | 72% |
| `apps/web/src/app` | 135 | 67% |
| `apps/web/src/modules` | 2,015 | 18% |
| `packages/pxl/bake` | 722 | 12% |
| `packages/pxl/spec` | 1,041 | 9% |
| `packages/pxl/render` | 424 | 5% |
| `packages/xpute/core` | 1,723 | 3% |
| `packages/xpute/spec` | 3,588 | 2% |
| `apps/{web,api}/src/xpute` | 1,333 | 1% |
| All | 12,948 | 14% |

A line is compared whitespace collapsed, wherever it lies, leaving out comments,
imports and lines under 8 characters. Short common lines match by chance, so the low
figures need not be inheritance.

## Not settled by the history

The upper bound and the data-oriented aim rest on this recollection. The structure
of the code shows the rewrite, not why it was done.

What crossed the rewrite as ideas rather than lines is in
[January origin salvage](../minutes/archive/20260910-january-origin-salvage.md) and
[the April build](../minutes/archive/20260909-april-checkpoint-salvage.md).
