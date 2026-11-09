# The commands by module, and names from the field

**2026-11-01** — recorded with the user. Main was at `d40a5bc3f5` when it began
and at `b671249199` when it closed; the work is `8ddea4eee3`, `eeb122b345`,
`8e3798a5df` and `de3c5c264b`. The commits are named as `original` keeps them.

## What was asked

A reader of the published bundle could still name the GPU's ops, the hostcalls,
the storage roots and the division levels, though the const enums were inlined.
The user asked why. That led to the command table: what `WORLD_BOOT` means,
whether its major and its names are right, and from there to every command's
name, the majors themselves, the words around them (`frame`, `space`, `world`,
`map`, `marker`, `pin`), and how xpute reads to someone who would embed it.

## What the user decided

- **A command's major is the module it is for**, as a microkernel's message is
  its server's (2026-11-01, the user's choice). The majors are `SYS`,
  `INPUT`, `SENSOR`, `FS`, `GPU`, `SURFACE`, `SETTING`, `SCENE`, `CAMERA`,
  `PERF`, `EDIT` and `SHELL`; a sensor's command goes to `SENSOR` and an input's
  to `INPUT` whoever sends it. Every command keeps its major as a prefix. An
  application feature is not a major beside the system's parts (`CHAT` alone
  was refused). From 2026-11-04 `SYS`'s, `FS`'s and `GPU`'s commands are xpute's
  predefined ABI, apart from the app's
  ([xpute is the core's way out of JavaScript](./20261104-kernel-and-shell-as-an-operating-system-means-them.md)).
- **Where a name comes from.** A part of the system takes its word from Unix,
  Linux or Wayland; a device outside them keeps its platform's word
  (`SENSOR_PERMISSION`, `INPUT_XR_POSE`); what the program alone does takes a
  plain word. A platform API, an engine or a map library (W3C, Unreal, Mapbox)
  is no authority for a major, and a metaphor no field uses is no name. Carried
  into the naming skill as one paragraph, kept short on the user's word that
  many rules restrict freedom rather than give it.
- **`frame` keeps its meanings**: a ring's message (xpute), a coordinate frame
  (`domain/view/frame/`, a local frame) and a drawn frame (`SURFACE_FRAME`, the
  profiler's `FRAME`). The user: names may overlap, as they do across computer science. The command a
  drawn frame is begun by is `SURFACE_FRAME`, after `KERNEL_TURN`, `KERNEL_VSYNC`
  and `KERNEL_FRAME` were each tried and left: a turn need not carry one, a vsync
  is every refresh, and the kernel is not its owner. `TURN_GRANT` replaced
  `SURFACE_FRAME` on 2026-11-04
  ([xpute is the core's way out of JavaScript](./20261104-kernel-and-shell-as-an-operating-system-means-them.md)).
- **`space` and `frame` stand side by side**: a frame is the origin and axes
  themselves, which move; a space is where a value measured in one lies. The
  move of `domain/view/frame/` to `space/` was taken back the same day.
- **The world, not the map.** "world" is `ARCHITECTURE.md`'s word for what the
  kernel carries and the shell's screen of it; the shell's comments and the
  device app's location prompt say world where they said map.
- **`marker` keeps its two meanings**: the kind of object someone puts up (its
  one shape the pin, `asset/marker.json`) and a scene marker, anything drawn
  flat to the camera over the world. They are qualified only where they meet.
- **Renames the user approved**: profiler as `perf` in the code (the UI's text
  stays), `interaction/` as `edit/` (its writes as `edit/write.rs`), the preview
  surface's role `CASE` as `PREVIEW`, the tool's state `Holding` as `Tool`, a
  marker's waiting as its draft (`ToolState.draft`, `draft_at`, `commit_draft`),
  a design's `words` as `text`, a marker tap's `Went` as `MarkerHit`.
  `interaction` as `interact` was asked about and not taken: a module is named
  by a noun.

  > Since 2026-11-08, the user: the surface's role `PREVIEW` was not theirs
  > and not one they had approved. It is `CHARACTER`, as a world surface's
  > is `WORLD`; preview is the edit's.
- **Gone**: `WORLD_VIEW`, since the world surface's open and close say the same;
  `INPUT_RECT`, since the view's size is the world surface's (`SURFACE_RESIZE`)
  and a pointer is placed from the canvas's top left.
- **The spec's number groups are modules of constants**, read as a namespace of
  the group's name, so the bundle inlines them; a group a reader takes by name
  keeps its names (the GPU's `op` and the profiler's `FRAME`, which the profiler
  shows; the division levels, which the search server sends by name).
  `STRERROR` stays.
- **The tracked pose stays**: the kernel's third camera controller, where an
  immersive mode's head pose (a Quest's, a Vision Pro's) would drive the camera.
  Nothing sends it yet, and its spec and its controller say so.

## What was weighed and not taken

- Moving the command table into `door/`, a module handling its own commands, and
  `pointer.rs` into `control/`: each would have a lower layer name a higher one,
  or one head the other. The door still decodes; only `door/apply.rs` split by
  major.
- `SURFACE_SUSPENDED` for `WORLD_VIEW`: the command was the world surface's
  lifetime, not a hidden surface's state.
- `HOST_*` for the system's commands: the host gives none of pixelet's numbers a
  meaning.

## Proposed, not decided

- **xpute's light layer.** The mechanism hidden as April's was
  (`checkpoint/20260405-rpc`: `on`, `send`, one manifest) and the contract shown
  as Unix shows it (results, errnos, limits). A verb each for the two ways a
  message crosses: the rings, taken later (`send`, a guest's `call` and its
  files, `raise`, `on`), and the direct call, answered before it returns
  (`call`, `call_now`). Deferred by the user. A layer of that name was built
  the same evening without a record of the user's (`e3be4b371` to
  `327465eaf`), beside the base rather than over it, and taken out on
  2026-11-04 (20261104-kernel-and-shell-as-an-operating-system-means-them.md).
  On 2026-11-07 the user took April's one manifest and its names (`Link`, `on`)
  ([what xpute answers for](./20261107-what-xpute-answers-for.md)).
- **Threads.** No use yet; the one candidate the code shows is a glTF's parse,
  unmeasured.

## Open

- The bundle's other names: the module's assert and panic messages and the
  name tables the shell reads (September's "authored half").
