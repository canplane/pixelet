// pxl-kernel/perf/perf.spec.rs
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
    pub const STEP_SLOTS: u32 = 32;
    pub const WINDOW_MS: u32 = 60000;
}

/// One frame turn and the interval it opened (perf/frame.rs FrameSample): every figure of one turn, landed when the next frame turn closes the interval. `refresh_ms` is the display's refresh and `paced_ms` the interval the host paced the frame at, a whole number of refreshes, both as the host granted them (@xpute/host sched/frame_pacing.ts `FrameGrant`); a moving frame whose interval ran past `paced_ms` by more than half a refresh missed a vsync (perf/frame.rs `missed`). `settled` is 0 or 1; `turns` and `busy_ms` count every turn the interval held, the frame's and those without a host frame, rise to fall: what they ran past the interval is read off the two (perf/frame.rs `over_ms`). `effects_ms`, `tick_ms`, `stage_ms`, `tasks_ms` and `perf_ms` are their parts, `stage_ms` the turn's pump, the stage's write and the render, and `perf_ms` what the profiler cost it, its own record of the frame and the shell's reads of it; what they leave of `busy_ms` is unmeasured. `effects` is each effect's time in the frame turn, in EFFECT_NAMES' order, `effects_ms` their sum. `gpu_read` is 1 where the GPU's read of this frame landed, which comes a frame or two later and is written back into this frame's record (`gpu_frame` the renderer's number of it, 0 where it drew none): its `gpu_spans` are then that read's, `gpu_busy_ms` the time they ran, those that overlap counted once, the frame's GPU time, which the spans' sum overstates where passes overlap (pxl-render gpu/api.rs `Gpu::gpu_busy_ms`), and `gpu_px` the world's pixels of the frame the read timed, what the spans were measured over; and 0 where nothing measured it (its read not landed yet or cut off by a stop, or the device keeps no timestamps), its spans, busy time and pixels 0 too. `scale` is the share of the display's pixels, each way, the world was drawn at, and `lattice` the camera grid's lattice its beam rays and cells were laid on, in cells a side (pxl-engine `beam::Grid::lattice`), each in every frame (pxl-kernel domain/view/frame/resolution.rs `drawn_scale`, `lattice`). `kind_name` indexes the renderer's FRAME_KINDS and `wake_name` WAKES. `draws` and `triangles` are the frame's, every pipeline a draw names (pxl-render frame/draw.rs `DRAW_PIPELINES`), counted while the profiler records. `light_moved` says why the light's map cast in the frame, a bit each in LIGHT_MOVED's order (pxl-render gpu/ray/cast.rs `light_moved`), 0 where it did not cast; `decor_boxes` is how many boxes decor's slots were marked by to generate again in it, the bricks handed in it and the edits and cuts held (pxl-render gpu/generator.rs `prepare`), 0 where it marked none. The ray columns are the `ray_counts` setting's, written into the frame they were counted in as the counts land a frame or two later, by the renderer's number of it as a GPU read is (pxl-render gpu/ray/tally.rs `RayCounts`), 0 where none landed: `eye_cast` and `light_cast` 1 where the eye cast in full or the light's map cast and its counts landed; then the eye's pixels (`eye_rays`) and the light's map's slots (`light_rays`), and an observer's each: the nodes its beam rays, its samples and its walk again asked, those of the last asked of the world, its pixels or slots that took the visible sample, those that walked and those that took the sky, together every one it cast again, so the light's three over `light_rays` are the share of its map a cast cast again, and the boxes drawn (pxl-engine `tally`). `terrain_landed` is how many terrain tiles the tile cache took in during the frame turn (pxl-kernel domain/layer/tile_cache.rs `take_landed`). `uploads` and `uploads_bytes` are the renderer's upload table and `pack_bytes` its texture pack (pxl-render gpu/renderer.rs `FrameStat`), and `heap_used_bytes` the heap's high-water mark (xpute-guest mem/heap.rs `HeapStat`). A column's name says its unit: `_ms`, `_bytes`, `_px`, and `_name` for an index into a list of names.
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
    pub const gpu_read: u32 = 30;
    pub const gpu_spans: u32 = 31;
    pub const gpu_busy_ms: u32 = 63;
    pub const gpu_px: u32 = 64;
    pub const scale: u32 = 65;
    pub const lattice: u32 = 66;
    pub const draws: u32 = 67;
    pub const triangles: u32 = 68;
    pub const gpu_frame: u32 = 69;
    pub const light_moved: u32 = 70;
    pub const decor_boxes: u32 = 71;
    pub const eye_cast: u32 = 72;
    pub const eye_rays: u32 = 73;
    pub const eye_beam: u32 = 74;
    pub const eye_sample: u32 = 75;
    pub const eye_miss: u32 = 76;
    pub const eye_asked: u32 = 77;
    pub const eye_took: u32 = 78;
    pub const eye_walked: u32 = 79;
    pub const eye_sky: u32 = 80;
    pub const eye_boxes: u32 = 81;
    pub const light_cast: u32 = 82;
    pub const light_rays: u32 = 83;
    pub const light_beam: u32 = 84;
    pub const light_sample: u32 = 85;
    pub const light_miss: u32 = 86;
    pub const light_asked: u32 = 87;
    pub const light_took: u32 = 88;
    pub const light_walked: u32 = 89;
    pub const light_sky: u32 = 90;
    pub const light_boxes: u32 = 91;
    pub const terrain_landed: u32 = 92;
    pub const uploads: u32 = 93;
    pub const uploads_bytes: u32 = 94;
    pub const pack_bytes: u32 = 95;
    pub const heap_used_bytes: u32 = 96;
    pub const WORDS: u32 = 97;
}

/// FRAME's columns in groups, every column in one (column.test.ts), in the order a reader asks in turn — where the time went, what was drawn and cast, what landed, and what it holds. The trace prints every column by its group's order, so a column the kernel adds is printed without a word written for it; the panel shows some groups, a few rows each, and the report its own lines. The `rays` group's columns lie in FRAME in its order, which the window's sums of them (STAT.frames.rays) keep.
pub mod FRAME_GROUP {
    pub const time: usize = 9;
    pub const turn: usize = 7;
    pub const frame: usize = 2;
    pub const gpu: usize = 11;
    pub const rays: usize = 20;
    pub const tiles: usize = 1;
    pub const device: usize = 3;
    pub const heap: usize = 1;
}

/// One settled task (perf/task.rs TaskRow). `path` is the bytes of the path it opened, zero past their end, and every other word an f64; `start_ms` is NaN for a task that never ran; `stream`, `outcome` and each of `phase_ids` index STREAM_NAMES, OUTCOME_NAMES and TASK_PHASE_NAMES. `at_file` is the address of the file name, `at_file_bytes` long, of the line a failure was made at, and 0 where the failure was the host's.
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
    pub const has_prev: u32 = 97;
    pub const uploads_delta: u32 = 98;
    pub const uploads_delta_bytes: u32 = 99;
    pub const heap_used_delta_bytes: u32 = 100;
    pub const WORDS: u32 = 101;
}

/// A value's percentiles over n values: p50 and p95 each the nearest rank, the least value at or above which lie no more than that share of the n (sorted, the value at ceil(q × n) − 1), and the most.
pub mod PERCENTILES {
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
    pub const WORDS: u32 = 24;
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
        pub const gpu_read: u32 = 9;
        pub const interval: u32 = 10;
        pub const busy: u32 = 13;
        pub const tick: u32 = 16;
        pub const effects: u32 = 19;
        pub const stage: u32 = 22;
        pub const tasks: u32 = 25;
        pub const perf: u32 = 28;
        pub const rest: u32 = 31;
        pub const effects_p95: u32 = 34;
        pub const gpu_busy: u32 = 43;
        pub const gpu_busy_kinds: u32 = 46;
        pub const phases_p95: u32 = 76;
        pub const gpu_spans_p95: u32 = 81;
        pub const gpu_spans_cnt: u32 = 113;
        pub const kinds: u32 = 145;
        pub const wakes: u32 = 155;
        pub const gpu_px: u32 = 158;
        pub const rays: u32 = 159;
        pub const first: u32 = 179;
        pub const last: u32 = 276;
        pub const WORDS: u32 = 373;
    }
    pub mod tasks {
        pub const AT: u32 = 373;
        pub const slots: u32 = 0;
        pub const WORDS: u32 = 144;
    }
    pub mod steps {
        pub const AT: u32 = 517;
        pub const cnt: u32 = 0;
        pub const slots: u32 = 1;
        pub const WORDS: u32 = 161;
    }
    /// What the frames held at the most or the least since the recording started, as the steps are timed, folded as each frame turn's record is taken (perf/record.rs `perf_fold_recording`): `frames` the frame turns folded; the heap's figures by their allocator's names (xpute-guest mem/heap.rs `HeapStat`), the most of each but `largest_free`, the least, and `refusals` as it last stood; the door's rings' peaks and the body slots' (xpute-guest ipc/door.rs `DoorStat`, sys/io/body.rs `take_peak`), `ring_left_peak` its `left_peak`; and `cap_terrain_refused` the terrain tiles the tile cache refused a slot, summed (domain/layer/tile_cache.rs `take_refusals`). Every word 0 before a frame is folded.
    pub mod recording {
        pub const AT: u32 = 678;
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
        pub const cap_terrain_refused: u32 = 11;
        pub const WORDS: u32 = 12;
    }
    pub const WORDS: u32 = 690;
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
    /// The eye is where the camera looks from and the pivot the point it turns about, the eye itself in the first-person camera, where the character stands (pxl-kernel domain/control/camera/controller.rs `eye_world`, `pivot`).
    /// The hover is what the pointer meets, exactly, at the zooms the eye's rays ask, whatever the tool could strike, and `cell` the zoom the eye's rays draw there (pxl-kernel domain/query/ray.rs `by_dist`).
    /// The pick is what the tool held strikes there (pxl-kernel domain/control/edit.rs `pick_at`): `hit_kind` what it met, an index into HIT_KINDS; `pick_zoom` the zoom of the box struck and `ray_zoom` the rays' there, `folded` 1 where they draw coarser than the box, which strikes nothing. `place_x`..`place_z` is the corner the tool takes (pxl-kernel domain/control/edit.rs `Hover`), `place_zoom` the zoom it places at, and `blocked` 1 where that room is taken (domain/control/edit/collision.rs `is_placement_blocked`).
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
        pub const cell: u32 = 15;
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
pub mod STATS {
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
pub mod RINGS {
    pub const slots: u32 = 0;
    pub const WORDS: u32 = 6;
}
