// @pxl/client/abi/perf.spec.ts
//
// GENERATED from spec/client/core/perf/perf.json5 (xpute-gen data.rs)
//
// The profiler's records and sections as the core lays them out and the client reads
// them from the replies that hand them over (perf/record.rs, ts/client/src/service/core/perf.ts):
// word offsets, eight bytes a word. The names the numbers index are their owners':
// a task's io's (spec/client/core/sys/io/demand.json5), a step's, an effect's and a part of the stage's the
// frame's (spec/client/core/perf/step.json5), a GPU span's the renderer's
// (spec/client/render/gpu/span.json5), a base layer's the bake's
// (spec/spec/bake/publication.json5). A packed table gives each field's
// width and the generator lays the offsets out. Nothing is built
// for a read: the core writes numbers where they stay, and the words for them are
// the UI's. LIMIT holds the bounds the profiler keeps, which the core and the client both read.

/** What the pick met (NOW.view `hit_kind`; pxl-core domain/query/ray.rs `HitKind`): the ground, a placed cube, placed cubes folded into a coarser box, or a decor. */
export const HIT_KINDS = ["ground", "object", "objects", "decor"] as const;

export type HitKind =
  | typeof HIT_KIND_GROUND
  | typeof HIT_KIND_OBJECT
  | typeof HIT_KIND_OBJECTS
  | typeof HIT_KIND_DECOR;
export const HIT_KIND_GROUND = 0;
export const HIT_KIND_OBJECT = 1;
export const HIT_KIND_OBJECTS = 2;
export const HIT_KIND_DECOR = 3;

/** The member `val` is, or null for a number no member has. */
export function of_HitKind(val: number): HitKind | null {
  return val === HIT_KIND_GROUND ||
    val === HIT_KIND_OBJECT ||
    val === HIT_KIND_OBJECTS ||
    val === HIT_KIND_DECOR
    ? val
    : null;
}

/** The member's name, as its source spells it. */
export function name_HitKind(val: HitKind): string {
  switch (val) {
    case HIT_KIND_GROUND:
      return "ground";
    case HIT_KIND_OBJECT:
      return "object";
    case HIT_KIND_OBJECTS:
      return "objects";
    case HIT_KIND_DECOR:
      return "decor";
  }
}

export const LIMIT = {
  REASON_CAP: 4 as number,
  /** A reason's words: its fail, its detail and how many (STREAM_STAT.reasons). */
  REASON_WORDS: 3 as number,
  /** How far back the tables keep, the core's frames and tasks and the UI's long frames alike: the last minute. */
  WINDOW_MS: 60000 as number,
  /** The most bytes one read hands over. A table is read a page at a time (perf.wit `table`), each read from the sequence the last ended at, up to the table's end; the worst frames, the string table and the statistics each fit one page. */
  PAGE_SZ: 8192 as number,
} as const;

/** One frame and the interval it opened (perf/frame.rs FrameSample): every figure of one frame, landed when the next frame closes the interval. `interval_ms` is that interval, `refresh_ms` the display's interval as the frame's pacing takes it, its measured one (pxl-core sys/sched/display.rs) or one interval at RATE_MAX_HZ before it is measured (pxl-core domain/scene/rate.rs `display_ms`), and `paced_ms` the frame's target interval, which it was paced at (rate.rs `target_ms`). `settled` is 1 where the frame asked for no next one, else 0. `budget_ms` is the frame's entry's budget, the time from the entry's start to its end (pxl-core sys/sched.rs `budget_ms`): the end is the frame's target interval less MARGIN_SHARE from the frame's start, the budget is 0 where the entry started past it, and one slice from the entry's start where it started the margin past the target or later (sys/sched.rs `Budget`); and `overran` is 1 where the scheduler found the budget spent as the tick ended, the clock at or past the entry's end (sys/sched.rs `spent`), else 0. The effects, the tick, the requests' pump and the stage are the frame's parts, in that order: `effects` is each effect's time in the frame, in EFFECT_NAME's order, `tick_ms` the tick's and `phases` each of its phases', `pump_ms` the requests declared and pumped (pxl-core frame.rs `run`), and `stage_parts` each part of the stage's, in STAGE_PART_NAME's order; the effects' time and the stage's are their sums. The pump and the stage's parts are timed while the profiler records, as the effects are. `fill_ms` is what the ground's fill took in the entry after the frame's, up to that entry's budget, and `fill_brick_cnt` the bricks it filled there (pxl-core domain/layer/terrain/clipmap.rs `fill_bricks`), each 0 where no brick was due. `perf_ms` is what the profiler cost the frame, its own record of it and the UI's reads of it. `invalidations` is how many times a frame was asked for between this frame's record and the one before, by reason, in INVALIDATION_REASON's order (pxl-core sys/sched.rs `take_invalidations`): what woke the frame, what was asked while it ran, and a refused frame's ask before it; an ask for a time counts when the time comes. `gpu_timed` is 1 where the GPU's read of this frame has landed, which comes a frame or two later and is written back into this frame's record (`gpu_frame` the renderer's number of it, counted from 1, by which a frame is found in every table, 0 where it drew none): its `gpu_spans` are then that read's, `gpu_busy_ms` the time they ran, those that overlap counted once, the frame's GPU time, which the spans' sum overstates where passes overlap (pxl-render gpu/api.rs `Gpu::gpu_busy_ms`), and `gpu_px` the world's pixels of the frame the read timed, what the spans were measured over; and 0 where nothing measured it (its read not landed yet or cut off by a stop, or the device keeps no timestamps), its spans, busy time and pixels 0 too. `scale` is the share of the display's pixels, each way, the world was drawn at, in every frame (pxl-core domain/scene/resolution.rs `drawn_scale`). `kind_name` indexes the renderer's FRAME_KIND. `draw_cnt` and `triangle_cnt` are the frame's, every pipeline a draw names (pxl-render gpu/draw.rs `DRAW_PIPELINES`), counted while the profiler records. `light_changed` says what changed that cast the light's map in the frame, a bit each in LIGHT_CHANGED's order (pxl-render ray/cast.rs `light_changed`), 0 where it did not cast. The ray lanes are the `ray_cnt` setting's, written into the frame they were counted in as the counts land a frame or two later, by the renderer's number of it as a GPU read is (pxl-render ray/count.rs `RayCount`), 0 where none landed: `eye_cast` and `light_cast` 1 where the eye cast in full or the light's map cast and its counts landed; then the eye's pixels (`eye_ray_cnt`) and the light's map's slots (`light_ray_cnt`), and a cast's each: the nodes its beam rays, its samples and its holes' walk visited (`_beam`, `_sample`, `_hole_walk`), the asks of the world among the last (`_ask_cnt`), its pixels or slots that took the visible voxel, a walk of their own as holes and the sky (`_voxel`, `_hole`, `_sky`), together every one it cast again, so the light's three over `light_ray_cnt` are the share of its map a cast cast again, and the boxes drawn (pxl-engine `count`). `loads` is how many tiles each base layer's tile cache took in during the frame, in BASE_KIND's order (pxl-core domain/layer/tile_cache.rs `take_load_cnt`). `upload_cnt` and `upload_bytes` are the renderer's upload table (pxl-render renderer.rs `FrameStat`), and `heap_used_bytes` the heap's high-water mark (pxl-core sys/heap.rs `HeapStat`). A lane's name says its unit: `_ms`, `_bytes`, `_px`, and `_name` for an index into a list of names. */
export const FRAME = {
  t: 0 as number,
  interval_ms: 1 as number,
  refresh_ms: 2 as number,
  paced_ms: 3 as number,
  settled: 4 as number,
  budget_ms: 5 as number,
  overran: 6 as number,
  tick_ms: 7 as number,
  phases: 8 as number,
  pump_ms: 10 as number,
  stage_parts: 11 as number,
  fill_ms: 15 as number,
  perf_ms: 16 as number,
  effects: 17 as number,
  invalidations: 24 as number,
  kind_name: 30 as number,
  gpu_timed: 31 as number,
  gpu_spans: 32 as number,
  gpu_busy_ms: 70 as number,
  gpu_px: 71 as number,
  scale: 72 as number,
  draw_cnt: 73 as number,
  triangle_cnt: 74 as number,
  gpu_frame: 75 as number,
  light_changed: 76 as number,
  eye_cast: 77 as number,
  eye_ray_cnt: 78 as number,
  eye_beam: 79 as number,
  eye_sample: 80 as number,
  eye_hole_walk: 81 as number,
  eye_ask_cnt: 82 as number,
  eye_voxel: 83 as number,
  eye_hole: 84 as number,
  eye_sky: 85 as number,
  eye_box_cnt: 86 as number,
  light_cast: 87 as number,
  light_ray_cnt: 88 as number,
  light_beam: 89 as number,
  light_sample: 90 as number,
  light_hole_walk: 91 as number,
  light_ask_cnt: 92 as number,
  light_voxel: 93 as number,
  light_hole: 94 as number,
  light_sky: 95 as number,
  light_box_cnt: 96 as number,
  loads: 97 as number,
  fill_brick_cnt: 101 as number,
  upload_cnt: 102 as number,
  upload_bytes: 103 as number,
  heap_used_bytes: 104 as number,
  WORDS: 105 as number,
} as const;

/** FRAME's lanes in groups, every lane in one (lane.test.ts), in the order a reader asks in turn — where the time went, what was drawn and cast, what landed, and what it holds. The trace prints every lane by its group's order, so a lane the core adds is printed without a word written for it; the panel shows some groups, a few rows each, and the report its own lines. The `rays` group's lanes lie in FRAME in its order, which the window's sums of them (STAT.frames.rays) keep. */
export const FRAME_GROUP = {
  time: [
    "t",
    "interval_ms",
    "refresh_ms",
    "paced_ms",
    "settled",
    "budget_ms",
    "overran",
  ],
  cpu: [
    "tick_ms",
    "phases",
    "pump_ms",
    "stage_parts",
    "fill_ms",
    "perf_ms",
    "effects",
    "invalidations",
  ],
  frame: ["kind_name"],
  gpu: [
    "gpu_timed",
    "gpu_spans",
    "gpu_busy_ms",
    "gpu_px",
    "scale",
    "draw_cnt",
    "triangle_cnt",
    "gpu_frame",
    "light_changed",
  ],
  rays: [
    "eye_cast",
    "eye_ray_cnt",
    "eye_beam",
    "eye_sample",
    "eye_hole_walk",
    "eye_ask_cnt",
    "eye_voxel",
    "eye_hole",
    "eye_sky",
    "eye_box_cnt",
    "light_cast",
    "light_ray_cnt",
    "light_beam",
    "light_sample",
    "light_hole_walk",
    "light_ask_cnt",
    "light_voxel",
    "light_hole",
    "light_sky",
    "light_box_cnt",
  ],
  tiles: ["loads", "fill_brick_cnt"],
  device: ["upload_cnt", "upload_bytes"],
  heap: ["heap_used_bytes"],
} as const;

/** One settled task (perf/task.rs TaskRecord). `path` is the bytes of the path it opened, zero past their end, and every other word an f64; `start_ms` is NaN for a task that never ran; `stream`, `outcome` and each of `phase_ids` index STREAM_NAME, OUTCOME_NAME and TASK_PHASE_NAME; a run reports each phase at most once, the first `phase_cnt` of `phase_ids` and `phase_vals` in the order it reported them. `at_file` is the offset of the file name, `at_file_sz` long, of the line a failure was made at, in the string table perf.wit `strtab` hands over, or 0 where the table had no room for the name, whose `at_line` is kept; every site word is 0 where the failure was the host's. */
export const TASK = {
  stream: 0 as number,
  path: 1 as number,
  priority: 4 as number,
  enqueue_ms: 5 as number,
  start_ms: 6 as number,
  end_ms: 7 as number,
  outcome: 8 as number,
  fail: 9 as number,
  detail: 10 as number,
  at_file: 11 as number,
  at_file_sz: 12 as number,
  at_line: 13 as number,
  phase_cnt: 14 as number,
  phase_ids: 15 as number,
  phase_vals: 18 as number,
  WORDS: 21 as number,
} as const;

/** One of the worst moving frames since the recording started, by its interval, of which the core keeps the longest few (pxl-core perf/record.rs `WORST_CAP`), which fit one page (LIMIT.PAGE_SZ), and hands them over together, longest first (perf.wit `worst`), and what changed from the frame before it: its uploads, their bytes and the heap's high-water mark, each this frame's less that one's, all 0 where `has_prev` is 0. A still interval is long on purpose and is never among them. */
export const WORST = {
  sample: 0 as number,
  has_prev: 105 as number,
  upload_cnt_delta: 106 as number,
  upload_delta_bytes: 107 as number,
  heap_used_delta_bytes: 108 as number,
  WORDS: 109 as number,
} as const;

/** A value's percentiles over n values: p50 and p95 each the nearest rank, the least value at or above which lie no more than that share of the n (sorted, the value at ceil(q × n) − 1), and the most. */
export const PERCENTILE = {
  p50: 0 as number,
  p95: 1 as number,
  max: 2 as number,
  WORDS: 3 as number,
} as const;

/** One stream's tasks in the window, at STAT.tasks.slots + its index × WORDS. Each reason is (fail, detail, n), commonest first; `phase_mean` is NaN for a phase no task reported. */
export const STREAM_STAT = {
  cnt: 0 as number,
  ok: 1 as number,
  aborted: 2 as number,
  err: 3 as number,
  lat_p50: 4 as number,
  lat_p95: 5 as number,
  reason_cnt: 6 as number,
  reasons: 7 as number,
  phase_n: 19 as number,
  phase_mean: 20 as number,
  WORDS: 23 as number,
} as const;

/** One registered step's timing, at STAT.steps.slots + its index × WORDS. `name` indexes STEP_NAME, whose phase is STEP_PHASE's (spec/client/core/perf/step.json5). */
export const STEP_STAT = {
  name: 0 as number,
  ms: 1 as number,
  cnt: 2 as number,
  max_ms: 3 as number,
  WORDS: 4 as number,
} as const;

/** The window's statistics `perf.stats` computes, over its `window_ms`: each section's words at its AT, the fields relative to it. Every word an f64. Packed: a section's AT is where the one before it ends, and WORDS where the last ends, both laid out by the generator. */
export const STAT = {
  frames: {
    AT: 0 as number,
    present: 0 as number,
    cnt: 1 as number,
    seconds: 2 as number,
    moving: 3 as number,
    moving_ms: 4 as number,
    overran: 5 as number,
    gpu_timed: 6 as number,
    interval: 7 as number,
    tick: 10 as number,
    effect: 13 as number,
    pump: 16 as number,
    stage: 19 as number,
    stage_parts_p95: 22 as number,
    fill: 26 as number,
    fill_brick_cnt: 29 as number,
    perf: 30 as number,
    effects_p95: 33 as number,
    invalidations: 40 as number,
    gpu_busy: 46 as number,
    gpu_busy_kinds: 49 as number,
    phases_p95: 76 as number,
    gpu_spans_p95: 78 as number,
    gpu_spans_cnt: 116 as number,
    kinds: 154 as number,
    gpu_px: 163 as number,
    rays: 164 as number,
    heap_used_delta_bytes: 184 as number,
    last: 185 as number,
    WORDS: 290 as number,
  },
  tasks: {
    AT: 290 as number,
    slots: 0 as number,
    WORDS: 138 as number,
  },
  steps: {
    AT: 428 as number,
    cnt: 0 as number,
    slots: 1 as number,
    WORDS: 41 as number,
  },
  /** What the frames held at the most or the least since the recording started, as the steps are timed, folded as each frame's record is taken (perf/record.rs `perf_fold_recording`): `frame_cnt` the frames folded; the heap's figures by their allocator's names (pxl-core sys/heap.rs `HeapStat`), the most of each but `largest_free`, the least, and `refusals` as it last stood; the body slots' peak (sys/io/body.rs `take_peak`); and `refusals` the tiles each base layer's tile cache refused a slot, summed, in BASE_KIND's order (domain/layer/tile_cache.rs `take_refusal_cnt`). Every word 0 before a frame is folded. */
  recording: {
    AT: 469 as number,
    frame_cnt: 0 as number,
    heap_live_max_bytes: 1 as number,
    heap_run_max_bytes: 2 as number,
    heap_slot_max_bytes: 3 as number,
    heap_range_max_bytes: 4 as number,
    heap_largest_free_min_bytes: 5 as number,
    heap_refusal_cnt: 6 as number,
    body_slots_peak: 7 as number,
    refusals: 8 as number,
    WORDS: 12 as number,
  },
  WORDS: 481 as number,
} as const;

/** What `perf.stats` finds as it reads, the window aside: each section's words at its AT, as STAT lays out its own. Every word an f64. */
export const NOW = {
  /** Whether it records, how long the recording has run, whether it lost what it had no room for (a table's oldest records, or a failure's file name the string table could not hold), and how many frames and tasks its tables hold. */
  recording_state: {
    AT: 0 as number,
    recording: 0 as number,
    elapsed_ms: 1 as number,
    truncated: 2 as number,
    frame_cnt: 3 as number,
    task_cnt: 4 as number,
    WORDS: 5 as number,
  },
  /** Whether the core draws, as the read finds it, recording or not, every count since the core booted: `frame_cnt` the frames the renderer encoded (pxl-render renderer.rs `drawn_frame_cnt`); `refused_admit` and `refused_stream` the frames that did not run, as the GPU admitted none (pxl-gpu-guest `admit::frame`) or the host still held the stream (pxl-gpu-guest stream.rs) (frame.rs `frame_task`).
   * Then the device, as the host wrote its block (spec/gpu/api.json5 DEVICE), the same on the web and natively: `generation` the devices lost so far (pxl-render gpu/api.rs `device_generation`); `status` its STATUS, 0 before the block is written; `err_cnt` the uncaptured errors it raised; and `unfinished` the frames whose work its queue has not done, each a frame the host asked the queue to say done and has not heard of. */
  draw: {
    AT: 5 as number,
    frame_cnt: 0 as number,
    refused_admit: 1 as number,
    refused_stream: 2 as number,
    generation: 3 as number,
    status: 4 as number,
    err_cnt: 5 as number,
    unfinished: 6 as number,
    WORDS: 7 as number,
  },
  /** Where the camera, its pivot and the pointer are as the read finds them, recording or not: `eye_x`..`eye_z`, `dist`, `pivot_x`..`pivot_z`, `hover_x`..`hover_z` and `place_x`..`place_z` in units, angles in radians, `pivot_lon` and `pivot_lat` in degrees and `pivot_ele` in meters, a zoom as its number, and NaN where there is none.
   * The eye is where the camera looks from and the pivot the point it turns about, under the eye in the first-person camera, where the character stands (pxl-core domain/control/camera.rs `eye_world`, camera/controller.rs `pivot`).
   * The hover is what the pointer meets, exactly, at the zooms the eye's rays ask, whatever the tool could strike, and `cell_zoom` the zoom the eye's rays draw there (pxl-core domain/query/support.rs `cell_zoom_read`).
   * The pick is what the tool held strikes there (pxl-core domain/control/edit/pick.rs `pick_at`): `hit_kind` what it met, an index into HIT_KIND; `pick_zoom` the zoom of the box struck and `ray_zoom` the rays' there, `folded` 1 where they draw coarser than the box, which strikes nothing. `place_x`..`place_z` is the corner the tool takes (pxl-core domain/control/edit.rs `Hover`), `place_zoom` the zoom it places at, and `blocked` 1 where that room is taken (domain/control/edit/collision.rs `is_placement_blocked`). */
  view: {
    AT: 12 as number,
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
    pivot_ele: 11 as number,
    hover_x: 12 as number,
    hover_y: 13 as number,
    hover_z: 14 as number,
    cell_zoom: 15 as number,
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
  WORDS: 37 as number,
} as const;

/** The tables the profiler keeps, each read by sequence (perf.wit `table`): the frames and the tasks, oldest first. A task's row never changes once it has landed; a frame's is written again a frame or two later, as its GPU read and its ray counts land (FRAME). */
export const TABLE_NAMES = ["frames", "tasks"] as const;
