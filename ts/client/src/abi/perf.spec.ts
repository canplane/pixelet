// @pxl/client/abi/perf.spec.ts
//
// GENERATED from spec/client/kernel/perf/perf.json
//
// The profiler's records and sections as the kernel lays them out and the client reads
// them in place (perf/record.rs, ts/client/src/port/perf.ts):
// word offsets, eight bytes a word. The names the numbers index are their owners':
// a task's io's (spec/client/kernel/sys/io/demand.json), a step's and an effect's the
// turn's (spec/client/kernel/turn/step.json), a GPU span's the renderer's
// (spec/client/render/gpu/span.json). A packed table gives each field's
// width and the generator lays the offsets out. Nothing is built
// for a read: the kernel writes numbers where they stay, and the words for them are
// the shell's.

import * as LIMIT from "./perf/limit.spec.ts";
import * as FRAME from "./perf/frame.spec.ts";
import * as TASK from "./perf/task.spec.ts";
import * as WORST from "./perf/worst.spec.ts";
import * as PERCENTILES from "./perf/percentiles.spec.ts";
import * as STREAM_STAT from "./perf/stream_stat.spec.ts";
import * as STEP_STAT from "./perf/step_stat.spec.ts";
import * as STATS from "./perf/stats.spec.ts";
import * as RING from "./perf/ring.spec.ts";
import * as RINGS from "./perf/rings.spec.ts";

export const WAKES = ["next", "timer", "none"] as const;

export const HIT_KINDS = ["ground", "object", "objects", "decor"] as const;

export { LIMIT };

/** One frame turn and the interval it opened (perf/frame.rs FrameSample): every figure of one turn, landed when the next frame turn closes the interval. `refresh_ms` is the display's refresh and `paced_ms` the interval the runtime paced the frame at, a whole number of refreshes, both as the runtime granted them (@xpute/runtime sched/frame_pacing.ts `FrameGrant`); a moving frame whose interval ran past `paced_ms` by more than half a refresh missed a vsync (perf/frame.rs `missed`). `settled` is 0 or 1; `turns` and `busy_ms` count every turn the interval held, the frame's and those without a runtime frame, rise to fall: what they ran past the interval is read off the two (perf/frame.rs `over_ms`). `effects_ms`, `tick_ms`, `stage_ms`, `tasks_ms` and `perf_ms` are their parts, `stage_ms` the turn's pump and the render, and `perf_ms` what the profiler cost it, its own record of the frame and the shell's reads of it; what they leave of `busy_ms` is unmeasured. `effects` is each effect's time in the frame turn, in EFFECT_NAMES' order, `effects_ms` their sum. `gpu_read` is 1 where the GPU's read of this frame landed, which comes a frame or two later and is written back into this frame's record (`gpu_frame` the renderer's number of it, 0 where it drew none): its `gpu_spans` are then that read's, `gpu_busy_ms` the time they ran, those that overlap counted once, the frame's GPU time, which the spans' sum overstates where passes overlap (pxl-render gpu/api.rs `Gpu::gpu_busy_ms`), and `gpu_px` the world's pixels of the frame the read timed, what the spans were measured over; and 0 where nothing measured it (its read not landed yet or cut off by a stop, or the device keeps no timestamps), its spans, busy time and pixels 0 too. `scale` is the share of the display's pixels, each way, the world was drawn at, in every frame (pxl-kernel domain/scene/resolution.rs `drawn_scale`). `kind_name` indexes the renderer's FRAME_KINDS and `wake_name` WAKES. `draws` and `triangles` are the frame's, every pipeline a draw names (pxl-render frame/draw.rs `DRAW_PIPELINES`), counted while the profiler records. `light_moved` says why the light's map cast in the frame, a bit each in LIGHT_MOVED's order (pxl-render gpu/ray/cast.rs `light_moved`), 0 where it did not cast; `decor_boxes` is how many boxes decor's slots were marked by to generate again in it, the bricks handed in it and the edits and cuts held (pxl-render gpu/generator.rs `prepare`), 0 where it marked none. The ray columns are the `ray_counts` setting's, written into the frame they were counted in as the counts land a frame or two later, by the renderer's number of it as a GPU read is (pxl-render gpu/ray/count.rs `RayCounts`), 0 where none landed: `eye_cast` and `light_cast` 1 where the eye cast in full or the light's map cast and its counts landed; then the eye's pixels (`eye_rays`) and the light's map's slots (`light_rays`), and an observer's each: the nodes its beam rays, its samples and its walk again asked, those of the last asked of the world, its pixels or slots that reused the visible sample, those that walked and those that took the sky, together every one it cast again, so the light's three over `light_rays` are the share of its map a cast cast again, and the boxes drawn (pxl-engine `count`). `terrain_landed` is how many terrain tiles the tile cache took in during the frame turn (pxl-kernel domain/layer/tile_cache.rs `take_landed`). `uploads` and `uploads_bytes` are the renderer's upload table and `pack_bytes` its texture pack (pxl-render gpu/renderer.rs `FrameStat`), and `heap_used_bytes` the heap's high-water mark (xpute-core mem/heap.rs `HeapStat`). A column's name says its unit: `_ms`, `_bytes`, `_px`, and `_name` for an index into a list of names. */
export { FRAME };

/** FRAME's columns in groups, every column in one (column.test.ts), in the order a reader asks in turn — where the time went, what was drawn and cast, what landed, and what it holds. The trace prints every column by its group's order, so a column the kernel adds is printed without a word written for it; the panel shows some groups, a few rows each, and the report its own lines. The `rays` group's columns lie in FRAME in its order, which the window's sums of them (STAT.frames.rays) keep. */
export const FRAME_GROUP = {
  time: [
    "t",
    "interval_ms",
    "refresh_ms",
    "paced_ms",
    "settled",
    "turns",
    "busy_ms",
    "over_ms",
    "budget_ms",
  ],
  turn: [
    "effects_ms",
    "tick_ms",
    "phases",
    "stage_ms",
    "tasks_ms",
    "perf_ms",
    "effects",
  ],
  frame: ["kind_name", "wake_name"],
  gpu: [
    "gpu_read",
    "gpu_spans",
    "gpu_busy_ms",
    "gpu_px",
    "scale",
    "draws",
    "triangles",
    "gpu_frame",
    "light_moved",
    "decor_boxes",
  ],
  rays: [
    "eye_cast",
    "eye_rays",
    "eye_beam",
    "eye_sample",
    "eye_miss",
    "eye_asked",
    "eye_reused",
    "eye_walked",
    "eye_sky",
    "eye_boxes",
    "light_cast",
    "light_rays",
    "light_beam",
    "light_sample",
    "light_miss",
    "light_asked",
    "light_reused",
    "light_walked",
    "light_sky",
    "light_boxes",
  ],
  tiles: ["terrain_landed"],
  device: ["uploads", "uploads_bytes", "pack_bytes"],
  heap: ["heap_used_bytes"],
} as const;

/** One settled task (perf/task.rs TaskRow). `path` is the bytes of the path it opened, zero past their end, and every other word an f64; `start_ms` is NaN for a task that never ran; `stream`, `outcome` and each of `phase_ids` index STREAM_NAMES, OUTCOME_NAMES and TASK_PHASE_NAMES. `at_file` is the address of the file name, `at_file_bytes` long, of the line a failure was made at, and 0 where the failure was the runtime's. */
export { TASK };

/** One of the worst moving frames since the recording started, by its interval, and what changed from the frame before it: its uploads, their bytes and the heap's high-water mark, each this frame's less that one's, all 0 where `has_prev` is 0. A still interval is long on purpose and is never among them. */
export { WORST };

/** A value's percentiles over n values: p50 and p95 each the nearest rank, the least value at or above which lie no more than that share of the n (sorted, the value at ceil(q × n) − 1), and the most. */
export { PERCENTILES };

/** One stream's tasks in the window, at STAT.tasks.slots + its index × WORDS. Each reason is (fail, detail, n), commonest first; `phase_mean` is NaN for a phase no task reported. */
export { STREAM_STAT };

/** One registered step's timing, at STAT.steps.slots + its index × WORDS. `name` indexes STEP_NAMES. */
export { STEP_STAT };

/** The window's statistics PERF_STATS computes, over its `window_ms`: each section's words at its AT, the fields relative to it. Every word an f64. Packed: a section's AT is where the one before it ends, and WORDS where the last ends, both laid out by the generator. */
export const STAT = {
  frames: {
    AT: 0 as number,
    present: 0 as number,
    cnt: 1 as number,
    seconds: 2 as number,
    moving: 3 as number,
    moving_ms: 4 as number,
    missed: 5 as number,
    overran: 6 as number,
    long: 7 as number,
    turns: 8 as number,
    gpu_read: 9 as number,
    interval: 10 as number,
    busy: 13 as number,
    tick: 16 as number,
    effects: 19 as number,
    stage: 22 as number,
    tasks: 25 as number,
    perf: 28 as number,
    rest: 31 as number,
    effects_p95: 34 as number,
    gpu_busy: 43 as number,
    gpu_busy_kinds: 46 as number,
    phases_p95: 76 as number,
    gpu_spans_p95: 81 as number,
    gpu_spans_cnt: 119 as number,
    kinds: 157 as number,
    wakes: 167 as number,
    gpu_px: 170 as number,
    rays: 171 as number,
    first: 191 as number,
    last: 293 as number,
    WORDS: 395 as number,
  },
  tasks: {
    AT: 395 as number,
    slots: 0 as number,
    WORDS: 144 as number,
  },
  steps: {
    AT: 539 as number,
    cnt: 0 as number,
    slots: 1 as number,
    WORDS: 161 as number,
  },
  /** What the frames held at the most or the least since the recording started, as the steps are timed, folded as each frame turn's record is taken (perf/record.rs `perf_fold_recording`): `frames` the frame turns folded; the heap's figures by their allocator's names (xpute-core mem/heap.rs `HeapStat`), the most of each but `largest_free`, the least, and `refusals` as it last stood; the door's rings' peaks and the body slots' (xpute-core ipc/door.rs `DoorStat`, sys/io/body.rs `take_peak`), `ring_left_peak` its `left_peak`; and `cap_terrain_refused` the terrain tiles the tile cache refused a slot, summed (domain/layer/tile_cache.rs `take_refusals`). Every word 0 before a frame is folded. */
  recording: {
    AT: 700 as number,
    frames: 0 as number,
    heap_live_max_bytes: 1 as number,
    heap_run_max_bytes: 2 as number,
    heap_slot_max_bytes: 3 as number,
    heap_range_max_bytes: 4 as number,
    heap_largest_free_min_bytes: 5 as number,
    heap_refusals: 6 as number,
    ring_cq_peak: 7 as number,
    ring_sq_peak: 8 as number,
    ring_left_peak: 9 as number,
    body_slots_peak: 10 as number,
    cap_terrain_refused: 11 as number,
    WORDS: 12 as number,
  },
  WORDS: 712 as number,
} as const;

/** What PERF_STATS finds as it reads, the window aside: each section's words at its AT, as STAT lays out its own. Every word an f64. */
export const NOW = {
  recording_state: {
    AT: 0 as number,
    recording: 0 as number,
    elapsed_ms: 1 as number,
    truncated: 2 as number,
    WORDS: 3 as number,
  },
  /** Where the camera, its pivot and the pointer are as the read finds them, recording or not: `eye_x`..`eye_z`, `dist`, `pivot_x`..`pivot_z`, `hover_x`..`hover_z` and `place_x`..`place_z` in units, angles in radians, `pivot_lon` and `pivot_lat` in degrees and `pivot_alt` in meters, a zoom as its number, and NaN where there is none.
   * The eye is where the camera looks from and the pivot the point it turns about, the eye itself in the first-person camera, where the character stands (pxl-kernel domain/control/camera.rs `eye_world`, camera/controller.rs `pivot`).
   * The hover is what the pointer meets, exactly, at the zooms the eye's rays ask, whatever the tool could strike, and `cell` the zoom the eye's rays draw there (pxl-kernel domain/query/ray.rs `by_dist`).
   * The pick is what the tool held strikes there (pxl-kernel domain/control/edit.rs `pick_at`): `hit_kind` what it met, an index into HIT_KINDS; `pick_zoom` the zoom of the box struck and `ray_zoom` the rays' there, `folded` 1 where they draw coarser than the box, which strikes nothing. `place_x`..`place_z` is the corner the tool takes (pxl-kernel domain/control/edit.rs `Hover`), `place_zoom` the zoom it places at, and `blocked` 1 where that room is taken (domain/control/edit/collision.rs `is_placement_blocked`). */
  view: {
    AT: 3 as number,
    eye_x: 0 as number,
    eye_y: 1 as number,
    eye_z: 2 as number,
    dist: 3 as number,
    polar: 4 as number,
    azimuth: 5 as number,
    pivot_x: 6 as number,
    pivot_y: 7 as number,
    pivot_z: 8 as number,
    pivot_lon: 9 as number,
    pivot_lat: 10 as number,
    pivot_alt: 11 as number,
    hover_x: 12 as number,
    hover_y: 13 as number,
    hover_z: 14 as number,
    cell: 15 as number,
    hit_kind: 16 as number,
    pick_zoom: 17 as number,
    ray_zoom: 18 as number,
    folded: 19 as number,
    place_x: 20 as number,
    place_y: 21 as number,
    place_z: 22 as number,
    place_zoom: 23 as number,
    blocked: 24 as number,
    WORDS: 25 as number,
  },
  WORDS: 28 as number,
} as const;

/** What PERF_STATS answers with the address of: u32 words, where STAT and NOW lie in the module's memory and their words. */
export { STATS };

/** Where one ring lies: its address in the module's memory and its record count. */
export { RING };

export const RING_NAMES = ["frames", "tasks", "worst"] as const;

/** What PERF_RINGS answers with the address of: u32 words, a RING apiece in RING_NAMES' order. */
export { RINGS };
