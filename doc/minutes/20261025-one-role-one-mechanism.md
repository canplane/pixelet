# One role, one mechanism: where the client does a job twice

**2026-10-25** — A sweep of the kernel, the renderer, the spec crates and the
host, role by role, for one job done by more than one mechanism, or a shared
mechanism left with a single user. Found, not fixed; each line says the one
mechanism it should be. Items already being changed elsewhere (the markers'
labels, the actors' table, the chunk rung, the clouds' definition, the texture
pack's load number, `variant.json`) are left out.

## Found

1. **Keeping a world answer until a change touches what it read: four
   mechanisms.**
   - `query/changed.rs` `Asked`, whose callers state the columns by hand: the
     companion (`scene/character/companion.rs`), the eraser's carve
     (`control/interaction/ghost.rs`), the navigation's support cache
     (`control/navigation/support.rs` `STOOD`).
   - Hand-rolled answers fed by `Columns` and `World::reading`, which report
     what a query actually read (`query/world.rs`): the puck
     (`scene/indicator/location_puck.rs`), the orbit's terrain gate
     (`control/camera/orbit/terrain_gate.rs`).
   - The edit preview's own, keyed on the store's revision too
     (`control/interaction/edit.rs`).
   - The decor walk's own copy of the written boxes and its own overlap test
     (`scene/decor/drawn.rs` `meets`), beside `changed::within`.
   - Cost: columns stated by hand can be narrower than what was read; the
     companion's floor passes `[at, at]` at a possibly coarse zoom, so an object
     written elsewhere in that cell changes the folded floor without letting the
     kept answer go.
   - One mechanism: `Asked`, taking its columns from `World::reading`.
2. **Reading a kept slot: two transports.** Objects are opened by the kernel on
   the storage root (`layer/object/repository.rs` → `io/file.rs`); settings are
   read by the client and sent inside `WORLD_BOOT` (`door/apply.rs`,
   `setting/store.rs`). The file path's settings half is counted and never read,
   and settings alone carry a size cap. One mechanism: both slots opened on the
   storage root.
3. **Retrying a read against the same server: two policies.** The kernel's
   request scheduler backs off with full jitter and gives up (`io/request.rs`,
   `io/policy.rs`); the client's read of the source declaration doubles to 60 s
   with no jitter and never gives up. The jitter's reason (clients failed by one
   outage should not return together) holds for both. One rule, its numbers in a
   spec.
4. **Tile reads and asset reads part after the open.** A tile's body refused
   room is held as a want (`Wait::Room`); an asset's credit is settled when the
   open answers, its bytes taken later in a task, a refusal kept in the loader's
   own state (`sys/io/loader.rs`). An asset's descriptor so outlives its credit,
   though the file table is sized one descriptor a credit. One mechanism: an
   asset takes its body in `opened` as a tile does.
5. **Registering a model once its glTF lands: two paths.** Decor has its own
   effect, task, settle flag and handle table (`query/decor/asset.rs`,
   `sys/io/loader.rs`, `model/asset.rs` `model_refusal_over`); everyone else
   polls `model_of`. One mechanism: `model_of` as the only table and wake.
6. **A tile's name and version written again in the client.** Morton's name, the
   finest-shard rule and which kinds are sharded are re-implemented in
   TypeScript and held equal to the Rust by a fixture recorded once. One source:
   generated from the spec, as the route already is.
7. **The CPU's instance block named for decor, used by the clouds alone.**
   `scene/decor/cell.rs` `present_block` and `scene/decor/instance.rs` have one
   caller, the clouds; decor is grown and drawn on the GPU. The ghost writes the
   same lane layout by hand, and `5 * count` restates `drawn::LANES`. One home,
   under the clouds or named for what it is, its lanes from `drawn::LANES`.
8. **Draw-group numbering written four times:** pxl-spec `asset.rs`
   `Decor::first_group`, pxl-spec-core `scatter.rs` `GROUPS`/`first_group`,
   `scene/environment/cloud.rs` `FIRST_GROUP`, and `scene/stage.rs`'s zip order.
   One source: `scatter.rs`.
9. **The camera grid's cell reaches the GPU in two words:** the frame uniform's
   `CELL` and the eye words' `TAU`/`GRID`. The camera gizmo draws from one, the
   cast reads the other. One word.
10. **`layer::Query` is described as every layer's seam and read generically by
    nothing:** two implementations, always called on their concrete types, and
    places, divisions and the Earth implement none. Either a generic reader
    takes it or the docs say it is these two.
11. **Smaller.**
    - A redraw flag beside `ctx.invalidate()` that only repeats it
      (`scene/stage.rs`, `turn.rs`).
    - `io/source.rs` `past_at` calls itself the one judgment for what is drawn,
      walked, edited and said, and has one reader.
    - The division layer's pivot line enters as a destination (`turn.rs`,
      `io/demand.rs` `Why::Destination`).
    - Stream to layer written by hand in several places (`door.rs`,
      `view/demand.rs`, `turn.rs`).
    - `layer/instance.rs` `Meta` keeps generality for one kind.
    - Cache-line rounding written twice (`layer/terrain/fixed.rs`,
      `layer/division.rs`).

## Found to be one mechanism

The turn's edge and tick (`sched/`) against io's request scheduler, which are
different jobs; residency, its floor and demand; the kept packets
(`layer/kept.rs`); the service area's rules (pxl-spec-core `area`); the
renderer's compare-last-input; the text layout.

## Order

Items 1, 4, 5, 7 and 8 sit in what the kernel's folders and the asset kinds
change
([what is drawn is an asset in a frame](./20261024-what-is-drawn-is-an-asset-in-a-frame.md)),
and are taken with that work. The rest stand alone.

> Since taken: 1, `Asked` reads its columns off `World::reading`, and the puck
> and the terrain gate keep their answers through it; the edit preview's check
> and decor's written boxes keep their own, as they stand on reads
> `World::reading` does not report. 4, an asset's bytes are taken in `opened`.
> 5, decor registers through `model_of` alone. 7, the lanes are
> `domain/scene/lane.rs`, the by-size draw the clouds'. 8, the groups are
> numbered by `scatter.rs`.

> Since 2026-10-26, 2 is taken: the settings slot is opened on the storage root
> as the objects' is, and `WORLD_BOOT` carries no settings (`door/apply.rs`,
> `view/setting/store.rs`). 3 in part: the client's read of the source
> declaration draws its wait with full jitter
> (`ts/client/src/namespace/source.ts`), but still never gives up, and its
> numbers are its own.

> Since 2026-10-28, 9 in part: the eye words' τ is gone (`2a43c327a`); the
> cell still reaches the GPU twice, as the frame uniform's `CELL` and the
> grid's `s` (`eye::GRID`). The code 5 and 7 cite (`model_refusal_over`,
> `scene/decor/cell.rs` `present_block`, `scene/decor/instance.rs`) and 11's
> `layer/instance.rs` `Meta` are gone.
