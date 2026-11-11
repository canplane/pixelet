---
name: naming
description: Naming in pixelet — short, plain names read where they stand; where a word comes from; the abbreviations and the words that stay whole; the names chosen with the user. Use before naming, renaming or reviewing names, in code, spec JSON or a backticked identifier in a document.
---

# Naming

- Use short, plain names, as in UNIX V6 and K&R's C.
- The system (xpute, the commands' modules, what the core asks of its runtime)
  takes its word from Unix, Linux or Wayland; a device outside them keeps its
  platform's word. The application takes its field's word (graphics, cartography,
  games). A metaphor no field uses is no name.
- In snake_case and SCREAMING, a word on this list is abbreviated and any other
  stays whole: index `idx`, value `val`, buffer `buf`, source `src`, destination `dst`,
  offset `off`, count `cnt`, current `cur` (not `curr`), position `pos`,
  direction `dir`, directory `dir`, pixel `px`, resolution `res`, distance
  `dist`, address `addr`, capacity `cap`, dimension `dim`, memory `mem`,
  component `comp`, channel `ch`, elevation `ele`, feature `feat`, latitude
  `lat`, longitude `lon`, radian `rad`, degree `deg`, orientation `orient`,
  rotation `rot`, altitude `alt`, error `err`, packet `pkt`, initial `init`,
  matrix `mat`, configuration or config `cfg`, descriptor `desc`, context `ctx`,
  coordinate `coord`, declaration `decl`, definition `def`, sequence `seq`,
  atmosphere `atmos`, record `rec`, header `hdr`, command `cmd`, vector `vec`,
  condition `cond`. A plural is `vals`, `bufs`, `offs`, `cnts`, `idxs`, `recs`,
  `hdrs`, `cmds`, `vecs` or `conds`, and whole otherwise. `pkt` is only a wire
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
  `first_zoom`, a grid with a value a cell its `cells`. A column holds many and
  is plural; the row read from it is singular.
- PascalCase types, camelCase and the React UI's own names stay whole (`let
  cfg: Config`). A name that comes up from the core or xpute keeps its spelling
  in the UI (`cnt`, `lat`), and a name copied from an outside API keeps that
  API's (`with_capacity`, `memory_hints`).
- A file or module is named for its layer: a system one takes its field's short
  word (`sys`, `io`, `mem`, `abi`), a domain one the whole word (`scene`,
  `query`, `count`), and this list alone renames neither.
- Use a unit suffix on a length (`_u` the world's units, `_merc` a meter on
  the EPSG:3857 plane, `_m` a meter on the ground), on the value where a
  person sees it (`29.0 m`).
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
  | a frame's number between xpute's core and runtime, its meaning the program's | a `cmd`, a `Command` | vec, vector, opcode |
  | what the UI asks of the core | a command | |
  | a core's command to its runtime, a frame it sends | an import | hostcall, system call, hypercall |
  | an import answered before it returns | a call | hostcall |
  | a node of the plane at a zoom | `Quadrant`, `quad_at` | Tile |
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
  | a value that differs from the turn before | `changed` | moved |
  | a box on a plane | a `rect` | box |
  | the most a thing holds | its `cap` | room, most, limit |
  | a buffer's fixed first words | its `hdr` | head |
  | the pixels a change covers, to be drawn again | the `damage` | mark, claimed |
  | what a player places in the world | an `object` | overwrite |
  | where a ray ends | its `hit` | stop |
  | decor drawn as a model, an impostor or not at all by its distance | its `lod` | tier |

  `at` is where a thing is, `to` where it is going.
- xpute's names are the mechanism's: the core, the runtime, the UI, a device, a
  frame and the host. pixelet names its parts by the side of xpute they sit on,
  the owner in the prefix (`pxl_core`, `xpute_core`, `Runtime.core`).
- A file, folder or type name is singular (`service/`, `remote.ts`,
  `component/`).
