// @pxl/client/abi/perf.spec.ts
//
// GENERATED from spec/client/core/perf/perf.json5 (xpute-gen data.rs)
//
// The profiler's records and sections as the core lays them out and the client reads
// them from the replies that hand them over (perf/record.rs, ts/client/src/service/core/perf.ts):
// word offsets, eight bytes a word. The names the numbers index are their owners':
// a task's io's (spec/client/core/sys/io/demand.json5), a step's and an effect's the
// frame's (spec/client/core/perf/step.json5), a GPU span's the renderer's
// (spec/client/render/gpu/span.json5). A packed table gives each field's
// width and the generator lays the offsets out. Nothing is built
// for a read: the core writes numbers where they stay, and the words for them are
// the UI's. LIMIT holds the bounds the profiler keeps, and the one a build made for
// measuring hands its GPU hosts, which the core and the client both read.

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

export const LIMIT = {
  REASON_CAP: 4 as number,
  /** A reason's words: its fail, its detail and how many (STREAM_STAT.reasons). */
  REASON_WORDS: 3 as number,
  STEP_CAP: 32 as number,
  /** How far back the rings keep, the core's frames and tasks and the UI's long frames alike: the last minute. */
  WINDOW_MS: 60000 as number,
  /** The most bytes one read of a ring hands over (perf.wit `ring`): a ring is read a page at a time, each from where the last ended, until a page comes short. The string table and the statistics fit one page. */
  PAGE_BYTES: 8192 as number,
  /** In a build made for measuring, the hosts log a record of the GPU stream, the stream's run or the core's entry slower than this, the page's and the device's alike (xpute-gpu-link `GpuConfig::slow_ms`, @xpute/gpu-link `GpuConfig`). */
  SLOW_MS: 1000 as number,
} as const;

/** One frame and the interval it opened (perf/frame.rs FrameSample): every figure of one frame, landed when the next frame closes the interval. `interval_ms` is that interval, `refresh_ms` the display's measured interval (pxl-core sys/sched/display.rs), and `paced_ms` the gap since the last frame, which the frame was paced at (pxl-core frame.rs `run`). `settled` is 1 where the frame asked for no next one, else 0. `effects_ms`, `tick_ms` and `stage_ms` are the frame's parts, `stage_ms` the requests' pump and the render, and `perf_ms` what the profiler cost it, its own record of the frame and the UI's reads of it. `effects` is each effect's time in the frame, in EFFECT_NAME's order, `effects_ms` their sum. `gpu_timed` is 1 where the GPU's read of this frame has landed, which comes a frame or two later and is written back into this frame's record (`gpu_frame` the renderer's number of it, 0 where it drew none): its `gpu_spans` are then that read's, `gpu_busy_ms` the time they ran, those that overlap counted once, the frame's GPU time, which the spans' sum overstates where passes overlap (pxl-render gpu/api.rs `Gpu::gpu_busy_ms`), and `gpu_px` the world's pixels of the frame the read timed, what the spans were measured over; and 0 where nothing measured it (its read not landed yet or cut off by a stop, or the device keeps no timestamps), its spans, busy time and pixels 0 too. `scale` is the share of the display's pixels, each way, the world was drawn at, in every frame (pxl-core domain/scene/resolution.rs `drawn_scale`). `kind_name` indexes the renderer's FRAME_KIND. `draw_cnt` and `triangle_cnt` are the frame's, every pipeline a draw names (pxl-render gpu/draw.rs `DRAW_PIPELINES`), counted while the profiler records. `light_changed` says what changed that cast the light's map in the frame, a bit each in LIGHT_CHANGED's order (pxl-render ray/cast.rs `light_changed`), 0 where it did not cast; `decor_rect_cnt` is how many rects decor's slots were marked by to generate again in it, the bricks handed in it and the edits and cuts held (pxl-render gpu/generator.rs `prepare`), 0 where it marked none. The ray columns are the `ray_cnt` setting's, written into the frame they were counted in as the counts land a frame or two later, by the renderer's number of it as a GPU read is (pxl-render ray/count.rs `RayCount`), 0 where none landed: `eye_cast` and `light_cast` 1 where the eye cast in full or the light's map cast and its counts landed; then the eye's pixels (`eye_ray_cnt`) and the light's map's slots (`light_ray_cnt`), and an observer's each: the nodes its beam rays, its samples and its holes' walk visited (`_beam`, `_sample`, `_hole_walk`), the asks of the world among the last (`_ask_cnt`), its pixels or slots that took the visible voxel, a walk of their own as holes and the sky (`_voxel`, `_hole`, `_sky`), together every one it cast again, so the light's three over `light_ray_cnt` are the share of its map a cast cast again, and the boxes drawn (pxl-engine `count`). `terrain_load_cnt` is how many terrain tiles the tile cache took in during the frame (pxl-core domain/layer/tile_cache.rs `take_load_cnt`). `upload_cnt` and `upload_bytes` are the renderer's upload table and `pack_bytes` its texture pack (pxl-render renderer.rs `FrameStat`), and `heap_used_bytes` the heap's high-water mark (pxl-core sys/heap.rs `HeapStat`). A column's name says its unit: `_ms`, `_bytes`, `_px`, and `_name` for an index into a list of names. */
export const FRAME = {
  t: 0 as number,
  interval_ms: 1 as number,
  refresh_ms: 2 as number,
  paced_ms: 3 as number,
  settled: 4 as number,
  budget_ms: 5 as number,
  effects_ms: 6 as number,
  tick_ms: 7 as number,
  phases: 8 as number,
  stage_ms: 10 as number,
  perf_ms: 11 as number,
  effects: 12 as number,
  kind_name: 20 as number,
  gpu_timed: 21 as number,
  gpu_spans: 22 as number,
  gpu_busy_ms: 60 as number,
  gpu_px: 61 as number,
  scale: 62 as number,
  draw_cnt: 63 as number,
  triangle_cnt: 64 as number,
  gpu_frame: 65 as number,
  light_changed: 66 as number,
  decor_rect_cnt: 67 as number,
  eye_cast: 68 as number,
  eye_ray_cnt: 69 as number,
  eye_beam: 70 as number,
  eye_sample: 71 as number,
  eye_hole_walk: 72 as number,
  eye_ask_cnt: 73 as number,
  eye_voxel: 74 as number,
  eye_hole: 75 as number,
  eye_sky: 76 as number,
  eye_box_cnt: 77 as number,
  light_cast: 78 as number,
  light_ray_cnt: 79 as number,
  light_beam: 80 as number,
  light_sample: 81 as number,
  light_hole_walk: 82 as number,
  light_ask_cnt: 83 as number,
  light_voxel: 84 as number,
  light_hole: 85 as number,
  light_sky: 86 as number,
  light_box_cnt: 87 as number,
  terrain_load_cnt: 88 as number,
  upload_cnt: 89 as number,
  upload_bytes: 90 as number,
  pack_bytes: 91 as number,
  heap_used_bytes: 92 as number,
  WORDS: 93 as number,
} as const;

/** FRAME's columns in groups, every column in one (column.test.ts), in the order a reader asks in turn — where the time went, what was drawn and cast, what landed, and what it holds. The trace prints every column by its group's order, so a column the core adds is printed without a word written for it; the panel shows some groups, a few rows each, and the report its own lines. The `rays` group's columns lie in FRAME in its order, which the window's sums of them (STAT.frames.rays) keep. */
export const FRAME_GROUP = {
  time: ["t", "interval_ms", "refresh_ms", "paced_ms", "settled", "budget_ms"],
  cpu: ["effects_ms", "tick_ms", "phases", "stage_ms", "perf_ms", "effects"],
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
    "decor_rect_cnt",
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
  tiles: ["terrain_load_cnt"],
  device: ["upload_cnt", "upload_bytes", "pack_bytes"],
  heap: ["heap_used_bytes"],
} as const;

/** One settled task (perf/task.rs TaskRecord). `path` is the bytes of the path it opened, zero past their end, and every other word an f64; `start_ms` is NaN for a task that never ran; `stream`, `outcome` and each of `phase_ids` index STREAM_NAME, OUTCOME_NAME and TASK_PHASE_NAME. `at_file` is the offset of the file name, `at_file_bytes` long, of the line a failure was made at, in the string table perf.wit `strtab` hands over, or 0 where the table had no room for the name, whose `at_line` is kept; every site word is 0 where the failure was the host's. */
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
  at_file_bytes: 12 as number,
  at_line: 13 as number,
  phase_cnt: 14 as number,
  phase_ids: 15 as number,
  phase_vals: 21 as number,
  WORDS: 27 as number,
} as const;

/** One of the worst moving frames since the recording started, by its interval, and what changed from the frame before it: its uploads, their bytes and the heap's high-water mark, each this frame's less that one's, all 0 where `has_prev` is 0. A still interval is long on purpose and is never among them. */
export const WORST = {
  sample: 0 as number,
  has_prev: 93 as number,
  upload_cnt_delta: 94 as number,
  upload_delta_bytes: 95 as number,
  heap_used_delta_bytes: 96 as number,
  WORDS: 97 as number,
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

/** One registered step's timing, at STAT.steps.slots + its index × WORDS. `name` indexes STEP_NAME. */
export const STEP_STAT = {
  phase: 0 as number,
  name: 1 as number,
  ms: 2 as number,
  cnt: 3 as number,
  max_ms: 4 as number,
  WORDS: 5 as number,
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
    stage: 16 as number,
    perf: 19 as number,
    effects_p95: 22 as number,
    gpu_busy: 30 as number,
    gpu_busy_kinds: 33 as number,
    phases_p95: 60 as number,
    gpu_spans_p95: 62 as number,
    gpu_spans_cnt: 100 as number,
    kinds: 138 as number,
    gpu_px: 147 as number,
    rays: 148 as number,
    first: 168 as number,
    last: 261 as number,
    WORDS: 354 as number,
  },
  tasks: {
    AT: 354 as number,
    slots: 0 as number,
    WORDS: 138 as number,
  },
  steps: {
    AT: 492 as number,
    cnt: 0 as number,
    slots: 1 as number,
    WORDS: 161 as number,
  },
  /** What the frames held at the most or the least since the recording started, as the steps are timed, folded as each frame's record is taken (perf/record.rs `perf_fold_recording`): `frame_cnt` the frames folded; the heap's figures by their allocator's names (pxl-core sys/heap.rs `HeapStat`), the most of each but `largest_free`, the least, and `refusals` as it last stood; the body slots' peak (sys/io/body.rs `take_peak`); and `terrain_refusal_cnt` the terrain tiles the tile cache refused a slot, summed (domain/layer/tile_cache.rs `take_refusal_cnt`). Every word 0 before a frame is folded. */
  recording: {
    AT: 653 as number,
    frame_cnt: 0 as number,
    heap_live_max_bytes: 1 as number,
    heap_run_max_bytes: 2 as number,
    heap_slot_max_bytes: 3 as number,
    heap_range_max_bytes: 4 as number,
    heap_largest_free_min_bytes: 5 as number,
    heap_refusal_cnt: 6 as number,
    body_slots_peak: 7 as number,
    terrain_refusal_cnt: 8 as number,
    WORDS: 9 as number,
  },
  WORDS: 662 as number,
} as const;

/** What `perf.stats` finds as it reads, the window aside: each section's words at its AT, as STAT lays out its own. Every word an f64. */
export const NOW = {
  /** Whether it records, how long the recording has run, whether it lost what it had no room for (a ring's oldest records, or a failure's file name the string table could not hold), and how many records each ring holds (RING_NAMES). */
  recording_state: {
    AT: 0 as number,
    recording: 0 as number,
    elapsed_ms: 1 as number,
    truncated: 2 as number,
    frame_cnt: 3 as number,
    task_cnt: 4 as number,
    worst_cnt: 5 as number,
    WORDS: 6 as number,
  },
  /** Whether the core draws, as the read finds it, recording or not, every count since the core booted: `frame_cnt` the frames the renderer encoded (pxl-render renderer.rs `drawn_frame_cnt`); `refused_admit` and `refused_stream` the frames that did not run, as the GPU admitted none (xpute-gpu-guest `admit::frame`) or the host still held the stream (xpute-gpu-guest stream.rs) (frame.rs `frame_task`).
   * Then the device, as the host wrote its block (xpute spec DEVICE), the same on a page and on a device: `generation` the devices lost so far (pxl-render gpu/api.rs `device_generation`); `status` its STATUS, 0 before the block is written; `err_cnt` the uncaptured errors it raised; and `unfinished` the frames whose work its queue has not done, each a frame the host asked the queue to say done and has not heard of. */
  draw: {
    AT: 6 as number,
    frame_cnt: 0 as number,
    refused_admit: 1 as number,
    refused_stream: 2 as number,
    generation: 3 as number,
    status: 4 as number,
    err_cnt: 5 as number,
    unfinished: 6 as number,
    WORDS: 7 as number,
  },
  /** Where the camera, its pivot and the pointer are as the read finds them, recording or not: `eye_x`..`eye_z`, `dist`, `pivot_x`..`pivot_z`, `hover_x`..`hover_z` and `place_x`..`place_z` in units, angles in radians, `pivot_lon` and `pivot_lat` in degrees and `pivot_alt` in meters, a zoom as its number, and NaN where there is none.
   * The eye is where the camera looks from and the pivot the point it turns about, the eye itself in the first-person camera, where the character stands (pxl-core domain/control/camera.rs `eye_world`, camera/controller.rs `pivot`).
   * The hover is what the pointer meets, exactly, at the zooms the eye's rays ask, whatever the tool could strike, and `cell_zoom` the zoom the eye's rays draw there (pxl-core domain/query/ray.rs `by_dist`).
   * The pick is what the tool held strikes there (pxl-core domain/control/edit/pick.rs `pick_at`): `hit_kind` what it met, an index into HIT_KIND; `pick_zoom` the zoom of the box struck and `ray_zoom` the rays' there, `folded` 1 where they draw coarser than the box, which strikes nothing. `place_x`..`place_z` is the corner the tool takes (pxl-core domain/control/edit.rs `Hover`), `place_zoom` the zoom it places at, and `blocked` 1 where that room is taken (domain/control/edit/collision.rs `is_placement_blocked`). */
  view: {
    AT: 13 as number,
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
  WORDS: 38 as number,
} as const;

/** The rings the profiler keeps: the frames, the tasks, and the worst moving frames since the recording started. */
export const RING_NAMES = ["frames", "tasks", "worst"] as const;
