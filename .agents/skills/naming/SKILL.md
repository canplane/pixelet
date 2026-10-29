---
name: naming
description: How a name in pixelet is chosen and spelled — a convention's or the 0405 checkpoint's word, never a coined one; abbreviations by default and the words that stay whole; the names chosen with the user; case by language and kind; bindings abbreviated, types whole; plurals, units, and names someone else owns; and how a rename is carried out. Use before naming or renaming anything, in code, spec JSON or a backticked identifier in a document.
---

# Naming

A name says what the thing is called where it comes from. Before naming or
renaming, find that word: what the field calls it (graphics, GIS, game
cameras, WAI-ARIA, the language's own idiom) and what
`origin/checkpoint/20260405-rpc` called it. Use it, and say in the proposal
where it comes from. A thing with no precedent is named with the user, never
coined alone.

Why each rule is what it is: [the naming minutes](../../../doc/minutes/20261029-a-name-is-the-field-s-and-abbreviates-by-default.md).

## Abbreviate by default

A word on this list is abbreviated; a word not on it stays whole until the
user adds it. The list is the abbreviations 0405 used and the ones the user
added:

index `idx`, indices `idxs`, value `val`, values `vals`, buffer `buf`, buffers
`bufs`, source `src`, destination `dst`, offset `off`, count `cnt`, current
`cur` (not `curr`), position `pos`, direction `dir`, pixel `px`, resolution
`res`, distance `dist`, address `addr`, capacity `cap`, dimension `dim`, memory
`mem`, component `comp`, channel `ch`, elevation `ele`, feature `feat`,
latitude `lat`, longitude `lon`, radian `rad`, degree `deg`, orientation
`orient`, rotation `rot`, altitude `alt`, error `err`, packet `pkt`, initial
`init`, matrix `mat`, configuration `cfg`, descriptor `desc`, context `ctx`.

- Longitude is `lon`, as PROJ, OpenStreetMap, PostGIS and the spec name it;
  0405's `lng` was a web map library's.
- `pkt` only for a real wire packet (an XTP packet: a command's arguments, a
  baked tile's bytes).
- `desc` is a descriptor (D3D12's `_DESC`, wgpu's `desc`, XTP's descriptor word);
  SQL's `DESC` (descending) is SQL's.

## Whole, though they look short

- `zoom`: `z` is the coordinate's.
- `size` in pixelet, `sz` in xpute: pixelet is domain, so `size` everywhere
  in it, GPU texture sizes too; xpute is system (`HDR_SZ`, `WORD_SZ`).
- `environment`: a domain word; `env` reads as the system's.
- `source` where it means a source of data (`WORLD_SOURCE`, `of_source`).
- `encode`, `decode`.

## Names chosen with the user

Each thing has one name:

| thing | name | not |
| --- | --- | --- |
| where the camera looks from | `eye` | camera position |
| the point the camera turns about (the eye itself in first person) | `pivot` | focus, center, target |
| where the pivot is going | its `to` (`pivot_to`, `put_pivot`, `resolve_to`) | target |
| the direction the camera faces | `facing` | look |
| the camera modes | `orbit`, `first_person` (the UI says 1인칭/3인칭) | free |
| what a mouse, touch or controller points with | `pointer` | cursor |
| the raster's nearest sample at each pixel | the visibility buffer; a pixel `take`s its sample | pick |

`at` is where a thing is, `to` where it is going.

## Case, by language and kind

**Rust:** snake_case bindings, fields, functions and a macro's arguments
abbreviate, lone words included (`let val`, `fn pkt()`, `$cfg`); SCREAMING
consts abbreviate (`MAX_PKT_BYTES`, `INIT_SIZE`, `CFG`). PascalCase types and
variants stay whole (`MarshalError`, `SensorStatus::Error`, `Config`).

**TypeScript:** camelCase stays whole. snake_case and SCREAMING abbreviate
(`kernel_pos`, `phase_cnt`, `ERR`), lone words too in snake-style code (xpute
TS, the apps, `ts/client/src/abi/`): `alt` in `kernel_pos`, an app guest's
linear memory `mem`, not `buf`. The React shell (`ts/client/src/shell/`) keeps
whole words for the names it makes, and reads a kernel or spec name as it is
(`s.location.err`).

**C++ (xpute):** snake_case abbreviates.

**Spec JSON:** keys, field `name`s and list entries follow the snake and
SCREAMING rules (`"val"`, `"pkt"`, `"err"`, `SENSOR_POS`, `XR_POSE`,
`INPUT_GOTO`). `$doc` prose is not. A generated type follows its enum's name
(`RES_MODES` → `ResMode`).

## Bindings and types

Abbreviate the binding, keep the type: `let idx: Index`, `let cfg: Config`,
`SensorState { status: SensorStatus, err: Option<Message> }`.

## Plurals

A plural names a collection only. One value is singular (`End`, `Take`,
`Write`, `Occupancy`, the object layer's `OBJECT`), whatever it chooses
among.

A plural abbreviates as 0405 did (`vals`); `counts`, `offsets`, `positions`
and `sizes` stay whole.

## Units

A length says its unit as a suffix: `_u` the world's units, `_merc` a meter on
the EPSG:3857 plane (PROJ's name), `_m` a meter on the ground, never the
plane's (`pxl-spec space/coord/adapter/3857.rs`). `_mat` is a matrix, never a
meter. A unit shown to a person goes on the value, not the label (`120.0 u`,
`29.0 m`).

## What is someone else's

These keep the owner's word:

- the language's, the platform's and a library's: the C++ coroutine protocol
  (`initial_suspend`, `return_value`…), `value()`, `error()`, `size()`, a JS
  collection's `size`, TypedArray `buffer`, `env.memory`, `devicePixelRatio`,
  the browser's `coords.altitude` and `SensorHeard`, i18next's `lng` (a
  language);
- xpute's `gpu/`, which is WebGPU's vocabulary;
- external formats: OSM Carto's `feature`, Overture's categories, a WKB's
  `M` (measure);
- what a generator writes (`*.gen.wgsl` is naga's).

A wire name both ends of which ship in one deploy is ours, and follows these
rules (the search's `dist_m`). A name kept in storage outlives a deploy — a
database column, an object in the bucket, a browser storage key, a baked
file's field — and changes only with its migration or version bump (the
`pxl:object:v15` key). A setting is the exception: storage keeps its `number`,
so its name may change freely.

## Renaming

- Rename through the language server (rust-analyzer, deno lsp, clangd), not by
  text. Where it cannot see (the `rust/client/shader` crate, generated code,
  the `.wgsl` files that read a render setting by its path's name), search,
  and say so.
- A name the language or std looks up by its spelling must not move: the C++
  coroutine protocol, a JS collection's `size`, a wasm import. The language
  server renames them anyway: C++ fails at the first coroutine, but a name
  looked up at run time fails only when it runs.
- A renamed C++ struct field can be taken by a same-named parameter of its own
  method, and the compiler says nothing (`Section::at` read `off + off` until
  `this->off`); the tests catch it. A class's private members end in `_` and
  are not taken so.
- A generated file is changed in its generator, then generated again
  (`cargo spec --write`, `cargo shader-wgsl`).
- Then check what a rename misses: every target (`--all-targets`, the cargo
  features, wasm, iOS and Android), a type literal written apart from its
  type, a macro's body, a build script. `make check` is the end of it.
- The language server renames a name, not its mentions: search the comments,
  the documents and `$doc` for the old name, backticked or not. A backticked
  identifier follows the code; a prose word ("count", "packet") does not.
