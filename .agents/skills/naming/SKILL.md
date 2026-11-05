---
name: naming
description: Naming in pixelet — short, plain names read where they stand; where a word comes from; the abbreviations and the words that stay whole; the names chosen with the user; names someone else owns; renaming. Use before naming, renaming or reviewing names, in code, spec JSON or a backticked identifier in a document.
---

# Naming

- Use short, plain names, as in UNIX V6 and K&R's C.
- The system — xpute, the commands' modules, what the kernel asks of its host
  — takes its word from Unix, Linux or Wayland; a device outside them keeps its
  platform's word. The application — the world, its drawing, its map, its UI —
  takes the word its own field uses (graphics, cartography, games). A metaphor
  no field uses is no name.
- A word on this list is abbreviated; any other stays whole: index `idx`,
  value `val`, values `vals`, buffer `buf`, source `src`, destination `dst`,
  offset `off`, count `cnt`, current `cur` (not `curr`), position `pos`,
  direction `dir`, directory `dir`, pixel `px`, resolution `res`, distance
  `dist`, address `addr`, capacity `cap`, dimension `dim`, memory `mem`,
  component `comp`, channel `ch`, elevation `ele`, feature `feat`, latitude
  `lat`, longitude `lon`, radian `rad`, degree `deg`, orientation `orient`,
  rotation `rot`, altitude `alt`, error `err`, packet `pkt`, initial `init`,
  matrix `mat`, configuration `cfg`, descriptor `desc`, context `ctx`. `pkt` is
  only a wire packet.
- Whole, though they look short: `zoom`; `size` in pixelet, GPU texture sizes
  included, and `sz` in xpute (`HDR_SZ`, `WORD_SZ`); `environment`; `source` as
  a source of data; `encode`, `decode`.
- Name what the thing is. A padding noun says little, and a placeholder word
  says nothing: one that could stand for anything, as "whatsit" could (`kept`,
  `held`, `goes`, `done`), which only the code around it explains (`Stop`, not
  `Held`). A plain word that reads at once (`reach`) is fine. A word in a
  comment is no precedent for a name.
- Mind a loose word where several modules use it for different things.
- Snake_case and SCREAMING abbreviate; PascalCase types, camelCase and the
  React shell's own names stay whole (`let cfg: Config`).
- In TypeScript, a `private` member takes a leading `_` (`private _cap`,
  `private _grow()`), `protected` is for a class that is extended, and no
  field is `#`.
- Use a plural for a collection, and a unit suffix on a length (`_u` the
  world's units, `_merc` a meter on the EPSG:3857 plane, `_m` a meter on the
  ground), on the value where a person sees it (`29.0 m`).
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
  | a frame's number on xpute's rings, its meaning the program's | a `cmd`, a `Command` | vec, vector, opcode |
  | what the shell asks of the kernel | a command | |
  | a guest's call into its host, run now or after the turn | a hostcall | system call, hypercall |
  | the number a frame names at the guest's far side, 0 the host | a port | endpoint, worker id |
  | a node of the plane at a zoom | `Quadrant`, `quad_at` | Tile |
  | a node under a shard, through the height as well | `Octant` | EntryAt |
  | a quadrant fetched and stored as one, and what it holds | a tile (`TerrainTile`, `TileCache`) | |
  | a node's name as text, a quadkey or an octkey | its `key` | |
  | a node's name as a number, its morton code | its `id` | subject, morton |

  `at` is where a thing is, `to` where it is going.
- Keep WebGPU's words in xpute's `gpu/`.
- Treat a name between the client and a server as protocol, and a name kept in
  storage (a bucket object, a storage key, a baked file's field) as needing a
  version bump to change; a setting's name can change, since storage keeps its
  `number`.
- When renaming, search what the language server doesn't see: generated
  code and the `.wgsl` files, which read pxl-engine's layouts and a render
  setting by their names.
- Watch a renamed C++ struct field being taken by a same-named parameter of its
  own method (`Section::at` read `off + off` until `this->off`).
- Let a backticked identifier in a comment or document follow the code; leave
  a prose word as it is.
