# What the comments carried

**2026-09-17** — The Rust's comments had been keeping two kinds of thing that
are not about the code as it stands: what a file used to be, and dated
measurements that decided a number. Both were taken out of the code, and the
comments now say only why the code is the way it is. This is where the
removed text went, by file, as it was, so a reason that rested on a
measurement can still be traced to it. A third group is the wording left over
from when the kernel was TypeScript beside React; it is listed so a reader
who remembers the old sentence can find what replaced it.

Nothing here is a decision. The measurements were taken on the dates they
carry and were not repeated.

## Measurements

- `crates/pxl/core/src/module/world/chunk_node.rs`: large block needs in one piece — at a quarter of the whole core reservation (128 MB of a 256 MB section) the section sat two-thirds full and fragmented, and refused the fine tiers' blocks 88,000 times in a minute (2026-09-14).
- `crates/pxl/core/src/module/world/chunk_node.rs`: not free — the direct runs are written whole before their block is asked for. A thousand chunks pending on a starved pool spun a hundred milliseconds a frame this way (2026-09-14).
- `crates/pxl/core/src/module/world/chunk_node.rs`: wanted: a chunk's worth is 0.15 ms to make again, and a thousand of them held is a hundred megabytes of small blocks the fine tiers' large ones then fail to find room among (2026-09-14). A scan's runs stay: those cost the scan.
- `crates/pxl/core/src/module/world/poi/poi_badge.rs`: thousand. `measureText` on a phone is around a millisecond for a Korean name — shaping — so measuring every candidate was a third of a second of main-thread time per new area (2026-09-13).
- `crates/pxl/core/src/module/world/poi/poi_index.rs`: origin and deduped the request against it. A fresh load with every tile arriving after the POI pages drew nothing until the first pan (2026-09-12).
- `crates/pxl/core/src/module/world/sched/chunk_table.rs`: room among. A zoom-in leaves a thousand chunks showing their last layout off screen; at two a frame that is eight seconds of the fine tiers finding no room (2026-09-14).
- `crates/pxl/core/src/module/world/sched/chunk_table.rs`: off its pressure mark. Seen on the desktop, 2026-09-14: a zoom through a city left 1,091 chunks pending on tiers the section could not hold, nothing off screen to reclaim, for the rest of the session.
- `crates/pxl/core/src/module/world/sched/tick.rs`: Without it a placement of a few dozen units took 25 frames and 3.3 s on a phone while a load kept the visible phase busy, and an index rebuild 49 frames (2026-09-13). A fixed share, not a feedback loop.
- `crates/pxl/core/src/module/world/surface_height.rs`: This replaces probing upward a cell at a time with a 27-cell neighborhood query at each step: with nothing placed at the XY the probe loop ran to its 64-cell guard every call, and the companion asked twice a frame — 4.8 ms a frame on a phone (2026-09-13). A placed object
- `crates/pxl/core/src/render/chunk_range.rs`: scan up: measured 2026-09-12, and it reads on screen
- `crates/pxl/core/src/render/chunk_range.rs`: them a moment later (200 terrain aborts in one session, 2026-09-12). One
- `crates/pxl/core/src/render/layer/pipeline/chunk_mesher.rs`: would from a pass over every cell's buckets held at once — the TypeScript's two phases — without holding them. Held, a dense chunk's buckets ran to megabytes and, doubling as they grew, past the module's heap (b3, 2026-09-13).

## What a file used to be

- `crates/pxl/core/src/app/setting/schema.json`: Off by default since 2026-09-13 — the tilt is the one control a map view is expected to have, and locking it read as the app being stuck.
- `crates/pxl/core/src/mem/mem.json`: The one before this was three copies: the shell picked the reservation, the kernel wrote the same number down beside a comment saying it was not read here, and nothing compared them — so a shell that asked for too little instantiated fine and the kernel asserted its way out of it a moment later, long after the point where the mistake could be named.
- `crates/pxl/core/src/mem/mem.json`: What a device's memory is counted in, and all that is left of the regions the pools used to cut: a reservation is a whole number of these and the bytes past the last one are left out.
- `crates/pxl/core/src/module/camera/store.rs`: and the follow-mode-off action actually mean — resetting polar (tilt) alongside it used to also flatten/steepen whatever view angle the user had deliberately set, on a click whose entire purpose was "point me north," not "also forget my tilt."
- `crates/pxl/core/src/module/world/billboard/frustum_budget.rs`: legible. The ranking used to be the hash alone, and the cap was a cliff: one candidate over, the whole set was ranked by a value unrelated to where anything is and cut, deleting a badge in the middle of the screen while keeping one at the edge.
- `crates/pxl/core/src/module/world/chunk_node.rs`: only ever about what is placed. The direct path used to stop at step 8 on the principle that the fine tiers keep the voxel pipeline; a zoom-in then scanned every chunk in view, about 25 a second, most of them empty of objects (2026-09-14).
- `crates/pxl/core/src/module/world/chunk_node.rs`: the terrain and the objects into a volume and scanning it. Its tiers are baked from those runs, one bake and one scan open in the whole world at a time, as before.
- `crates/pxl/core/src/module/world/decor/decor_hit.rs`: as the hover ghost flickering while the cursor sat still (2026-09-05). 1e-3
- `crates/pxl/core/src/module/world/decor/decor_layer.rs`: The tier split is Minecraft's own block model, taken literally (2026-09-05):
- `crates/pxl/core/src/module/world/decor/decor_layer.rs`: range at which Google's map products stop drawing a building in 3D (2026-09-13, decided by eye).
- `crates/pxl/core/src/module/world/environment/config.json`: in the theme through environment.rs). One shared constant, not two separate copies, specifically so the two can't drift apart again the way they did before this existed (terrain used to be immune to scene-light dimming by construction while objects weren't, then briefly the reverse once terrain adopted real lighting too). Tune this one number to retune every surface's night floor at once.
- `crates/pxl/core/src/module/world/environment/theme.json`: of it, DAY before DAY_END, EVENING after (37.5% / 37.5% / 25%, the old 6h/6h/4h split of a 16h fixed daylight span, scaled to however long daylight actually is today).
- `crates/pxl/core/src/module/world/environment/theme.rs`: the map is centered (crates/pxl/core/src/module/map/environment.rs) — replaces a fixed 05:00/21:00 split that didn't track season or location. MORNING/DAY/EVENING still split the daylight span the same way the old fixed version did (theme.json's daylight_split), just scaled to however long daylight actually is today.
- `crates/pxl/core/src/module/world/fetch.rs`: is for. A terrain body no longer outlives its command
- `crates/pxl/core/src/module/world/object/material.rs`: for a terrain-styled pick (2026-09-06), the exact
- `crates/pxl/core/src/module/world/object/material.rs`: class here would trade away real, already-fixed correctness (a solid object no longer shields whatever's beneath it from occlusion culling, the mesher's own 2026-08-16 bug) for a distinction layer-tap blending never needs
- `crates/pxl/core/src/module/world/object/store.rs`: v9 (2026-09-17): a page is five bare lanes — x, y, z, size, value — with no record header, column names or id and heading lanes (the persistence section below). A v8 blob is a self-describing table this code does not read. v8 (2026-09-17): a terrain-styled cube's leaf value is its channel and its type (spec/cell's TERRAIN_STYLE_BASE), not a number counted out as the catalog was walked. A v7 blob's terrain-styled values are 1..32 in walk order and name different cubes under the new rule, which is exactly the case this version exists for. v7 (2026-09-13): one Placement record per page in a page-keyed container (the persistence section below) — the same form every placed thing has. Not migrated.
- `crates/pxl/core/src/module/world/object/store.rs`: an infinite loop ("Maximum update depth exceeded", 2026-09-03).
- `crates/pxl/core/src/module/world/poi/poi_badge.rs`: Supersampling, and it should equal the renderer's own device pixel ratio. The tick clamps that to [1, 2], and the distance fade returns at most 1, so a billboard's drawn size never exceeds its logical size in CSS pixels — two is the exact requirement rather than a guess. It was 3, which is 2.25x the pixels for nothing: drawing at 3 to display at 2 is a minification, so it went through the mipmap chain and came out softer. pub const RENDER_SCALE: f64 = 2.0;
- `crates/pxl/core/src/module/world/poi/poi_layer.rs`: sprite and draw count if something goes wrong. It used to be 80, and it used to be the whole answer — four POI in one building meant one drawn and three deleted. Aggregation is what a budget should buy; deletion is what it bought.
- `crates/pxl/core/src/module/world/poi/poi_theme.json`: `icon` is a plain emoji — tried swapping these for tinted Lucide glyphs (2026-09-08) and reverted: at this badge's actual on-screen size a thin stroked icon reads far less recognizably than an emoji's own dense, multi-shape glyph does, and that shape-level legibility (not stroke weight, tried thickening it first) was the real problem.
- `crates/pxl/core/src/module/world/poi/poi_theme.json`: Food & drink — restaurant/fast_food/bar used to all share this exact amber; fine with the emoji's own glyph carrying enough identity on its own, but worth keeping separated regardless of icon style.
- `crates/pxl/core/src/module/world/sched/demand.rs`: member. It used to be `"<category>:<subject>"` spelled out, which the arrival then split back apart and parsed — two numbers folded into text so that the text could be unfolded into two numbers.
- `crates/pxl/core/src/module/world/sched/ledger.rs`: client with no dev overlay should be paying for none of this, and until this was gated it paid for all of it: the rings are
- `crates/pxl/core/src/module/world/sched/summary.rs`: The overlay and the report (log.rs) each used to reduce the rings themselves, and they drifted: the report named failure reasons the panel did not, and both counted missed frames against a 60 Hz constant the display did not have. Both now format what this returns, so a number on the panel and the same number in a pasted report agree.
- `crates/pxl/core/src/render/lod.rs`: reference. Twice the 120u and 320u the fade was first tuned at, which read as too near once the badges gathered by the cube ladder (2026-09-12); and moving with the projection now rather than staying fixed while the tiers under them moved.
- `crates/pxl/core/src/render/lod.rs`: inside render_cfg.RENDER_DIST, so including it removes both its hard pop and the old 29% non-blended hysteresis gap.
- `crates/pxl/shared/src/bake/constant.rs`: unlike the old TERRAIN_LAYOUT above) because objects never merge past
- `crates/pxl/shared/src/bake/ground.rs`: anything the terrain does not already show. They disagreed before this was one definition — a road over bare ground reads `land = BARE` while rendering as road, so comparing one channel canceled a bare-ground cube into a road.
- `crates/pxl/shared/src/spec/cell/mod.rs`: explicit air — the only real distinction left (2026-09-06) now that a
- `crates/pxl/shared/src/spec/cell/mod.rs`: hand-typed per-type catalog to keep in sync — 2026-09-06, see
- `crates/pxl/shared/src/spec/cell/mod.rs`: the original 15 flat colors (2026-09-06 UX decision — the full catalog
- `crates/pxl/shared/src/spec/space/adapter/yup.rs`: The engine is natively Z-up throughout, and the 2026-04 Z-up unification deleted the per-subsystem Y-up adaptations that used to call helpers like these. Inside the engine there is no longer anywhere for a conversion to happen — ENU maps straight onto the render axes.
- `crates/pxl/wasm/src/lib.rs`: It used to — a line per settled task, raised as `WORLD_LOG` every tick — and that gave a debug feature the ring's failure policy, which is that `Outbox::post` asserts when the completion queue and its spill are both full. Turning the stream on during a fetch burst is the case that fills them, and it is also the only case anyone turns it on for.
- `packages/pxl/shared/src/bake/constant.ts`: unlike the old TERRAIN_LAYOUT above) because objects never merge past
- `packages/pxl/shared/src/bake/ground.ts`: anything the terrain does not already show. They disagreed before this was one definition — a road over bare ground reads `land = BARE` while rendering as road, so comparing one channel canceled a bare-ground cube into a road.
- `packages/pxl/shared/src/spec/cell/cube_type.json`: Flat user-painted colors only — no sentinels of its own anymore.
- `packages/pxl/shared/src/spec/cell/mod.ts`: explicit air — the only real distinction left (2026-09-06) now that a
- `packages/pxl/shared/src/spec/cell/mod.ts`: hand-typed per-type catalog to keep in sync — 2026-09-06, see
- `packages/pxl/shared/src/spec/cell/mod.ts`: the original 15 flat colors (2026-09-06 UX decision — the full catalog
- `packages/pxl/shared/src/spec/space/adapter/yup.ts`: The engine is natively Z-up: `bootstrap/three.ts` sets `THREE.Object3D.DEFAULT_UP` to (0, 0, 1) globally, and the 2026-04 Z-up unification deleted the per-subsystem Y-up adaptations that used to call helpers like these. Inside the engine there is no longer anywhere for a conversion to happen — ENU maps straight onto the render axes.

## TypeScript and React wording

- `crates/pxl/core/src/app/setting/store.rs`: Stored under the same key and in the same shape zustand's persist wrote, `{"state":{"overrides":{…}},"version":0}`, so a browser keeps its settings across the move.
- `crates/pxl/core/src/app/shell.rs`: compares whole and hands to React only when it changed
- `crates/pxl/core/src/module/map/navigation/map_bootstrap.rs`: The effect's dependencies as they were last run.
- `crates/pxl/core/src/module/map/store.rs`: The TypeScript enum's string.
- `crates/pxl/core/src/module/world/decor/decor_layer.rs`: decoration in the world. The TypeScript literal is wider than a word; a seed derivation masks to 64 bits, so this is the part of it that ever mattered.
- `crates/pxl/core/src/module/world/decor/decor_layer.rs`: cells across; why, and every other rule, is the TypeScript's.
- `crates/pxl/core/src/module/world/environment/celestial_position.rs`: construction, acos and a sign from the hour angle, as the glue had it.
- `crates/pxl/core/src/module/world/environment/theme.rs`: The TypeScript enum's string, and theme.json's key.
- `crates/pxl/core/src/module/world/object/store.test.rs`: store.ts's subscriber test is a revision test here: a col_idx's changes are read as its revisions moving, which is what chunk_node.rs follows.
- `crates/pxl/core/src/module/world/sched/chunk_table.rs`: whatever the last frame left — nothing else asks for a frame now that chunks are not React children.
- `crates/pxl/core/src/module/world/world.rs`: the tick (`world_turn`), each comparing what it last saw, the way their React effects compared their dependencies.
- `crates/pxl/core/src/render/layer/pipeline/chunk_mesher.rs`: What bakes have counted, for the ledger. Cumulative, like the context's in the TypeScript.
- `crates/pxl/core/src/render/layer/pipeline/chunk_mesher.rs`: visits them in the order the TypeScript's repeated minimum search did.
- `crates/pxl/core/src/render/layer/pipeline/chunk_mesher.rs`: JavaScript's `Math.round` and Rust's `round` differ only on negative ties, and the averages rounded here are never negative.
- `crates/pxl/core/src/render/layer/pipeline/chunk_mesher.rs`: two libms may disagree in the last bit, which would break the mesh's byte-for-byte agreement with the TypeScript bake — and the determinism the destination issue asks to pin across toolchains.
- `crates/pxl/core/src/render/layer/pipeline/volume.rs`: JavaScript's `Math.round` breaks ties upward and Rust's `round` away from zero; for the non-negative fractions this packs, the two agree.
- `crates/pxl/core/src/sys/kernel.rs`: first use. The modules translated from the client (module/, app/, render/) read and write it directly, as their TypeScript read its stores; pxl-wasm
- `crates/pxl/wasm/src/lib.rs`: One bake is open at a time, as with the TypeScript context; the module is single-threaded, so the state is a plain global.

## Where it stands

**Done.** The comments describe the current code; the text above is kept
as it was written and is not maintained.
