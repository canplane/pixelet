# Where the migration moved the files

**2026-09-17** — The Rust core was meant to take the client's TypeScript at
its paths. It did not everywhere: `crates/pxl/core/src` has a `world/` beside
`module/world/` with seven names in common, a `bake/` that shares a name and
not a meaning with `pxl-shared`'s, and single TypeScript files split across
two or three Rust files in different directories.

This records, file by file, where the TypeScript of the last checkpoint before
the move ended up, what appeared with no TypeScript behind it, and which
concepts now have two homes. Nothing is moved here.

## How it was compared

The source is `origin/checkpoint/20260912-linear-memory`, read with
`git ls-tree` and `git show` without checking it out. It has no Rust at all.
The target is the working tree as of `91aa011`.

- A path is the same when `apps/web/src/X.ts(x)` has `crates/pxl/core/src/X.rs`,
  or `packages/pxl/shared/src/X.ts` has `crates/pxl/shared/src/X.rs`. 140 files
  match that way.
- 57 checkpoint files are still TypeScript at the same path and are not
  counted.
- For the rest, each TypeScript file's `export`ed function, class, constant and
  type names were snake-cased and looked up among the definitions in
  `crates/pxl/core`, `crates/pxl/shared` and `crates/xpute/core`. The counts
  below are matched names over exported names.
- A directory's module file (`decor.rs` for `decor/`) is not a new file.
- The history between the checkpoint and now was not read. Where names did not
  match and the placement below rests on what the files say they do, it is
  marked as inferred.

## Top-level directories the checkpoint did not have

### `world/`, beside `module/world/`

There was no `apps/web/src/world/`. What was in `module/world/*` and
`render/layer/pipeline/*` went to two places, `module/world/` and a new
`world/`:

| Rust                                                 | TypeScript it came from                                                            |
| ---------------------------------------------------- | ---------------------------------------------------------------------------------- |
| `world/sched.rs`                                     | `module/world/sched/chunk_table.ts` (5/9), `render/chunk_lod.ts`                   |
| `world/decor.rs`                                     | `module/world/decor/instance.ts` (3/8)                                             |
| `world/environment.rs`                               | `module/world/environment/celestial_position.ts` (`sun_position`, `moon_position`) |
| `world/raycast.rs` and `module/world/raycast.rs`     | `render/layer/pipeline/chunk_raycast.ts`, one file made two                        |
| `world/billboard.rs`, `world/marker.rs`              | no source; the drawing half taken out of `module/world/billboard/` and `marker/`   |
| `world/object.rs`                                    | one name from `contract.ts`; none of `compositor.ts`'s names survive               |
| `world/build.rs`, `debug.rs`, `ghost.rs`, `model.rs` | no source (see below)                                                              |

Seven names exist in both trees: `billboard`, `decor`, `environment`,
`marker`, `object`, `raycast` and `sched`. `lod` was the eighth and has
already been folded into `module/world/lod.rs`.

### `bake/`, from `render/layer/pipeline` and `render/layer/terrain`

| Rust                                                      | TypeScript it came from                                                                                           |
| --------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `bake/mesher.rs`                                          | `render/layer/pipeline/chunk_mesher.ts` (no exported name kept)                                                   |
| `bake/scan.rs`                                            | `render/layer/pipeline/voxel_scan.ts`                                                                             |
| `bake/volume.rs`, `bake/voxel.rs`, `render/mesh/voxel.rs` | `render/layer/pipeline/volume.ts`: one file into three across two directories, its 34 exports landing 1, 7 and 11 |
| `bake/terrain.rs`                                         | part of `render/layer/terrain/chunk.ts` and `elevation.ts`                                                        |
| `bake/run_set.rs`, `bake/srgb.rs`                         | no source                                                                                                         |

`crates/pxl/shared/src/bake/` is the offline tile bake. The two directories
share the name and not the concept.

### `space/constant.rs`

Its header says it holds forms derived from `@pxl/shared/bake/constant.ts`.
`pxl-shared` already has `spec/space/`, so `space` now names two places. The
checkpoint's `module/world/anchor/space.tsx` is a React component and has
nothing to do with either.

### `render/gpu/`, `render/mesh/`, `render/gltf/`, `render/draw.rs`

The WebGPU renderer that replaced three.js, so mostly new. Two pieces came
from elsewhere:

- `render/gpu/layer_table.rs` from `render/tex_array.ts` and
  `render/layer/terrain/shader_meta.ts`
- `render/gltf/load.rs` from `module/world/gltf_cache.ts`, which moved from
  `module/world` to `render`

`sys/` and `mem/` are this week's kernel sections and not part of the
migration.

## Files that moved or were renamed

| TypeScript                            | Rust                                                                                                                                                                                |
| ------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `render/lod.ts`                       | `module/world/lod.rs` (11/16)                                                                                                                                                       |
| `render/chunk_range.ts`               | `module/world/frame/range.rs` (8/9)                                                                                                                                                 |
| `render/view_range.test.ts`           | `module/world/frame/view_range.test.rs`                                                                                                                                             |
| `render/layer/terrain/elevation.ts`   | `module/world/terrain/elevation_sample.rs` (5/5), next to `module/world/terrain/elevation.rs`, which is `module/world/terrain/elevation.ts` at its own path, so two elevation files |
| `module/world/decor/decor_layer.ts`   | `module/world/decor/decor_kinds.rs` (4/6), beside the generated `kind.rs` and the `*_kind.rs` files                                                                                 |
| `module/world/marker/frame_gizmo.tsx` | `module/world/marker/debug_gizmos.rs` (inferred; the source's exports are React components)                                                                                         |
| `module/world/chunk_stage.tsx`        | `module/world/core_tick.rs` (inferred: the world's turn, the scheduler's pass, applied to the chunk nodes)                                                                          |

## Files with no TypeScript behind them

`crates/pxl/core`:

- `app/shell.rs`: writes the UI's snapshot as JSON after each turn and applies
  the UI's actions by name to the stores
- `module/world/change.rs`: the world's face over xpute's `ipc::change`
- `module/world/fetch.rs`: the shell's edge of a fetch — the URL, room for the
  body, the status
- `module/world/frame/view.rs`, `use_view_frame.rs`: the camera and the frame
  as doubles
- `module/world/object/material.rs`, `object/teach.rs`, `terrain/teach.rs`:
  what the kernel is taught once about materials and decor kinds
- `module/world/terrain/fixed.rs`: `.fixed.terrain`
- `module/world/sched/report.rs`: the ledger's numbers; the words are still the
  shell's `ledger_report.ts`
- `world/build.rs`: the chunk build under the frame's budget
- `world/debug.rs`: the debug segments as a block
- `world/ghost.rs`: the hover ghost box, in the mesher's layout
- `world/model.rs`: glTF model data and the instances that animate it
- `bake/run_set.rs`, `bake/srgb.rs`
- `render/draw.rs`, the fifteen files of `render/gpu/`,
  `render/mesh/decor_block.rs`, `render/mesh/mesh_block.rs`,
  `render/gltf/gltf.rs`

Files generated from JSON (`*_spec.rs`, `billboard/order.rs`,
`decor/kind.rs`) are not counted.

`crates/pxl/shared`: `spec/cmd.rs`, `spec/fetch.rs`, `spec/message.rs` and
`spec/cell/cube_type.rs` arrived after the checkpoint together with their
TypeScript. They are new protocol, not moved files.

## TypeScript with no Rust found

No exported name of these matches a Rust definition. Their content was not
compared, so whether each was replaced or dropped is not established here.

- `render/layer/pipeline/chunk_bucket.ts`, `compositor.ts`, `core_pool.ts`
- `render/lod_visibility.ts`, `render/scene_depth.ts`
- `module/world/sched/slab.ts`, `category.ts`, `alloc_probe.ts`,
  `gpu_timer.ts`
- `module/world/decor/field.ts`, `module/world/billboard/field.ts`,
  `anchor_occlusion.ts`
- `module/world/anchor/*`, and the three.js materials under
  `module/world/render/`

## Concepts with two homes

1. `world/` and `module/world/`: seven names
2. `world/raycast.rs` and `module/world/raycast.rs`: one TypeScript file
3. `bake/voxel.rs` and `render/mesh/voxel.rs`: both from `volume.ts`
4. `pxl-core`'s `bake/` and `pxl-shared`'s `bake/`: one name, two meanings
5. `pxl-core`'s `space/` and `pxl-shared`'s `spec/space/`
6. `module/world/terrain/elevation.rs` and `elevation_sample.rs`
7. `module/world/decor/kind.rs` and `decor_kinds.rs`
8. `world/environment.rs`, `module/map/environment.rs` and
   `module/world/environment/`: three

## Where it stands

**Open.** The survey is done and nothing is moved. `lod` is the one pair
already folded.
