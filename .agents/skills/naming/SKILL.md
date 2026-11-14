---
name: naming
description: Naming in pixelet — short, plain names read where they stand; where a word comes from; the abbreviations and the words that stay whole; the names chosen with the user. Use before naming, renaming or reviewing names, in code, spec JSON or a backticked identifier in a document. How the code itself is shaped is the code skill's.
---

# Naming

- Use short, plain names, as in UNIX V6 and K&R's C.
- The system (xpute, the commands' modules, what the guest asks of its host)
  takes its word from Unix, Linux or Wayland; a device outside them keeps its
  platform's word. The application takes its field's word (graphics, cartography,
  games). A metaphor no field uses is no name.
- In a name of the mechanism, a word on this list is abbreviated, in every
  casing (`asset_cnt`, `assetCnt`), and any other stays whole: index `idx`, value `val`, buffer `buf`, source `src`, destination `dst`,
  offset `off`, count `cnt`, current `cur` (not `curr`), position `pos`,
  direction `dir`, directory `dir`, pixel `px`, resolution `res`, distance
  `dist`, address `addr`, capacity `cap`, dimension `dim`, memory `mem`,
  component `comp`, channel `ch`, elevation `ele`, feature `feat`, latitude
  `lat`, longitude `lon`, radian `rad`, degree `deg`, orientation `orient`,
  rotation `rot`, altitude `alt`, error `err`, packet `pkt`, initial `init`,
  matrix `mat`, configuration or config `cfg`, descriptor `desc`, context `ctx`,
  coordinate `coord`, declaration `decl`, definition `def`, sequence `seq`,
  atmosphere `atmos`, record `rec`, header `hdr`, command `cmd`, vector `vec`,
  condition `cond`, resource `resrc`, array `arr`, constructor `ctor`, previous
  `prev`. A plural
  is `vals`, `bufs`, `offs`, `cnts`, `idxs`, `recs`, `hdrs`, `cmds`, `vecs`,
  `conds`, `resrcs`, `arrs` or `ctors`, and whole otherwise. `pkt` is only a wire
  packet; `rec` only the noun, and to record stays whole. A condition variable
  is `cv`.
- Whole, though they look short: `zoom`; `size` in pixelet, GPU texture sizes
  included, and `sz` in xpute (`HDR_SZ`, `WORD_SZ`); `environment`; `source` as
  a source of data; `encode`, `decode`.
- Name what the thing is now: what it holds, does and which boundary it crosses.
  A placeholder says nothing (`kept`, `held`, `goes`, `done`; `Stop`, not
  `Held`); a plain word that reads at once (`reach`) is fine. A word in a comment
  or an earlier module's use is no reason for a name.
- How exact a name must be follows how far it is read. Inside one module it may
  be loose; across modules it says what the thing is, never what happened to it
  (`handed`, `asked`, `told`). A rename that lengthens a name its context made
  plain goes back.
- A word may mean different things where each is its field's word (a map's tile,
  a shadow map's tile). A name changes when it is not its field's word or not
  what the thing is, not to keep words apart.
- The same thing has the same name in every type, whatever its source calls
  it: a row's class is its `kind`, the coarsest zoom it shows at its
  `first_zoom`, a grid with a value a cell its `cells`.
- Spelling is American (`color`, `behavior`, `center`). A word copied from
  outside keeps its own (OpenStreetMap's `community_centre`).
- Casing, generated names too: functions, variables and fields in snake_case,
  types in Pascal, constants in SCREAMING, in Rust, C++ and TypeScript alike, as
  Svelte writes its own code; so a spec's name is one spelling in JSON, Rust and
  TypeScript. Kotlin and Swift keep their own casing. Only what React requires
  takes React's form: a hook's `use` prefix, a component, a prop. A type spells
  its words whole (`let cfg: Config`), and so does the name of a thing that is
  there only for the UI to show or handle, wherever it is defined (a toast's
  tone `ERROR`, the overlay's sensor status `error`, `toast_count`); a name of
  the mechanism keeps its form where the UI reads it (`ray_cnt`). A generated helper of a type is its verb, an
  underscore and the type's own spelling (`store_LonLat`, `of_Tool`), so the
  type's name finds it. A name copied from an outside API keeps that API's
  (`with_capacity`, `memory_hints`, `devicePixelRatio`).
- A file or module is named for its layer: a system one takes its field's short
  word (`sys`, `io`, `mem`, `abi`), a domain one the whole word (`scene`,
  `query`, `count`), and this list alone renames neither.
- A symmetric pair's part is the base, a dot and its side (`x.native.ts`,
  `sample.eye.wgsl` beside `sample.wgsl`); a stand-in replacing a part at
  assembly is the part, a dot and what it stands in as (`fold.none.wgsl`).
- Name a new concept the parts share with the user, and use the names already
  chosen with the user:

  | thing | name | not |
  | --- | --- | --- |
  | where the camera looks from | `eye` | camera position |
  | the point the camera turns about (the eye itself in first person) | `pivot` | focus, center, target |
  | where the pivot is going | its `to` (`pivot_to`, `put_pivot`, `resolve_to`) | target |
  | the direction the camera faces | `facing` | look |
  | the camera modes | `orbit`, `first_person` (the UI says 1인칭/3인칭) | free |
  | what a mouse, touch or controller points with | `pointer` | cursor |
  | the raster's nearest sample at each pixel | the visibility buffer | pick |
  | a frame's word naming its target handle and method | its `cmd` | vec, vector, opcode |
  | a function of an interface the other side serves (the UI calls the core's exports, the guest its imports) | a method | command, hostcall, system call, hypercall |
  | a method answered before it returns | a call | hostcall |
  | a node of the plane at a zoom | `Quadrant` (`Quadrant::new`) | Tile |
  | a node under a shard, through the height as well | `Octant` | EntryAt |
  | a quadrant fetched and stored as one, and what it holds | a tile (`TerrainTile`, `TileCache`) | |
  | a node's name as text, a quadkey or an octkey | its `key` | |
  | a node's name as a number, its morton code | its `id` | subject, morton |
  | a zoom told by its cells | `cell_zoom` | cell |
  | a level of a brick's min-max pyramid | a `mip` | level |
  | a cell's four asset ids in one word, its 2×2 one zoom finer | its `fold` (`folds`, `fold_at`) | palette |
  | how high the ground is above sea level, stored or worked out | `ele` | height |
  | how high a thing in the air is above sea level: a device's, an eye's, a cloud's | `alt` | height |
  | a thing's size up, how far it stands over a surface, or the Z a bridge or a tunnel runs at | `height` | |
  | a value that differs from the frame before | `changed` | moved |
  | a box on a plane | a `rect` | box |
  | the most a thing holds | its `cap` | room, most, limit |
  | a buffer's fixed first words | its `hdr` | head |
  | the pixels a change covers, to be drawn again | the `damage` | mark, claimed |
  | what a player places in the world | an `object` | overwrite |
  | where a ray ends | its `hit` | stop |
  | decor drawn as a model, an impostor or not at all by its distance | its `lod` | tier |

  `at` is where a thing is, `to` where it is going.
- xpute's names are the mechanism's, as the component model has them: the
  guest, the host that serves it, the UI, a device and a frame. Two are
  outside the standard: the `link`, xpute's own word, the protocol both sides
  run, and its code; and the runtime, the platform's execution environment the
  host runs in, as the JS field has it (a browser, Node). xpute's tree is laid
  out by platform: `link/`, `guest/`, `web/host`, `web/runtime`, `native/host`
  and `native/runtime`, and the GPU's `gpu/link`, `gpu/guest`, `gpu/web/host`,
  `gpu/native/host` and `gpu/native/runtime`. pixelet's core is its guest.
  pixelet names its parts by the side of xpute they sit on, the owner in the
  prefix (`pxl_core`, `xpute_guest`, `@xpute/web-host`).
- A name is singular: a file, a folder, a type, an enum and its members, a
  function, a variable and a constant (`service/`, `remote.ts`, `component/`).
  It is plural only when the thing holds many of what it names, read by
  meaning, not by type: a column is plural, the row read from it singular, and
  one range's first and last is a `range` or singular (`GROUND`).
- A quantity is named by its form: a count ends in `_cnt` (`LAYER_CNT`), a cap
  in `_cap` (`CREDIT_CAP`), a size in bytes in `_sz` in xpute (`HDR_SZ`), and
  any other size or length in its unit (`HDR_WORDS`, `PAGE_BYTES`, `_ms`, `_px`; `_u` the world's units, `_merc` a meter on the
  EPSG:3857 plane, `_m` a meter on the ground), the unit shown where a person
  reads the value (`29.0 m`). A length takes its unit where it crosses into
  what does not say it (a prop `spanPx`, a dp value `layoutLiftDp` before it
  becomes pixels); where its context says it, as a UI style's width, it stays
  bare.
- A predicate's call reads as a yes-or-no assertion about its subject
  (`v.is_ground()`, `pool.fits(n)`, `list.contains(x)`).
