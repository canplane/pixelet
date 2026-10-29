# A name is the field's, and abbreviates by default

**2026-10-29** — recorded with the user over the day's renames, which two
sessions carried out: the abbreviation sweep and the convention renames. Written
against main at `bfa17c7d2`. The rules themselves are the `naming` skill's
(`.agents/skills/naming/SKILL.md`), which binds; this records why each one is
what it is. What is said here of PROJ, OpenStreetMap, PostGIS, D3D12, WebGPU,
Unix and Linux is general knowledge, not read from their sources for this
record.

> Follow convention, and stop making names up. Abbreviations are the right
> default. Whether a word is domain or system is what matters: a name that
> could really be reused elsewhere, that is not about this Minecraft-like
> world, is abbreviated. TypeScript is camelCase, where abbreviations read
> awkwardly, so there they are for snake_case alone.

## What was asked

- Agents had given one thing several names of their own making: the point the
  camera turns about was a focus, a center and a target in different files;
  the pointer was a cursor; the raster's nearest sample was a pick. The user
  asked that a name be the one the field uses, or the one the April checkpoint
  (`origin/checkpoint/20260405-rpc`) used, and never one an agent coins.
- A comparison with that checkpoint showed the code had drifted from its
  abbreviations (`idx`, `val`, `buf`, `cnt`) to whole words. The user chose the
  abbreviations as the default and asked for the existing code to be brought
  to them, in pixelet and in xpute, the spec's JSON included.

## What was resolved, and why

### Where a name comes from

A name is the field's word (graphics, GIS, game cameras, WAI-ARIA, a language's
idiom) or the checkpoint's, and a proposal names which. A thing with no
precedent is named with the user. The camera's names were settled this way:
`eye` and `pivot`, the `orbit` and `first_person` modes, a `pointer`, the
visibility buffer a pixel `take`s its sample from, and `at` and `to` for where
a thing is and where it is going.

### The abbreviations

- **A closed list.** Only the checkpoint's abbreviations and the ones the user
  added are used. An open rule ("abbreviate what is long") would have every
  agent inventing its own (`num`, `msg`, `cfg` before anyone chose it).
- **Plurals as the checkpoint had them.** It wrote `vals` but kept `counts`,
  `offsets` and `positions` whole, and so does the code; `sizes` has no short
  form either. Unix and Linux abbreviate a count in the singular (`argc`,
  `iovcnt`, `refcnt`) far more often than in the plural.
- **`err`, not `error`.** The checkpoint used `err` more than twice as often as
  `error` in xpute.
- **`cur`, not `curr`;** **`init`** for initial, `INIT` for `INITIAL`.
- **`orient`, `rot`, `pos`, `dir`, `alt`** sit beside `lat` and `lon` because
  they read as naturally as those already did.
- **`lon`, not `lng`.** PROJ, OpenStreetMap, PostGIS and the spec all say
  `lon`; the checkpoint's `lng` came from web map libraries. i18next's `lng` is
  a language and stays.
- **`pkt` only for a real wire packet.** Every `packet` in pixelet's code turned
  out to be an XTP packet, a command's arguments or a baked tile's bytes, so all
  of them became `pkt`. A coined name that merely contains the word is judged by
  what it names, not by the word.
- **`cfg`** for a configuration and **`desc`** for a descriptor were added
  later the same day. `desc` follows D3D12's `_DESC`, wgpu's `desc` and XTP's
  descriptor word; SQL's `DESC` means descending and is SQL's.

### Words that stay whole

- **Types.** A binding abbreviates and its type stays whole (`let idx: Index`,
  `let cfg: Config`). A generated type is the one exception: the generator
  makes a type of an enum's name, so the type follows the name (`RES_MODES`
  gives `ResMode`).
- **`zoom`,** because `z` is the coordinate.
- **`environment`, and `source` where it means a source of data.** The tile
  source's command was briefly `WORLD_SRC` and went back to `WORLD_SOURCE`: it
  names where the world's data comes from, not a pointer to read from.
- **`encode` and `decode`,** and `control`, the module's name.

### `size` and `sz`

The user's first thought was `size` everywhere, as recent code tends to write
it. They then kept `sz` for what is truly binary and low level, and put the
test as domain against system: a word that could be reused outside this world
is system. A GPU texture's pixel size the user put with `size`. So all of
pixelet writes `size`, GPU textures included, and xpute, which is system
code throughout, writes `sz` (`HDR_SZ`, `WORD_SZ`). A section's list of byte
lengths is named after the field it fills, `bytes`, rather than given a plural
`szs` the checkpoint never had.

### By language

- **TypeScript's camelCase stays whole,** for the reason the user gave. Its
  snake_case and SCREAMING names abbreviate, and so do lone words in code
  written in snake style: xpute's TypeScript, the apps and `abi/`. Two names the
  user pointed at set that: `altitude` in `kernel_pos` became `alt`, and an
  app's guest hands over its linear memory as `mem`, the name the guest gives
  it, rather than as a `buffer` or `buf`.
- **The React shell keeps whole words** for the names it makes. Counted, most
  of its candidate words were React's or React Native's (`.current` was 96 of
  its 102 uses of `current`, `position` a style's), so abbreviating the few of
  its own would have set them against their camelCase neighbors. What comes from
  the kernel or the spec it reads as it is (`s.location.err`).
- **xpute's names match across Rust, TypeScript and C++** (`pkt_at`,
  `Frame.cnt`, `Section.off`), since its conformance traces hold the three to
  one contract.

### Someone else's names

What the language, a library or the platform defines keeps its word. C++'s
coroutine members are looked up by their spelling: when the sweep renamed
`initial_suspend` and `return_value`, the first coroutine stopped compiling.
The same holds for a JavaScript collection's `size`, a TypedArray's `buffer`,
the wasm import `env.memory`, xpute's `gpu/` (WebGPU's vocabulary), OSM Carto's
`feature`, and the render names that flow into WGSL.

A wire name is ours when both of its ends ship in one deploy; the search's
`distance_m` became `dist_m` on the wire too. A name kept in storage outlives a
deploy and changes only with its migration or version bump; a setting is the
exception, since storage keeps its number.

### What was renamed with the code

Comments, documents and the records under `doc/` follow a rename, as the user
asked when the sun's map became the shadow map. A language server renames a
name and not its mentions, and the sweep at first left 20 old names in
comments and minutes (`CELLS_A_RADIAN`, `wrap_index`, `index_of` among them),
brought up to date in `2733e41d1`.

## What a rename misses

A rename through a language server compiled clean and was still wrong in these
places, each found by a check that came after it:

- a type literal written apart from its type: `(out as { value: T }).value` in
  the doorbell kept the old field and would have read `undefined`;
- a member reached through a template parameter in C++, which clangd did not
  follow;
- a macro's body, a build script, and code behind a cargo feature (geo-bake's
  `serve`), which rust-analyzer did not see;
- a C++ struct's field taken by a same-named parameter of its own method:
  `Section::at` returned `off + off` until it read `this->off`, and only the
  tests caught it.

## The commits

The abbreviation sweep runs from `3000c7628` to `e31a101da`: pixelet's Rust,
the shaders and the generators, then `size`, `pkt`, `err`, `alt`, xpute's Rust,
TypeScript and C++ (`ed8e386d4`, `2294b51e6`, `d1d149f3e`), and the documents.
The convention renames are `adbe1d0a8` (the pivot), `8e937c31a` (the visibility
buffer), `85b8a31d2` (`merc`), `1cf9f5c10` (`lon`) and `e955a50b8` (imports).
The skill is `2593e1fac`, cut to its role in `bfa17c7d2`.
