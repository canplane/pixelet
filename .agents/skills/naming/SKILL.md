---
name: naming
description: Naming in pixelet — short, plain names read where they stand; where a word comes from; the abbreviations and the words that stay whole; the names chosen with the user. Use before naming, renaming or reviewing names, in code, spec JSON or a backticked identifier in a document. How the code itself is shaped is the code skill's.
---

# Naming

- Use short, plain names, as in UNIX V6 and K&R's C.
- The system (xpute, the commands' modules, what the guest asks of its host)
  takes its word from Unix, Linux or Wayland; a device outside them keeps its
  platform's word. The application takes its field's word (graphics, cartography,
  games). A metaphor no field uses is no name. A word in prose, a comment's or a
  document's, follows the same rule and says what the code names: the platforms
  are web and native, as `x.web.ts`, `runtime/native` and React Native's
  `Platform.OS` have them; a page is the HTML document and a device the hardware
  or the GPU's.
- In a snake_case or SCREAMING name of the mechanism, a word on this list is
  abbreviated, standing alone too (`asset_cnt`, `ASSET_CNT`, `idx`), and any
  other stays whole. pixelet's own TypeScript names its camelCase names and
  its types by TypeScript's general convention, as Google's TypeScript style
  guide writes it, and this list does not reach them (`assetCount`); an
  acronym is a word there, as in Rust (`customerId`, never `customerID`). The
  list: index `idx`, value `val`, buffer `buf`, source `src`, destination `dst`,
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
- Whole, though they look short: `zoom`; `size` for a size that is not in
  bytes (a GPU texture's, in texels); `environment`; `source` as a source of
  data; `encode`, `decode`.
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
- Casing: types in Pascal and constants in SCREAMING everywhere. Functions,
  variables and fields are snake_case in Rust and C++, and in xpute's
  TypeScript, as Svelte writes its own code: its kit, link, guest, host,
  runtime and UI end, a plugin's own of them (`ts/gpu`), and what its
  generators write (`abi/`), so a spec's name is one spelling in JSON, Rust and
  TypeScript and keeps it wherever it is read. pixelet's own TypeScript, the
  application over xpute, writes them in camelCase, as an application over
  Svelte does, its words as the abbreviation rule says. Kotlin and Swift keep their own casing. The TypeScript kit's
  word casts and bit helpers keep the capitals of a system's macros (`U32`,
  `BIT`, `FIELD_GET64`), where Rust and C++ write theirs as functions in
  snake_case. Only what React requires takes React's form: a hook's `use`
  prefix, a component, a prop. A type spells its words whole, an acronym as
  a word (`Config`, `AssetId`), and so does the name of a thing that is there
  only for the UI to show or handle, wherever it is defined (a toast's tone `ERROR`, a
  sensor's status `error`, `toastCount`); a name of the mechanism keeps its
  form where the UI reads it (`ray_cnt`). A generated helper of a type is its verb, an
  underscore and the type's own spelling (`store_LonLat`, `of_Tool`), so the
  type's name finds it. A name copied from an outside API keeps that API's
  (`with_capacity`, `memory_hints`, `devicePixelRatio`), and so does a name
  another side reads by its spelling: a key of a format kept or sent (a
  server's JSON, a stored value, the perf trace), a method a native module
  binds (`storage_get`).
- A file or module is named for its layer: a system one takes its field's short
  word (`sys`, `io`, `mem`, `abi`), a domain one the whole word (`scene`,
  `query`, `count`), and this list alone renames neither.
- A symmetric pair's part is the base, a dot and its side, on both halves
  (`x.web.ts` beside `x.native.ts`,
  `location.uniform.wgsl` beside `location.slot.wgsl`); a body both sides
  take is the plain base (`sample.wgsl` under `sample.eye.wgsl` and
  `sample.light.wgsl`); a stand-in replacing a part at assembly is the part, a
  dot and what it stands in as (`fold.none.wgsl`).
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
  | a function of an interface the other side implements (the UI calls the core's exports, the guest its imports) | a method | command, hostcall, system call, hypercall |
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
- xpute's names are the mechanism's, as the component model has them: the guest,
  the host that implements its imports, the UI, a device and a frame. Two are
  outside the standard: the `link`, xpute's own word, the protocol both sides
  run, and its code, named for the data link layer (L2): it knows its two
  ends and not the wire under them, and what crosses it is a `frame`; and the
  runtime, the platform's execution environment the
  host runs in, as the JS field has it (a browser, Node). xpute's words for
  its parts are `kit`, `link`, `guest`, `host`, `runtime`, `expo` (React
  Native's UI end), `gen`, `tool` and `golden`. A plugin repeats the link, the
  guest, the host and the UI end under its own name. pixelet's core is its guest, and pixelet names its
  parts by the side of xpute they sit on (`pxl-core`, `pxl-runtime`).
- An xpute crate's or package's name is its folder: `xpute-` or `@xpute/`,
  then the path under `xpute/` with `/` as `-` and its language's leaf left
  out, one name in every language. A plugin's name stands first, as an owner's;
  then the base word, and its qualifier after (`runtime/web` is
  `@xpute/runtime-web`, `host/rust` is `xpute-host`). No folder only
  groups. A name says what a part depends on that its siblings do not: a
  shell's (`expo`), a platform's (`web`, `native`).
- A name is singular: a file, a folder, a type, an enum and its members, a
  function, a variable and a constant (`service/`, `remote.ts`, `component/`).
  It is plural only when the thing holds many of what it names, read by
  meaning, not by type: a column is plural, the row read from it singular, and
  one range's first and last is a `range` or singular (`GROUND`).
- A quantity is named by its form: a count ends in `_cnt` (`LAYER_CNT`), a cap
  in `_cap` (`CREDIT_CAP`), a size in bytes in `_sz` in both trees (`HDR_SZ`,
  `PAGE_SZ`), in camelCase a count and a cap spelled whole and a size in bytes
  by its unit (`frameCount`, `uploadBytes`), and any other size or length in
  its unit (`HDR_WORDS`, `_ms`,
  `_px`; `_u` the world's units, `_merc` a meter on the
  EPSG:3857 plane, `_m` a meter on the ground), the unit shown where a person
  reads the value (`29.0 m`). A length takes its unit where it crosses into
  what does not say it (a prop `spanPx`); where its context says it, as a UI
  style's width, it stays bare.
- A predicate's call reads as a yes-or-no assertion about its subject
  (`v.is_ground()`, `pool.fits(n)`, `list.contains(x)`).
