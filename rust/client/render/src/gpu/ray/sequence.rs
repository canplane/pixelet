// pxl-render/gpu/ray/sequence.rs

//! The frame's calls over the buffers that hold indirect counts, in the order the host encodes
//! them (gpu/renderer.rs `frame`: the light's tiles' marks, then `RaysPass::dispatch`, then the
//! shadow map's pass): each call's pipeline, the groups set for it and the buffers they view,
//! and where it reads its count. Every indirect call in ray/cast.rs, ray/raster.rs,
//! gpu/generator.rs and gpu/shadow.rs takes its count's place from its step here, and names
//! them in this order (sequence.test.rs). The hazards over them are refused as the crate is
//! built (frame/shader/access.rs `check`), each the frame holds today named in `KNOWN`.

use super::cast::{
    ASK_GROUP, CAST_GROUP, CAST_TILES_GROUP, CLAIM_BINDING, DRAWS_BINDING, LIGHT_CLAIM_GROUP,
    LIGHT_GROUP, LIGHT_MARK_DRAWS_GROUP, LIGHT_MARK_DRAWS_OPEN_GROUP, LIGHT_MARK_GROUP,
    LIGHT_PROPAGATE_GROUP, LIGHT_SAMPLE_HIT_GROUP, LIGHT_SLIDE_GROUP, QUEUE_ASK_GROUP, QUEUE_GROUP,
    QUEUE_LAST_GROUP, SAMPLE_HIT_GROUP, WORLD_GROUP,
};
use super::raster::{
    DEFERRED_BINDING, DEFERRED_GROUP, FACE_GROUP, LIGHT_DEFERRED_GROUP, LIGHT_SAMPLE_BOX_GROUP,
    LIGHT_STOPS_GROUP, RECORD_GROUP, SAMPLE_BOX_GROUP, STOPS_GROUP,
};
use crate::frame::shader::access::{
    check, Bind, Buffer, Frame, Group, Known, Range, Step, ARGS, LIST, NO_RANGE, STATE,
};
use crate::frame::shader::record::*;
use crate::gpu::generator::{ARGS_GROUP, GENERATE_GROUP, MARK_GROUP, THIN_GROUP};
use crate::gpu::shadow::SIZE;
use crate::gpu::tiles::BOX_ARGS_GROUP;
use pxl_content::generation;
use pxl_engine::{count, light, node_cache, raster, ray};

/// The buffers, by their place in `BUFFERS`.
const MARKS: u32 = 0;
const NODE_CACHE: u32 = 1;
const QUEUE: u32 = 2;
const BEAM_LEFT: u32 = 3;
const SAMPLE_LEFT: u32 = 4;
const CAST_TILES: u32 = 5;
const RASTER_ARGS: u32 = 6;
const RASTER_DEFERRED: u32 = 7;
const LIGHT_ARGS: u32 = 8;
const LIGHT_LIST: u32 = 9;
const LIGHT_STOOD: u32 = 10;
const LIGHT_DRAWS: u32 = 11;
const LIGHT_CLAIMS: u32 = 12;
const LIGHT_BEAM_LEFT: u32 = 13;
const LIGHT_SAMPLE_LEFT: u32 = 14;
const LIGHT_MISSES: u32 = 15;
const LIGHT_QUEUE: u32 = 16;
const LIGHT_RASTER_ARGS: u32 = 17;
const LIGHT_RASTER_DEFERRED: u32 = 18;
const LIGHT_BOX_ARGS: u32 = 19;
const GENERATION_LIST: u32 = 20;
const GENERATION_ARGS: u32 = 21;

/// Each buffer, as the host names its field, and its role.
const BUFFERS: &[Buffer] = &[
    buffer("marks", ARGS),
    buffer("node_cache", STATE),
    buffer("queue", STATE),
    buffer("beam_left", ARGS),
    buffer("sample_left", ARGS),
    buffer("cast_tiles", ARGS),
    buffer("raster.args", ARGS),
    buffer("raster.deferred", ARGS),
    buffer("light_args", ARGS),
    buffer("light_list", LIST),
    buffer("light_stood", ARGS),
    buffer("light_draws_dispatch", ARGS),
    buffer("light_claim_dispatch", ARGS),
    buffer("light_beam_left", ARGS),
    buffer("light_sample_left", ARGS),
    buffer("light_misses", ARGS),
    buffer("light_queue", STATE),
    buffer("raster.light.args", ARGS),
    buffer("raster.light.deferred", ARGS),
    buffer("light_tiles.box_args", ARGS),
    buffer("generator.list", ARGS),
    buffer("generator.args", ARGS),
];

const fn buffer(name: &'static str, role: u32) -> Buffer {
    Buffer { name, role }
}

const fn at(buf: u32, at: u32, words: u32) -> Range {
    Range { buf, at, words }
}

const fn bind(binding: u32, buf: u32) -> Bind {
    Bind {
        binding,
        buf,
        at: 0,
    }
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
const STAGES: &[&str] = &[
    "the host's writes",
    "the light's tiles' marks",
    "the eye's cast",
    "the eye's holes",
    "the decor generated",
    "the light's cast",
    "the light's holes",
    "the shadow map",
];

// The groups as the host makes them (ray/cast.rs `RaysPass::prepare`, `light_groups`,
// `queue_groups`; ray/raster.rs `Raster::prepare`, `record_group`, `light_record_group`;
// gpu/generator.rs `regroup`; gpu/tiles.rs `Tiles::new`): each binding to a buffer here.
const BEAM_HIT_BINDS: Group = Group {
    layout: &SAMPLE_HIT_GROUP,
    binds: &[bind(9, NODE_CACHE), bind(10, BEAM_LEFT)],
};
const SAMPLE_HIT_BINDS: Group = Group {
    layout: &SAMPLE_HIT_GROUP,
    binds: &[bind(9, NODE_CACHE), bind(10, SAMPLE_LEFT)],
};
const ASK_BINDS: Group = Group {
    layout: &ASK_GROUP,
    binds: &[bind(9, NODE_CACHE)],
};
const WORLD_BINDS: Group = Group {
    layout: &WORLD_GROUP,
    binds: &[bind(9, NODE_CACHE)],
};
const SAMPLE_BOX_BINDS: Group = Group {
    layout: &SAMPLE_BOX_GROUP,
    binds: &[bind(16, RASTER_ARGS)],
};
const RECORD_BINDS: Group = Group {
    layout: &RECORD_GROUP,
    binds: &[
        bind(9, NODE_CACHE),
        bind(16, RASTER_ARGS),
        bind(21, LIGHT_ARGS),
        bind(22, LIGHT_STOOD),
    ],
};
const FACES_BINDS: Group = Group {
    layout: &FACE_GROUP,
    binds: &[Bind {
        binding: 16,
        buf: RASTER_ARGS,
        at: raster::ASKED_AT,
    }],
};
const STOP_FACES_OF_BINDS: Group = Group {
    layout: &FACE_GROUP,
    binds: &[Bind {
        binding: 16,
        buf: RASTER_ARGS,
        at: raster::STOP_ASKED_AT,
    }],
};
const CAST_TILES_OF_BINDS: Group = Group {
    layout: &CAST_TILES_GROUP,
    binds: &[bind(0, CAST_TILES)],
};
const PIXELS_BINDS: Group = Group {
    layout: &CAST_GROUP,
    binds: &[bind(9, NODE_CACHE), bind(14, MARKS), bind(30, CAST_TILES)],
};
const WALK_BINDS: Group = Group {
    layout: &QUEUE_GROUP,
    binds: &[bind(9, NODE_CACHE), bind(10, QUEUE)],
};
const WALK_ASK_BINDS: Group = Group {
    layout: &QUEUE_ASK_GROUP,
    binds: &[bind(9, NODE_CACHE), bind(10, QUEUE)],
};
const LAST_BINDS: Group = Group {
    layout: &QUEUE_LAST_GROUP,
    binds: &[bind(9, NODE_CACHE)],
};
const STOPS_OF_BINDS: Group = Group {
    layout: &STOPS_GROUP,
    binds: &[
        bind(9, NODE_CACHE),
        bind(16, RASTER_ARGS),
        bind(DEFERRED_BINDING, RASTER_DEFERRED),
    ],
};
const DEFERRED_BINDS: Group = Group {
    layout: &DEFERRED_GROUP,
    binds: &[bind(16, RASTER_ARGS)],
};
const GENERATION_MARK_BINDS: Group = Group {
    layout: &MARK_GROUP,
    binds: &[bind(11, GENERATION_LIST)],
};
const GENERATION_THIN_BINDS: Group = Group {
    layout: &THIN_GROUP,
    binds: &[bind(11, GENERATION_LIST)],
};
const GENERATION_BINDS: Group = Group {
    layout: &GENERATE_GROUP,
    binds: &[bind(11, GENERATION_LIST)],
};
const GENERATION_ARGS_OF_BINDS: Group = Group {
    layout: &ARGS_GROUP,
    binds: &[bind(2, GENERATION_ARGS)],
};
const LIGHT_BOX_BINDS: Group = Group {
    layout: &BOX_ARGS_GROUP,
    binds: &[bind(1, LIGHT_BOX_ARGS)],
};
const LIGHT_RECORD_BINDS: Group = Group {
    layout: &RECORD_GROUP,
    binds: &[
        bind(9, NODE_CACHE),
        bind(16, LIGHT_RASTER_ARGS),
        bind(21, LIGHT_ARGS),
        bind(22, LIGHT_STOOD),
    ],
};
const LIGHT_MARK_BINDS: Group = Group {
    layout: &LIGHT_MARK_GROUP,
    binds: &[bind(21, LIGHT_ARGS), bind(22, LIGHT_STOOD)],
};
const LIGHT_MARK_DRAWS_OPEN_BINDS: Group = Group {
    layout: &LIGHT_MARK_DRAWS_OPEN_GROUP,
    binds: &[
        bind(21, LIGHT_ARGS),
        bind(22, LIGHT_STOOD),
        bind(DRAWS_BINDING, LIGHT_DRAWS),
    ],
};
const LIGHT_MARK_DRAWS_BINDS: Group = Group {
    layout: &LIGHT_MARK_DRAWS_GROUP,
    binds: &[bind(21, LIGHT_ARGS), bind(22, LIGHT_STOOD)],
};
const LIGHT_SLIDE_BINDS: Group = Group {
    layout: &LIGHT_SLIDE_GROUP,
    binds: &[],
};
const LIGHT_PROPAGATE_BINDS: Group = Group {
    layout: &LIGHT_PROPAGATE_GROUP,
    binds: &[bind(21, LIGHT_ARGS), bind(CLAIM_BINDING, LIGHT_CLAIMS)],
};
const LIGHT_CLAIM_BINDS: Group = Group {
    layout: &LIGHT_CLAIM_GROUP,
    binds: &[bind(21, LIGHT_ARGS)],
};
const LIGHT_BEAM_HIT_BINDS: Group = Group {
    layout: &LIGHT_SAMPLE_HIT_GROUP,
    binds: &[bind(9, NODE_CACHE), bind(10, LIGHT_BEAM_LEFT)],
};
const LIGHT_SAMPLE_HIT_BINDS: Group = Group {
    layout: &LIGHT_SAMPLE_HIT_GROUP,
    binds: &[bind(9, NODE_CACHE), bind(10, LIGHT_SAMPLE_LEFT)],
};
const LIGHT_SAMPLE_BOX_BINDS: Group = Group {
    layout: &LIGHT_SAMPLE_BOX_GROUP,
    binds: &[bind(16, LIGHT_RASTER_ARGS)],
};
const LIGHT_SLOTS_BINDS: Group = Group {
    layout: &LIGHT_GROUP,
    binds: &[bind(17, LIGHT_MISSES)],
};
const LIGHT_WALK_BINDS: Group = Group {
    layout: &QUEUE_GROUP,
    binds: &[bind(9, NODE_CACHE), bind(10, LIGHT_QUEUE)],
};
const LIGHT_WALK_ASK_BINDS: Group = Group {
    layout: &QUEUE_ASK_GROUP,
    binds: &[bind(9, NODE_CACHE), bind(10, LIGHT_QUEUE)],
};
const LIGHT_STOPS_BINDS: Group = Group {
    layout: &LIGHT_STOPS_GROUP,
    binds: &[
        bind(9, NODE_CACHE),
        bind(16, LIGHT_RASTER_ARGS),
        bind(DEFERRED_BINDING, LIGHT_RASTER_DEFERRED),
    ],
};
const LIGHT_DEFERRED_BINDS: Group = Group {
    layout: &LIGHT_DEFERRED_GROUP,
    binds: &[bind(16, LIGHT_RASTER_ARGS)],
};

// Each step a host's call names, in the order the frame encodes them.

/// The eye's beam rays left, the samples left and the pixels' claimed tiles (ray/cast.rs
/// `RaysPass::dispatch`).
pub const BEAM: Step =
    Step::dispatch_off("beam", IN_EYE, COMPUTE_BEAM, &[&ASK_BINDS], (BEAM_LEFT, 0));
pub const SAMPLE: Step = Step::dispatch_off(
    "sample",
    IN_EYE,
    COMPUTE_SAMPLE,
    &[&ASK_BINDS],
    (SAMPLE_LEFT, 0),
);
/// The record's draw and its faces' pass (ray/raster.rs `Raster::draw`).
pub const RASTER: Step = Step::draw_off("raster", IN_EYE, PIPELINE_RASTER, &[], (RASTER_ARGS, 0));
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
/// The holes' stops into the eye's record (ray/raster.rs `Raster::stops`).
pub const STOPS: Step = Step::dispatch_off(
    "stops",
    IN_EYE_HOLES,
    COMPUTE_RECORD_STOPS,
    &[&STOPS_OF_BINDS],
    (NODE_CACHE, node_cache::HITS_ARGS),
);
pub const STOPS_DEFERRED: Step = Step::dispatch_off(
    "stops_deferred",
    IN_EYE_HOLES,
    COMPUTE_RECORD_DEFERRED,
    &[&DEFERRED_BINDS],
    (RASTER_DEFERRED, 0),
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
    (NODE_CACHE, node_cache::HITS_ARGS),
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
pub const LIGHT_MARK_WALKED: Step = Step::dispatch_off(
    "light_mark_walked",
    IN_LIGHT,
    COMPUTE_LIGHT_MARK_WALKED,
    &[&LIGHT_MARK_BINDS],
    (MARKS, ray::MISS_ARGS_LIST),
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
pub const LIGHT_CLAIM: Step = Step::dispatch_off(
    "light_claim",
    IN_LIGHT,
    COMPUTE_LIGHT_CLAIM,
    &[&LIGHT_CLAIM_BINDS],
    (LIGHT_CLAIMS, 0),
);
pub const LIGHT_BEAM_HIT: Step = Step::dispatch_off(
    "light_beam_hit",
    IN_LIGHT,
    COMPUTE_LIGHT_BEAM_HIT,
    &[&LIGHT_BEAM_HIT_BINDS],
    (LIGHT_ARGS, light::ARGS_BEAM),
);
pub const LIGHT_BEAM: Step = Step::dispatch_off(
    "light_beam",
    IN_LIGHT,
    COMPUTE_LIGHT_BEAM,
    &[&ASK_BINDS],
    (LIGHT_BEAM_LEFT, 0),
);
pub const LIGHT_CELLS: Step = Step::dispatch_off(
    "light_cells",
    IN_LIGHT,
    COMPUTE_LIGHT_CELL,
    &[&WORLD_BINDS],
    (LIGHT_ARGS, light::ARGS_CELLS),
);
pub const LIGHT_SAMPLE_HIT: Step = Step::dispatch_off(
    "light_sample_hit",
    IN_LIGHT,
    COMPUTE_LIGHT_SAMPLE_HIT,
    &[&LIGHT_SAMPLE_HIT_BINDS],
    (LIGHT_ARGS, light::ARGS_SAMPLE),
);
pub const LIGHT_SAMPLE: Step = Step::dispatch_off(
    "light_sample",
    IN_LIGHT,
    COMPUTE_LIGHT_SAMPLE,
    &[&ASK_BINDS],
    (LIGHT_SAMPLE_LEFT, 0),
);
pub const LIGHT_SAMPLE_BOX: Step = Step::dispatch_off(
    "light_sample_box",
    IN_LIGHT,
    COMPUTE_LIGHT_SAMPLE_BOX,
    &[&LIGHT_SAMPLE_BOX_BINDS],
    (LIGHT_ARGS, light::ARGS_SAMPLE_BOX),
);
pub const LIGHT_SAMPLE_OBSERVE: Step = Step::dispatch_off(
    "light_sample_observe",
    IN_LIGHT,
    COMPUTE_LIGHT_SAMPLE_OBSERVE,
    &[&LIGHT_SAMPLE_BOX_BINDS],
    (LIGHT_ARGS, light::ARGS_SAMPLE_BOX),
);
/// The light's record's draw (ray/raster.rs `Raster::light_draw`).
pub const LIGHT_RASTER: Step = Step::draw_off(
    "light_raster",
    IN_LIGHT,
    PIPELINE_RASTER,
    &[],
    (LIGHT_RASTER_ARGS, 0),
);
pub const LIGHT_SLOTS: Step = Step::dispatch_off(
    "light_slots",
    IN_LIGHT,
    COMPUTE_LIGHT,
    &[&LIGHT_SLOTS_BINDS],
    (LIGHT_ARGS, light::ARGS_SLOT),
);
/// The holes' stops into the light's record (ray/raster.rs `Raster::light_stops`).
pub const LIGHT_STOPS: Step = Step::dispatch_off(
    "light_stops",
    IN_LIGHT_HOLES,
    COMPUTE_LIGHT_RECORD_STOPS,
    &[&LIGHT_STOPS_BINDS],
    (NODE_CACHE, node_cache::HITS_ARGS),
);
pub const LIGHT_STOPS_DEFERRED: Step = Step::dispatch_off(
    "light_stops_deferred",
    IN_LIGHT_HOLES,
    COMPUTE_LIGHT_RECORD_DEFERRED,
    &[&LIGHT_DEFERRED_BINDS],
    (LIGHT_RASTER_DEFERRED, 0),
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
.one_of(1);
pub const MAP_LAY: Step = Step::draw_off(
    "map_lay",
    IN_MAP,
    PIPELINE_LIGHT_MAP,
    &[],
    (LIGHT_ARGS, light::ARGS_LAY),
)
.one_of(1);

/// The holes' last walk's count of stops copied into the node cache's head, where their passes
/// read it (ray/cast.rs `walk_holes`), the eye's and the light's.
pub const HITS: [Step; 2] = [
    Step::copy(
        "hits",
        IN_EYE_HOLES,
        at(QUEUE, ray::QUEUE_HITS, 4),
        at(NODE_CACHE, node_cache::HITS_ARGS, 4),
    ),
    Step::copy(
        "light_hits",
        IN_LIGHT_HOLES,
        at(LIGHT_QUEUE, ray::QUEUE_HITS, 4),
        at(NODE_CACHE, node_cache::HITS_ARGS, 4),
    ),
];

/// The frame's calls, the host's writes first, which land before its first call.
pub const FRAME: Frame = Frame {
    stages: STAGES,
    buffers: BUFFERS,
    steps: &[
        // The host's writes as a frame is prepared (ray/cast.rs `RaysPass::prepare`,
        // ray/raster.rs `Raster::prepare`, `light_prepare`, `record_draw`, gpu/generator.rs
        // `prepare`).
        Step::write(
            "node_cache's head",
            at(NODE_CACHE, 0, node_cache::HEAD_WORDS),
        ),
        Step::write("marks", at(MARKS, 0, ray::MISS_ARGS_WORDS as u32)),
        Step::write("queue", at(QUEUE, 0, ray::QUEUE_EMPTY.len() as u32)),
        Step::write("beam_left", at(BEAM_LEFT, 0, ray::LEFT_EMPTY.len() as u32)),
        Step::write(
            "sample_left",
            at(SAMPLE_LEFT, 0, ray::LEFT_EMPTY.len() as u32),
        ),
        Step::write("cast_tiles", at(CAST_TILES, 0, 4)),
        Step::write(
            "raster.args",
            at(RASTER_ARGS, 0, raster::ASKED_AT + raster::ARGS_WORDS),
        ),
        Step::write(
            "raster.args's stops' asks",
            at(RASTER_ARGS, raster::STOP_ASKED_AT, raster::ARGS_WORDS),
        ),
        Step::write(
            "raster.args's stops put off",
            at(RASTER_ARGS, raster::DEFERRED_CNT, raster::DEFERRED_WORDS),
        ),
        Step::write("raster.deferred", at(RASTER_DEFERRED, 0, 3)),
        Step::write("light_args", at(LIGHT_ARGS, 0, light::ARGS_WORDS)),
        Step::write(
            "light_args's rings",
            at(
                LIGHT_ARGS,
                light::ARGS_WORDS + light::rings_at(SIZE),
                light::rings(SIZE),
            ),
        ),
        Step::write(
            "light_stood",
            at(
                LIGHT_STOOD,
                light::STOOD_KEPT,
                light::STOOD_WORDS - light::STOOD_KEPT,
            ),
        ),
        Step::write("light_claim_dispatch", at(LIGHT_CLAIMS, 0, 3)),
        Step::write(
            "light_misses",
            at(LIGHT_MISSES, 0, ray::MISS_ARGS_WORDS as u32),
        ),
        Step::write(
            "light_queue",
            at(LIGHT_QUEUE, 0, ray::QUEUE_EMPTY.len() as u32),
        ),
        Step::write(
            "light_beam_left",
            at(LIGHT_BEAM_LEFT, 0, ray::LEFT_EMPTY.len() as u32),
        ),
        Step::write(
            "light_sample_left",
            at(LIGHT_SAMPLE_LEFT, 0, ray::LEFT_EMPTY.len() as u32),
        ),
        Step::write(
            "raster.light.args",
            at(LIGHT_RASTER_ARGS, 0, raster::ARGS_WORDS),
        ),
        Step::write(
            "raster.light.args's stops put off",
            at(
                LIGHT_RASTER_ARGS,
                raster::DEFERRED_CNT,
                raster::DEFERRED_WORDS,
            ),
        ),
        Step::write("raster.light.deferred", at(LIGHT_RASTER_DEFERRED, 0, 3)),
        Step::write(
            "generator.list",
            at(GENERATION_LIST, 0, generation::BOXES_AT),
        ),
        // The light's tiles' marks' box counted into its gate's draw (gpu/tiles.rs
        // `dispatch_box_args`).
        Step::dispatch(
            "light_box_args",
            IN_LIGHT_MARKS,
            COMPUTE_BOX_ARGS,
            &[&LIGHT_BOX_BINDS],
        ),
        // The eye's cast: its beam rays, their cells, its samples, its record drawn, and its
        // pixels.
        Step::dispatch("beam_hit", IN_EYE, COMPUTE_BEAM_HIT, &[&BEAM_HIT_BINDS]),
        Step::copy(
            "beam_left's count",
            IN_EYE,
            at(BEAM_LEFT, ray::LEFT_CNT, 1),
            NO_RANGE,
        ),
        BEAM,
        Step::dispatch("cell", IN_EYE, COMPUTE_CELL, &[&WORLD_BINDS]),
        Step::dispatch(
            "sample_hit",
            IN_EYE,
            COMPUTE_SAMPLE_HIT,
            &[&SAMPLE_HIT_BINDS],
        ),
        Step::copy(
            "sample_left's count",
            IN_EYE,
            at(SAMPLE_LEFT, ray::LEFT_CNT, 1),
            NO_RANGE,
        ),
        SAMPLE,
        Step::dispatch(
            "sample_box",
            IN_EYE,
            COMPUTE_SAMPLE_BOX,
            &[&SAMPLE_BOX_BINDS],
        ),
        Step::dispatch(
            "sample_observe",
            IN_EYE,
            COMPUTE_SAMPLE_OBSERVE,
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
        Step::copy(
            "marks' count",
            IN_EYE,
            at(MARKS, ray::MISS_ARGS_CNT, 1),
            NO_RANGE,
        ),
        // The eye's holes walked, their stops into its record, and its owed counted.
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
        HITS[0],
        Step::dispatch(
            "queue_last",
            IN_EYE_HOLES,
            COMPUTE_QUEUE_LAST,
            &[&LAST_BINDS],
        ),
        STOPS,
        STOPS_DEFERRED,
        STOP_FACES,
        STOP_FACES_PUT,
        Step::copy(
            "the eye's owed",
            IN_EYE_HOLES,
            at(NODE_CACHE, node_cache::owed_at(count::EYE), 1),
            NO_RANGE,
        ),
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
        Step::copy(
            "light_stood kept",
            IN_LIGHT,
            at(LIGHT_LIST, light::kept_at(SIZE), light::STOOD_KEPT),
            at(LIGHT_STOOD, light::STOOD_LIST, light::STOOD_KEPT),
        ),
        Step::dispatch(
            "record_draw_for_light",
            IN_LIGHT,
            COMPUTE_RECORD_DRAW,
            &[&RECORD_BINDS],
        ),
        LIGHT_MARK_WALKED,
        Step::copy(
            "light_stood to keep",
            IN_LIGHT,
            at(LIGHT_STOOD, light::STOOD_LIST, light::STOOD_KEPT),
            at(LIGHT_LIST, light::kept_at(SIZE), light::STOOD_KEPT),
        ),
        Step::dispatch(
            "light_mark_draws_open",
            IN_LIGHT,
            COMPUTE_LIGHT_MARK_DRAWS_OPEN,
            &[&LIGHT_MARK_DRAWS_OPEN_BINDS],
        ),
        LIGHT_MARK_DRAWS,
        Step::copy(
            "light_stood's count",
            IN_LIGHT,
            at(LIGHT_STOOD, light::STOOD_CNT, 1),
            at(LIGHT_LIST, light::listed_at(SIZE, light::LISTED_STOOD), 1),
        ),
        LIGHT_SLIDE,
        LIGHT_PROPAGATE,
        LIGHT_CLAIM,
        Step::copy(
            "light_args' corners' count",
            IN_LIGHT,
            at(LIGHT_ARGS, light::ARGS_COUNTS + light::LISTED_CORNERS, 1),
            at(LIGHT_LIST, light::listed_at(SIZE, light::LISTED_CORNERS), 1),
        ),
        Step::copy(
            "light_args' fed count",
            IN_LIGHT,
            at(LIGHT_ARGS, light::ARGS_COUNTS + light::LISTED_FED, 1),
            at(LIGHT_LIST, light::listed_at(SIZE, light::LISTED_FED), 1),
        ),
        Step::copy(
            "light_args' claimed count",
            IN_LIGHT,
            at(LIGHT_ARGS, light::ARGS_COUNTS + light::LISTED_CLAIMED, 1),
            at(LIGHT_LIST, light::listed_at(SIZE, light::LISTED_CLAIMED), 1),
        ),
        Step::copy(
            "light_args' left",
            IN_LIGHT,
            at(LIGHT_ARGS, light::ARGS_LEFT, 1),
            at(NODE_CACHE, node_cache::owed_at(count::LIGHT), 1),
        ),
        LIGHT_BEAM_HIT,
        Step::copy(
            "light_beam_left's count",
            IN_LIGHT,
            at(LIGHT_BEAM_LEFT, ray::LEFT_CNT, 1),
            at(LIGHT_LIST, light::corners_left_at(SIZE), 1),
        ),
        LIGHT_BEAM,
        LIGHT_CELLS,
        LIGHT_SAMPLE_HIT,
        Step::copy(
            "light_sample_left's count",
            IN_LIGHT,
            at(LIGHT_SAMPLE_LEFT, ray::LEFT_CNT, 1),
            at(LIGHT_LIST, light::lefts_at(SIZE), 1),
        ),
        LIGHT_SAMPLE,
        LIGHT_SAMPLE_BOX,
        LIGHT_SAMPLE_OBSERVE,
        Step::dispatch(
            "light_record_draw",
            IN_LIGHT,
            COMPUTE_LIGHT_RECORD_DRAW,
            &[&LIGHT_RECORD_BINDS],
        ),
        LIGHT_RASTER,
        LIGHT_SLOTS,
        Step::copy(
            "light_misses' count",
            IN_LIGHT,
            at(LIGHT_MISSES, ray::MISS_ARGS_CNT, 1),
            at(LIGHT_LIST, light::misses_cnt(SIZE), 1),
        ),
        // The light's holes walked and their stops into its record.
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
        HITS[1],
        Step::dispatch(
            "light_queue_last",
            IN_LIGHT_HOLES,
            COMPUTE_QUEUE_LAST,
            &[&LAST_BINDS],
        ),
        LIGHT_STOPS,
        LIGHT_STOPS_DEFERRED,
        MAP_GATE,
        MAP_LAY,
    ],
};

/// The hazards the frame holds today, each by its rule, its step and what it holds it with.
pub const KNOWN: &[Known] = &[
    // Counts several passes of a stage add to: the eye's faces' asks, laid by both of the
    // samples' passes and by the holes' stops and those put off; the stops the holes' two walks
    // over the node cache list, copied into the node cache's head and raised by the last walk;
    // and the stood list the record's draw and both receivers' passes append to, after the
    // kept part copied back in.
    known(2, "sample_box", "sample_observe"),
    known(2, "stops", "stops_deferred"),
    known(2, "queue", "queue_parked"),
    known(2, "light_queue", "light_queue_parked"),
    known(2, "hits", "queue_last"),
    known(2, "light_hits", "light_queue_last"),
    known(2, "light_stood kept", "record_draw_for_light"),
    known(2, "record_draw_for_light", "light_mark_walked"),
    known(2, "light_mark_walked", "light_mark_draws"),
    // Words kept over frames in the light's args, which no call writes before a pass reads them
    // in a frame: each list's marks, a tile's last cast's number (the stood, the drawn, the
    // read, and the claimed, fed and cornered).
    known(3, "record_draw", "light_args"),
    known(3, "light_mark_draws", "light_args"),
    known(3, "light_propagate", "light_args"),
    known(3, "light_claim", "light_args"),
    // The stood list's args the world's receivers left, kept over casts: copied back in before
    // the copy that keeps them this cast, and kept before the drawn receivers add theirs.
    known(3, "light_stood kept", "light_stood to keep"),
    known(3, "light_stood to keep", "light_mark_draws"),
];

const fn known(rule: u32, step: &'static str, with: &'static str) -> Known {
    Known { rule, step, with }
}

const _: () = check(&FRAME, KNOWN);

#[cfg(test)]
#[path = "sequence.test.rs"]
mod test;
