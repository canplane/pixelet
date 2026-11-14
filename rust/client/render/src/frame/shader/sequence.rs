// pxl-render/frame/shader/sequence.rs

//! The frame's calls over the buffers that hold indirect counts, in the order the host encodes
//! them (renderer.rs `frame`): the light's tiles' marks, then `RayPass::dispatch`, then
//! the shadow map's pass. Each call states its pipeline, the group set for it and the buffers
//! it views, and where it reads its count. Every indirect call in ray/cast.rs,
//! ray/raster.rs, gpu/generator.rs and pass/shadow.rs takes the place of its count from its
//! step here, and those files name the steps in this order (sequence.test.rs). access.rs
//! `check` refuses the hazards among the calls as the crate is built.

use crate::frame::shader::access::{
    check, light_mark, Bound, Frame, Persistent, Range, Step, END, NO_RANGE,
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
const fn hdr(queue: u32, word: u32, words: u32) -> Range {
    at(QUEUES, ray::queue_at(queue) + word, words)
}

/// The stages, by their place in `STAGES`. Place 0 is the host's writes, the stage every
/// `Step::write` takes (access.rs).
const IN_LIGHT_MARKS: u32 = 1;
const IN_EYE: u32 = 2;
const IN_EYE_HOLES: u32 = 3;
const IN_GENERATE: u32 = 4;
const IN_LIGHT: u32 = 5;
const IN_LIGHT_HOLES: u32 = 6;
const IN_MAP: u32 = 7;
const IN_SCREEN_MASK: u32 = 8;
const IN_LIGHT_MASK: u32 = 9;
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
    "the decor through the screen's mask",
    "the decor through the light's mask",
    "the world's lit pass",
];

// The steps the host's calls name, in the order the frame encodes them.

/// The render pipelines each render pass with an indirect draw draws. The groups of these
/// pipelines are all the groups the pass sets. The passes are the eye's raster
/// (ray/raster.rs `Raster::draw`), the light's raster (`Raster::light_draw`), the shadow
/// map's pass (pass/shadow.rs `Shadow::begin`, then the decor's and the characters' depth in
/// renderer.rs `frame`), and the world's lit pass from its gate to the ghosts
/// (renderer.rs `frame`).
pub const RASTER_PASS: &[u32] = &[PIPELINE_RASTER_MASK, PIPELINE_RASTER];
pub const LIGHT_RASTER_PASS: &[u32] = &[PIPELINE_RASTER];
pub const MAP_PASS: &[u32] = &[
    PIPELINE_LIGHT_MASK,
    PIPELINE_LIGHT_MAP,
    PIPELINE_DECOR_DEPTH,
    PIPELINE_SKINNED_DEPTH,
];
pub const LIT_PASS: &[u32] = &[
    PIPELINE_SCREEN_MASK,
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

/// The eye's raster draw of the record and its faces' pass (ray/raster.rs `Raster::draw`),
/// then the cast of the pixels in the damaged tiles (ray/cast.rs `RayPass::dispatch`).
pub const RASTER: Step =
    Step::draw_off("raster", IN_EYE, PIPELINE_RASTER, (RASTER_ARGS, 0)).in_pass(RASTER_PASS);
pub const FACE: Step = Step::dispatch_off(
    "face",
    IN_EYE,
    COMPUTE_FACE,
    &FACES_BINDS,
    (RASTER_ARGS, raster::ASKED_AT + raster::ARGS_FACES),
);
pub const PX: Step = Step::dispatch_off("px", IN_EYE, COMPUTE_CAST, &PIXELS_BINDS, (CAST_TILES, 0));
/// The eye's holes' hits into the node cache's drawable entries (ray/raster.rs
/// `Raster::rec_hits`). Their dispatch is first copied out of the node cache's head, because the
/// hits bind the node cache writable and a call cannot read its count from a buffer a group
/// set for it binds writable (access.rs rule 1).
pub const REC_HITS_DISPATCH: Step = Step::copy(
    "rec_hits_dispatch",
    IN_EYE_HOLES,
    at(NODE_CACHE, node_cache::HITS_ARGS, 3),
    at(RASTER_HITS, 0, 3),
);
pub const REC_HITS: Step = Step::dispatch_off(
    "rec_hits",
    IN_EYE_HOLES,
    COMPUTE_REC_HITS,
    &REC_HITS_OF_BINDS,
    (RASTER_HITS, 0),
);
pub const HIT_FACES: Step = Step::dispatch_off(
    "hit_faces",
    IN_EYE_HOLES,
    COMPUTE_FACE,
    &HIT_FACES_OF_BINDS,
    (RASTER_ARGS, raster::HIT_ASKED_AT + raster::ARGS_FACES),
);
pub const HIT_FACES_PUT: Step = Step::dispatch_off(
    "hit_faces_put",
    IN_EYE_HOLES,
    COMPUTE_REC_FACES,
    &REC_HITS_OF_BINDS,
    (RASTER_HITS, 0),
);
/// The thinning and the generation of the decor's slots, each dispatched off the list's args
/// (gpu/generator.rs `dispatch`).
pub const THIN: Step = Step::dispatch_off(
    "thin",
    IN_GENERATE,
    COMPUTE_GENERATION_THIN,
    &GENERATION_THIN_BINDS,
    (GENERATION_LIST, generation::THIN_ARGS),
);
pub const GENERATE: Step = Step::dispatch_off(
    "generate",
    IN_GENERATE,
    COMPUTE_GENERATION,
    &GENERATION_BINDS,
    (GENERATION_LIST, generation::GENERATE_ARGS),
);
/// The light's cast (ray/cast.rs `RayPass::dispatch`).
pub const LIGHT_MARK_WALKED: Step = Step::dispatch(
    "light_mark_walked",
    IN_LIGHT,
    COMPUTE_LIGHT_MARK_WALKED,
    &LIGHT_MARK_BINDS,
);
pub const LIGHT_MARK_DRAWS: Step = Step::dispatch_off(
    "light_mark_draws",
    IN_LIGHT,
    COMPUTE_LIGHT_MARK_DRAWS,
    &LIGHT_MARK_DRAWS_BINDS,
    (LIGHT_DRAWS, 0),
);
pub const LIGHT_SLIDE: Step = Step::dispatch_off(
    "light_slide",
    IN_LIGHT,
    COMPUTE_LIGHT_SLIDE,
    &LIGHT_SLIDE_BINDS,
    (LIGHT_RECEIVER_TILES, light::RECEIVER_TILES_SLIDE),
);
pub const LIGHT_PROPAGATE: Step = Step::dispatch_off(
    "light_propagate",
    IN_LIGHT,
    COMPUTE_LIGHT_PROPAGATE,
    &LIGHT_PROPAGATE_BINDS,
    (LIGHT_RECEIVER_TILES, light::RECEIVER_TILES_LIST),
);
pub const LIGHT_CELLS: Step = Step::dispatch_off(
    "light_cells",
    IN_LIGHT,
    COMPUTE_LIGHT_CELL,
    &LIGHT_WORLD_BINDS,
    (LIGHT_ARGS, light::ARGS_CELLS),
);
pub const LIGHT_SAMPLE_HIT: Step = Step::dispatch_off(
    "light_sample_hit",
    IN_LIGHT,
    COMPUTE_LIGHT_SAMPLE_HIT,
    &LIGHT_SAMPLE_HIT_BINDS,
    (LIGHT_ARGS, light::ARGS_SAMPLE_HIT),
);
/// The light's raster draw of the record (ray/raster.rs `Raster::light_draw`).
pub const LIGHT_RASTER: Step = Step::draw_off(
    "light_raster",
    IN_LIGHT,
    PIPELINE_RASTER,
    (LIGHT_RASTER_ARGS, 0),
)
.in_pass(LIGHT_RASTER_PASS);
pub const LIGHT_SLOTS: Step = Step::dispatch_off(
    "light_slots",
    IN_LIGHT,
    COMPUTE_LIGHT,
    &LIGHT_SLOTS_BINDS,
    (LIGHT_ARGS, light::ARGS_SLOT),
);
/// The light's holes' hits into the node cache's drawable entries (ray/raster.rs
/// `Raster::light_rec_hits`). Their dispatch is first copied out of the node cache's head,
/// as the eye's is.
pub const LIGHT_REC_HITS_DISPATCH: Step = Step::copy(
    "light_rec_hits_dispatch",
    IN_LIGHT_HOLES,
    at(NODE_CACHE, node_cache::HITS_ARGS, 3),
    at(LIGHT_RASTER_HITS, 0, 3),
);
pub const LIGHT_REC_HITS: Step = Step::dispatch_off(
    "light_rec_hits",
    IN_LIGHT_HOLES,
    COMPUTE_LIGHT_REC_HITS,
    &LIGHT_REC_HITS_BINDS,
    (LIGHT_RASTER_HITS, 0),
);
/// The shadow map's pass opens with one of two draws (pass/shadow.rs `Shadow::begin`): its gate
/// over the box the marks cover, or the world's depth laid in the tiles the receivers read.
pub const MAP_MASK: Step = Step::draw_off(
    "map_mask",
    IN_MAP,
    PIPELINE_LIGHT_MASK,
    (LIGHT_RECT_ARGS, 0),
)
.one_of(1)
.in_pass(MAP_PASS);
pub const MAP_LAY: Step = Step::draw_off(
    "map_lay",
    IN_MAP,
    PIPELINE_LIGHT_MAP,
    (LIGHT_ARGS, light::ARGS_LAY),
)
.one_of(1)
.in_pass(MAP_PASS);

/// The draws of the generated decor: as depth into the shadow map, and in the world's lit pass
/// with the impostors. Each is drawn whole off the generation's args, or through its gate off
/// the args the gate's cull wrote, and the two run in place of each other (pass/decor.rs
/// `draw_generated`, pass/impostor.rs `draw_generated`).
pub const DECOR_MAP: Step =
    Step::draw_regions("decor_map", IN_MAP, PIPELINE_DECOR_DEPTH, GENERATION_ARGS)
        .one_of(2)
        .in_pass(MAP_PASS);
pub const DECOR_MAP_MASKED: Step = Step::draw_regions(
    "decor_map_masked",
    IN_MAP,
    PIPELINE_DECOR_DEPTH,
    CULLED_LIGHT_ARGS,
)
.one_of(2)
.in_pass(MAP_PASS);
pub const DECOR_LIT: Step =
    Step::draw_regions("decor_lit", IN_LIT, PIPELINE_DECOR, GENERATION_ARGS)
        .one_of(3)
        .in_pass(LIT_PASS);
pub const DECOR_LIT_MASKED: Step = Step::draw_regions(
    "decor_lit_masked",
    IN_LIT,
    PIPELINE_DECOR,
    CULLED_SCREEN_ARGS,
)
.one_of(3)
.in_pass(LIT_PASS);
pub const IMPOSTOR_LIT: Step =
    Step::draw_regions("impostor_lit", IN_LIT, PIPELINE_IMPOSTOR, GENERATION_ARGS)
        .one_of(4)
        .in_pass(LIT_PASS);
pub const IMPOSTOR_LIT_MASKED: Step = Step::draw_regions(
    "impostor_lit_masked",
    IN_LIT,
    PIPELINE_IMPOSTOR,
    CULLED_SCREEN_ARGS,
)
.one_of(4)
.in_pass(LIT_PASS);

/// The count of the hits the holes' walks listed, copied into the node cache's head before
/// the last walk adds its own, where the hits' passes read it (ray/cast.rs `walk_run`).
/// One copy is the eye's and one the light's.
pub const HITS: Step = Step::copy(
    "hits",
    IN_EYE_HOLES,
    hdr(ray::QUEUE_EYE_HOLES, ray::QUEUE_HITS, 4),
    at(NODE_CACHE, node_cache::HITS_ARGS, 4),
);
pub const LIGHT_HITS: Step = Step::copy(
    "light_hits",
    IN_LIGHT_HOLES,
    hdr(ray::QUEUE_LIGHT_HOLES, ray::QUEUE_HITS, 4),
    at(NODE_CACHE, node_cache::HITS_ARGS, 4),
);

/// The host's writes, which land before the frame's first call.
pub const OPEN_NODE_CACHE: Step =
    Step::write("open_node_cache", at(NODE_CACHE, 0, node_cache::HDR_WORDS));
pub const OPEN_MARKS: Step = Step::write("open_marks", at(MARKS, 0, ray::MISS_ARGS_WORDS as u32));
pub const OPEN_QUEUE: Step = Step::write(
    "open_queue",
    hdr(ray::QUEUE_EYE_HOLES, 0, ray::QUEUE_EMPTY.len() as u32),
);
pub const OPEN_CORNERS_QUEUE: Step = Step::write(
    "open_corners_queue",
    hdr(ray::QUEUE_EYE_CORNERS, 0, ray::QUEUE_EMPTY.len() as u32),
);
pub const OPEN_SAMPLES_QUEUE: Step = Step::write(
    "open_samples_queue",
    hdr(ray::QUEUE_EYE_SAMPLES, 0, ray::QUEUE_EMPTY.len() as u32),
);
pub const OPEN_CAST_TILES: Step = Step::write("open_cast_tiles", at(CAST_TILES, 0, 4));
pub const OPEN_RASTER_ASKS: Step = Step::write(
    "open_raster_asks",
    at(RASTER_ARGS, raster::ASKED_AT, raster::ARGS_WORDS),
);
pub const OPEN_RASTER_HIT_ASKS: Step = Step::write(
    "open_raster_hit_asks",
    at(RASTER_ARGS, raster::HIT_ASKED_AT, raster::ARGS_WORDS),
);
pub const OPEN_LIGHT_ARGS: Step =
    Step::write("open_light_args", at(LIGHT_ARGS, 0, light::ARGS_WORDS));
/// The host writes the stood list's args whole when the world's receivers are stood afresh,
/// instead of the copy of what earlier casts left (`RECEIVER_TILES_BACK`).
pub const OPEN_LIGHT_RECEIVER_TILES_AFRESH: Step = Step::write(
    "open_light_receiver_tiles_afresh",
    at(LIGHT_RECEIVER_TILES, 0, light::RECEIVER_TILES_WORDS),
);
pub const OPEN_LIGHT_RECEIVER_TILES: Step = Step::write(
    "open_light_receiver_tiles",
    at(
        LIGHT_RECEIVER_TILES,
        light::RECEIVER_TILES_CARRY,
        light::RECEIVER_TILES_WORDS - light::RECEIVER_TILES_CARRY,
    ),
);
/// When the world's receivers are stood afresh, the host writes that none of the stood list
/// has been propagated yet (`light::carry_at`).
pub const OPEN_RECEIVER_TILES_PROPAGATED: Step = Step::write(
    "open_receiver_tiles_propagated",
    at(
        LIGHT_LIST,
        light::carry_at(SHADOW_SIZE) + light::RECEIVER_TILES_CNT,
        1,
    ),
);
pub const OPEN_LIGHT_MISSES: Step = Step::write(
    "open_light_misses",
    at(LIGHT_MISSES, 0, ray::MISS_ARGS_WORDS as u32),
);
pub const OPEN_LIGHT_QUEUE: Step = Step::write(
    "open_light_queue",
    hdr(ray::QUEUE_LIGHT_HOLES, 0, ray::QUEUE_EMPTY.len() as u32),
);
pub const OPEN_LIGHT_CORNERS_QUEUE: Step = Step::write(
    "open_light_corners_queue",
    hdr(ray::QUEUE_LIGHT_CORNERS, 0, ray::QUEUE_EMPTY.len() as u32),
);
pub const OPEN_LIGHT_SAMPLES_QUEUE: Step = Step::write(
    "open_light_samples_queue",
    hdr(ray::QUEUE_LIGHT_SAMPLES, 0, ray::QUEUE_EMPTY.len() as u32),
);
pub const OPEN_LIGHT_RASTER_ARGS: Step = Step::write(
    "open_light_raster_args",
    at(LIGHT_RASTER_ARGS, 0, raster::ARGS_WORDS),
);
pub const OPEN_GENERATION_LIST: Step = Step::write(
    "open_generation_list",
    at(GENERATION_LIST, 0, generation::RECTS_AT),
);
pub const RECEIVER_TILES_BACK: Step = Step::copy(
    "receiver_tiles_back",
    IN_EYE,
    at(
        LIGHT_LIST,
        light::carry_at(SHADOW_SIZE),
        light::RECEIVER_TILES_CARRY,
    ),
    at(
        LIGHT_RECEIVER_TILES,
        light::RECEIVER_TILES_LIST,
        light::RECEIVER_TILES_CARRY,
    ),
);
/// Copies back the read list's count that the casts since the world's receivers were last
/// stood afresh left, into the args where this cast adds to it. This cast's propagation goes on
/// counting from it.
pub const READ_CNT_BACK: Step = Step::copy(
    "read_cnt_back",
    IN_LIGHT,
    at(
        LIGHT_LIST,
        light::carry_at(SHADOW_SIZE) + light::CARRY_READ,
        1,
    ),
    at(LIGHT_ARGS, light::ARGS_CNTS + light::LIST_READS, 1),
);
/// Copies the eye's holes' count to where the queue reads it, as `LIGHT_HOLES_CNT` does the
/// light's. Its destination lies in the eye's beam rays' buffer, which is not one of the
/// frame's buffers.
pub const EYE_HOLES_CNT: Step = Step::copy(
    "eye_holes_cnt",
    IN_EYE_HOLES,
    at(MARKS, ray::MISS_ARGS_CNT, 1),
    NO_RANGE,
);
pub const RECEIVER_TILES_KEEP: Step = Step::copy(
    "receiver_tiles_keep",
    IN_LIGHT,
    at(
        LIGHT_RECEIVER_TILES,
        light::RECEIVER_TILES_LIST,
        light::RECEIVER_TILES_CARRY,
    ),
    at(
        LIGHT_LIST,
        light::carry_at(SHADOW_SIZE),
        light::RECEIVER_TILES_CARRY,
    ),
);
pub const READ_CNT_KEEP: Step = Step::copy(
    "read_cnt_keep",
    IN_LIGHT,
    at(LIGHT_ARGS, light::ARGS_CNTS + light::LIST_READS, 1),
    at(
        LIGHT_LIST,
        light::carry_at(SHADOW_SIZE) + light::CARRY_READ,
        1,
    ),
);
pub const RECEIVER_TILES_CNT: Step = Step::copy(
    "receiver_tiles_cnt",
    IN_LIGHT,
    at(LIGHT_RECEIVER_TILES, light::RECEIVER_TILES_CNT, 1),
    at(
        LIGHT_LIST,
        light::list_cnt_at(SHADOW_SIZE, light::LIST_RECEIVERS),
        1,
    ),
);
pub const CORNERS_CNT: Step = Step::copy(
    "corners_cnt",
    IN_LIGHT,
    at(LIGHT_ARGS, light::ARGS_CNTS + light::LIST_CORNERS, 1),
    at(
        LIGHT_LIST,
        light::list_cnt_at(SHADOW_SIZE, light::LIST_CORNERS),
        1,
    ),
);
pub const SOURCES_CNT: Step = Step::copy(
    "sources_cnt",
    IN_LIGHT,
    at(LIGHT_ARGS, light::ARGS_CNTS + light::LIST_SOURCES, 1),
    at(
        LIGHT_LIST,
        light::list_cnt_at(SHADOW_SIZE, light::LIST_SOURCES),
        1,
    ),
);
pub const DAMAGE_CNT: Step = Step::copy(
    "damage_cnt",
    IN_LIGHT,
    at(LIGHT_ARGS, light::ARGS_CNTS + light::LIST_DAMAGE, 1),
    at(
        LIGHT_LIST,
        light::list_cnt_at(SHADOW_SIZE, light::LIST_DAMAGE),
        1,
    ),
);
pub const LIGHT_HOLES_CNT: Step = Step::copy(
    "light_holes_cnt",
    IN_LIGHT,
    at(LIGHT_MISSES, ray::MISS_ARGS_CNT, 1),
    at(LIGHT_LIST, light::holes_at(SHADOW_SIZE), 1),
);

/// More of the host's writes. The host sets the raster's draw args to none where the record's
/// draw runs (ray/raster.rs `open_draw`), writes the node cache's asks and frame when
/// nothing casts, and writes the decor's boxes (gpu/generator.rs `prepare`).
pub const OPEN_RASTER_DRAW_ARGS: Step = Step::write(
    "open_raster_draw_args",
    at(RASTER_ARGS, 0, raster::ARGS_WORDS),
);
pub const OPEN_NODE_CACHE_CNTS: Step =
    Step::write("open_node_cache_cnts", at(NODE_CACHE, node_cache::ASKED, 1));
pub const OPEN_NODE_CACHE_FRAME: Step = Step::write(
    "open_node_cache_frame",
    at(NODE_CACHE, node_cache::FRAME, 1),
);
pub const OPEN_GENERATION_RECTS: Step = Step::write(
    "open_generation_rects",
    at(
        GENERATION_LIST,
        generation::RECTS_AT,
        generation::RECT_CAP * generation::RECT_WORDS,
    ),
);
/// The light's tiles' reads move with the box: they are copied out, then back in where the box
/// slid them (ray/cast.rs `RayPass::dispatch`).
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
    bufs: BUFS,
    appends: APPENDS,
    seeds: SEEDS,
    bounds: BOUNDS,
    persistent: PERSISTENT,
    steps: &[
        // The host's writes as it prepares the frame (ray/cast.rs `RayPass::prepare`,
        // ray/raster.rs `Raster::prepare`, `light_prepare`, `open_draw`, gpu/generator.rs
        // `prepare`).
        OPEN_NODE_CACHE,
        OPEN_MARKS,
        OPEN_QUEUE,
        OPEN_CORNERS_QUEUE,
        OPEN_SAMPLES_QUEUE,
        OPEN_CAST_TILES,
        OPEN_RASTER_ASKS,
        OPEN_RASTER_HIT_ASKS,
        OPEN_LIGHT_ARGS,
        OPEN_LIGHT_RECEIVER_TILES,
        OPEN_LIGHT_RECEIVER_TILES_AFRESH,
        OPEN_RECEIVER_TILES_PROPAGATED,
        OPEN_LIGHT_MISSES,
        OPEN_LIGHT_QUEUE,
        OPEN_LIGHT_CORNERS_QUEUE,
        OPEN_LIGHT_SAMPLES_QUEUE,
        OPEN_LIGHT_RASTER_ARGS,
        OPEN_GENERATION_LIST,
        OPEN_RASTER_DRAW_ARGS,
        OPEN_NODE_CACHE_CNTS,
        OPEN_NODE_CACHE_FRAME,
        OPEN_GENERATION_RECTS,
        // The cull of the decor generated last frame, which is drawn through the screen's
        // gate: it opens the regions' args, then lists each region's instances in the damaged
        // tiles (gpu/generator.rs `Generator::cull`).
        Step::dispatch(
            "decor_cull_args",
            IN_SCREEN_MASK,
            COMPUTE_DECOR_CULL_ARGS,
            &CULL_SCREEN_BINDS,
        ),
        Step::dispatch(
            "decor_cull",
            IN_SCREEN_MASK,
            COMPUTE_DECOR_CULL,
            &CULL_SCREEN_BINDS,
        ),
        // The box the light's tiles' marks cover, written as the args of the light's gate's
        // draw (gpu/tile.rs `dispatch_rect_args`).
        Step::dispatch(
            "light_rect_args",
            IN_LIGHT_MARKS,
            COMPUTE_RECT_ARGS,
            &LIGHT_RECT_BINDS,
        ),
        // What the casts since the world's receivers were last stood afresh left in the stood
        // list's args and in the read list's count is copied back in before every pass that
        // stands a tile, the eye's record draw among them.
        RECEIVER_TILES_BACK,
        READ_CNT_BACK,
        // The same cull, for the decor drawn through the light's gate.
        Step::dispatch(
            "decor_cull_args_light",
            IN_LIGHT_MASK,
            COMPUTE_DECOR_CULL_ARGS,
            &CULL_LIGHT_BINDS,
        ),
        Step::dispatch(
            "decor_cull_light",
            IN_LIGHT_MASK,
            COMPUTE_DECOR_CULL,
            &CULL_LIGHT_BINDS,
        ),
        // The eye's cast: its beam rays, their cells, its samples, the draw of its record, and
        // its pixels. First the run of its beam rays (ray/cast.rs `walk_run`).
        Step::dispatch(
            "corners",
            IN_EYE,
            COMPUTE_QUEUE_CORNERS,
            &CORNERS_WALK_BINDS,
        ),
        Step::dispatch("corners_ask", IN_EYE, COMPUTE_QUEUE_ASK, &CORNERS_ASK_BINDS),
        Step::dispatch(
            "corners_parked",
            IN_EYE,
            COMPUTE_QUEUE_CORNERS_PARKED,
            &CORNERS_WALK_BINDS,
        ),
        Step::dispatch(
            "corners_ask_parked",
            IN_EYE,
            COMPUTE_QUEUE_ASK_PARKED,
            &CORNERS_ASK_BINDS,
        ),
        Step::dispatch(
            "corners_last",
            IN_EYE,
            COMPUTE_QUEUE_LAST_CORNERS,
            &LATTICE_LAST_BINDS,
        ),
        Step::dispatch("cell", IN_EYE, COMPUTE_CELL, &WORLD_BINDS),
        // The run of the eye's samples (ray/cast.rs `walk_run`).
        Step::dispatch(
            "samples",
            IN_EYE,
            COMPUTE_QUEUE_SAMPLES,
            &SAMPLES_WALK_BINDS,
        ),
        Step::dispatch("samples_ask", IN_EYE, COMPUTE_QUEUE_ASK, &SAMPLES_ASK_BINDS),
        Step::dispatch(
            "samples_parked",
            IN_EYE,
            COMPUTE_QUEUE_SAMPLES_PARKED,
            &SAMPLES_WALK_BINDS,
        ),
        Step::dispatch(
            "samples_ask_parked",
            IN_EYE,
            COMPUTE_QUEUE_ASK_PARKED,
            &SAMPLES_ASK_BINDS,
        ),
        Step::dispatch(
            "samples_last",
            IN_EYE,
            COMPUTE_QUEUE_LAST_SAMPLES,
            &LATTICE_LAST_BINDS,
        ),
        Step::dispatch("sample_hit", IN_EYE, COMPUTE_SAMPLE_HIT, &SAMPLE_HIT_BINDS),
        Step::dispatch("rec_draw", IN_EYE, COMPUTE_REC_DRAW, &REC_BINDS),
        RASTER,
        FACE,
        Step::dispatch(
            "cast_tiles",
            IN_EYE,
            COMPUTE_CAST_TILES,
            &CAST_TILES_OF_BINDS,
        ),
        PX,
        // The eye's holes are walked, and their hits written into the record.
        EYE_HOLES_CNT,
        Step::dispatch("queue", IN_EYE_HOLES, COMPUTE_QUEUE, &WALK_BINDS),
        Step::dispatch(
            "queue_ask",
            IN_EYE_HOLES,
            COMPUTE_QUEUE_ASK,
            &WALK_ASK_BINDS,
        ),
        Step::dispatch(
            "queue_parked",
            IN_EYE_HOLES,
            COMPUTE_QUEUE_PARKED,
            &WALK_BINDS,
        ),
        Step::dispatch(
            "queue_ask_parked",
            IN_EYE_HOLES,
            COMPUTE_QUEUE_ASK_PARKED,
            &WALK_ASK_BINDS,
        ),
        HITS,
        Step::dispatch("queue_last", IN_EYE_HOLES, COMPUTE_QUEUE_LAST, &LAST_BINDS),
        REC_HITS_DISPATCH,
        REC_HITS,
        HIT_FACES,
        HIT_FACES_PUT,
        // The decor's generation, after the eye's rays.
        Step::dispatch(
            "generation_mark",
            IN_GENERATE,
            COMPUTE_GENERATION_MARK,
            &GENERATION_MARK_BINDS,
        ),
        THIN,
        GENERATE,
        Step::dispatch(
            "generation_args",
            IN_GENERATE,
            COMPUTE_GENERATION_ARGS,
            &GENERATION_ARGS_OF_BINDS,
        ),
        // The light's cast: its receivers stand, its tiles are read and damaged, and then come
        // its beam rays, its samples, the draw of its record and its slots.
        LIGHT_READS_OUT,
        LIGHT_READS_IN,
        Step::dispatch("rec_draw_for_light", IN_LIGHT, COMPUTE_REC_DRAW, &REC_BINDS),
        LIGHT_MARK_WALKED,
        Step::dispatch(
            "light_mark_draws_open",
            IN_LIGHT,
            COMPUTE_LIGHT_MARK_DRAWS_OPEN,
            &LIGHT_MARK_DRAWS_OPEN_BINDS,
        ),
        LIGHT_MARK_DRAWS,
        RECEIVER_TILES_CNT,
        LIGHT_SLIDE,
        LIGHT_PROPAGATE,
        RECEIVER_TILES_KEEP,
        READ_CNT_KEEP,
        Step::dispatch(
            "light_damage",
            IN_LIGHT,
            COMPUTE_LIGHT_DAMAGE,
            &LIGHT_PROPAGATE_BINDS,
        ),
        CORNERS_CNT,
        SOURCES_CNT,
        DAMAGE_CNT,
        // The run of the light's beam rays (ray/cast.rs `walk_run`).
        Step::dispatch(
            "light_corners",
            IN_LIGHT,
            COMPUTE_QUEUE_LIGHT_CORNERS,
            &LIGHT_CORNERS_WALK_BINDS,
        ),
        Step::dispatch(
            "light_corners_ask",
            IN_LIGHT,
            COMPUTE_QUEUE_ASK,
            &LIGHT_CORNERS_ASK_BINDS,
        ),
        Step::dispatch(
            "light_corners_parked",
            IN_LIGHT,
            COMPUTE_QUEUE_LIGHT_CORNERS_PARKED,
            &LIGHT_CORNERS_WALK_BINDS,
        ),
        Step::dispatch(
            "light_corners_ask_parked",
            IN_LIGHT,
            COMPUTE_QUEUE_ASK_PARKED,
            &LIGHT_CORNERS_ASK_BINDS,
        ),
        Step::dispatch(
            "light_corners_last",
            IN_LIGHT,
            COMPUTE_QUEUE_LAST_LIGHT_CORNERS,
            &LIGHT_LATTICE_LAST_BINDS,
        ),
        LIGHT_CELLS,
        // The run of the light's samples (ray/cast.rs `walk_run`).
        Step::dispatch(
            "light_samples",
            IN_LIGHT,
            COMPUTE_QUEUE_LIGHT_SAMPLES,
            &LIGHT_SAMPLES_WALK_BINDS,
        ),
        Step::dispatch(
            "light_samples_ask",
            IN_LIGHT,
            COMPUTE_QUEUE_ASK,
            &LIGHT_SAMPLES_ASK_BINDS,
        ),
        Step::dispatch(
            "light_samples_parked",
            IN_LIGHT,
            COMPUTE_QUEUE_LIGHT_SAMPLES_PARKED,
            &LIGHT_SAMPLES_WALK_BINDS,
        ),
        Step::dispatch(
            "light_samples_ask_parked",
            IN_LIGHT,
            COMPUTE_QUEUE_ASK_PARKED,
            &LIGHT_SAMPLES_ASK_BINDS,
        ),
        Step::dispatch(
            "light_samples_last",
            IN_LIGHT,
            COMPUTE_QUEUE_LAST_LIGHT_SAMPLES,
            &LIGHT_LATTICE_LAST_BINDS,
        ),
        LIGHT_SAMPLE_HIT,
        Step::dispatch(
            "light_rec_draw",
            IN_LIGHT,
            COMPUTE_LIGHT_REC_DRAW,
            &LIGHT_REC_BINDS,
        ),
        LIGHT_RASTER,
        LIGHT_SLOTS,
        LIGHT_HOLES_CNT,
        // The light's holes are walked, and their hits written into the record.
        Step::dispatch(
            "light_queue",
            IN_LIGHT_HOLES,
            COMPUTE_QUEUE_LIGHT,
            &LIGHT_WALK_BINDS,
        ),
        Step::dispatch(
            "light_queue_ask",
            IN_LIGHT_HOLES,
            COMPUTE_QUEUE_ASK,
            &LIGHT_WALK_ASK_BINDS,
        ),
        Step::dispatch(
            "light_queue_parked",
            IN_LIGHT_HOLES,
            COMPUTE_QUEUE_LIGHT_PARKED,
            &LIGHT_WALK_BINDS,
        ),
        Step::dispatch(
            "light_queue_ask_parked",
            IN_LIGHT_HOLES,
            COMPUTE_QUEUE_ASK_PARKED,
            &LIGHT_WALK_ASK_BINDS,
        ),
        LIGHT_HITS,
        Step::dispatch(
            "light_queue_last",
            IN_LIGHT_HOLES,
            COMPUTE_QUEUE_LAST_LIGHT,
            &LIGHT_LAST_BINDS,
        ),
        LIGHT_REC_HITS_DISPATCH,
        LIGHT_REC_HITS,
        MAP_MASK,
        MAP_LAY,
        DECOR_MAP,
        DECOR_MAP_MASKED,
        DECOR_LIT,
        DECOR_LIT_MASKED,
        IMPOSTOR_LIT,
        IMPOSTOR_LIT_MASKED,
    ],
};

/// The counts several passes of a stage add to through atomics. A copy may seed each one ahead
/// of those passes (`SEEDS`), and every pass that adds to a count runs before any pass that
/// reads it. They are: the eye's faces' asks, which the samples' passes lay and the holes'
/// hits lay at a place of their own (record_put.wgsl `observe`); the hits each observer's two
/// walks of the holes over the node cache list (queue_walk.wgsl); the stood list, which the
/// record's draw and both receivers' passes list tiles on (light_receiver_tile.wgsl
/// `receiver_tiles_put`); the hits' dispatch in the node cache's head, which the last walk
/// raises; the stood list's marks; the args the decor's culls write; the read list's count, by
/// which the propagation lists a tile (light_propagate.wgsl `read_tile`); and the frame's asks
/// (ask.wgsl `may_ask`).
const APPENDS: &[Range] = &[
    at(RASTER_ARGS, raster::ASKED_AT, raster::ARGS_WORDS),
    at(RASTER_ARGS, raster::HIT_ASKED_AT, raster::ARGS_WORDS),
    hdr(ray::QUEUE_EYE_HOLES, ray::QUEUE_HITS, 4),
    hdr(ray::QUEUE_LIGHT_HOLES, ray::QUEUE_HITS, 4),
    at(
        LIGHT_RECEIVER_TILES,
        light::RECEIVER_TILES_LIST,
        light::RECEIVER_TILES_CARRY,
    ),
    at(NODE_CACHE, node_cache::HITS_ARGS, 4),
    receiver_tiles_marks(),
    at(CULLED_SCREEN_ARGS, 0, END),
    at(CULLED_LIGHT_ARGS, 0, END),
    at(LIGHT_ARGS, light::ARGS_CNTS + light::LIST_READS, 1),
    at(NODE_CACHE, node_cache::ASKED, 1),
];

/// The copies that open a count before the passes that add to it. `hits` and `light_hits` copy
/// the hits each observer's holes' walks over the node cache listed into the node cache's
/// head, before the last walk adds its own (ray/cast.rs `walk_run`; queue.wgsl
/// `keep_hit`). `receiver_tiles_back` and `read_cnt_back` copy back the stood list's args and
/// the read list's count that the casts since the world's receivers were last stood afresh
/// left, before this cast's receivers and propagation add to them.
const SEEDS: &[&str] = &["hits", "light_hits", "receiver_tiles_back", "read_cnt_back"];

/// The counts read as a bound while passes still add to them: the frame's asks. Each ask is
/// counted one at a time, and the count runs past a reader's bound by none.
const BOUNDS: &[Bound] = &[Bound {
    range: at(NODE_CACHE, node_cache::ASKED, 1),
    overrun: 0,
}];

/// The stood list's marks in the light's args.
const fn receiver_tiles_marks() -> Range {
    let [first, words] = light_mark(light::LIST_RECEIVERS);
    at(LIGHT_ARGS, first, words)
}

const fn persistent_marks(which: u32, owner: &'static str) -> Persistent {
    let [first, words] = light_mark(which);
    Persistent {
        range: at(LIGHT_ARGS, first, words),
        owner,
        by: &[],
        invalidation: "a tile's mark is the number of the cast that last listed it, stale under \
            any other cast's (light_enlist.wgsl `enlist_in`)",
    }
}

/// A kept range of the stood or the read list's marks. Each mark is the number of the cast in
/// which the world's receivers were last stood afresh (pxl-engine `light::HDR_WORLD`).
const fn persistent_world_marks(which: u32, owner: &'static str) -> Persistent {
    let [first, words] = light_mark(which);
    Persistent {
        range: at(LIGHT_ARGS, first, words),
        owner,
        by: &[],
        invalidation: "a tile's mark is the number of the cast the world's receivers were last \
            stood afresh in, stale under any other: every tile stood on or read is listed again \
            where they are (light_receiver_tile.wgsl `stand`, light_propagate.wgsl `read_tile`)",
    }
}

/// What the frame keeps over frames, each with its owner and what makes it stale.
const PERSISTENT: &[Persistent] = &[
    Persistent {
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
    Persistent {
        range: at(GENERATION_ARGS, 0, END),
        owner: "generation_args",
        by: &[],
        invalidation: "the regions' args the last generation laid out, drawn through the masks \
            before this frame's lays them anew; none drawn where decor is off \
            (gpu/generator.rs `indirect_draw`)",
    },
    // The stood list's marks have no single owner step. One rule sets a tile's mark to the
    // cast's number, atomically and alike wherever it runs, and four steps apply it: the
    // record's draw for the eye and for the light, and the walked and the drawn receivers'
    // marks (light_receiver_tile.wgsl `stand`, `stand_box`).
    Persistent {
        by: &[
            "rec_draw",
            "rec_draw_for_light",
            "light_mark_walked",
            "light_mark_draws",
        ],
        ..persistent_world_marks(light::LIST_RECEIVERS, "light_receiver_tile.wgsl `stand`")
    },
    persistent_world_marks(light::LIST_READS, "light_propagate"),
    persistent_marks(light::LIST_CORNERS, "light_damage"),
    persistent_marks(light::LIST_SOURCES, "light_damage"),
    persistent_marks(light::LIST_DAMAGE, "light_damage"),
    Persistent {
        range: at(
            LIGHT_LIST,
            light::carry_at(SHADOW_SIZE),
            light::RECEIVER_TILES_CARRY,
        ),
        owner: "receiver_tiles_keep",
        by: &[],
        invalidation: "the stood list's args the casts since the world's receivers were stood \
            afresh left, its count the tiles propagated, stale where they are stood afresh: the \
            host then writes the stood list empty and none of it propagated, and copies none \
            back in (ray/cast.rs `RayPass::prepare`, `dispatch`)",
    },
    Persistent {
        range: at(
            LIGHT_LIST,
            light::carry_at(SHADOW_SIZE) + light::CARRY_READ,
            1,
        ),
        owner: "read_cnt_keep",
        by: &[],
        invalidation: "the read list's count the casts since the world's receivers were stood \
            afresh left, stale where they are stood afresh: the host then writes the light's \
            args with the count none and copies none back in (ray/cast.rs `RayPass::prepare`, \
            `dispatch`)",
    },
];

const _: () = check(&FRAME);

#[cfg(test)]
#[path = "sequence.test.rs"]
mod test;
