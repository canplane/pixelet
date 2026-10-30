---
name: naming
description: How a name in pixelet is chosen and spelled — a convention's word; abbreviations by default and the words that stay whole; the names chosen with the user; case by language and kind; bindings abbreviated, types whole; plurals, units, and names someone else owns; and how a rename is carried out. Use before naming or renaming anything, in code, spec JSON or a backticked identifier in a document.
---

# Naming

A name says what the thing is called where it comes from. Before naming or
renaming, find that word: what the field calls it (graphics, GIS, game
cameras, WAI-ARIA, the language's own idiom). Use it, and say in the proposal
where it comes from. A thing with no precedent is named with the user.

## Abbreviate by default

A word on this list is abbreviated; any other stays whole:

index `idx`, value `val`, values `vals`, buffer `buf`, source `src`,
destination `dst`, offset `off`, count `cnt`, current `cur` (not `curr`),
position `pos`, direction `dir`, pixel `px`, resolution `res`, distance
`dist`, address `addr`, capacity `cap`, dimension `dim`, memory
`mem`, component `comp`, channel `ch`, elevation `ele`, feature `feat`,
latitude `lat`, longitude `lon`, radian `rad`, degree `deg`, orientation
`orient`, rotation `rot`, altitude `alt`, error `err`, packet `pkt`, initial
`init`, matrix `mat`, configuration `cfg`, descriptor `desc`, context `ctx`.

`pkt` is only a wire packet (an XTP packet: a command's arguments, a baked
tile's bytes).

## Whole, though they look short

- `zoom`.
- `size` in pixelet, GPU texture sizes included; `sz` in xpute (`HDR_SZ`,
  `WORD_SZ`).
- `environment`.
- `source` as a source of data (`WORLD_SOURCE`, `of_source`).
- `encode`, `decode`.

## Names chosen with the user

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
TS, the apps, `ts/client/src/abi/`). The React shell (`ts/client/src/shell/`) keeps
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
`Write`, `Occupancy`, the object layer's `OBJECT`).

Of the plurals only `vals` abbreviates; `counts`, `offsets`, `positions` and
`sizes` stay whole.

## Units

A length says its unit as a suffix: `_u` the world's units, `_merc` a meter on
the EPSG:3857 plane, `_m` a meter on the ground
(`pxl-spec space/coord/adapter/3857.rs`). `_mat` is a matrix. A unit shown to a person goes on the value, not the label
(`120.0 u`, `29.0 m`).

## What is someone else's

xpute's `gpu/` keeps WebGPU's words; i18next's `lng` (a language) stays.

A wire name both ends of which are one build (the client's host and kernel)
follows these rules. A name between the client and a server is a protocol: a
client already loaded keeps talking to the next server, so a rename there is a
protocol change, made as one (the server takes the new name beside the old
before the client moves to it). No policy says how long an old client lives, so
when the old name goes is the user's call. A name kept in storage (a database
column, a bucket object, a browser storage key, a baked file's field) changes
only with a migration or version bump. A setting's name may change: storage
keeps its `number`.

## Renaming

- The language server does not see the `rust/client/shader` crate, generated
  code, or the `.wgsl` files that read a render setting by its path's name:
  search those.
- A renamed C++ struct field can be taken by a same-named parameter of its own
  method, and the compiler says nothing (`Section::at` read `off + off` until
  `this->off`).
- A backticked identifier in a comment or document follows the code; a prose
  word ("count", "packet") does not.
