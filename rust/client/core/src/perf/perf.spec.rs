// pxl-core/perf/perf.spec.rs
//
// GENERATED from spec/client/core/perf/perf.json (xpute-gen data.rs)
//
// The profiler's records and sections as the core lays them out and the client reads
// them from the replies that hand them over (perf/record.rs, ts/client/src/service/core/perf.ts):
// word offsets, eight bytes a word. The names the numbers index are their owners':
// a task's io's (spec/client/core/sys/io/demand.json), a step's and an effect's the
// frame's (spec/client/core/perf/step.json), a GPU span's the renderer's
// (spec/client/render/gpu/span.json). A packed table gives each field's
// width and the generator lays the offsets out. Nothing is built
// for a read: the core writes numbers where they stay, and the words for them are
// the UI's. LIMIT holds the bounds the profiler keeps, and the one a build made for
// measuring hands its GPU hosts, which the core and the client both read.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    non_camel_case_types,
    dead_code,
    reason = "generated from the spec"
)]

pub const HIT_KINDS: &[&str] = &["ground", "object", "objects", "decor"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum HitKind {
    ground = 0,
    object = 1,
    objects = 2,
    decor = 3,
}

impl HitKind {
    /// Every member, in its source's own order.
    pub const ALL: [HitKind; 4] = [
        HitKind::ground,
        HitKind::object,
        HitKind::objects,
        HitKind::decor,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<HitKind> {
        match val {
            0 => Some(HitKind::ground),
            1 => Some(HitKind::object),
            2 => Some(HitKind::objects),
            3 => Some(HitKind::decor),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            HitKind::ground => "ground",
            HitKind::object => "object",
            HitKind::objects => "objects",
            HitKind::decor => "decor",
        }
    }
}

pub mod LIMIT {
    pub const REASONS: u32 = 4;
    pub const REASON_WORDS: u32 = 3;
    pub const STEP_CAP: u32 = 32;
    pub const WINDOW_MS: u32 = 60000;
    pub const PAGE_BYTES: u32 = 8192;
    pub const SLOW_MS: u32 = 1000;
}

/// One frame and the interval it opened (perf/frame.rs FrameSample): every figure of one frame, landed when the next frame closes the interval. `interval_ms` is that interval, `refresh_ms` the display's measured interval (pxl-core sys/sched/display.rs), and `paced_ms` the gap since the last frame, which the frame was paced at (pxl-core frame.rs `run`). `settled` is 1 where the frame asked for no next one, else 0. `effects_ms`, `tick_ms` and `stage_ms` are the frame's parts, `stage_ms` the requests' pump and the render, and `perf_ms` what the profiler cost it, its own record of the frame and the UI's reads of it. `effects` is each effect's time in the frame, in EFFECT_NAMES' order, `effects_ms` their sum. `gpu_timed` is 1 where the GPU's read of this frame has landed, which comes a frame or two later and is written back into this frame's record (`gpu_frame` the renderer's number of it, 0 where it drew none): its `gpu_spans` are then that read's, `gpu_busy_ms` the time they ran, those that overlap counted once, the frame's GPU time, which the spans' sum overstates where passes overlap (pxl-render gpu/api.rs `Gpu::gpu_busy_ms`), and `gpu_px` the world's pixels of the frame the read timed, what the spans were measured over; and 0 where nothing measured it (its read not landed yet or cut off by a stop, or the device keeps no timestamps), its spans, busy time and pixels 0 too. `scale` is the share of the display's pixels, each way, the world was drawn at, in every frame (pxl-core domain/scene/resolution.rs `drawn_scale`). `kind_name` indexes the renderer's FRAME_KINDS. `draws` and `triangles` are the frame's, every pipeline a draw names (pxl-render gpu/draw.rs `DRAW_PIPELINES`), counted while the profiler records. `light_changed` says what changed that cast the light's map in the frame, a bit each in LIGHT_CHANGED's order (pxl-render ray/cast.rs `light_changed`), 0 where it did not cast; `decor_rects` is how many rects decor's slots were marked by to generate again in it, the bricks handed in it and the edits and cuts held (pxl-render gpu/generator.rs `prepare`), 0 where it marked none. The ray columns are the `ray_cnts` setting's, written into the frame they were counted in as the counts land a frame or two later, by the renderer's number of it as a GPU read is (pxl-render ray/count.rs `RayCount`), 0 where none landed: `eye_cast` and `light_cast` 1 where the eye cast in full or the light's map cast and its counts landed; then the eye's pixels (`eye_rays`) and the light's map's slots (`light_rays`), and an observer's each: the nodes its beam rays, its samples and its holes' walk visited (`_beam`, `_sample`, `_hole_walk`), the asks of the world among the last (`_asks`), its pixels or slots that took the visible voxel, a walk of their own as holes and the sky (`_voxel`, `_hole`, `_sky`), together every one it cast again, so the light's three over `light_rays` are the share of its map a cast cast again, and the boxes drawn (pxl-engine `count`). `terrain_loads` is how many terrain tiles the tile cache took in during the frame (pxl-core domain/layer/tile_cache.rs `take_loads`). `uploads` and `uploads_bytes` are the renderer's upload table and `pack_bytes` its texture pack (pxl-render renderer.rs `FrameStat`), and `heap_used_bytes` the heap's high-water mark (pxl-core sys/heap.rs `HeapStat`). A column's name says its unit: `_ms`, `_bytes`, `_px`, and `_name` for an index into a list of names.
pub mod FRAME {
    pub const t: usize = 0;
    pub const interval_ms: usize = 1;
    pub const refresh_ms: usize = 2;
    pub const paced_ms: usize = 3;
    pub const settled: usize = 4;
    pub const budget_ms: usize = 5;
    pub const effects_ms: usize = 6;
    pub const tick_ms: usize = 7;
    pub const phases: usize = 8;
    pub const stage_ms: usize = 10;
    pub const perf_ms: usize = 11;
    pub const effects: usize = 12;
    pub const kind_name: usize = 21;
    pub const gpu_timed: usize = 22;
    pub const gpu_spans: usize = 23;
    pub const gpu_busy_ms: usize = 61;
    pub const gpu_px: usize = 62;
    pub const scale: usize = 63;
    pub const draws: usize = 64;
    pub const triangles: usize = 65;
    pub const gpu_frame: usize = 66;
    pub const light_changed: usize = 67;
    pub const decor_rects: usize = 68;
    pub const eye_cast: usize = 69;
    pub const eye_rays: usize = 70;
    pub const eye_beam: usize = 71;
    pub const eye_sample: usize = 72;
    pub const eye_hole_walk: usize = 73;
    pub const eye_asks: usize = 74;
    pub const eye_voxel: usize = 75;
    pub const eye_hole: usize = 76;
    pub const eye_sky: usize = 77;
    pub const eye_boxes: usize = 78;
    pub const light_cast: usize = 79;
    pub const light_rays: usize = 80;
    pub const light_beam: usize = 81;
    pub const light_sample: usize = 82;
    pub const light_hole_walk: usize = 83;
    pub const light_asks: usize = 84;
    pub const light_voxel: usize = 85;
    pub const light_hole: usize = 86;
    pub const light_sky: usize = 87;
    pub const light_boxes: usize = 88;
    pub const terrain_loads: usize = 89;
    pub const uploads: usize = 90;
    pub const uploads_bytes: usize = 91;
    pub const pack_bytes: usize = 92;
    pub const heap_used_bytes: usize = 93;
    pub const WORDS: usize = 94;
}

/// FRAME's columns in groups, every column in one (column.test.ts), in the order a reader asks in turn — where the time went, what was drawn and cast, what landed, and what it holds. The trace prints every column by its group's order, so a column the core adds is printed without a word written for it; the panel shows some groups, a few rows each, and the report its own lines. The `rays` group's columns lie in FRAME in its order, which the window's sums of them (STAT.frames.rays) keep.
pub mod FRAME_GROUP {
    pub const time: &[&str] = &[
        "t",
        "interval_ms",
        "refresh_ms",
        "paced_ms",
        "settled",
        "budget_ms",
    ];
    pub const cpu: &[&str] = &[
        "effects_ms",
        "tick_ms",
        "phases",
        "stage_ms",
        "perf_ms",
        "effects",
    ];
    pub const frame: &[&str] = &["kind_name"];
    pub const gpu: &[&str] = &[
        "gpu_timed",
        "gpu_spans",
        "gpu_busy_ms",
        "gpu_px",
        "scale",
        "draws",
        "triangles",
        "gpu_frame",
        "light_changed",
        "decor_rects",
    ];
    pub const rays: &[&str] = &[
        "eye_cast",
        "eye_rays",
        "eye_beam",
        "eye_sample",
        "eye_hole_walk",
        "eye_asks",
        "eye_voxel",
        "eye_hole",
        "eye_sky",
        "eye_boxes",
        "light_cast",
        "light_rays",
        "light_beam",
        "light_sample",
        "light_hole_walk",
        "light_asks",
        "light_voxel",
        "light_hole",
        "light_sky",
        "light_boxes",
    ];
    pub const tiles: &[&str] = &["terrain_loads"];
    pub const device: &[&str] = &["uploads", "uploads_bytes", "pack_bytes"];
    pub const heap: &[&str] = &["heap_used_bytes"];
}

/// One settled task (perf/task.rs TaskRecord). `path` is the bytes of the path it opened, zero past their end, and every other word an f64; `start_ms` is NaN for a task that never ran; `stream`, `outcome` and each of `phase_ids` index STREAM_NAMES, OUTCOME_NAMES and TASK_PHASE_NAMES. `at_file` is the offset of the file name, `at_file_bytes` long, of the line a failure was made at, in the string table PERF_STRTAB hands over, or 0 where the table had no room for the name, whose `at_line` is kept; every site word is 0 where the failure was the host's.
pub mod TASK {
    pub const stream: usize = 0;
    pub const path: usize = 1;
    pub const priority: usize = 4;
    pub const enqueue_ms: usize = 5;
    pub const start_ms: usize = 6;
    pub const end_ms: usize = 7;
    pub const outcome: usize = 8;
    pub const fail: usize = 9;
    pub const detail: usize = 10;
    pub const at_file: usize = 11;
    pub const at_file_bytes: usize = 12;
    pub const at_line: usize = 13;
    pub const phase_cnt: usize = 14;
    pub const phase_ids: usize = 15;
    pub const phase_vals: usize = 21;
    pub const WORDS: usize = 27;
}

/// One of the worst moving frames since the recording started, by its interval, and what changed from the frame before it: its uploads, their bytes and the heap's high-water mark, each this frame's less that one's, all 0 where `has_prev` is 0. A still interval is long on purpose and is never among them.
pub mod WORST {
    pub const sample: usize = 0;
    pub const has_prev: usize = 94;
    pub const uploads_delta: usize = 95;
    pub const uploads_delta_bytes: usize = 96;
    pub const heap_used_delta_bytes: usize = 97;
    pub const WORDS: usize = 98;
}

/// A value's percentiles over n values: p50 and p95 each the nearest rank, the least value at or above which lie no more than that share of the n (sorted, the value at ceil(q × n) − 1), and the most.
pub mod PERCENTILE {
    pub const p50: usize = 0;
    pub const p95: usize = 1;
    pub const max: usize = 2;
    pub const WORDS: usize = 3;
}

/// One stream's tasks in the window, at STAT.tasks.slots + its index × WORDS. Each reason is (fail, detail, n), commonest first; `phase_mean` is NaN for a phase no task reported.
pub mod STREAM_STAT {
    pub const cnt: usize = 0;
    pub const ok: usize = 1;
    pub const aborted: usize = 2;
    pub const err: usize = 3;
    pub const lat_p50: usize = 4;
    pub const lat_p95: usize = 5;
    pub const reason_cnt: usize = 6;
    pub const reasons: usize = 7;
    pub const phase_n: usize = 19;
    pub const phase_mean: usize = 20;
    pub const WORDS: usize = 23;
}

/// One registered step's timing, at STAT.steps.slots + its index × WORDS. `name` indexes STEP_NAMES.
pub mod STEP_STAT {
    pub const phase: usize = 0;
    pub const name: usize = 1;
    pub const ms: usize = 2;
    pub const cnt: usize = 3;
    pub const max_ms: usize = 4;
    pub const WORDS: usize = 5;
}

/// The window's statistics PERF_STATS computes, over its `window_ms`: each section's words at its AT, the fields relative to it. Every word an f64. Packed: a section's AT is where the one before it ends, and WORDS where the last ends, both laid out by the generator.
pub mod STAT {
    pub mod frames {
        pub const AT: usize = 0;
        pub const present: usize = 0;
        pub const cnt: usize = 1;
        pub const seconds: usize = 2;
        pub const moving: usize = 3;
        pub const moving_ms: usize = 4;
        pub const overran: usize = 5;
        pub const gpu_timed: usize = 6;
        pub const interval: usize = 7;
        pub const tick: usize = 10;
        pub const effect: usize = 13;
        pub const stage: usize = 16;
        pub const perf: usize = 19;
        pub const effects_p95: usize = 22;
        pub const gpu_busy: usize = 31;
        pub const gpu_busy_kinds: usize = 34;
        pub const phases_p95: usize = 61;
        pub const gpu_spans_p95: usize = 63;
        pub const gpu_spans_cnt: usize = 101;
        pub const kinds: usize = 139;
        pub const gpu_px: usize = 148;
        pub const rays: usize = 149;
        pub const first: usize = 169;
        pub const last: usize = 263;
        pub const WORDS: usize = 357;
    }
    pub mod tasks {
        pub const AT: usize = 357;
        pub const slots: usize = 0;
        pub const WORDS: usize = 138;
    }
    pub mod steps {
        pub const AT: usize = 495;
        pub const cnt: usize = 0;
        pub const slots: usize = 1;
        pub const WORDS: usize = 161;
    }
    /// What the frames held at the most or the least since the recording started, as the steps are timed, folded as each frame's record is taken (perf/record.rs `perf_fold_recording`): `frames` the frames folded; the heap's figures by their allocator's names (pxl-core sys/heap.rs `HeapStat`), the most of each but `largest_free`, the least, and `refusals` as it last stood; the body slots' peak (sys/io/body.rs `take_peak`); and `terrain_refusals` the terrain tiles the tile cache refused a slot, summed (domain/layer/tile_cache.rs `take_refusals`). Every word 0 before a frame is folded.
    pub mod recording {
        pub const AT: usize = 656;
        pub const frames: usize = 0;
        pub const heap_live_max_bytes: usize = 1;
        pub const heap_run_max_bytes: usize = 2;
        pub const heap_slot_max_bytes: usize = 3;
        pub const heap_range_max_bytes: usize = 4;
        pub const heap_largest_free_min_bytes: usize = 5;
        pub const heap_refusals: usize = 6;
        pub const body_slots_peak: usize = 7;
        pub const terrain_refusals: usize = 8;
        pub const WORDS: usize = 9;
    }
    pub const WORDS: usize = 665;
}

/// What PERF_STATS finds as it reads, the window aside: each section's words at its AT, as STAT lays out its own. Every word an f64.
pub mod NOW {
    /// Whether it records, how long the recording has run, whether it lost what it had no room for (a ring's oldest records, or a failure's file name the string table could not hold), and how many records each ring holds (RING_NAMES).
    pub mod recording_state {
        pub const AT: usize = 0;
        pub const recording: usize = 0;
        pub const elapsed_ms: usize = 1;
        pub const truncated: usize = 2;
        pub const frames: usize = 3;
        pub const tasks: usize = 4;
        pub const worst: usize = 5;
        pub const WORDS: usize = 6;
    }
    /// Whether the core draws, as the read finds it, recording or not, every count since the core booted: `frames` the frames the renderer encoded (pxl-render renderer.rs `drawn_frames`); `refused_admit` and `refused_stream` the frames that did not run, as the GPU admitted none (xpute-gpu-guest `admit::frame`) or the host still held the stream (xpute-gpu-guest stream.rs) (frame.rs `frame_task`).
    /// Then the device, as the host wrote its block (xpute spec DEVICE), the same on a page and on a device: `generation` the devices lost so far (pxl-render gpu/api.rs `device_generation`); `status` its STATUS, 0 before the block is written; `errors` the uncaptured errors it raised; and `unfinished` the frames whose work its queue has not done, each a frame the host asked the queue to say done and has not heard of.
    pub mod draw {
        pub const AT: usize = 6;
        pub const frames: usize = 0;
        pub const refused_admit: usize = 1;
        pub const refused_stream: usize = 2;
        pub const generation: usize = 3;
        pub const status: usize = 4;
        pub const errors: usize = 5;
        pub const unfinished: usize = 6;
        pub const WORDS: usize = 7;
    }
    /// Where the camera, its pivot and the pointer are as the read finds them, recording or not: `eye_x`..`eye_z`, `dist`, `pivot_x`..`pivot_z`, `hover_x`..`hover_z` and `place_x`..`place_z` in units, angles in radians, `pivot_lon` and `pivot_lat` in degrees and `pivot_alt` in meters, a zoom as its number, and NaN where there is none.
    /// The eye is where the camera looks from and the pivot the point it turns about, the eye itself in the first-person camera, where the character stands (pxl-core domain/control/camera.rs `eye_world`, camera/controller.rs `pivot`).
    /// The hover is what the pointer meets, exactly, at the zooms the eye's rays ask, whatever the tool could strike, and `cell_zoom` the zoom the eye's rays draw there (pxl-core domain/query/ray.rs `by_dist`).
    /// The pick is what the tool held strikes there (pxl-core domain/control/edit/pick.rs `pick_at`): `hit_kind` what it met, an index into HIT_KINDS; `pick_zoom` the zoom of the box struck and `ray_zoom` the rays' there, `folded` 1 where they draw coarser than the box, which strikes nothing. `place_x`..`place_z` is the corner the tool takes (pxl-core domain/control/edit.rs `Hover`), `place_zoom` the zoom it places at, and `blocked` 1 where that room is taken (domain/control/edit/collision.rs `is_placement_blocked`).
    pub mod view {
        pub const AT: usize = 13;
        pub const eye_x: usize = 0;
        pub const eye_y: usize = 1;
        pub const eye_z: usize = 2;
        pub const dist: usize = 3;
        pub const polar: usize = 4;
        pub const azimuth: usize = 5;
        pub const pivot_x: usize = 6;
        pub const pivot_y: usize = 7;
        pub const pivot_z: usize = 8;
        pub const pivot_lon: usize = 9;
        pub const pivot_lat: usize = 10;
        pub const pivot_alt: usize = 11;
        pub const hover_x: usize = 12;
        pub const hover_y: usize = 13;
        pub const hover_z: usize = 14;
        pub const cell_zoom: usize = 15;
        pub const hit_kind: usize = 16;
        pub const pick_zoom: usize = 17;
        pub const ray_zoom: usize = 18;
        pub const folded: usize = 19;
        pub const place_x: usize = 20;
        pub const place_y: usize = 21;
        pub const place_z: usize = 22;
        pub const place_zoom: usize = 23;
        pub const blocked: usize = 24;
        pub const WORDS: usize = 25;
    }
    pub const WORDS: usize = 38;
}

pub const RING_NAMES: &[&str] = &["frames", "tasks", "worst"];
