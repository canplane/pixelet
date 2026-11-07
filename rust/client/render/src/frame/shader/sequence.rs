// pxl-render/frame/shader/sequence.rs

//! The frame's calls over the buffers that hold indirect counts, in the order the host encodes
//! them (gpu/renderer.rs `frame`: the light's tiles' marks, then `RaysPass::dispatch`, then the
//! shadow map's pass): each call's pipeline, the groups set for it and the buffers they view,
//! and where it reads its count. Every indirect call in ray/cast.rs, ray/raster.rs,
//! gpu/generator.rs and gpu/shadow.rs takes its count's place from its step here, and names
//! them in this order (sequence.test.rs). The hazards over them are refused as the crate is
//! built (frame/shader/access.rs `check`), each the frame holds today named in `KNOWN`.

use crate::frame::shader::access::{
    check, light_marks, Bound, Frame, Kept, Known, Range, Step, END, NO_RANGE,
};
use crate::frame::shader::group::*;
use crate::frame::shader::record::*;
use crate::frame::shader::record::{
    PIPELINE_DECOR, PIPELINE_GHOST_BOX, PIPELINE_GHOST_DECOR, PIPELINE_IMPOSTOR,
    PIPELINE_LOCATION_BEACON, PIPELINE_LOCATION_ZONE, PIPELINE_SKINNED,
};
use crate::frame::shader::word::SHADOW_SIZE;
use pxl_content::generation;
use pxl_engine::{light, node_cache, raster, ray};

const fn at(buf: u32, at: u32, words: u32) -> Range {
    Range { buf, at, words }
}

/// Words `word` to `word + words` of queue `queue`'s head.
const fn head(queue: u32, word: u32, words: u32) -> Range {
    at(QUEUES, ray::queue_at(queue) + word, words)
}

/// The stages, by their place in `STAGES`; the host's writes the first (access.rs
/// `Step::write`).
const IN_LIGHT_MARKS: u32 = 1;
const IN_EYE: u32 = 2;
const IN_EYE_HOLES: u32 = 3;
const IN_GENERATE: u32 = 4;
const IN_LIGHT: u32 = 5;
const IN_LIGHT_HOLES: u32 = 6;
const IN_MAP: u32 = 7;
const IN_SCREEN_GATE: u32 = 8;
const IN_LIGHT_GATE: u32 = 9;
const IN_LIT: u32 = 10;
const STAGES: &[&str] = &[
    "the host's writes",
    "the light's tiles' marks",
    "the eye's cast",
    "the eye's holes",
    "the decor generated",
    "the light's cast",
    "the light's holes",
    "the shadow map",
    "the decor through the screen's gate",
    "the decor through the light's gate",
    "the world's lit pass",
];

// Each step a host's call names, in the order the frame encodes them.

/// The render pipelines each render pass with an indirect draw draws, whose groups are all the
/// pass sets: the eye's raster (ray/raster.rs `Raster::draw`) and the light's
/// (`Raster::light_draw`); the shadow map's pass (gpu/shadow.rs `Shadow::begin`, then the
/// decor's and the characters' depth, gpu/renderer.rs `frame`); and the world's lit pass, from its
/// gate to the ghosts (gpu/renderer.rs `frame`).
pub const RASTER_PASS: &[u32] = &[PIPELINE_RASTER_GATE, PIPELINE_RASTER];
pub const LIGHT_RASTER_PASS: &[u32] = &[PIPELINE_RASTER];
pub const MAP_PASS: &[u32] = &[
    PIPELINE_LIGHT_GATE,
    PIPELINE_LIGHT_MAP,
    PIPELINE_DECOR_DEPTH,
    PIPELINE_SKINNED_DEPTH,
];
pub const LIT_PASS: &[u32] = &[
    PIPELINE_SCREEN_GATE,
    PIPELINE_DECOR,
    PIPELINE_SCREEN_RAY,
    PIPELINE_IMPOSTOR,
    PIPELINE_AO_APPLY,
    PIPELINE_SKY,
    PIPELINE_STAR,
    PIPELINE_SKINNED,
    PIPELINE_CLOUD,
    PIPELINE_LOCATION_BEACON,
    PIPELINE_LOCATION_ZONE,
    PIPELINE_GHOST_BOX,
    PIPELINE_GHOST_DECOR,
];

/// The pixels' claimed tiles (ray/cast.rs `RaysPass::dispatch`).
/// The record's draw and its faces' pass (ray/raster.rs `Raster::draw`).
pub const RASTER: Step =
    Step::draw_off("raster", IN_EYE, PIPELINE_RASTER, &[], (RASTER_ARGS, 0)).in_pass(RASTER_PASS);
pub const FACE: Step = Step::dispatch_off(
    "face",
    IN_EYE,
    COMPUTE_FACE,
    &[&FACES_BINDS],
    (RASTER_ARGS, raster::ASKED_AT + raster::ARGS_FACES),
);
pub const PX: Step = Step::dispatch_off(
    "px",
    IN_EYE,
    COMPUTE_CAST,
    &[&PIXELS_BINDS],
    (CAST_TILES, 0),
);
/// The holes' stops into the node cache's drawable entries (ray/raster.rs `Raster::stops`):
/// their dispatch copied out of the node cache's head, which they bind writable, then the
/// stops off it.
pub const STOPS_DISPATCH: Step = Step::copy(
    "stops_dispatch",
    IN_EYE_HOLES,
    at(NODE_CACHE, node_cache::HITS_ARGS, 3),
    at(RASTER_STOPS, 0, 3),
);
pub const STOPS: Step = Step::dispatch_off(
    "stops",
    IN_EYE_HOLES,
    COMPUTE_RECORD_STOPS,
    &[&STOPS_OF_BINDS],
    (RASTER_STOPS, 0),
);
pub const STOP_FACES: Step = Step::dispatch_off(
    "stop_faces",
    IN_EYE_HOLES,
    COMPUTE_FACE,
    &[&STOP_FACES_OF_BINDS],
    (RASTER_ARGS, raster::STOP_ASKED_AT + raster::ARGS_FACES),
);
pub const STOP_FACES_PUT: Step = Step::dispatch_off(
    "stop_faces_put",
    IN_EYE_HOLES,
    COMPUTE_RECORD_FACES,
    &[&STOPS_OF_BINDS],
    (RASTER_STOPS, 0),
);
/// The decor's slots thinned and generated off the list's args (gpu/generator.rs `dispatch`).
pub const THIN: Step = Step::dispatch_off(
    "thin",
    IN_GENERATE,
    COMPUTE_GENERATION_THIN,
    &[&GENERATION_THIN_BINDS],
    (GENERATION_LIST, generation::THIN_ARGS),
);
pub const GENERATE: Step = Step::dispatch_off(
    "generate",
    IN_GENERATE,
    COMPUTE_GENERATION,
    &[&GENERATION_BINDS],
    (GENERATION_LIST, generation::GENERATE_ARGS),
);
/// The light's cast (ray/cast.rs `RaysPass::dispatch`).
pub const LIGHT_MARK_WALKED: Step = Step::dispatch(
    "light_mark_walked",
    IN_LIGHT,
    COMPUTE_LIGHT_MARK_WALKED,
    &[&LIGHT_MARK_BINDS],
);
pub const LIGHT_MARK_DRAWS: Step = Step::dispatch_off(
    "light_mark_draws",
    IN_LIGHT,
    COMPUTE_LIGHT_MARK_DRAWS,
    &[&LIGHT_MARK_DRAWS_BINDS],
    (LIGHT_DRAWS, 0),
);
pub const LIGHT_SLIDE: Step = Step::dispatch_off(
    "light_slide",
    IN_LIGHT,
    COMPUTE_LIGHT_SLIDE,
    &[&LIGHT_SLIDE_BINDS],
    (LIGHT_STOOD, light::STOOD_SLIDE),
);
pub const LIGHT_PROPAGATE: Step = Step::dispatch_off(
    "light_propagate",
    IN_LIGHT,
    COMPUTE_LIGHT_PROPAGATE,
    &[&LIGHT_PROPAGATE_BINDS],
    (LIGHT_STOOD, light::STOOD_LIST),
);
pub const LIGHT_CELLS: Step = Step::dispatch_off(
    "light_cells",
    IN_LIGHT,
    COMPUTE_LIGHT_CELL,
    &[&LIGHT_WORLD_BINDS],
    (LIGHT_ARGS, light::ARGS_CELLS),
);
pub const LIGHT_SAMPLE_BOX: Step = Step::dispatch_off(
    "light_sample_box",
    IN_LIGHT,
    COMPUTE_LIGHT_SAMPLE_BOX,
    &[&LIGHT_SAMPLE_BOX_BINDS],
    (LIGHT_ARGS, light::ARGS_SAMPLE_BOX),
);
/// The light's draw of the record (ray/raster.rs `Raster::light_draw`).
pub const LIGHT_RASTER: Step = Step::draw_off(
    "light_raster",
    IN_LIGHT,
    PIPELINE_RASTER,
    &[],
    (LIGHT_RASTER_ARGS, 0),
)
.in_pass(LIGHT_RASTER_PASS);
pub const LIGHT_SLOTS: Step = Step::dispatch_off(
    "light_slots",
    IN_LIGHT,
    COMPUTE_LIGHT,
    &[&LIGHT_SLOTS_BINDS],
    (LIGHT_ARGS, light::ARGS_SLOT),
);
/// The holes' stops into the node cache's drawable entries (ray/raster.rs
/// `Raster::light_stops`), their dispatch copied out of the node cache's head first.
pub const LIGHT_STOPS_DISPATCH: Step = Step::copy(
    "light_stops_dispatch",
    IN_LIGHT_HOLES,
    at(NODE_CACHE, node_cache::HITS_ARGS, 3),
    at(LIGHT_RASTER_STOPS, 0, 3),
);
pub const LIGHT_STOPS: Step = Step::dispatch_off(
    "light_stops",
    IN_LIGHT_HOLES,
    COMPUTE_LIGHT_RECORD_STOPS,
    &[&LIGHT_STOPS_BINDS],
    (LIGHT_RASTER_STOPS, 0),
);
/// The shadow map's pass: through its gate, off the marks' box, or the world's depth laid in
/// the tiles read (gpu/shadow.rs `Shadow::begin`).
pub const MAP_GATE: Step = Step::draw_off(
    "map_gate",
    IN_MAP,
    PIPELINE_LIGHT_GATE,
    &[],
    (LIGHT_BOX_ARGS, 0),
)
.one_of(1)
.in_pass(MAP_PASS);
pub const MAP_LAY: Step = Step::draw_off(
    "map_lay",
    IN_MAP,
    PIPELINE_LIGHT_MAP,
    &[],
    (LIGHT_ARGS, light::ARGS_LAY),
)
.one_of(1)
.in_pass(MAP_PASS);

/// The decor generated drawn: into the shadow map, as its depth, and in the world's lit pass,
/// with the impostors; each whole, off the generation's args, or through its gate, off the
/// gate's (gpu/pass/decor.rs `draw_generated`, gpu/pass/impostor.rs `draw_generated`).
pub const DECOR_MAP: Step = Step::draw_regions(
    "decor_map",
    IN_MAP,
    PIPELINE_DECOR_DEPTH,
    &[],
    GENERATION_ARGS,
)
.one_of(2)
.in_pass(MAP_PASS);
pub const DECOR_MAP_GATED: Step = Step::draw_regions(
    "decor_map_gated",
    IN_MAP,
    PIPELINE_DECOR_DEPTH,
    &[],
    GATED_LIGHT_ARGS,
)
.one_of(2)
.in_pass(MAP_PASS);
pub const DECOR_LIT: Step =
    Step::draw_regions("decor_lit", IN_LIT, PIPELINE_DECOR, &[], GENERATION_ARGS)
        .one_of(3)
        .in_pass(LIT_PASS);
pub const DECOR_LIT_GATED: Step = Step::draw_regions(
    "decor_lit_gated",
    IN_LIT,
    PIPELINE_DECOR,
    &[],
    GATED_SCREEN_ARGS,
)
.one_of(3)
.in_pass(LIT_PASS);
pub const IMPOSTOR_LIT: Step = Step::draw_regions(
    "impostor_lit",
    IN_LIT,
    PIPELINE_IMPOSTOR,
    &[],
    GENERATION_ARGS,
)
.one_of(4)
.in_pass(LIT_PASS);
pub const IMPOSTOR_LIT_GATED: Step = Step::draw_regions(
    "impostor_lit_gated",
    IN_LIT,
    PIPELINE_IMPOSTOR,
    &[],
    GATED_SCREEN_ARGS,
)
.one_of(4)
.in_pass(LIT_PASS);

/// The holes' last walk's count of stops copied into the node cache's head, where their passes
/// read it (ray/cast.rs `walk_holes`), the eye's and the light's.
pub const HITS: Step = Step::copy(
    "hits",
    IN_EYE_HOLES,
    head(ray::QUEUE_EYE_HOLES, ray::QUEUE_HITS, 4),
    at(NODE_CACHE, node_cache::HITS_ARGS, 4),
);
pub const LIGHT_HITS: Step = Step::copy(
    "light_hits",
    IN_LIGHT_HOLES,
    head(ray::QUEUE_LIGHT_HOLES, ray::QUEUE_HITS, 4),
    at(NODE_CACHE, node_cache::HITS_ARGS, 4),
);

/// The frame's calls, the host's writes first, which land before its first call.
pub const OPEN_NODE_CACHE: Step =
    Step::write("open_node_cache", at(NODE_CACHE, 0, node_cache::HEAD_WORDS));
pub const OPEN_MARKS: Step = Step::write("open_marks", at(MARKS, 0, ray::MISS_ARGS_WORDS as u32));
pub const OPEN_QUEUE: Step = Step::write(
    "open_queue",
    head(ray::QUEUE_EYE_HOLES, 0, ray::QUEUE_EMPTY.len() as u32),
);
pub const OPEN_CORNERS_QUEUE: Step = Step::write(
    "open_corners_queue",
    head(ray::QUEUE_EYE_CORNERS, 0, ray::QUEUE_EMPTY.len() as u32),
);
pub const OPEN_SAMPLES_QUEUE: Step = Step::write(
    "open_samples_queue",
    head(ray::QUEUE_EYE_SAMPLES, 0, ray::QUEUE_EMPTY.len() as u32),
);
pub const OPEN_CAST_TILES: Step = Step::write("open_cast_tiles", at(CAST_TILES, 0, 4));
pub const OPEN_RASTER_ARGS: Step = Step::write(
    "open_raster_args",
    at(RASTER_ARGS, 0, raster::ASKED_AT + raster::ARGS_WORDS),
);
pub const OPEN_RASTER_STOP_ASKS: Step = Step::write(
    "open_raster_stop_asks",
    at(RASTER_ARGS, raster::STOP_ASKED_AT, raster::ARGS_WORDS),
);
pub const OPEN_LIGHT_ARGS: Step =
    Step::write("open_light_args", at(LIGHT_ARGS, 0, light::ARGS_WORDS));
/// The stood list's args written whole where the world's receivers are stood afresh, in place
/// of those they left copied back in (`STOOD_BACK`).
pub const OPEN_LIGHT_STOOD_AFRESH: Step = Step::write(
    "open_light_stood_afresh",
    at(LIGHT_STOOD, 0, light::STOOD_WORDS),
);
pub const OPEN_LIGHT_STOOD: Step = Step::write(
    "open_light_stood",
    at(
        LIGHT_STOOD,
        light::STOOD_KEPT,
        light::STOOD_WORDS - light::STOOD_KEPT,
    ),
);
/// None of the stood list propagated where the world's receivers are stood afresh
/// (`light::kept_at`).
pub const OPEN_STOOD_PROPAGATED: Step = Step::write(
    "open_stood_propagated",
    at(
        LIGHT_LIST,
        light::kept_at(SHADOW_SIZE) + light::STOOD_CNT,
        1,
    ),
);
pub const OPEN_LIGHT_MISSES: Step = Step::write(
    "open_light_misses",
    at(LIGHT_MISSES, 0, ray::MISS_ARGS_WORDS as u32),
);
pub const OPEN_LIGHT_QUEUE: Step = Step::write(
    "open_light_queue",
    head(ray::QUEUE_LIGHT_HOLES, 0, ray::QUEUE_EMPTY.len() as u32),
);
pub const OPEN_LIGHT_CORNERS_QUEUE: Step = Step::write(
    "open_light_corners_queue",
    head(ray::QUEUE_LIGHT_CORNERS, 0, ray::QUEUE_EMPTY.len() as u32),
);
pub const OPEN_LIGHT_SAMPLES_QUEUE: Step = Step::write(
    "open_light_samples_queue",
    head(ray::QUEUE_LIGHT_SAMPLES, 0, ray::QUEUE_EMPTY.len() as u32),
);
pub const OPEN_LIGHT_RASTER_ARGS: Step = Step::write(
    "open_light_raster_args",
    at(LIGHT_RASTER_ARGS, 0, raster::ARGS_WORDS),
);
pub const OPEN_GENERATION_LIST: Step = Step::write(
    "open_generation_list",
    at(GENERATION_LIST, 0, generation::BOXES_AT),
);
pub const STOOD_BACK: Step = Step::copy(
    "stood_back",
    IN_EYE,
    at(LIGHT_LIST, light::kept_at(SHADOW_SIZE), light::STOOD_KEPT),
    at(LIGHT_STOOD, light::STOOD_LIST, light::STOOD_KEPT),
);
/// The read list's count the casts since the world's receivers were stood afresh left, copied
/// back in where this cast adds to them, the count its propagation goes on from.
pub const READ_COUNT_BACK: Step = Step::copy(
    "read_count_back",
    IN_LIGHT,
    at(
        LIGHT_LIST,
        light::kept_at(SHADOW_SIZE) + light::KEPT_READ,
        1,
    ),
    at(LIGHT_ARGS, light::ARGS_COUNTS + light::LISTED_READ, 1),
);
/// The eye's holes' count where the queue reads it, as the light's is (`LIGHT_HOLES_CNT`).
pub const EYE_HOLES_CNT: Step = Step::copy(
    "eye_holes_cnt",
    IN_EYE_HOLES,
    at(MARKS, ray::MISS_ARGS_CNT, 1),
    NO_RANGE,
);
/// The rays each observer's holes' walks over the node cache parked, copied into its pool, where
/// the last walk reads it (ray/cast.rs `walk_run`, pxl-engine `ray::POOL_CNT`).
pub const EYE_POOL_CNT: Step = Step::copy(
    "eye_pool_cnt",
    IN_EYE_HOLES,
    head(ray::QUEUE_EYE_HOLES, ray::QUEUE_PARKED, 1),
    NO_RANGE,
);
pub const LIGHT_POOL_CNT: Step = Step::copy(
    "light_pool_cnt",
    IN_LIGHT_HOLES,
    head(ray::QUEUE_LIGHT_HOLES, ray::QUEUE_PARKED, 1),
    NO_RANGE,
);
pub const STOOD_KEEP: Step = Step::copy(
    "stood_keep",
    IN_LIGHT,
    at(LIGHT_STOOD, light::STOOD_LIST, light::STOOD_KEPT),
    at(LIGHT_LIST, light::kept_at(SHADOW_SIZE), light::STOOD_KEPT),
);
pub const READ_COUNT_KEEP: Step = Step::copy(
    "read_count_keep",
    IN_LIGHT,
    at(LIGHT_ARGS, light::ARGS_COUNTS + light::LISTED_READ, 1),
    at(
        LIGHT_LIST,
        light::kept_at(SHADOW_SIZE) + light::KEPT_READ,
        1,
    ),
);
pub const STOOD_COUNT: Step = Step::copy(
    "stood_count",
    IN_LIGHT,
    at(LIGHT_STOOD, light::STOOD_CNT, 1),
    at(
        LIGHT_LIST,
        light::listed_at(SHADOW_SIZE, light::LISTED_STOOD),
        1,
    ),
);
pub const CORNERS_COUNT: Step = Step::copy(
    "corners_count",
    IN_LIGHT,
    at(LIGHT_ARGS, light::ARGS_COUNTS + light::LISTED_CORNERS, 1),
    at(
        LIGHT_LIST,
        light::listed_at(SHADOW_SIZE, light::LISTED_CORNERS),
        1,
    ),
);
pub const FED_COUNT: Step = Step::copy(
    "fed_count",
    IN_LIGHT,
    at(LIGHT_ARGS, light::ARGS_COUNTS + light::LISTED_FED, 1),
    at(
        LIGHT_LIST,
        light::listed_at(SHADOW_SIZE, light::LISTED_FED),
        1,
    ),
);
pub const CLAIMED_COUNT: Step = Step::copy(
    "claimed_count",
    IN_LIGHT,
    at(LIGHT_ARGS, light::ARGS_COUNTS + light::LISTED_CLAIMED, 1),
    at(
        LIGHT_LIST,
        light::listed_at(SHADOW_SIZE, light::LISTED_CLAIMED),
        1,
    ),
);
pub const LIGHT_HOLES_CNT: Step = Step::copy(
    "light_holes_cnt",
    IN_LIGHT,
    at(LIGHT_MISSES, ray::MISS_ARGS_CNT, 1),
    at(LIGHT_LIST, light::holes_at(SHADOW_SIZE), 1),
);

/// The host's writes past the opens above: the raster's draw args set to none where the
/// record's draw runs (ray/raster.rs `record_draw`); the node cache's asks and frame where
/// nothing casts; the decor's boxes (gpu/generator.rs `prepare`).
pub const OPEN_RASTER_DRAW_ARGS: Step = Step::write(
    "open_raster_draw_args",
    at(RASTER_ARGS, 0, raster::ARGS_WORDS),
);
pub const OPEN_NODE_CACHE_COUNTS: Step = Step::write(
    "open_node_cache_counts",
    at(NODE_CACHE, node_cache::ASKED, 1),
);
pub const OPEN_NODE_CACHE_FRAME: Step = Step::write(
    "open_node_cache_frame",
    at(NODE_CACHE, node_cache::FRAME, 1),
);
pub const OPEN_GENERATION_BOXES: Step = Step::write(
    "open_generation_boxes",
    at(
        GENERATION_LIST,
        generation::BOXES_AT,
        generation::BOX_CAP * generation::BOX_WORDS,
    ),
);
/// The light's tiles' reads, moved with the box: copied out, and back in where the box slid
/// them (ray/cast.rs `RaysPass::dispatch`).
pub const LIGHT_READS_OUT: Step = Step::copy(
    "light_reads_out",
    IN_LIGHT,
    at(
        LIGHT_LIST,
        light::reads_at(SHADOW_SIZE),
        light::tiles(SHADOW_SIZE) * light::tiles(SHADOW_SIZE),
    ),
    NO_RANGE,
);
pub const LIGHT_READS_IN: Step = Step::copy(
    "light_reads_in",
    IN_LIGHT,
    NO_RANGE,
    at(
        LIGHT_LIST,
        light::reads_at(SHADOW_SIZE),
        light::tiles(SHADOW_SIZE) * light::tiles(SHADOW_SIZE),
    ),
);

pub const FRAME: Frame = Frame {
    stages: STAGES,
    buffers: BUFFERS,
    appends: APPENDS,
    seeds: SEEDS,
    bounds: BOUNDS,
    kept: KEPT,
    steps: &[
        // The host's writes as a frame is prepared (ray/cast.rs `RaysPass::prepare`,
        // ray/raster.rs `Raster::prepare`, `light_prepare`, `record_draw`, gpu/generator.rs
        // `prepare`).
        OPEN_NODE_CACHE,
        OPEN_MARKS,
        OPEN_QUEUE,
        OPEN_CORNERS_QUEUE,
        OPEN_SAMPLES_QUEUE,
        OPEN_CAST_TILES,
        OPEN_RASTER_ARGS,
        OPEN_RASTER_STOP_ASKS,
        OPEN_LIGHT_ARGS,
        OPEN_LIGHT_STOOD,
        OPEN_LIGHT_STOOD_AFRESH,
        OPEN_STOOD_PROPAGATED,
        OPEN_LIGHT_MISSES,
        OPEN_LIGHT_QUEUE,
        OPEN_LIGHT_CORNERS_QUEUE,
        OPEN_LIGHT_SAMPLES_QUEUE,
        OPEN_LIGHT_RASTER_ARGS,
        OPEN_GENERATION_LIST,
        OPEN_RASTER_DRAW_ARGS,
        OPEN_NODE_CACHE_COUNTS,
        OPEN_NODE_CACHE_FRAME,
        OPEN_GENERATION_BOXES,
        // The decor generated last frame drawn through the screen's gate: its regions' args
        // opened, and those of its instances in the claimed tiles listed (gpu/generator.rs
        // `Generator::gate`).
        Step::dispatch(
            "decor_gate_args",
            IN_SCREEN_GATE,
            COMPUTE_DECOR_GATE_ARGS,
            &[&GATE_SCREEN_BINDS],
        ),
        Step::dispatch(
            "decor_gate",
            IN_SCREEN_GATE,
            COMPUTE_DECOR_GATE,
            &[&GATE_SCREEN_BINDS],
        ),
        // The light's tiles' marks' box counted into its gate's draw (gpu/tiles.rs
        // `dispatch_box_args`).
        Step::dispatch(
            "light_box_args",
            IN_LIGHT_MARKS,
            COMPUTE_BOX_ARGS,
            &[&LIGHT_BOX_BINDS],
        ),
        // What the casts since the world's receivers were stood afresh left in the stood list's
        // args and the read list's count copied back in, before every pass that stands a tile,
        // the eye's draw among them.
        STOOD_BACK,
        READ_COUNT_BACK,
        // The same through the light's gate.
        Step::dispatch(
            "decor_gate_args_light",
            IN_LIGHT_GATE,
            COMPUTE_DECOR_GATE_ARGS,
            &[&GATE_LIGHT_BINDS],
        ),
        Step::dispatch(
            "decor_gate_light",
            IN_LIGHT_GATE,
            COMPUTE_DECOR_GATE,
            &[&GATE_LIGHT_BINDS],
        ),
        // The eye's cast: its beam rays, their cells, its samples, its record drawn, and its
        // pixels.
        // The eye's beam rays' run (ray/cast.rs `walk_run`).
        Step::dispatch(
            "corners",
            IN_EYE,
            COMPUTE_QUEUE_CORNERS,
            &[&CORNERS_WALK_BINDS],
        ),
        Step::dispatch(
            "corners_ask",
            IN_EYE,
            COMPUTE_QUEUE_ASK_CORNERS,
            &[&CORNERS_ASK_BINDS],
        ),
        Step::dispatch(
            "corners_parked",
            IN_EYE,
            COMPUTE_QUEUE_CORNERS_PARKED,
            &[&CORNERS_WALK_BINDS],
        ),
        Step::dispatch(
            "corners_ask_parked",
            IN_EYE,
            COMPUTE_QUEUE_ASK_CORNERS_PARKED,
            &[&CORNERS_ASK_BINDS],
        ),
        Step::dispatch(
            "corners_last",
            IN_EYE,
            COMPUTE_QUEUE_LAST_CORNERS,
            &[&LATTICE_LAST_BINDS],
        ),
        Step::dispatch("cell", IN_EYE, COMPUTE_CELL, &[&WORLD_BINDS]),
        // The eye's samples' run (ray/cast.rs `walk_run`).
        Step::dispatch(
            "samples",
            IN_EYE,
            COMPUTE_QUEUE_SAMPLES,
            &[&SAMPLES_WALK_BINDS],
        ),
        Step::dispatch(
            "samples_ask",
            IN_EYE,
            COMPUTE_QUEUE_ASK_SAMPLES,
            &[&SAMPLES_ASK_BINDS],
        ),
        Step::dispatch(
            "samples_parked",
            IN_EYE,
            COMPUTE_QUEUE_SAMPLES_PARKED,
            &[&SAMPLES_WALK_BINDS],
        ),
        Step::dispatch(
            "samples_ask_parked",
            IN_EYE,
            COMPUTE_QUEUE_ASK_SAMPLES_PARKED,
            &[&SAMPLES_ASK_BINDS],
        ),
        Step::dispatch(
            "samples_last",
            IN_EYE,
            COMPUTE_QUEUE_LAST_SAMPLES,
            &[&LATTICE_LAST_BINDS],
        ),
        Step::dispatch(
            "sample_box",
            IN_EYE,
            COMPUTE_SAMPLE_BOX,
            &[&SAMPLE_BOX_BINDS],
        ),
        Step::dispatch("record_draw", IN_EYE, COMPUTE_RECORD_DRAW, &[&RECORD_BINDS]),
        RASTER,
        FACE,
        Step::dispatch(
            "cast_tiles",
            IN_EYE,
            COMPUTE_CAST_TILES,
            &[&CAST_TILES_OF_BINDS],
        ),
        PX,
        // The eye's holes walked, their stops into the record.
        EYE_HOLES_CNT,
        Step::dispatch("queue", IN_EYE_HOLES, COMPUTE_QUEUE, &[&WALK_BINDS]),
        Step::dispatch(
            "queue_ask",
            IN_EYE_HOLES,
            COMPUTE_QUEUE_ASK,
            &[&WALK_ASK_BINDS],
        ),
        Step::dispatch(
            "queue_parked",
            IN_EYE_HOLES,
            COMPUTE_QUEUE_PARKED,
            &[&WALK_BINDS],
        ),
        Step::dispatch(
            "queue_ask_parked",
            IN_EYE_HOLES,
            COMPUTE_QUEUE_ASK_PARKED,
            &[&WALK_ASK_BINDS],
        ),
        HITS,
        EYE_POOL_CNT,
        Step::dispatch(
            "queue_last",
            IN_EYE_HOLES,
            COMPUTE_QUEUE_LAST,
            &[&LAST_BINDS],
        ),
        STOPS_DISPATCH,
        STOPS,
        STOP_FACES,
        STOP_FACES_PUT,
        // The decor generated over the rays.
        Step::dispatch(
            "generation_mark",
            IN_GENERATE,
            COMPUTE_GENERATION_MARK,
            &[&GENERATION_MARK_BINDS],
        ),
        THIN,
        GENERATE,
        Step::dispatch(
            "generation_args",
            IN_GENERATE,
            COMPUTE_GENERATION_ARGS,
            &[&GENERATION_ARGS_OF_BINDS],
        ),
        // The light's cast: its receivers stood, its tiles read and claimed, its beam, its
        // samples, its record drawn and its slots.
        LIGHT_READS_OUT,
        LIGHT_READS_IN,
        Step::dispatch(
            "record_draw_for_light",
            IN_LIGHT,
            COMPUTE_RECORD_DRAW,
            &[&RECORD_BINDS],
        ),
        LIGHT_MARK_WALKED,
        Step::dispatch(
            "light_mark_draws_open",
            IN_LIGHT,
            COMPUTE_LIGHT_MARK_DRAWS_OPEN,
            &[&LIGHT_MARK_DRAWS_OPEN_BINDS],
        ),
        LIGHT_MARK_DRAWS,
        STOOD_COUNT,
        LIGHT_SLIDE,
        LIGHT_PROPAGATE,
        STOOD_KEEP,
        READ_COUNT_KEEP,
        Step::dispatch(
            "light_claim",
            IN_LIGHT,
            COMPUTE_LIGHT_CLAIM,
            &[&LIGHT_PROPAGATE_BINDS],
        ),
        CORNERS_COUNT,
        FED_COUNT,
        CLAIMED_COUNT,
        // The light's beam rays' run (ray/cast.rs `walk_run`).
        Step::dispatch(
            "light_corners",
            IN_LIGHT,
            COMPUTE_QUEUE_LIGHT_CORNERS,
            &[&LIGHT_CORNERS_WALK_BINDS],
        ),
        Step::dispatch(
            "light_corners_ask",
            IN_LIGHT,
            COMPUTE_QUEUE_ASK_LIGHT_CORNERS,
            &[&LIGHT_CORNERS_ASK_BINDS],
        ),
        Step::dispatch(
            "light_corners_parked",
            IN_LIGHT,
            COMPUTE_QUEUE_LIGHT_CORNERS_PARKED,
            &[&LIGHT_CORNERS_WALK_BINDS],
        ),
        Step::dispatch(
            "light_corners_ask_parked",
            IN_LIGHT,
            COMPUTE_QUEUE_ASK_LIGHT_CORNERS_PARKED,
            &[&LIGHT_CORNERS_ASK_BINDS],
        ),
        Step::dispatch(
            "light_corners_last",
            IN_LIGHT,
            COMPUTE_QUEUE_LAST_LIGHT_CORNERS,
            &[&LIGHT_LATTICE_LAST_BINDS],
        ),
        LIGHT_CELLS,
        // The light's samples' run (ray/cast.rs `walk_run`).
        Step::dispatch(
            "light_samples",
            IN_LIGHT,
            COMPUTE_QUEUE_LIGHT_SAMPLES,
            &[&LIGHT_SAMPLES_WALK_BINDS],
        ),
        Step::dispatch(
            "light_samples_ask",
            IN_LIGHT,
            COMPUTE_QUEUE_ASK_LIGHT_SAMPLES,
            &[&LIGHT_SAMPLES_ASK_BINDS],
        ),
        Step::dispatch(
            "light_samples_parked",
            IN_LIGHT,
            COMPUTE_QUEUE_LIGHT_SAMPLES_PARKED,
            &[&LIGHT_SAMPLES_WALK_BINDS],
        ),
        Step::dispatch(
            "light_samples_ask_parked",
            IN_LIGHT,
            COMPUTE_QUEUE_ASK_LIGHT_SAMPLES_PARKED,
            &[&LIGHT_SAMPLES_ASK_BINDS],
        ),
        Step::dispatch(
            "light_samples_last",
            IN_LIGHT,
            COMPUTE_QUEUE_LAST_LIGHT_SAMPLES,
            &[&LIGHT_LATTICE_LAST_BINDS],
        ),
        LIGHT_SAMPLE_BOX,
        Step::dispatch(
            "light_record_draw",
            IN_LIGHT,
            COMPUTE_LIGHT_RECORD_DRAW,
            &[&LIGHT_RECORD_BINDS],
        ),
        LIGHT_RASTER,
        LIGHT_SLOTS,
        LIGHT_HOLES_CNT,
        // The light's holes walked and their stops into the record.
        Step::dispatch(
            "light_queue",
            IN_LIGHT_HOLES,
            COMPUTE_QUEUE,
            &[&LIGHT_WALK_BINDS],
        ),
        Step::dispatch(
            "light_queue_ask",
            IN_LIGHT_HOLES,
            COMPUTE_QUEUE_ASK,
            &[&LIGHT_WALK_ASK_BINDS],
        ),
        Step::dispatch(
            "light_queue_parked",
            IN_LIGHT_HOLES,
            COMPUTE_QUEUE_PARKED,
            &[&LIGHT_WALK_BINDS],
        ),
        Step::dispatch(
            "light_queue_ask_parked",
            IN_LIGHT_HOLES,
            COMPUTE_QUEUE_ASK_PARKED,
            &[&LIGHT_WALK_ASK_BINDS],
        ),
        LIGHT_HITS,
        LIGHT_POOL_CNT,
        Step::dispatch(
            "light_queue_last",
            IN_LIGHT_HOLES,
            COMPUTE_QUEUE_LAST,
            &[&LIGHT_LAST_BINDS],
        ),
        LIGHT_STOPS_DISPATCH,
        LIGHT_STOPS,
        MAP_GATE,
        MAP_LAY,
        DECOR_MAP,
        DECOR_MAP_GATED,
        DECOR_LIT,
        DECOR_LIT_GATED,
        IMPOSTOR_LIT,
        IMPOSTOR_LIT_GATED,
    ],
};

/// The counts several passes of a stage add to through atomics, each a copy may seed ahead of
/// them, each pass before any that
/// reads it: the eye's faces' asks, laid by both of the samples' passes, and by the holes'
/// stops and those put off (record_put.wgsl `observe`); the stops each observer's holes' two
/// walks over the node cache list (queue_walk.wgsl); the stood list the record's draw and both
/// receivers' passes list a tile on (light_stood.wgsl `stood_put`); the node cache's stops'
/// dispatch the last walk raises; the read list's count the propagation lists a tile by
/// (light_propagate.wgsl `read_tile`); the frame's asks (queue_last.wgsl `may_ask`).
const APPENDS: &[Range] = &[
    at(RASTER_ARGS, raster::ASKED_AT, raster::ARGS_WORDS),
    at(RASTER_ARGS, raster::STOP_ASKED_AT, raster::ARGS_WORDS),
    head(ray::QUEUE_EYE_HOLES, ray::QUEUE_HITS, 4),
    head(ray::QUEUE_LIGHT_HOLES, ray::QUEUE_HITS, 4),
    at(LIGHT_STOOD, light::STOOD_LIST, light::STOOD_KEPT),
    at(NODE_CACHE, node_cache::HITS_ARGS, 4),
    stood_marks(),
    at(GATED_SCREEN_ARGS, 0, END),
    at(GATED_LIGHT_ARGS, 0, END),
    at(LIGHT_ARGS, light::ARGS_COUNTS + light::LISTED_READ, 1),
    at(NODE_CACHE, node_cache::ASKED, 1),
];

/// The copies that open a count before the passes that add to it: the stops the holes' walks
/// over the node cache listed, copied into the node cache's head before the last walk adds its
/// own (ray/cast.rs `walk_holes`, queue_last.wgsl `keep_hit`), each observer's; the stood
/// list's args and the read list's count the casts since the world's receivers were stood
/// afresh left, copied back in before this cast's receivers and propagation add to them.
const SEEDS: &[&str] = &["hits", "light_hits", "stood_back", "read_count_back"];

/// The counts read as a bound while they are added to: the frame's asks, counted one at a time
/// past none.
const BOUNDS: &[Bound] = &[Bound {
    range: at(NODE_CACHE, node_cache::ASKED, 1),
    overrun: 0,
}];

/// The stood marks in the light's args.
const fn stood_marks() -> Range {
    let [first, words] = light_marks(light::LISTED_STOOD);
    at(LIGHT_ARGS, first, words)
}

const fn marks_kept(which: u32, owner: &'static str) -> Kept {
    let [first, words] = light_marks(which);
    Kept {
        range: at(LIGHT_ARGS, first, words),
        owner,
        by: &[],
        invalidation: "a tile's mark is the number of the cast that last listed it, stale under \
            any other cast's (light_enlist.wgsl `enlist_in`)",
    }
}

/// The stood and the read lists' marks: the number of the cast the world's receivers were last
/// stood afresh in (pxl-engine `light::HEAD_WORLD`).
const fn world_marks_kept(which: u32, owner: &'static str) -> Kept {
    let [first, words] = light_marks(which);
    Kept {
        range: at(LIGHT_ARGS, first, words),
        owner,
        by: &[],
        invalidation: "a tile's mark is the number of the cast the world's receivers were last \
            stood afresh in, stale under any other: every tile stood on or read is listed again \
            where they are (light_stood.wgsl `stand`, light_propagate.wgsl `read_tile`)",
    }
}

/// What the frame keeps over frames, each with its owner and what makes it stale.
const KEPT: &[Kept] = &[
    Kept {
        range: at(
            LIGHT_LIST,
            light::reads_at(SHADOW_SIZE),
            light::tiles(SHADOW_SIZE) * light::tiles(SHADOW_SIZE),
        ),
        owner: "light_reads_in",
        by: &[],
        invalidation: "a tile's read: the cast that last read its texels, moved with the box \
            and none where the box slid onto it or into another band (light_slide.wgsl)",
    },
    Kept {
        range: at(GENERATION_ARGS, 0, END),
        owner: "generation_args",
        by: &[],
        invalidation: "the regions' args the last generation laid out, drawn through the gates \
            before this frame's lays them anew; none drawn where decor is off \
            (gpu/generator.rs `indirect_draw`)",
    },
    // The stood marks: one rule stands a tile to the cast's number, atomic and alike wherever
    // it runs, applied by the record's draw, the eye's or the light's cast's, and the walked and
    // the drawn receivers' marks (light_stood.wgsl `stand`, `stand_box`).
    Kept {
        by: &[
            "record_draw",
            "record_draw_for_light",
            "light_mark_walked",
            "light_mark_draws",
        ],
        ..world_marks_kept(light::LISTED_STOOD, "light_stood.wgsl `stand`")
    },
    world_marks_kept(light::LISTED_READ, "light_propagate"),
    marks_kept(light::LISTED_CORNERS, "light_claim"),
    marks_kept(light::LISTED_FED, "light_claim"),
    marks_kept(light::LISTED_CLAIMED, "light_claim"),
    Kept {
        range: at(LIGHT_LIST, light::kept_at(SHADOW_SIZE), light::STOOD_KEPT),
        owner: "stood_keep",
        by: &[],
        invalidation: "the stood list's args the casts since the world's receivers were stood \
            afresh left, its count the tiles propagated, stale where they are stood afresh: the \
            host then writes the stood list empty and none of it propagated, and copies none \
            back in (ray/cast.rs `RaysPass::prepare`, `dispatch`)",
    },
    Kept {
        range: at(
            LIGHT_LIST,
            light::kept_at(SHADOW_SIZE) + light::KEPT_READ,
            1,
        ),
        owner: "read_count_keep",
        by: &[],
        invalidation: "the read list's count the casts since the world's receivers were stood \
            afresh left, stale where they are stood afresh: the host then writes the light's \
            args with the count none and copies none back in (ray/cast.rs `RaysPass::prepare`, \
            `dispatch`)",
    },
];

/// The hazards the frame holds today, each by its rule, its step and what it holds it with.
pub const KNOWN: &[Known] = &[];

const _: () = check(&FRAME, KNOWN);

#[cfg(test)]
#[path = "sequence.test.rs"]
mod test;
