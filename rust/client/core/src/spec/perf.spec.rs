// pxl-core/spec/perf.spec.rs
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

/// What the pick met (NOW.view `hit_kind`; pxl-core domain/query/ray.rs `HitKind`): the ground, a placed cube, placed cubes folded into a coarser box, or a decor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum HitKind {
    Ground = 0,
    Object = 1,
    Objects = 2,
    Decor = 3,
}

impl HitKind {
    /// Every member, in its source's own order.
    pub const ALL: [HitKind; 4] = [
        HitKind::Ground,
        HitKind::Object,
        HitKind::Objects,
        HitKind::Decor,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<HitKind> {
        match val {
            0 => Some(HitKind::Ground),
            1 => Some(HitKind::Object),
            2 => Some(HitKind::Objects),
            3 => Some(HitKind::Decor),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            HitKind::Ground => "ground",
            HitKind::Object => "object",
            HitKind::Objects => "objects",
            HitKind::Decor => "decor",
        }
    }
}

pub mod limit {
    pub const REASON_CAP: u32 = 4;
    /// A reason's words: its fail, its detail and how many (STREAM_STAT.reasons).
    pub const REASON_WORDS: u32 = 3;
    /// How far back the tables keep, the core's frames and tasks and the UI's long frames alike: the last minute.
    pub const WINDOW_MS: u32 = 60000;
    /// The most bytes one read hands over. A table is read a page at a time (perf.wit `table`), each read from the sequence the last ended at, up to the table's end; the worst frames, the string table and the statistics each fit one page.
    pub const PAGE_SZ: u32 = 8192;
}

/// One frame and the interval it opened (perf/frame.rs FrameSample): every figure of one frame, landed when the next frame closes the interval. `interval_ms` is that interval, `refresh_ms` the display's interval as the frame's pacing takes it, its measured one (pxl-core sys/sched/display.rs) or one interval at RATE_MAX_HZ before it is measured (pxl-core domain/scene/rate.rs `display_ms`), and `paced_ms` the frame's target interval, which it was paced at (rate.rs `target_ms`). `settled` is 1 where the frame asked for no next one, else 0. `budget_ms` is the frame's entry's budget, the time from the entry's start to its end (pxl-core sys/sched.rs `budget_ms`): the end is the frame's target interval less MARGIN_SHARE from the frame's start, the budget is 0 where the entry started past it, and one slice from the entry's start where it started the margin past the target or later (sys/sched.rs `Budget`); and `overran` is 1 where the scheduler found the budget spent as the tick ended, the clock at or past the entry's end (sys/sched.rs `spent`), else 0. The effects, the tick, the requests' pump and the stage are the frame's parts, in that order: `effects` is each effect's time in the frame, in EFFECT_NAME's order, `tick_ms` the tick's and `phases` each of its phases', `pump_ms` the requests declared and pumped (pxl-core frame.rs `run`), and `stage_parts` each part of the stage's, in STAGE_PART_NAME's order; the effects' time and the stage's are their sums. The pump and the stage's parts are timed while the profiler records, as the effects are. `fill_ms` is what the ground's fill took in the entry after the frame's, up to that entry's budget, and `fill_cnt` the bricks it filled there (pxl-core domain/layer/terrain/clipmap.rs `fill_share`), each 0 where no brick was due. `perf_ms` is what the profiler cost the frame, its own record of it and the UI's reads of it. `invalidations` is how many times a frame was asked for between this frame's record and the one before, by reason, in INVALIDATION_REASON's order (pxl-core sys/sched.rs `take_invalidations`): what woke the frame, what was asked while it ran, and a refused frame's ask before it; an ask for a time counts when the time comes. `gpu_timed` is 1 where the GPU's read of this frame has landed, which comes a frame or two later and is written back into this frame's record (`gpu_frame` the renderer's number of it, counted from 1, by which a frame is found in every table, 0 where it drew none): its `gpu_spans` are then that read's, `gpu_busy_ms` the time they ran, those that overlap counted once, the frame's GPU time, which the spans' sum overstates where passes overlap (pxl-render gpu/api.rs `Gpu::gpu_busy_ms`), and `gpu_px` the world's pixels of the frame the read timed, what the spans were measured over; and 0 where nothing measured it (its read not landed yet or cut off by a stop, or the device keeps no timestamps), its spans, busy time and pixels 0 too. `scale` is the share of the display's pixels, each way, the world was drawn at, in every frame (pxl-core domain/scene/resolution.rs `drawn_scale`). `kind_name` indexes the renderer's FRAME_KIND. `draw_cnt` and `triangle_cnt` are the frame's, every pipeline a draw names (pxl-render gpu/draw.rs `DRAW_PIPELINES`), counted while the profiler records. `light_changed` says what changed that cast the light's map in the frame, a bit each in LIGHT_CHANGED's order (pxl-render ray/cast.rs `light_changed`), 0 where it did not cast. The ray columns are the `ray_cnt` setting's, written into the frame they were counted in as the counts land a frame or two later, by the renderer's number of it as a GPU read is (pxl-render ray/count.rs `RayCount`), 0 where none landed: `eye_cast` and `light_cast` 1 where the eye cast in full or the light's map cast and its counts landed; then the eye's pixels (`eye_ray_cnt`) and the light's map's slots (`light_ray_cnt`), and an observer's each: the nodes its beam rays, its samples and its holes' walk visited (`_beam`, `_sample`, `_hole_walk`), the asks of the world among the last (`_ask_cnt`), its pixels or slots that took the visible voxel, a walk of their own as holes and the sky (`_voxel`, `_hole`, `_sky`), together every one it cast again, so the light's three over `light_ray_cnt` are the share of its map a cast cast again, and the boxes drawn (pxl-engine `count`). `loads` is how many tiles each base layer's tile cache took in during the frame, in BASE_KIND's order (pxl-core domain/layer/tile_cache.rs `take_load_cnt`). `upload_cnt` and `upload_bytes` are the renderer's upload table (pxl-render renderer.rs `FrameStat`), and `heap_used_bytes` the heap's high-water mark (pxl-core sys/heap.rs `HeapStat`). A column's name says its unit: `_ms`, `_bytes`, `_px`, and `_name` for an index into a list of names.
pub mod frame {
    pub const T: usize = 0;
    pub const INTERVAL_MS: usize = 1;
    pub const REFRESH_MS: usize = 2;
    pub const PACED_MS: usize = 3;
    pub const SETTLED: usize = 4;
    pub const BUDGET_MS: usize = 5;
    pub const OVERRAN: usize = 6;
    pub const TICK_MS: usize = 7;
    pub const PHASES: usize = 8;
    pub const PUMP_MS: usize = 10;
    pub const STAGE_PARTS: usize = 11;
    pub const FILL_MS: usize = 15;
    pub const PERF_MS: usize = 16;
    pub const EFFECTS: usize = 17;
    pub const INVALIDATIONS: usize = 24;
    pub const KIND_NAME: usize = 30;
    pub const GPU_TIMED: usize = 31;
    pub const GPU_SPANS: usize = 32;
    pub const GPU_BUSY_MS: usize = 70;
    pub const GPU_PX: usize = 71;
    pub const SCALE: usize = 72;
    pub const DRAW_CNT: usize = 73;
    pub const TRIANGLE_CNT: usize = 74;
    pub const GPU_FRAME: usize = 75;
    pub const LIGHT_CHANGED: usize = 76;
    pub const EYE_CAST: usize = 77;
    pub const EYE_RAY_CNT: usize = 78;
    pub const EYE_BEAM: usize = 79;
    pub const EYE_SAMPLE: usize = 80;
    pub const EYE_HOLE_WALK: usize = 81;
    pub const EYE_ASK_CNT: usize = 82;
    pub const EYE_VOXEL: usize = 83;
    pub const EYE_HOLE: usize = 84;
    pub const EYE_SKY: usize = 85;
    pub const EYE_BOX_CNT: usize = 86;
    pub const LIGHT_CAST: usize = 87;
    pub const LIGHT_RAY_CNT: usize = 88;
    pub const LIGHT_BEAM: usize = 89;
    pub const LIGHT_SAMPLE: usize = 90;
    pub const LIGHT_HOLE_WALK: usize = 91;
    pub const LIGHT_ASK_CNT: usize = 92;
    pub const LIGHT_VOXEL: usize = 93;
    pub const LIGHT_HOLE: usize = 94;
    pub const LIGHT_SKY: usize = 95;
    pub const LIGHT_BOX_CNT: usize = 96;
    pub const LOADS: usize = 97;
    pub const FILL_CNT: usize = 101;
    pub const UPLOAD_CNT: usize = 102;
    pub const UPLOAD_BYTES: usize = 103;
    pub const HEAP_USED_BYTES: usize = 104;
    pub const WORDS: usize = 105;
}

/// FRAME's columns in groups, every column in one (column.test.ts), in the order a reader asks in turn — where the time went, what was drawn and cast, what landed, and what it holds. The trace prints every column by its group's order, so a column the core adds is printed without a word written for it; the panel shows some groups, a few rows each, and the report its own lines. The `rays` group's columns lie in FRAME in its order, which the window's sums of them (STAT.frames.rays) keep.
pub mod frame_group {
    pub const TIME: &[&str] = &[
        "t",
        "interval_ms",
        "refresh_ms",
        "paced_ms",
        "settled",
        "budget_ms",
        "overran",
    ];
    pub const CPU: &[&str] = &[
        "tick_ms",
        "phases",
        "pump_ms",
        "stage_parts",
        "fill_ms",
        "perf_ms",
        "effects",
        "invalidations",
    ];
    pub const FRAME: &[&str] = &["kind_name"];
    pub const GPU: &[&str] = &[
        "gpu_timed",
        "gpu_spans",
        "gpu_busy_ms",
        "gpu_px",
        "scale",
        "draw_cnt",
        "triangle_cnt",
        "gpu_frame",
        "light_changed",
    ];
    pub const RAYS: &[&str] = &[
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
    ];
    pub const TILES: &[&str] = &["loads", "fill_cnt"];
    pub const DEVICE: &[&str] = &["upload_cnt", "upload_bytes"];
    pub const HEAP: &[&str] = &["heap_used_bytes"];
}

/// One settled task (perf/task.rs TaskRecord). `path` is the bytes of the path it opened, zero past their end, and every other word an f64; `start_ms` is NaN for a task that never ran; `stream`, `outcome` and each of `phase_ids` index STREAM_NAME, OUTCOME_NAME and TASK_PHASE_NAME; a run reports each phase at most once, the first `phase_cnt` of `phase_ids` and `phase_vals` in the order it reported them. `at_file` is the offset of the file name, `at_file_sz` long, of the line a failure was made at, in the string table perf.wit `strtab` hands over, or 0 where the table had no room for the name, whose `at_line` is kept; every site word is 0 where the failure was the host's.
pub mod task {
    pub const STREAM: usize = 0;
    pub const PATH: usize = 1;
    pub const PRIORITY: usize = 4;
    pub const ENQUEUE_MS: usize = 5;
    pub const START_MS: usize = 6;
    pub const END_MS: usize = 7;
    pub const OUTCOME: usize = 8;
    pub const FAIL: usize = 9;
    pub const DETAIL: usize = 10;
    pub const AT_FILE: usize = 11;
    pub const AT_FILE_SZ: usize = 12;
    pub const AT_LINE: usize = 13;
    pub const PHASE_CNT: usize = 14;
    pub const PHASE_IDS: usize = 15;
    pub const PHASE_VALS: usize = 18;
    pub const WORDS: usize = 21;
}

/// One of the worst moving frames since the recording started, by its interval, of which the core keeps the longest few (pxl-core perf/record.rs `WORST_CAP`), which fit one page (LIMIT.PAGE_SZ), and hands them over together, longest first (perf.wit `worst`), and what changed from the frame before it: its uploads, their bytes and the heap's high-water mark, each this frame's less that one's, all 0 where `has_prev` is 0. A still interval is long on purpose and is never among them.
pub mod worst {
    pub const SAMPLE: usize = 0;
    pub const HAS_PREV: usize = 105;
    pub const UPLOAD_CNT_DELTA: usize = 106;
    pub const UPLOAD_DELTA_BYTES: usize = 107;
    pub const HEAP_USED_DELTA_BYTES: usize = 108;
    pub const WORDS: usize = 109;
}

/// A value's percentiles over n values: p50 and p95 each the nearest rank, the least value at or above which lie no more than that share of the n (sorted, the value at ceil(q × n) − 1), and the most.
pub mod percentile {
    pub const P50: usize = 0;
    pub const P95: usize = 1;
    pub const MAX: usize = 2;
    pub const WORDS: usize = 3;
}

/// One stream's tasks in the window, at STAT.tasks.slots + its index × WORDS. Each reason is (fail, detail, n), commonest first; `phase_mean` is NaN for a phase no task reported.
pub mod stream_stat {
    pub const CNT: usize = 0;
    pub const OK: usize = 1;
    pub const ABORTED: usize = 2;
    pub const ERR: usize = 3;
    pub const LAT_P50: usize = 4;
    pub const LAT_P95: usize = 5;
    pub const REASON_CNT: usize = 6;
    pub const REASONS: usize = 7;
    pub const PHASE_N: usize = 19;
    pub const PHASE_MEAN: usize = 20;
    pub const WORDS: usize = 23;
}

/// One registered step's timing, at STAT.steps.slots + its index × WORDS. `name` indexes STEP_NAME, whose phase is STEP_PHASE's (spec/client/core/perf/step.json5).
pub mod step_stat {
    pub const NAME: usize = 0;
    pub const MS: usize = 1;
    pub const CNT: usize = 2;
    pub const MAX_MS: usize = 3;
    pub const WORDS: usize = 4;
}

/// The window's statistics `perf.stats` computes, over its `window_ms`: each section's words at its AT, the fields relative to it. Every word an f64. Packed: a section's AT is where the one before it ends, and WORDS where the last ends, both laid out by the generator.
pub mod stat {
    pub mod frames {
        pub const AT: usize = 0;
        pub const PRESENT: usize = 0;
        pub const CNT: usize = 1;
        pub const SECONDS: usize = 2;
        pub const MOVING: usize = 3;
        pub const MOVING_MS: usize = 4;
        pub const OVERRAN: usize = 5;
        pub const GPU_TIMED: usize = 6;
        pub const INTERVAL: usize = 7;
        pub const TICK: usize = 10;
        pub const EFFECT: usize = 13;
        pub const PUMP: usize = 16;
        pub const STAGE: usize = 19;
        pub const STAGE_PARTS_P95: usize = 22;
        pub const FILL: usize = 26;
        pub const FILL_CNT: usize = 29;
        pub const PERF: usize = 30;
        pub const EFFECTS_P95: usize = 33;
        pub const INVALIDATIONS: usize = 40;
        pub const GPU_BUSY: usize = 46;
        pub const GPU_BUSY_KINDS: usize = 49;
        pub const PHASES_P95: usize = 76;
        pub const GPU_SPANS_P95: usize = 78;
        pub const GPU_SPANS_CNT: usize = 116;
        pub const KINDS: usize = 154;
        pub const GPU_PX: usize = 163;
        pub const RAYS: usize = 164;
        pub const HEAP_USED_DELTA_BYTES: usize = 184;
        pub const LAST: usize = 185;
        pub const WORDS: usize = 290;
    }
    pub mod tasks {
        pub const AT: usize = 290;
        pub const SLOTS: usize = 0;
        pub const WORDS: usize = 138;
    }
    pub mod steps {
        pub const AT: usize = 428;
        pub const CNT: usize = 0;
        pub const SLOTS: usize = 1;
        pub const WORDS: usize = 41;
    }
    /// What the frames held at the most or the least since the recording started, as the steps are timed, folded as each frame's record is taken (perf/record.rs `perf_fold_recording`): `frame_cnt` the frames folded; the heap's figures by their allocator's names (pxl-core sys/heap.rs `HeapStat`), the most of each but `largest_free`, the least, and `refusals` as it last stood; the body slots' peak (sys/io/body.rs `take_peak`); and `refusals` the tiles each base layer's tile cache refused a slot, summed, in BASE_KIND's order (domain/layer/tile_cache.rs `take_refusal_cnt`). Every word 0 before a frame is folded.
    pub mod recording {
        pub const AT: usize = 469;
        pub const FRAME_CNT: usize = 0;
        pub const HEAP_LIVE_MAX_BYTES: usize = 1;
        pub const HEAP_RUN_MAX_BYTES: usize = 2;
        pub const HEAP_SLOT_MAX_BYTES: usize = 3;
        pub const HEAP_RANGE_MAX_BYTES: usize = 4;
        pub const HEAP_LARGEST_FREE_MIN_BYTES: usize = 5;
        pub const HEAP_REFUSAL_CNT: usize = 6;
        pub const BODY_SLOTS_PEAK: usize = 7;
        pub const REFUSALS: usize = 8;
        pub const WORDS: usize = 12;
    }
    pub const WORDS: usize = 481;
}

/// What `perf.stats` finds as it reads, the window aside: each section's words at its AT, as STAT lays out its own. Every word an f64.
pub mod now {
    /// Whether it records, how long the recording has run, whether it lost what it had no room for (a table's oldest records, or a failure's file name the string table could not hold), and how many frames and tasks its tables hold.
    pub mod recording_state {
        pub const AT: usize = 0;
        pub const RECORDING: usize = 0;
        pub const ELAPSED_MS: usize = 1;
        pub const TRUNCATED: usize = 2;
        pub const FRAME_CNT: usize = 3;
        pub const TASK_CNT: usize = 4;
        pub const WORDS: usize = 5;
    }
    /// Whether the core draws, as the read finds it, recording or not, every count since the core booted: `frame_cnt` the frames the renderer encoded (pxl-render renderer.rs `drawn_frame_cnt`); `refused_admit` and `refused_stream` the frames that did not run, as the GPU admitted none (pxl-gpu-guest `admit::frame`) or the host still held the stream (pxl-gpu-guest stream.rs) (frame.rs `frame_task`).
    /// Then the device, as the host wrote its block (spec/gpu/api.json5 DEVICE), the same on the web and natively: `generation` the devices lost so far (pxl-render gpu/api.rs `device_generation`); `status` its STATUS, 0 before the block is written; `err_cnt` the uncaptured errors it raised; and `unfinished` the frames whose work its queue has not done, each a frame the host asked the queue to say done and has not heard of.
    pub mod draw {
        pub const AT: usize = 5;
        pub const FRAME_CNT: usize = 0;
        pub const REFUSED_ADMIT: usize = 1;
        pub const REFUSED_STREAM: usize = 2;
        pub const GENERATION: usize = 3;
        pub const STATUS: usize = 4;
        pub const ERR_CNT: usize = 5;
        pub const UNFINISHED: usize = 6;
        pub const WORDS: usize = 7;
    }
    /// Where the camera, its pivot and the pointer are as the read finds them, recording or not: `eye_x`..`eye_z`, `dist`, `pivot_x`..`pivot_z`, `hover_x`..`hover_z` and `place_x`..`place_z` in units, angles in radians, `pivot_lon` and `pivot_lat` in degrees and `pivot_alt` in meters, a zoom as its number, and NaN where there is none.
    /// The eye is where the camera looks from and the pivot the point it turns about, under the eye in the first-person camera, where the character stands (pxl-core domain/control/camera.rs `eye_world`, camera/controller.rs `pivot`).
    /// The hover is what the pointer meets, exactly, at the zooms the eye's rays ask, whatever the tool could strike, and `cell_zoom` the zoom the eye's rays draw there (pxl-core domain/query/detail.rs `cell_zoom_read`).
    /// The pick is what the tool held strikes there (pxl-core domain/control/edit/pick.rs `pick_at`): `hit_kind` what it met, an index into HIT_KIND; `pick_zoom` the zoom of the box struck and `ray_zoom` the rays' there, `folded` 1 where they draw coarser than the box, which strikes nothing. `place_x`..`place_z` is the corner the tool takes (pxl-core domain/control/edit.rs `Hover`), `place_zoom` the zoom it places at, and `blocked` 1 where that room is taken (domain/control/edit/collision.rs `is_placement_blocked`).
    pub mod view {
        pub const AT: usize = 12;
        pub const EYE_X: usize = 0;
        pub const EYE_Y: usize = 1;
        pub const EYE_Z: usize = 2;
        pub const DIST: usize = 3;
        pub const POLAR: usize = 4;
        pub const AZIMUTH: usize = 5;
        pub const PIVOT_X: usize = 6;
        pub const PIVOT_Y: usize = 7;
        pub const PIVOT_Z: usize = 8;
        pub const PIVOT_LON: usize = 9;
        pub const PIVOT_LAT: usize = 10;
        pub const PIVOT_ALT: usize = 11;
        pub const HOVER_X: usize = 12;
        pub const HOVER_Y: usize = 13;
        pub const HOVER_Z: usize = 14;
        pub const CELL_ZOOM: usize = 15;
        pub const HIT_KIND: usize = 16;
        pub const PICK_ZOOM: usize = 17;
        pub const RAY_ZOOM: usize = 18;
        pub const FOLDED: usize = 19;
        pub const PLACE_X: usize = 20;
        pub const PLACE_Y: usize = 21;
        pub const PLACE_Z: usize = 22;
        pub const PLACE_ZOOM: usize = 23;
        pub const BLOCKED: usize = 24;
        pub const WORDS: usize = 25;
    }
    pub const WORDS: usize = 37;
}

/// The tables the profiler keeps, each read by sequence (perf.wit `table`): the frames and the tasks, oldest first. A task's row never changes once it has landed; a frame's is written again a frame or two later, as its GPU read and its ray counts land (FRAME).
pub const TABLE_NAMES: &[&str] = &["frames", "tasks"];
