// pxl-core/perf/perf.spec.rs
//
// GENERATED from spec/client/core/perf/perf.json
//
// The profiler's records and sections as the core lays them out and the client reads
// them in place (perf/record.rs, ts/client/src/service/core/perf.ts):
// word offsets, eight bytes a word. The names the numbers index are their owners':
// a task's io's (spec/client/core/sys/io/demand.json), a step's and an effect's the
// turn's (spec/client/core/turn/step.json), a GPU span's the renderer's
// (spec/client/render/gpu/span.json). A packed table gives each field's
// width and the generator lays the offsets out. Nothing is built
// for a read: the core writes numbers where they stay, and the words for them are
// the UI's.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    non_camel_case_types,
    dead_code,
    reason = "generated from the spec"
)]

pub const WAKES: usize = 3;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Wake {
    next = 0,
    timer = 1,
    none = 2,
}

impl Wake {
    /// Every member, in the JSON's own order.
    pub const ALL: [Wake; 3] = [Wake::next, Wake::timer, Wake::none];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<Wake> {
        Wake::ALL.into_iter().find(|member| *member as u8 == val)
    }
}

pub const HIT_KINDS: usize = 4;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum HitKind {
    ground = 0,
    object = 1,
    objects = 2,
    decor = 3,
}

impl HitKind {
    /// Every member, in the JSON's own order.
    pub const ALL: [HitKind; 4] = [
        HitKind::ground,
        HitKind::object,
        HitKind::objects,
        HitKind::decor,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<HitKind> {
        HitKind::ALL.into_iter().find(|member| *member as u8 == val)
    }
}

pub mod LIMIT {
    pub const REASONS: u32 = 4;
    pub const REASON_WORDS: u32 = 3;
    pub const STEP_CAP: u32 = 32;
    pub const WINDOW_MS: u32 = 60000;
}

/// One frame turn and the interval it opened (perf/frame.rs FrameSample): every figure of one turn, landed when the next frame turn closes the interval. `refresh_ms` is the display's refresh and `paced_ms` the interval the runtime paced the frame at, a whole number of refreshes, both as the runtime granted them (@xpute/runtime sched/frame_pacing.ts `FrameGrant`); a moving frame whose interval ran past `paced_ms` by more than half a refresh missed a vsync (perf/frame.rs `missed`). `settled` is 0 or 1; `turns` and `busy_ms` count every turn the interval held, the frame's and those without a runtime frame, rise to fall: what they ran past the interval is read off the two (perf/frame.rs `over_ms`). `effects_ms`, `tick_ms`, `stage_ms`, `tasks_ms` and `perf_ms` are their parts, `stage_ms` the turn's pump and the render, and `perf_ms` what the profiler cost it, its own record of the frame and the UI's reads of it; what they leave of `busy_ms` is unmeasured. `effects` is each effect's time in the frame turn, in EFFECT_NAMES' order, `effects_ms` their sum. `gpu_timed` is 1 where the GPU's read of this frame has landed, which comes a frame or two later and is written back into this frame's record (`gpu_frame` the renderer's number of it, 0 where it drew none): its `gpu_spans` are then that read's, `gpu_busy_ms` the time they ran, those that overlap counted once, the frame's GPU time, which the spans' sum overstates where passes overlap (pxl-render gpu/api.rs `Gpu::gpu_busy_ms`), and `gpu_px` the world's pixels of the frame the read timed, what the spans were measured over; and 0 where nothing measured it (its read not landed yet or cut off by a stop, or the device keeps no timestamps), its spans, busy time and pixels 0 too. `scale` is the share of the display's pixels, each way, the world was drawn at, in every frame (pxl-core domain/scene/resolution.rs `drawn_scale`). `kind_name` indexes the renderer's FRAME_KINDS and `wake_name` WAKES. `draws` and `triangles` are the frame's, every pipeline a draw names (pxl-render gpu/draw.rs `DRAW_PIPELINES`), counted while the profiler records. `light_changed` says what changed that cast the light's map in the frame, a bit each in LIGHT_CHANGED's order (pxl-render ray/cast.rs `light_changed`), 0 where it did not cast; `decor_rects` is how many rects decor's slots were marked by to generate again in it, the bricks handed in it and the edits and cuts held (pxl-render gpu/generator.rs `prepare`), 0 where it marked none. The ray columns are the `ray_cnts` setting's, written into the frame they were counted in as the counts land a frame or two later, by the renderer's number of it as a GPU read is (pxl-render ray/count.rs `RayCount`), 0 where none landed: `eye_cast` and `light_cast` 1 where the eye cast in full or the light's map cast and its counts landed; then the eye's pixels (`eye_rays`) and the light's map's slots (`light_rays`), and an observer's each: the nodes its beam rays, its samples and its holes' walk visited (`_beam`, `_sample`, `_hole_walk`), the asks of the world among the last (`_asks`), its pixels or slots that took the visible voxel, a walk of their own as holes and the sky (`_voxel`, `_hole`, `_sky`), together every one it cast again, so the light's three over `light_rays` are the share of its map a cast cast again, and the boxes drawn (pxl-engine `count`). `terrain_loads` is how many terrain tiles the tile cache took in during the frame turn (pxl-core domain/layer/tile_cache.rs `take_loads`). `uploads` and `uploads_bytes` are the renderer's upload table and `pack_bytes` its texture pack (pxl-render renderer.rs `FrameStat`), and `heap_used_bytes` the heap's high-water mark (xpute-core mem/heap.rs `HeapStat`). A column's name says its unit: `_ms`, `_bytes`, `_px`, and `_name` for an index into a list of names.
pub mod FRAME {
    pub const t: u32 = 0;
    pub const interval_ms: u32 = 1;
    pub const refresh_ms: u32 = 2;
    pub const paced_ms: u32 = 3;
    pub const settled: u32 = 4;
    pub const turns: u32 = 5;
    pub const busy_ms: u32 = 6;
    pub const over_ms: u32 = 7;
    pub const budget_ms: u32 = 8;
    pub const effects_ms: u32 = 9;
    pub const tick_ms: u32 = 10;
    pub const phases: u32 = 11;
    pub const stage_ms: u32 = 16;
    pub const tasks_ms: u32 = 17;
    pub const perf_ms: u32 = 18;
    pub const effects: u32 = 19;
    pub const kind_name: u32 = 28;
    pub const wake_name: u32 = 29;
    pub const gpu_timed: u32 = 30;
    pub const gpu_spans: u32 = 31;
    pub const gpu_busy_ms: u32 = 69;
    pub const gpu_px: u32 = 70;
    pub const scale: u32 = 71;
    pub const draws: u32 = 72;
    pub const triangles: u32 = 73;
    pub const gpu_frame: u32 = 74;
    pub const light_changed: u32 = 75;
    pub const decor_rects: u32 = 76;
    pub const eye_cast: u32 = 77;
    pub const eye_rays: u32 = 78;
    pub const eye_beam: u32 = 79;
    pub const eye_sample: u32 = 80;
    pub const eye_hole_walk: u32 = 81;
    pub const eye_asks: u32 = 82;
    pub const eye_voxel: u32 = 83;
    pub const eye_hole: u32 = 84;
    pub const eye_sky: u32 = 85;
    pub const eye_boxes: u32 = 86;
    pub const light_cast: u32 = 87;
    pub const light_rays: u32 = 88;
    pub const light_beam: u32 = 89;
    pub const light_sample: u32 = 90;
    pub const light_hole_walk: u32 = 91;
    pub const light_asks: u32 = 92;
    pub const light_voxel: u32 = 93;
    pub const light_hole: u32 = 94;
    pub const light_sky: u32 = 95;
    pub const light_boxes: u32 = 96;
    pub const terrain_loads: u32 = 97;
    pub const uploads: u32 = 98;
    pub const uploads_bytes: u32 = 99;
    pub const pack_bytes: u32 = 100;
    pub const heap_used_bytes: u32 = 101;
    pub const WORDS: u32 = 102;
}

/// FRAME's columns in groups, every column in one (column.test.ts), in the order a reader asks in turn — where the time went, what was drawn and cast, what landed, and what it holds. The trace prints every column by its group's order, so a column the core adds is printed without a word written for it; the panel shows some groups, a few rows each, and the report its own lines. The `rays` group's columns lie in FRAME in its order, which the window's sums of them (STAT.frames.rays) keep.
pub mod FRAME_GROUP {
    pub const time: usize = 9;
    pub const turn: usize = 7;
    pub const frame: usize = 2;
    pub const gpu: usize = 10;
    pub const rays: usize = 20;
    pub const tiles: usize = 1;
    pub const device: usize = 3;
    pub const heap: usize = 1;
}

/// One settled task (perf/task.rs TaskRecord). `path` is the bytes of the path it opened, zero past their end, and every other word an f64; `start_ms` is NaN for a task that never ran; `stream`, `outcome` and each of `phase_ids` index STREAM_NAMES, OUTCOME_NAMES and TASK_PHASE_NAMES. `at_file` is the address of the file name, `at_file_bytes` long, of the line a failure was made at, and 0 where the failure was the runtime's.
pub mod TASK {
    pub const stream: u32 = 0;
    pub const path: u32 = 1;
    pub const priority: u32 = 4;
    pub const enqueue_ms: u32 = 5;
    pub const start_ms: u32 = 6;
    pub const end_ms: u32 = 7;
    pub const outcome: u32 = 8;
    pub const fail: u32 = 9;
    pub const detail: u32 = 10;
    pub const at_file: u32 = 11;
    pub const at_file_bytes: u32 = 12;
    pub const at_line: u32 = 13;
    pub const phase_cnt: u32 = 14;
    pub const phase_ids: u32 = 15;
    pub const phase_vals: u32 = 21;
    pub const WORDS: u32 = 27;
}

/// One of the worst moving frames since the recording started, by its interval, and what changed from the frame before it: its uploads, their bytes and the heap's high-water mark, each this frame's less that one's, all 0 where `has_prev` is 0. A still interval is long on purpose and is never among them.
pub mod WORST {
    pub const sample: u32 = 0;
    pub const has_prev: u32 = 102;
    pub const uploads_delta: u32 = 103;
    pub const uploads_delta_bytes: u32 = 104;
    pub const heap_used_delta_bytes: u32 = 105;
    pub const WORDS: u32 = 106;
}

/// A value's percentiles over n values: p50 and p95 each the nearest rank, the least value at or above which lie no more than that share of the n (sorted, the value at ceil(q × n) − 1), and the most.
pub mod PERCENTILE {
    pub const p50: u32 = 0;
    pub const p95: u32 = 1;
    pub const max: u32 = 2;
    pub const WORDS: u32 = 3;
}

/// One stream's tasks in the window, at STAT.tasks.slots + its index × WORDS. Each reason is (fail, detail, n), commonest first; `phase_mean` is NaN for a phase no task reported.
pub mod STREAM_STAT {
    pub const cnt: u32 = 0;
    pub const ok: u32 = 1;
    pub const aborted: u32 = 2;
    pub const err: u32 = 3;
    pub const lat_p50: u32 = 4;
    pub const lat_p95: u32 = 5;
    pub const reason_cnt: u32 = 6;
    pub const reasons: u32 = 7;
    pub const phase_n: u32 = 19;
    pub const phase_mean: u32 = 20;
    pub const WORDS: u32 = 23;
}

/// One registered step's timing, at STAT.steps.slots + its index × WORDS. `name` indexes STEP_NAMES.
pub mod STEP_STAT {
    pub const phase: u32 = 0;
    pub const name: u32 = 1;
    pub const ms: u32 = 2;
    pub const cnt: u32 = 3;
    pub const max_ms: u32 = 4;
    pub const WORDS: u32 = 5;
}

/// The window's statistics PERF_STATS computes, over its `window_ms`: each section's words at its AT, the fields relative to it. Every word an f64. Packed: a section's AT is where the one before it ends, and WORDS where the last ends, both laid out by the generator.
pub mod STAT {
    pub mod frames {
        pub const AT: u32 = 0;
        pub const present: u32 = 0;
        pub const cnt: u32 = 1;
        pub const seconds: u32 = 2;
        pub const moving: u32 = 3;
        pub const moving_ms: u32 = 4;
        pub const missed: u32 = 5;
        pub const overran: u32 = 6;
        pub const long: u32 = 7;
        pub const turns: u32 = 8;
        pub const gpu_timed: u32 = 9;
        pub const interval: u32 = 10;
        pub const busy: u32 = 13;
        pub const tick: u32 = 16;
        pub const effect: u32 = 19;
        pub const stage: u32 = 22;
        pub const task: u32 = 25;
        pub const perf: u32 = 28;
        pub const rest: u32 = 31;
        pub const effects_p95: u32 = 34;
        pub const gpu_busy: u32 = 43;
        pub const gpu_busy_kinds: u32 = 46;
        pub const phases_p95: u32 = 73;
        pub const gpu_spans_p95: u32 = 78;
        pub const gpu_spans_cnt: u32 = 116;
        pub const kinds: u32 = 154;
        pub const wakes: u32 = 163;
        pub const gpu_px: u32 = 166;
        pub const rays: u32 = 167;
        pub const first: u32 = 187;
        pub const last: u32 = 289;
        pub const WORDS: u32 = 391;
    }
    pub mod tasks {
        pub const AT: u32 = 391;
        pub const slots: u32 = 0;
        pub const WORDS: u32 = 138;
    }
    pub mod steps {
        pub const AT: u32 = 529;
        pub const cnt: u32 = 0;
        pub const slots: u32 = 1;
        pub const WORDS: u32 = 161;
    }
    /// What the frames held at the most or the least since the recording started, as the steps are timed, folded as each frame turn's record is taken (perf/record.rs `perf_fold_recording`): `frames` the frame turns folded; the heap's figures by their allocator's names (xpute-core mem/heap.rs `HeapStat`), the most of each but `largest_free`, the least, and `refusals` as it last stood; the door's rings' peaks and the body slots' (xpute-core ipc/door.rs `DoorStat`, sys/io/body.rs `take_peak`), `ring_left_peak` its `left_peak`; and `terrain_refusals` the terrain tiles the tile cache refused a slot, summed (domain/layer/tile_cache.rs `take_refusals`). Every word 0 before a frame is folded.
    pub mod recording {
        pub const AT: u32 = 690;
        pub const frames: u32 = 0;
        pub const heap_live_max_bytes: u32 = 1;
        pub const heap_run_max_bytes: u32 = 2;
        pub const heap_slot_max_bytes: u32 = 3;
        pub const heap_range_max_bytes: u32 = 4;
        pub const heap_largest_free_min_bytes: u32 = 5;
        pub const heap_refusals: u32 = 6;
        pub const ring_cq_peak: u32 = 7;
        pub const ring_sq_peak: u32 = 8;
        pub const ring_left_peak: u32 = 9;
        pub const body_slots_peak: u32 = 10;
        pub const terrain_refusals: u32 = 11;
        pub const WORDS: u32 = 12;
    }
    pub const WORDS: u32 = 702;
}

/// What PERF_STATS finds as it reads, the window aside: each section's words at its AT, as STAT lays out its own. Every word an f64.
pub mod NOW {
    pub mod recording_state {
        pub const AT: u32 = 0;
        pub const recording: u32 = 0;
        pub const elapsed_ms: u32 = 1;
        pub const truncated: u32 = 2;
        pub const WORDS: u32 = 3;
    }
    /// Where the camera, its pivot and the pointer are as the read finds them, recording or not: `eye_x`..`eye_z`, `dist`, `pivot_x`..`pivot_z`, `hover_x`..`hover_z` and `place_x`..`place_z` in units, angles in radians, `pivot_lon` and `pivot_lat` in degrees and `pivot_alt` in meters, a zoom as its number, and NaN where there is none.
    /// The eye is where the camera looks from and the pivot the point it turns about, the eye itself in the first-person camera, where the character stands (pxl-core domain/control/camera.rs `eye_world`, camera/controller.rs `pivot`).
    /// The hover is what the pointer meets, exactly, at the zooms the eye's rays ask, whatever the tool could strike, and `cell_zoom` the zoom the eye's rays draw there (pxl-core domain/query/ray.rs `by_dist`).
    /// The pick is what the tool held strikes there (pxl-core domain/control/edit/pick.rs `pick_at`): `hit_kind` what it met, an index into HIT_KINDS; `pick_zoom` the zoom of the box struck and `ray_zoom` the rays' there, `folded` 1 where they draw coarser than the box, which strikes nothing. `place_x`..`place_z` is the corner the tool takes (pxl-core domain/control/edit.rs `Hover`), `place_zoom` the zoom it places at, and `blocked` 1 where that room is taken (domain/control/edit/collision.rs `is_placement_blocked`).
    pub mod view {
        pub const AT: u32 = 3;
        pub const eye_x: u32 = 0;
        pub const eye_y: u32 = 1;
        pub const eye_z: u32 = 2;
        pub const dist: u32 = 3;
        pub const polar: u32 = 4;
        pub const azimuth: u32 = 5;
        pub const pivot_x: u32 = 6;
        pub const pivot_y: u32 = 7;
        pub const pivot_z: u32 = 8;
        pub const pivot_lon: u32 = 9;
        pub const pivot_lat: u32 = 10;
        pub const pivot_alt: u32 = 11;
        pub const hover_x: u32 = 12;
        pub const hover_y: u32 = 13;
        pub const hover_z: u32 = 14;
        pub const cell_zoom: u32 = 15;
        pub const hit_kind: u32 = 16;
        pub const pick_zoom: u32 = 17;
        pub const ray_zoom: u32 = 18;
        pub const folded: u32 = 19;
        pub const place_x: u32 = 20;
        pub const place_y: u32 = 21;
        pub const place_z: u32 = 22;
        pub const place_zoom: u32 = 23;
        pub const blocked: u32 = 24;
        pub const WORDS: u32 = 25;
    }
    pub const WORDS: u32 = 28;
}

/// What PERF_STATS answers with the address of: u32 words, where STAT and NOW lie in the module's memory and their words.
pub mod STAT_SLOT {
    pub const stats_at: u32 = 0;
    pub const stats_words: u32 = 1;
    pub const now_at: u32 = 2;
    pub const now_words: u32 = 3;
    pub const WORDS: u32 = 4;
}

/// Where one ring lies: its address in the module's memory and its record count.
pub mod RING {
    pub const at: u32 = 0;
    pub const cnt: u32 = 1;
    pub const WORDS: u32 = 2;
}

pub const RING_NAMES: usize = 3;

/// What PERF_RINGS answers with the address of: u32 words, a RING apiece in RING_NAMES' order.
pub mod RING_SLOT {
    pub const slots: u32 = 0;
    pub const WORDS: u32 = 6;
}
