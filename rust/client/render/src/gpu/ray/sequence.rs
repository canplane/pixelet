// pxl-render/gpu/ray/sequence.rs

//! The frame's calls over the buffers that hold indirect counts, in the order the host encodes
//! them (gpu/renderer.rs `frame`: the light's tiles' marks, then `RaysPass::dispatch`, then the
//! shadow map's pass): each call's pipeline, the groups set for it and the buffers they view,
//! and where it reads its count. Every indirect call in ray/cast.rs, ray/raster.rs,
//! gpu/generator.rs and gpu/shadow.rs takes its count's place from its step here, and names
//! them in this order (sequence.test.rs). The hazards over them are refused as the crate is
//! built (frame/shader/access.rs `check`), each the frame holds today named in `KNOWN`.

use super::cast::{
    CAST_GROUP, CAST_TILES_GROUP, CLAIM_BINDING, DRAWS_BINDING, LATTICE_ASK_GROUP,
    LATTICE_LAST_GROUP, LATTICE_WALK_GROUP, LIGHT_CLAIM_GROUP, LIGHT_GROUP,
    LIGHT_LATTICE_LAST_GROUP, LIGHT_LATTICE_WALK_GROUP, LIGHT_MARK_DRAWS_GROUP,
    LIGHT_MARK_DRAWS_OPEN_GROUP, LIGHT_MARK_GROUP, LIGHT_PROPAGATE_GROUP, LIGHT_SLIDE_GROUP,
    PARKED_TILES_GROUP, QUEUE_ASK_GROUP, QUEUE_GROUP, QUEUE_LAST_GROUP, ROLL_GROUP, WORLD_GROUP,
};
use super::raster::{
    FACE_GROUP, LIGHT_SAMPLE_BOX_GROUP, LIGHT_STOPS_GROUP, RECORD_GROUP, SAMPLE_BOX_GROUP,
    STOPS_GROUP,
};
use crate::frame::draw::{
    PIPELINE_DECOR, PIPELINE_GHOST_BOX, PIPELINE_GHOST_DECOR, PIPELINE_IMPOSTOR,
    PIPELINE_LOCATION_BEACON, PIPELINE_LOCATION_ZONE, PIPELINE_SKINNED,
};
use crate::frame::shader::access::{
    check, light_marks, Bind, Bound, Buffer, Frame, Group, Kept, Known, Range, Step, ARGS, END,
    LIST, NO_RANGE, STATE,
};
use crate::frame::shader::record::*;
use crate::gpu::generator::{ARGS_GROUP, GATE_GROUP, GENERATE_GROUP, MARK_GROUP, THIN_GROUP};
use crate::gpu::shadow::SIZE;
use crate::gpu::tiles::BOX_ARGS_GROUP;
use pxl_content::generation;
use pxl_engine::{count, light, node_cache, raster, ray};

/// The buffers, by their place in `BUFFERS`.
const MARKS: u32 = 0;
const NODE_CACHE: u32 = 1;
const QUEUES: u32 = 2;
const CAST_TILES: u32 = 3;
const RASTER_ARGS: u32 = 4;
const RASTER_STOPS: u32 = 5;
const LIGHT_ARGS: u32 = 6;
const LIGHT_LIST: u32 = 7;
const LIGHT_STOOD: u32 = 8;
const LIGHT_DRAWS: u32 = 9;
const LIGHT_CLAIMS: u32 = 10;
const LIGHT_MISSES: u32 = 11;
const LIGHT_RASTER_ARGS: u32 = 12;
const LIGHT_RASTER_STOPS: u32 = 13;
const LIGHT_BOX_ARGS: u32 = 14;
const GENERATION_LIST: u32 = 15;
const GENERATION_ARGS: u32 = 16;
const GATED_SCREEN_ARGS: u32 = 17;
const GATED_LIGHT_ARGS: u32 = 18;

/// Each buffer, as the host names its field, and its role.
const BUFFERS: &[Buffer] = &[
    buffer("marks", ARGS),
    buffer("node_cache", STATE),
    buffer("queues", STATE),
    buffer("cast_tiles", ARGS),
    buffer("raster.args", ARGS),
    buffer("raster.stops", ARGS),
    buffer("light_args", ARGS),
    buffer("light_list", LIST),
    buffer("light_stood", ARGS),
    buffer("light_draws_dispatch", ARGS),
    buffer("light_claim_dispatch", ARGS),
    buffer("light_misses", ARGS),
    buffer("raster.light.args", ARGS),
    buffer("raster.light.stops", ARGS),
    buffer("light_tiles.box_args", ARGS),
    buffer("generator.list", ARGS),
    buffer("generator.args", ARGS),
    buffer("generator.gated[Screen].args", ARGS),
    buffer("generator.gated[Light].args", ARGS),
];

const fn buffer(name: &'static str, role: u32) -> Buffer {
    Buffer { name, role }
}

const fn at(buf: u32, at: u32, words: u32) -> Range {
    Range { buf, at, words }
}

/// A run's queue's view, binding 10 from its head (pxl-engine `ray::queue_at`).
const fn queue_bind(queue: u32) -> Bind {
    Bind {
        binding: 10,
        buf: QUEUES,
        at: ray::queue_at(queue),
    }
}

/// Words `word` to `word + words` of queue `queue`'s head.
const fn head(queue: u32, word: u32, words: u32) -> Range {
    at(QUEUES, ray::queue_at(queue) + word, words)
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
const IN_EYE_CLOSE: u32 = 4;
const IN_GENERATE: u32 = 5;
const IN_LIGHT: u32 = 6;
const IN_LIGHT_HOLES: u32 = 7;
const IN_MAP: u32 = 8;
const IN_SCREEN_GATE: u32 = 9;
const IN_LIGHT_GATE: u32 = 10;
const IN_LIT: u32 = 11;
const STAGES: &[&str] = &[
    "the host's writes",
    "the light's tiles' marks",
    "the eye's cast",
    "the eye's holes",
    "the eye's close",
    "the decor generated",
    "the light's cast",
    "the light's holes",
    "the shadow map",
    "the decor through the screen's gate",
    "the decor through the light's gate",
    "the world's lit pass",
];

// The groups as the host makes them (ray/cast.rs `RaysPass::prepare`, `light_groups`,
// `queue_groups`; ray/raster.rs `Raster::prepare`, `record_group`, `light_record_group`;
// gpu/generator.rs `regroup`; gpu/tiles.rs `Tiles::new`): each binding to a buffer here.
const CORNERS_WALK_BINDS: Group = Group {
    layout: &LATTICE_WALK_GROUP,
    binds: &[bind(9, NODE_CACHE), queue_bind(ray::QUEUE_EYE_CORNERS)],
};
const CORNERS_ASK_BINDS: Group = Group {
    layout: &LATTICE_ASK_GROUP,
    binds: &[bind(9, NODE_CACHE), queue_bind(ray::QUEUE_EYE_CORNERS)],
};
const SAMPLES_WALK_BINDS: Group = Group {
    layout: &LATTICE_WALK_GROUP,
    binds: &[bind(9, NODE_CACHE), queue_bind(ray::QUEUE_EYE_SAMPLES)],
};
const SAMPLES_ASK_BINDS: Group = Group {
    layout: &LATTICE_ASK_GROUP,
    binds: &[bind(9, NODE_CACHE), queue_bind(ray::QUEUE_EYE_SAMPLES)],
};
const LATTICE_LAST_BINDS: Group = Group {
    layout: &LATTICE_LAST_GROUP,
    binds: &[bind(9, NODE_CACHE)],
};
const PARKED_TILES_BINDS: Group = Group {
    layout: &PARKED_TILES_GROUP,
    binds: &[bind(9, NODE_CACHE)],
};
const ROLL_BINDS: Group = Group {
    layout: &ROLL_GROUP,
    binds: &[bind(9, NODE_CACHE)],
};
const WORLD_BINDS: Group = Group {
    layout: &WORLD_GROUP,
    binds: &[bind(9, NODE_CACHE)],
};
const SAMPLE_BOX_BINDS: Group = Group {
    layout: &SAMPLE_BOX_GROUP,
    binds: &[bind(9, NODE_CACHE), bind(16, RASTER_ARGS)],
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
    binds: &[
        bind(9, NODE_CACHE),
        Bind {
            binding: 16,
            buf: RASTER_ARGS,
            at: raster::ASKED_AT,
        },
    ],
};
const STOP_FACES_OF_BINDS: Group = Group {
    layout: &FACE_GROUP,
    binds: &[
        bind(9, NODE_CACHE),
        Bind {
            binding: 16,
            buf: RASTER_ARGS,
            at: raster::STOP_ASKED_AT,
        },
    ],
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
    binds: &[bind(9, NODE_CACHE), queue_bind(ray::QUEUE_EYE_HOLES)],
};
const WALK_ASK_BINDS: Group = Group {
    layout: &QUEUE_ASK_GROUP,
    binds: &[bind(9, NODE_CACHE), queue_bind(ray::QUEUE_EYE_HOLES)],
};
const LAST_BINDS: Group = Group {
    layout: &QUEUE_LAST_GROUP,
    binds: &[bind(9, NODE_CACHE)],
};
const STOPS_OF_BINDS: Group = Group {
    layout: &STOPS_GROUP,
    binds: &[bind(9, NODE_CACHE), bind(16, RASTER_ARGS)],
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
const GATE_SCREEN_BINDS: Group = Group {
    layout: &GATE_GROUP,
    binds: &[bind(2, GENERATION_ARGS), bind(5, GATED_SCREEN_ARGS)],
};
const GATE_LIGHT_BINDS: Group = Group {
    layout: &GATE_GROUP,
    binds: &[bind(2, GENERATION_ARGS), bind(5, GATED_LIGHT_ARGS)],
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
const LIGHT_CORNERS_WALK_BINDS: Group = Group {
    layout: &LIGHT_LATTICE_WALK_GROUP,
    binds: &[bind(9, NODE_CACHE), queue_bind(ray::QUEUE_LIGHT_CORNERS)],
};
const LIGHT_CORNERS_ASK_BINDS: Group = Group {
    layout: &LATTICE_ASK_GROUP,
    binds: &[bind(9, NODE_CACHE), queue_bind(ray::QUEUE_LIGHT_CORNERS)],
};
const LIGHT_SAMPLES_WALK_BINDS: Group = Group {
    layout: &LIGHT_LATTICE_WALK_GROUP,
    binds: &[bind(9, NODE_CACHE), queue_bind(ray::QUEUE_LIGHT_SAMPLES)],
};
const LIGHT_SAMPLES_ASK_BINDS: Group = Group {
    layout: &LATTICE_ASK_GROUP,
    binds: &[bind(9, NODE_CACHE), queue_bind(ray::QUEUE_LIGHT_SAMPLES)],
};
const LIGHT_LATTICE_LAST_BINDS: Group = Group {
    layout: &LIGHT_LATTICE_LAST_GROUP,
    binds: &[bind(9, NODE_CACHE)],
};
const LIGHT_SAMPLE_BOX_BINDS: Group = Group {
    layout: &LIGHT_SAMPLE_BOX_GROUP,
    binds: &[bind(9, NODE_CACHE), bind(16, LIGHT_RASTER_ARGS)],
};
const LIGHT_SLOTS_BINDS: Group = Group {
    layout: &LIGHT_GROUP,
    binds: &[bind(9, NODE_CACHE), bind(17, LIGHT_MISSES)],
};
const LIGHT_WALK_BINDS: Group = Group {
    layout: &QUEUE_GROUP,
    binds: &[bind(9, NODE_CACHE), queue_bind(ray::QUEUE_LIGHT_HOLES)],
};
const LIGHT_WALK_ASK_BINDS: Group = Group {
    layout: &QUEUE_ASK_GROUP,
    binds: &[bind(9, NODE_CACHE), queue_bind(ray::QUEUE_LIGHT_HOLES)],
};
const LIGHT_STOPS_BINDS: Group = Group {
    layout: &LIGHT_STOPS_GROUP,
    binds: &[bind(9, NODE_CACHE), bind(16, LIGHT_RASTER_ARGS)],
};

// Each step a host's call names, in the order the frame encodes them.

/// The render pipelines each render pass with an indirect draw draws, whose groups are all the
/// pass sets: the eye's raster (ray/raster.rs `Raster::draw`) and the light's
/// (`Raster::light_draw`); the shadow map's pass (gpu/shadow.rs `Shadow::begin`, then the
/// decor's and the actors' depth, gpu/renderer.rs `frame`); and the world's lit pass, from its
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
pub const LIGHT_CLAIM: Step = Step::dispatch_off(
    "light_claim",
    IN_LIGHT,
    COMPUTE_LIGHT_CLAIM,
    &[&LIGHT_CLAIM_BINDS],
    (LIGHT_CLAIMS, 0),
);
pub const LIGHT_CELLS: Step = Step::dispatch_off(
    "light_cells",
    IN_LIGHT,
    COMPUTE_LIGHT_CELL,
    &[&WORLD_BINDS],
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
pub const OPEN_LIGHT_RINGS: Step = Step::write(
    "open_light_rings",
    at(
        LIGHT_ARGS,
        light::ARGS_WORDS + light::rings_at(SIZE),
        light::rings(SIZE),
    ),
);
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
    at(LIGHT_LIST, light::kept_at(SIZE) + light::STOOD_CNT, 1),
);
pub const OPEN_LIGHT_CLAIMS: Step = Step::write("open_light_claims", at(LIGHT_CLAIMS, 0, 3));
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
/// The light's beam rays' parked states set to none where its cast carries no parked ray from
/// the cast before (pxl-engine `light::HEAD_CARRIES`): a beam ray's state goes on only within a
/// run of casts each of which carries, its content, box and bands as they were (corners.wgsl
/// `taken`, beam.light.wgsl `beams_resume`).
pub const OPEN_LIGHT_CORNER_STATES: Step = Step::write(
    "open_light_corner_states",
    at(
        LIGHT_LIST,
        light::corner_states_at(SIZE),
        light::beam_across(SIZE) * light::beam_across(SIZE) * ray::STATE_WORDS,
    ),
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
    at(LIGHT_LIST, light::kept_at(SIZE), light::STOOD_KEPT),
    at(LIGHT_STOOD, light::STOOD_LIST, light::STOOD_KEPT),
);
/// The read list's count the casts since the world's receivers were stood afresh left, copied
/// back in where this cast adds to them, the count its propagation goes on from.
pub const READ_COUNT_BACK: Step = Step::copy(
    "read_count_back",
    IN_LIGHT,
    at(LIGHT_LIST, light::kept_at(SIZE) + light::KEPT_READ, 1),
    at(LIGHT_ARGS, light::ARGS_COUNTS + light::LISTED_READ, 1),
);
/// The eye's rays its holes' walks over the node cache parked, copied into its run's pool,
/// where the last walk and the runs after read it (ray/cast.rs `walk_run`, pxl-engine
/// `ray::POOL_CNT`).
pub const EYE_PARKED: Step = Step::copy(
    "eye_parked",
    IN_EYE_HOLES,
    head(ray::QUEUE_EYE_HOLES, ray::QUEUE_PARKED, 1),
    NO_RANGE,
);
/// Each observer's owed work copied out of the node cache's head into a word of its own where
/// its queue ran (ray/count.rs `Counter::owed`).
pub const EYE_OWED_KEEP: Step = Step::copy(
    "eye_owed_keep",
    IN_LIGHT_HOLES,
    at(NODE_CACHE, node_cache::owed_at(count::EYE), 1),
    NO_RANGE,
);
pub const LIGHT_OWED_KEEP: Step = Step::copy(
    "light_owed_keep",
    IN_LIGHT_HOLES,
    at(NODE_CACHE, node_cache::owed_at(count::LIGHT), 1),
    NO_RANGE,
);
pub const STOOD_KEEP: Step = Step::copy(
    "stood_keep",
    IN_LIGHT,
    at(LIGHT_STOOD, light::STOOD_LIST, light::STOOD_KEPT),
    at(LIGHT_LIST, light::kept_at(SIZE), light::STOOD_KEPT),
);
pub const READ_COUNT_KEEP: Step = Step::copy(
    "read_count_keep",
    IN_LIGHT,
    at(LIGHT_ARGS, light::ARGS_COUNTS + light::LISTED_READ, 1),
    at(LIGHT_LIST, light::kept_at(SIZE) + light::KEPT_READ, 1),
);
pub const STOOD_COUNT: Step = Step::copy(
    "stood_count",
    IN_LIGHT,
    at(LIGHT_STOOD, light::STOOD_CNT, 1),
    at(LIGHT_LIST, light::listed_at(SIZE, light::LISTED_STOOD), 1),
);
pub const CORNERS_COUNT: Step = Step::copy(
    "corners_count",
    IN_LIGHT,
    at(LIGHT_ARGS, light::ARGS_COUNTS + light::LISTED_CORNERS, 1),
    at(LIGHT_LIST, light::listed_at(SIZE, light::LISTED_CORNERS), 1),
);
pub const FED_COUNT: Step = Step::copy(
    "fed_count",
    IN_LIGHT,
    at(LIGHT_ARGS, light::ARGS_COUNTS + light::LISTED_FED, 1),
    at(LIGHT_LIST, light::listed_at(SIZE, light::LISTED_FED), 1),
);
pub const CLAIMED_COUNT: Step = Step::copy(
    "claimed_count",
    IN_LIGHT,
    at(LIGHT_ARGS, light::ARGS_COUNTS + light::LISTED_CLAIMED, 1),
    at(LIGHT_LIST, light::listed_at(SIZE, light::LISTED_CLAIMED), 1),
);
pub const LIGHT_LEFT: Step = Step::copy(
    "light_left",
    IN_LIGHT,
    at(LIGHT_ARGS, light::ARGS_LEFT, 1),
    at(NODE_CACHE, node_cache::owed_at(count::LIGHT), 1),
);
pub const LIGHT_MISSES_COUNT: Step = Step::copy(
    "light_misses_count",
    IN_LIGHT,
    at(LIGHT_MISSES, ray::MISS_ARGS_CNT, 1),
    at(LIGHT_LIST, light::misses_cnt(SIZE), 1),
);

/// The host's writes past the opens above: the raster's draw args set to none where the
/// record's draw runs (ray/raster.rs `record_draw`); each run's caps (ray/cast.rs
/// `RaysPass::prepare`); the node cache's counts and frame where nothing casts; the decor's
/// boxes (gpu/generator.rs `prepare`).
pub const OPEN_RASTER_DRAW_ARGS: Step = Step::write(
    "open_raster_draw_args",
    at(RASTER_ARGS, 0, raster::ARGS_WORDS),
);
pub const OPEN_EYE_LATTICE_CAPS: Step = Step::write(
    "open_eye_lattice_caps",
    at(NODE_CACHE, node_cache::caps_at(count::EYE_LATTICE), 2),
);
pub const OPEN_LIGHT_LATTICE_CAPS: Step = Step::write(
    "open_light_lattice_caps",
    at(NODE_CACHE, node_cache::caps_at(count::LIGHT_LATTICE), 2),
);
/// The eye's rolled lattice set to none where its grid is laid anew (ray/cast.rs
/// `RaysPass::prepare`, `node_cache::ROLL_K`).
pub const OPEN_ROLL: Step = Step::write(
    "open_roll",
    at(NODE_CACHE, node_cache::ROLL_K, node_cache::ROLL_WORDS),
);
pub const OPEN_EYE_HOLES_CAPS: Step = Step::write(
    "open_eye_holes_caps",
    at(NODE_CACHE, node_cache::caps_at(count::EYE_HOLES), 2),
);
pub const OPEN_LIGHT_HOLES_CAPS: Step = Step::write(
    "open_light_holes_caps",
    at(NODE_CACHE, node_cache::caps_at(count::LIGHT_HOLES), 2),
);
pub const OPEN_NODE_CACHE_COUNTS: Step = Step::write(
    "open_node_cache_counts",
    at(
        NODE_CACHE,
        node_cache::VISITED,
        node_cache::owed_at(count::EYE) + 1 - node_cache::VISITED,
    ),
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
        light::reads_at(SIZE),
        light::tiles(SIZE) * light::tiles(SIZE),
    ),
    NO_RANGE,
);
pub const LIGHT_READS_IN: Step = Step::copy(
    "light_reads_in",
    IN_LIGHT,
    NO_RANGE,
    at(
        LIGHT_LIST,
        light::reads_at(SIZE),
        light::tiles(SIZE) * light::tiles(SIZE),
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
        OPEN_LIGHT_RINGS,
        OPEN_LIGHT_STOOD,
        OPEN_LIGHT_STOOD_AFRESH,
        OPEN_STOOD_PROPAGATED,
        OPEN_LIGHT_CLAIMS,
        OPEN_LIGHT_MISSES,
        OPEN_LIGHT_QUEUE,
        OPEN_LIGHT_CORNERS_QUEUE,
        OPEN_LIGHT_CORNER_STATES,
        OPEN_LIGHT_SAMPLES_QUEUE,
        OPEN_LIGHT_RASTER_ARGS,
        OPEN_GENERATION_LIST,
        OPEN_RASTER_DRAW_ARGS,
        OPEN_EYE_LATTICE_CAPS,
        OPEN_EYE_HOLES_CAPS,
        OPEN_LIGHT_LATTICE_CAPS,
        OPEN_LIGHT_HOLES_CAPS,
        OPEN_ROLL,
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
        // The eye's holes walked, their stops into the record, and its owed counted.
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
        EYE_PARKED,
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
        // What the eye's holes leave owed, counted (parked_tiles.wgsl).
        Step::dispatch(
            "parked_tiles",
            IN_EYE_HOLES,
            COMPUTE_PARKED_TILES,
            &[&PARKED_TILES_BINDS],
        ),
        // The eye's close: its rolled lattice's offset counted or held where its samples were
        // cast, else its lattice owed alone (roll.wgsl).
        Step::dispatch("roll", IN_EYE_CLOSE, COMPUTE_ROLL, &[&ROLL_BINDS]).one_of(5),
        Step::dispatch("roll_owed", IN_EYE_CLOSE, COMPUTE_ROLL_OWED, &[&ROLL_BINDS]).one_of(5),
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
            "light_want",
            IN_LIGHT,
            COMPUTE_LIGHT_WANT,
            &[&LIGHT_PROPAGATE_BINDS],
        ),
        LIGHT_CLAIM,
        CORNERS_COUNT,
        FED_COUNT,
        CLAIMED_COUNT,
        LIGHT_LEFT,
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
        LIGHT_MISSES_COUNT,
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
        Step::dispatch(
            "light_queue_last",
            IN_LIGHT_HOLES,
            COMPUTE_QUEUE_LAST,
            &[&LAST_BINDS],
        ),
        LIGHT_STOPS_DISPATCH,
        LIGHT_STOPS,
        // Each observer's owed work kept apart where its queue ran, which the readback's copy
        // carries (ray/count.rs `Counter::copy`).
        EYE_OWED_KEEP,
        LIGHT_OWED_KEEP,
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
/// (light_propagate.wgsl `read_tile`); the eye's lattice left parked, its beam rays' last walk's
/// and its samples' (corners_run.eye.wgsl `corner_owed`, samples_run.eye.wgsl `sample_owed`).
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
    at(NODE_CACHE, node_cache::VISITED, 2),
    at(NODE_CACHE, node_cache::LATTICE_LEFT, 1),
    head(ray::QUEUE_EYE_HOLES, ray::QUEUE_VISITED, 1),
    head(ray::QUEUE_LIGHT_HOLES, ray::QUEUE_VISITED, 1),
    head(ray::QUEUE_EYE_SAMPLES, ray::QUEUE_VISITED, 1),
    head(ray::QUEUE_LIGHT_SAMPLES, ray::QUEUE_VISITED, 1),
    head(ray::QUEUE_EYE_CORNERS, ray::QUEUE_VISITED, 1),
    head(ray::QUEUE_LIGHT_CORNERS, ray::QUEUE_VISITED, 1),
];

/// The copies that open a count before the passes that add to it: the stops the holes' walks
/// over the node cache listed, copied into the node cache's head before the last walk adds its
/// own (ray/cast.rs `walk_holes`, queue_last.wgsl `keep_hit`), each observer's; the stood
/// list's args and the read list's count the casts since the world's receivers were stood
/// afresh left, copied back in before this cast's receivers and propagation add to them.
const SEEDS: &[&str] = &["hits", "light_hits", "stood_back", "read_count_back"];

/// The counts read as a bound while they are added to: the frame's work so far, which every
/// walk's budget reads before the asks after it fold its own visits in (queue_walk.wgsl
/// `spent`, queue_ask.wgsl `fold_visits`). The visits run past the cap a walk read by what its
/// lanes may visit before they read it again (pxl-engine `ray::OVERRUN`); the asks are counted
/// one at a time and run past none.
const BOUNDS: &[Bound] = &[
    Bound {
        range: at(NODE_CACHE, node_cache::VISITED, 1),
        overrun: ray::OVERRUN,
    },
    Bound {
        range: at(NODE_CACHE, node_cache::ASKED, 1),
        overrun: 0,
    },
];

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
        range: at(NODE_CACHE, node_cache::ROLL_K, node_cache::ROLL_WORDS),
        owner: "roll",
        by: &[],
        invalidation: "the eye's rolled lattice: the offset its samples are cast at, the offsets \
            counted, whether its parked samples go on and the casts since the grid was laid, \
            which its parked beam rays go on past the first of, stale where the grid is laid anew, \
            where the host sets it to none (ray/cast.rs `RaysPass::prepare`, `OPEN_ROLL`)",
    },
    Kept {
        range: at(
            LIGHT_LIST,
            light::reads_at(SIZE),
            light::tiles(SIZE) * light::tiles(SIZE),
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
        range: at(LIGHT_LIST, light::kept_at(SIZE), light::STOOD_KEPT),
        owner: "stood_keep",
        by: &[],
        invalidation: "the stood list's args the casts since the world's receivers were stood \
            afresh left, its count the tiles propagated, stale where they are stood afresh: the \
            host then writes the stood list empty and none of it propagated, and copies none \
            back in (ray/cast.rs `RaysPass::prepare`, `dispatch`)",
    },
    Kept {
        range: at(LIGHT_LIST, light::kept_at(SIZE) + light::KEPT_READ, 1),
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
