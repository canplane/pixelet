# How pixelet's terrain/object/decor split compares to other voxel engines

**2026-09-06** — same-day follow-up to the LOD-blend work
([20260906-decor-lod-blend.md](./20260906-decor-lod-blend.md)) and the
rendering-pipeline cleanup pass
([20260905-terrain-decor-scheduling-merged.md](./20260905-terrain-decor-scheduling-merged.md)'s
own thread). Prompted by a direct question during that review: is decor
staying a separate mesh from cube content (rather than merging all the way,
Minecraft-style) actually how other voxel engines do it, or is pixelet
cutting a corner? Answer, from a short web-research pass: pixelet's current
split tracks a real, documented industry pattern closely, on both axes that
came up.

## Decor as a separate instancing layer, not merged into the terrain mesh

[Godot Voxel Tools](https://voxel-tools.readthedocs.io/en/latest/instancing/)
(a production C++ voxel module for Godot) ships a `VoxelInstancer` with
almost exactly pixelet's own decor shape:

- **A fully separate rendering layer** from the terrain mesh — MultiMesh
  (GPU instancing) or scene instances, never merged into the terrain
  geometry itself. Same relationship chunk_stage.tsx's `Chunk` keeps between
  its own cube mesh and its decor meshes (same chunk, same scheduling, two
  separate draw calls).
- **Procedural instances regenerate** on terrain changes according to
  placement rules — the same shape as `scatter_decor_candidates` recomputing
  on every relevant revision, not stored anywhere.
- **Persistent (user-affected) instances are saved separately** and skip
  regeneration — the same role `objects/store.ts`'s `"decor"` entry kind
  plays for a user-placed tree, read by `build_decor_instances` unconditionally
  ahead of the procedural scatter.
- Godot Voxel Tools' own docs flag manual placement of instances as **"not
  supported yet"** — pixelet's decor tool (mode_bar.tsx) already covers
  ground this reference engine's own instancer doesn't.

## Full-chunk rescan on a single voxel edit — the default, not a mistake

The classic voxel-meshing reference,
[0fps' "Meshing in a Minecraft Game"](https://0fps.net/2012/07/07/meshing-minecraft-part-2/),
doesn't cover incremental updates in the article body, but a commenter
(Lazlo Bonin) describes hitting the exact cost pixelet's own
`objects_revision` path pays today: naive full-chunk remeshing on every edit
being "too long" for frequent modification, worked around with a
coordinate→quad-list hashmap so a single voxel edit only re-derives that
voxel's own ~36 neighbor reads instead of rescanning the whole chunk.

This confirms the shape of the tradeoff rather than resolving it: **full
rescan-on-any-change is the default/naive starting point industry-wide**,
and per-voxel incremental patching is a real, known, but nontrivial
optimization built on top only when edit frequency actually demands it — not
something pixelet is missing by oversight. Directly relevant to this
session's earlier finding that placing/erasing/hover-hiding a solid object
forces a full `column_runs` rescan of that chunk
(`l2_column_revisions` → `chunk_stage.tsx`'s source-revision effect →
`reset_all_caches()`): that's the same default every voxel engine starts
from, and the documented alternative (per-voxel patch cache) is a concrete,
citable next step if this ever becomes a measured problem rather than a
theoretical one.

## Greedy meshing — a different, largely orthogonal lever

[Veloren](https://veloren.net/devblog-81/) (an open-source Rust voxel RPG)
uses greedy meshing for both terrain and its own decorative/prop content
("figures"/"sprites"). Greedy meshing merges same-material adjacent faces
into larger quads to cut vertex/triangle count — a different axis entirely
from "does decor share a buffer with terrain": it's about how efficiently
_one_ buffer is triangulated, not about which content shares that buffer.
pixelet's own `chunk_mesher.ts` bakes one box per run rather than greedy-
merging runs together. Search results (and
[a maintained implementation](https://github.com/cgerikj/binary-greedy-meshing))
agree this is a real but debated lever: it "comes with drawbacks of being
significantly slower [to compute] and much more complicated to implement,"
and "in real games with many different voxel materials, lighting, and
ambient occlusion, you might not actually reduce vertex count by much" — not
a clear win pixelet is obviously leaving on the table, just a separate,
uninvestigated axis.

## Bottom line

| Axis                        | pixelet today                                    | Industry observation                                                                                  |
| --------------------------- | ------------------------------------------------ | ----------------------------------------------------------------------------------------------------- |
| decor vs. terrain mesh      | separate instancing, procedural/persistent split | Matches Godot Voxel Tools' `VoxelInstancer` closely                                                   |
| full rescan on a voxel edit | yes, whole chunk, every time                     | The documented default; incremental per-voxel patching is a known but nontrivial next step, not a gap |
| greedy meshing              | not used (per-run box baking)                    | Used by some (Veloren), but a debated, orthogonal optimization with real implementation cost          |

**Sources:**

- [Voxel Tools documentation – Instancing](https://voxel-tools.readthedocs.io/en/latest/instancing/)
- [Meshing in a Minecraft Game (Part 2) – 0 FPS](https://0fps.net/2012/07/07/meshing-minecraft-part-2/)
- [This Week In Veloren 81](https://veloren.net/devblog-81/)
- [binary greedy meshing (GitHub)](https://github.com/cgerikj/binary-greedy-meshing)
- [Voxel Meshing for the Rest of us – Spacefarer](https://playspacefarer.com/voxel-meshing/)
