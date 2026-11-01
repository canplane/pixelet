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
import * as SPREAD from "./perf/spread.spec.ts";
import * as STREAM_STAT from "./perf/stream_stat.spec.ts";
import * as STEP_STAT from "./perf/step_stat.spec.ts";
import * as STATS from "./perf/stats.spec.ts";
import * as RING from "./perf/ring.spec.ts";
import * as RINGS from "./perf/rings.spec.ts";

export const CENSUS_TAGS = [
  "decor",
  "ghost_box",
  "ghost_decor",
  "skinned",
  "puck_zone",
  "puck_beacon",
  "marker",
  "debug",
  "impostor",
  "camera_grid",
] as const;

export const WAKES = ["next", "timer", "none"] as const;

export { LIMIT };

/** One frame turn and the interval it opened (perf/frame.rs FrameSample): every figure of one turn, landed when the next frame turn closes the interval. `refresh_ms` is the display's refresh and `paced_ms` the interval the host paced the frame at, a whole number of refreshes, both as the host granted them (@xpute/host sched/frame_pacing.ts `FrameGrant`); a moving frame whose interval ran past `paced_ms` by more than half a refresh missed a vsync (perf/frame.rs `missed`). `settled` is 0 or 1; `turns` and `busy_ms` count every turn the interval held, the frame's and those without a host frame, rise to fall: what they ran past the interval is read off the two (perf/frame.rs `over_ms`). `effects_ms`, `tick_ms`, `stage_ms`, `tasks_ms` and `perf_ms` are their parts, `stage_ms` the turn's pump, the stage's write and the render, and `perf_ms` what the profiler cost it, its own record of the frame and the shell's reads of it; what they leave of `busy_ms` is unmeasured. `effects` is each effect's time in the frame turn, in EFFECT_NAMES' order, `effects_ms` their sum. `gpu_read` is 1 where the GPU's read of this frame landed, which comes a frame or two later and is written back into this frame's record (`gpu_frame` the renderer's number of it, 0 where it drew none): its `gpu_spans` are then that read's, their sum (pxl-render gpu/renderer.rs `gpu_ms`), `gpu_busy_ms` the time they ran, those that overlap counted once, the frame's GPU time, which the sum overstates where passes overlap (pxl-render gpu/api.rs `Gpu::gpu_busy_ms`), and `gpu_px` the world's pixels of the frame the read timed, what the spans were measured over; `scale` is the share of the display's pixels, each way, the world was drawn at, in every frame (pxl-kernel domain/view/frame/resolution.rs `drawn_scale`); and 0 where nothing measured it (its read not landed yet or cut off by a stop, or the device keeps no timestamps), its spans, busy time and pixels 0 too. `kind_name` indexes the renderer's FRAME_KINDS and `wake_name` WAKES; `census` pairs draws and triangles in CENSUS_TAGS' order, a tag each pipeline a draw names (pxl-render frame/draw.rs `DRAW_PIPELINES`), so the frame's draws and triangles are its sums, `zoom_tiles` holds zoom z in slot z, `waiting` and `out` a stream's requests waiting for a credit and out on one in STREAM_NAMES' order (sys/io/request.rs `stream_counts`), and `gpu_objects` the GPU objects the renderer holds by the op that made them, op k + 1 in slot k (xpute spec/gpu/api.json `op`, from the first to DESTROY). `uploads` and `uploads_bytes` are the renderer's upload table, `depth_bytes` its depth targets and `pack_bytes` its texture pack (pxl-render gpu/renderer.rs `FrameStat`), the heap's lanes are its allocator's figures by their own names (xpute-guest mem/heap.rs `HeapStat`), and `body_slots_peak` the most body slots out at once since the frame before (sys/io/body.rs `take_peak`); the ring lanes are the door's peaks since the frame before (xpute-guest ipc/door.rs `DoorStat`), `ring_held_peak` its `left_peak`. The rays, eye and light lanes are the `ray_counts` setting's, summed since boot over the frames whose counts were read back (pxl-render gpu/ray/tally.rs `RAY_SUMS`): the frames (`rays_frames`); then an observer's each, the eye's and the light's: of them those it cast in, its pixels or its map's slots in them, the nodes its beam rays, its samples and its walk again asked, its pixels or slots that took the visible sample and those that walked, and the boxes drawn (pxl-spec-core `tally`). A lane's name says its unit: `_ms`, `_bytes`, `_px`, and `_name` for an index into a list of names. */
export { FRAME };

/** What each of the report's lines prints of a frame, FRAME's lanes in groups: a lane's label is its name without the group's prefix, and every lane is in one group (lane.test.ts), so a lane the kernel adds is printed without a word written for it. The order is what a reader asks in turn — where the time went, what was drawn, what was fetched, and what it all holds — and the report and the panel both follow it. */
export const FRAME_GROUP = {
  time: [
    "t",
    "interval_ms",
    "refresh_ms",
    "paced_ms",
    "settled",
    "turns",
    "busy_ms",
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
  frame: ["kind_name", "wake_name", "wake_in_ms"],
  gpu: [
    "gpu_read",
    "gpu_spans",
    "gpu_busy_ms",
    "gpu_px",
    "scale",
    "census",
    "gpu_frame",
  ],
  cap: ["cap_terrain_refused"],
  rays: ["rays_frames"],
  eye: [
    "eye_casts",
    "eye_rays",
    "eye_beam",
    "eye_sample",
    "eye_miss",
    "eye_took",
    "eye_walked",
    "eye_boxes",
  ],
  light: [
    "light_casts",
    "light_rays",
    "light_beam",
    "light_sample",
    "light_miss",
    "light_took",
    "light_walked",
    "light_boxes",
  ],
  requests: ["waiting", "out"],
  tiles: ["zoom_tiles", "tiles_place"],
  device: [
    "uploads",
    "uploads_bytes",
    "depth_bytes",
    "pack_bytes",
    "gpu_objects",
  ],
  heap: [
    "heap_used_bytes",
    "heap_live_bytes",
    "heap_run_bytes",
    "heap_slot_bytes",
    "heap_range_bytes",
    "heap_largest_free_bytes",
    "heap_refusals",
  ],
  ring: ["ring_cq_peak", "ring_sq_peak", "ring_held_peak"],
  body: ["body_slots_peak"],
} as const;

/** One settled task (perf/task.rs TaskRow). `subject` is a u64 word and every other word an f64; `start_ms` is NaN for a task that never ran; `stream`, `outcome` and each of `phase_ids` index STREAM_NAMES, OUTCOME_NAMES and TASK_PHASE_NAMES. `at_file` is the address of the file name, `at_file_bytes` long, of the line a failure was made at, and 0 where the failure was the host's. */
export { TASK };

/** One of the worst moving frames since the recording started, by its interval, and the frame before it, which is zero where `has_prev` is 0: a still interval is long on purpose and is never among them. */
export { WORST };

export { SPREAD };

/** One stream's tasks in the window, at STAT.tasks.slots + its index × WORDS. Each reason is (fail, detail, n), commonest first; `phase_mean` is NaN for a phase no task reported. */
export { STREAM_STAT };

/** One registered step's timing, at STAT.steps.slots + its index × WORDS. `name` indexes STEP_NAMES. */
export { STEP_STAT };

/** The window's statistics PERF_STATS computes, over its `window_ms`: each section's words at its AT, the fields relative to it. Every word an f64. Packed: a section's AT is where the one before it ends, and WORDS where the last ends, both laid out by the generator. */
export const STAT = {
  frames: {
    AT: 0 as number,
    present: 0 as number,
    n: 1 as number,
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
    gpu: 43 as number,
    gpu_busy: 46 as number,
    gpu_busy_kinds: 49 as number,
    phases_p95: 69 as number,
    gpu_spans_p95: 74 as number,
    kinds: 100 as number,
    wakes: 110 as number,
    heap_used_delta_bytes: 113 as number,
    first: 114 as number,
    last: 274 as number,
    WORDS: 434 as number,
  },
  tasks: {
    AT: 434 as number,
    slots: 0 as number,
    WORDS: 120 as number,
  },
  steps: {
    AT: 554 as number,
    cnt: 0 as number,
    slots: 1 as number,
    WORDS: 161 as number,
  },
  WORDS: 715 as number,
} as const;

/** What PERF_STATS finds as it reads, the window aside: each section's words at its AT, as STAT lays out its own. Every word an f64. */
export const NOW = {
  rec: {
    AT: 0 as number,
    recording: 0 as number,
    elapsed_ms: 1 as number,
    truncated: 2 as number,
    WORDS: 3 as number,
  },
  /** Where the camera, its pivot and the pointer are as the read finds them, recording or not: `eye_x`..`eye_z`, `dist`, `pivot_x`..`pivot_z`, `hover_x`..`hover_z` and `place_x`..`place_z` in units, angles in radians, `pivot_lon` and `pivot_lat` in degrees and `pivot_alt` in meters, a zoom as its number, and NaN where there is none.
   * The eye is where the camera looks from and the pivot the point it turns about, the eye itself in the first-person camera, where the character stands (pxl-kernel domain/control/camera/controller.rs `eye_world`, `pivot`).
   * The hover is what the pointer meets, exactly, at the zooms the eye's rays ask, whatever the tool could strike, and `cell` the zoom the eye's rays draw there (pxl-kernel domain/query/ray.rs `by_dist`).
   * The pick is what the tool held strikes there (pxl-kernel domain/control/edit.rs `pick_at`): `struck` what it met, 0 the ground, 1 a placed cube, 2 placed cubes folded into a coarser box, 3 a decor; `held` the zoom of the box struck and `drawn` the rays' there, `folded` 1 where they draw coarser than the box, which strikes nothing. `place_x`..`place_z` is the corner the tool takes (pxl-kernel domain/control/edit.rs `Hover`), `place_zoom` the zoom it places at, and `blocked` 1 where that room is taken (domain/control/edit/collision.rs `is_placement_blocked`). */
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
    struck: 16 as number,
    held: 17 as number,
    drawn: 18 as number,
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
