# The last TypeScript that was not the shell

2026-09-18 into 09-19, following
[the server in Rust](./20260918-the-server-in-rust-and-the-line-already-drawn.md)
and [the gate that deploys](./20260918-the-gate-that-deploys.md).

## What was left, and why it looked settled

After the server moved, the TypeScript that remained fell into two piles that
seemed to have nothing to do with each other: the tools under `tool/`, run by
hand a few times a year, and `@pxl/shared`, the package both ends of the wire
encoded from.

Both were argued as staying. The tools because their cost is Overture's
download and `sharp`'s image work rather than the language; `@pxl/shared`
because the client needs the catalogs it holds. Neither argument survived being
looked at.

## `shared` had stopped meaning anything

The package's name is its purpose: shared between the client and the server.
It had two jobs and both had already ended.

The first went when the server became Rust — there is no second encoder of the
wire format to stay in step with. The second was subtler: `@pxl/shared` was the
reference implementation the Rust was checked against, through the golden
vectors under each crate's `vector/`. Those generators were deleted in
`ef560912` on the morning of 09-18, and the `.tsv` files became a record rather
than something regenerable. Nine Rust test files still cited
`script/vector/<path>.ts's output` — a directory that no longer existed.

So what was left under the name was the shell's own code.

## The line that decided where each piece went

Not "is it TypeScript" but:

|                                                                                                  | Where it belongs                                        |
| :----------------------------------------------------------------------------------------------- | :------------------------------------------------------ |
| **What a thing is** — the cube catalog's numbering, the size ladder's values, the channel names  | `spec/` as JSON, generated into Rust and into the shell |
| **How the UI arranges it** — a palette's order, a drawer's sections, which tiers a flyout offers | the shell, hand-written, beside the list it is about    |

That line is what made the rest mechanical, and it caught a real inversion:
`spec/cell/mod.ts` imported `GroundChannelKey` from `bake/format.ts` — the
cell catalog reading its own channel names out of a binary wire format. The
names are the catalog's; the _order a drawer shows them in_ is the drawer's.
Both moved, in opposite directions.

## What each cut found

**Nothing imported a fifth of it.** `sparse_tree` (856 lines with its test),
`ground.ts`, `xy.ts`, `yup.ts` and a barrel: 1,027 lines with no importer
anywhere. They looked alive because `deno task check` compiles a package whole.

**One line was holding 1,874.** `use_reverse_geocode` keyed its label by a page
it derived itself — `L3_IDX(epsg4326.project({i:0n,f:0n}, map_center).i)` — and
that one call was the only reason the shell carried an implementation of the
coordinate space. The kernel holds `map_center` as a PVF3 and already calls
`L3_IDX` on it; what it _sent_ was degrees, so the shell projected them back and
indexed them again to reach the number the first one had. The snapshot carries
`map.page` now. They could disagree before: a round trip through degrees is not
the identity, so a center near a page boundary could be labeled from the page
next door.

**One derivation was written twice.** `OBJECT_SIZES` was
`[U_PER_CELL, U_PER_CELL/2, …]` in both TypeScript and Rust, with the same
doc comment word for word — both written as a derivation on purpose, which
makes it arithmetic with two chances to disagree. It is `spec/bake/cell.json`
now, and `pxl-spec`'s own `CELL_SHAMT` is re-exported from the generated file
rather than declared beside it.

**The rest was the kernel test's fixture.** `encode_terrain` and an empty POI
page were what the test answers the module's fetches with — it drives the
module from outside and has nothing else to hand it bytes. Written out in
`apps/web/src/kernel/fixture.ts` rather than imported, which is what a fixture
is. Nothing checks it against the Rust and nothing needs to: a page the kernel
cannot decode is a page it does not draw, so a drifted shape takes the test's
own assertions down with it.

## The tools were the blocker, and moving them was the answer

`tool/ingest/asset/ingest_asset.ts` read `cube_type.ts`. That made the cell
catalog shared between the shell and a tool, not shell-only, and the question
became whether to generate the table into two places.

The question was wrong. Generated TypeScript is justified where TypeScript is
unavoidable, which is the shell and nowhere else; generating a table for a tool
is making a second copy of something that should not exist. **The tools become
Rust, and the question disappears.**

And once they are Rust, the bake's own shape applies: a tool is a command line
over a library function, and the function is the same one everything else
calls. The Overture ingests went into `pxl-geo-bake` — the crate that bakes
from the tables they fill — rather than a crate of their own, because the seam
they had was in the middle of one pipeline. `ingest_places.rs` is 70 lines
where its TypeScript was 226: the venv handling, the line reader and the flush
loop were copied into all five.

## Three float bugs the ingest port found, none of them visible by reading

**An untyped NULL is not a NULL.** A parameter is serialized by its Rust type,
so a missing `height` bound as `None::<&str>` reaches a `double precision`
column as text and the driver refuses it. Absence had to move inside each
variant: a missing height is a missing _float_.

**jsonb keeps a number's decimal text.** Postgres stores a jsonb number as
`numeric`, so its digits are part of the value — and re-rendering a parsed
double does not reproduce them. serde_json writes `0.9018452233076095` as
`0.9018452233076096`: the same double, a different numeric. 1,145 of 10,979
places differed on `properties` alone. JSON that arrives as text is carried as
the source's own bytes now (`RawValue`) and parsed by Postgres, and the
geometry is never parsed at all — it is the largest field, nothing reads into
it, and a re-rendered coordinate lands an ULP away once projected.

**serde_json's default float parser is not correctly rounded.**

```
127.02105069516375
  std::parse, and Postgres's own strtod -> 0x405fc158e503d254
  serde_json (default)                  -> 0x405fc158e503d255
```

227 of 10,979 POI sat an ULP east of where the TypeScript put them.
`float_roundtrip` is the feature for exactly this; with it, zero.

All three were found by running both implementations at the same bbox into a
scratch database and comparing every table column by column with the geometry
as WKB. Nine tables, 22,411 rows, zero differences in both directions.

## What the image tools cost, measured rather than assumed

The asset tools were expected to change bytes, being a different image library.
Two of the three did not:

- **The WebP sheet is byte-identical.** sharp reaches libwebp through libvips
  and the `webp` crate reaches it directly, so the same RGBA at the same
  quality is the same 407,680 bytes under the same key. The prediction that
  this would cost one new object was wrong.
- **The 125 per-file keys were never at risk**, being the sha256 of bytes
  copied verbatim, and `glb.rs` measures all 58 models to the committed table
  exactly — a test now rather than a claim.
- **The icons and the tiles did change**, and the difference was measured
  rather than eyeballed. The icons differ by a pixel of placement plus about
  0.65% per channel, because sharp's `trim` is libvips `find_trim` — a
  composite difference against the background, not a per-channel one — so its
  box is a pixel wider and everything inside shifts. The tiles differ by at
  most 2 of 255, mean 0.17, with no placement to shift.

Matching libvips exactly means reimplementing `find_trim` and its Lanczos,
which a favicon is not worth. The PNGs come out 30% smaller for the same
picture. mozjpeg is a dependency because without it the link preview went from
30 kB to 55.

The sixty tiles were re-normalized rather than left for the next pack change,
so the tree and the tools agree from here: 61 new objects, uploaded before the
deploy that names them.

## Where it stands

`packages/` holds `@xpute/core` alone, which is what that package is for —
generic kit with no domain, mirrored in Rust and portable out. `tool/` is bash
and GDAL. Every other language boundary now follows a deployment unit rather
than a URL: `crates/` is the servers and everything offline, `apps/web` is the
shell.

AGENTS.md's second boundary is restated rather than removed. It said
`@pxl/shared` is a contract between the client and the servers, both ends
encoding from one source. There is one end now: the shell reads a page through
the kernel and never decodes one, so a wire format has a single implementation.
What the shell holds is what it draws with, generated from `spec/` beside the
Rust it is generated with.
