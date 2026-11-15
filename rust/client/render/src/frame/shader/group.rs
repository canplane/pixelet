// pxl-render/frame/shader/group.rs

//! The buffers the frame's calls share, and the groups the frame sets over them. Each group is
//! stated by the frame's buffers it binds, and is laid out as the one group of its dispatch's
//! compute pipeline (record.rs `compute`). The host makes the group from that statement
//! (gpu/api.rs `Gpu::frame_group`), and the frame's calls are checked against the same
//! statement (sequence.rs), so the two cannot disagree.

use crate::frame::shader::access::{ARGS, Bind, Buffer, END, Group, LIST, STATE};
use crate::frame::shader::record::LIGHT_DRAWS_BINDING;
use pxl_engine::{raster, ray};

/// The buffers, by their place in `BUFS`.
pub const MARKS: u32 = 0;
pub const NODE_CACHE: u32 = 1;
pub const QUEUES: u32 = 2;
pub const CAST_TILES: u32 = 3;
pub const RASTER_ARGS: u32 = 4;
pub const RASTER_HITS: u32 = 5;
pub const LIGHT_ARGS: u32 = 6;
pub const LIGHT_LIST: u32 = 7;
pub const LIGHT_RECEIVER_TILES: u32 = 8;
pub const LIGHT_DRAWS: u32 = 9;
pub const LIGHT_MISSES: u32 = 10;
pub const LIGHT_RASTER_ARGS: u32 = 11;
pub const LIGHT_RASTER_HITS: u32 = 12;
pub const LIGHT_RECT_ARGS: u32 = 13;
pub const GENERATION_LIST: u32 = 14;
pub const GENERATION_ARGS: u32 = 15;
pub const CULLED_SCREEN_ARGS: u32 = 16;
pub const CULLED_LIGHT_ARGS: u32 = 17;
pub const BEAMS: u32 = 18;

/// Each buffer by the name of the host's field that holds it, and its role.
pub const BUFS: &[Buffer] = &[
    buf("marks", ARGS),
    buf("node_cache", STATE),
    buf("queues", STATE),
    buf("cast_tiles", ARGS),
    buf("raster.args", ARGS),
    buf("raster.hits", ARGS),
    buf("light_args", ARGS),
    buf("light_list", LIST),
    buf("light_receiver_tiles", ARGS),
    buf("light_draws_dispatch", ARGS),
    buf("light_misses", ARGS),
    buf("raster.light.args", ARGS),
    buf("raster.light.hits", ARGS),
    buf("light_tiles.rect_args", ARGS),
    buf("generator.list", ARGS),
    buf("generator.args", ARGS),
    buf("generator.culled[Screen].args", ARGS),
    buf("generator.culled[Light].args", ARGS),
    buf("beams", LIST),
];

pub const fn buf(name: &'static str, role: u32) -> Buffer {
    Buffer { name, role }
}

/// The view of a run's queue at binding 10, from the queue's head to the end of the queues
/// (pxl-engine `ray::queue_at`).
const fn queue_bind(queue: u32) -> Bind {
    Bind {
        binding: 10,
        buf: QUEUES,
        at: ray::queue_at(queue),
        words: ray::QUEUE_WORDS - ray::queue_at(queue),
    }
}

/// The view at binding 16 of the faces' asks at word `at` of the raster's args, which the
/// faces' pass reads.
const fn args_bind(at: u32) -> Bind {
    Bind {
        binding: 16,
        buf: RASTER_ARGS,
        at,
        words: raster::ARGS_WORDS,
    }
}

pub const fn bind(binding: u32, buf: u32) -> Bind {
    Bind {
        binding,
        buf,
        at: 0,
        words: END,
    }
}

pub const CORNERS_WALK_BINDS: Group = Group {
    binds: &[
        bind(8, BEAMS),
        bind(9, NODE_CACHE),
        queue_bind(ray::QUEUE_EYE_CORNERS),
    ],
};
pub const CORNERS_ASK_BINDS: Group = Group {
    binds: &[bind(9, NODE_CACHE), queue_bind(ray::QUEUE_EYE_CORNERS)],
};
pub const SAMPLES_WALK_BINDS: Group = Group {
    binds: &[
        bind(8, BEAMS),
        bind(9, NODE_CACHE),
        queue_bind(ray::QUEUE_EYE_SAMPLES),
    ],
};
pub const SAMPLES_ASK_BINDS: Group = Group {
    binds: &[bind(9, NODE_CACHE), queue_bind(ray::QUEUE_EYE_SAMPLES)],
};
pub const LATTICE_LAST_BINDS: Group = Group {
    binds: &[bind(8, BEAMS), bind(9, NODE_CACHE)],
};
pub const WORLD_BINDS: Group = Group {
    binds: &[bind(8, BEAMS), bind(9, NODE_CACHE)],
};
pub const SAMPLE_HIT_BINDS: Group = Group {
    binds: &[bind(8, BEAMS), bind(9, NODE_CACHE), bind(16, RASTER_ARGS)],
};
pub const REC_BINDS: Group = Group {
    binds: &[
        bind(12, LIGHT_LIST),
        bind(9, NODE_CACHE),
        bind(16, RASTER_ARGS),
        bind(21, LIGHT_ARGS),
        bind(22, LIGHT_RECEIVER_TILES),
    ],
};
pub const FACES_BINDS: Group = Group {
    binds: &[
        bind(8, BEAMS),
        bind(9, NODE_CACHE),
        args_bind(raster::ASKED_AT),
    ],
};
pub const HIT_FACES_OF_BINDS: Group = Group {
    binds: &[
        bind(8, BEAMS),
        bind(9, NODE_CACHE),
        args_bind(raster::HIT_ASKED_AT),
    ],
};
pub const CAST_TILES_OF_BINDS: Group = Group {
    binds: &[bind(0, CAST_TILES)],
};
pub const PIXELS_BINDS: Group = Group {
    binds: &[
        bind(8, BEAMS),
        bind(9, NODE_CACHE),
        bind(14, MARKS),
        bind(30, CAST_TILES),
    ],
};
pub const LIGHT_WORLD_BINDS: Group = Group {
    binds: &[bind(8, LIGHT_LIST)],
};
pub const WALK_BINDS: Group = Group {
    binds: &[
        bind(8, BEAMS),
        bind(9, NODE_CACHE),
        queue_bind(ray::QUEUE_EYE_HOLES),
    ],
};
pub const WALK_ASK_BINDS: Group = Group {
    binds: &[bind(9, NODE_CACHE), queue_bind(ray::QUEUE_EYE_HOLES)],
};
pub const LAST_BINDS: Group = Group {
    binds: &[bind(8, BEAMS), bind(9, NODE_CACHE)],
};
pub const REC_HITS_OF_BINDS: Group = Group {
    binds: &[bind(8, BEAMS), bind(9, NODE_CACHE), bind(16, RASTER_ARGS)],
};
pub const GENERATION_MARK_BINDS: Group = Group {
    binds: &[bind(11, GENERATION_LIST)],
};
pub const GENERATION_THIN_BINDS: Group = Group {
    binds: &[bind(11, GENERATION_LIST)],
};
pub const GENERATION_BINDS: Group = Group {
    binds: &[bind(11, GENERATION_LIST)],
};
pub const GENERATION_ARGS_OF_BINDS: Group = Group {
    binds: &[bind(2, GENERATION_ARGS)],
};
pub const CULL_SCREEN_BINDS: Group = Group {
    binds: &[bind(2, GENERATION_ARGS), bind(5, CULLED_SCREEN_ARGS)],
};
pub const CULL_LIGHT_BINDS: Group = Group {
    binds: &[bind(2, GENERATION_ARGS), bind(5, CULLED_LIGHT_ARGS)],
};
pub const LIGHT_RECT_BINDS: Group = Group {
    binds: &[bind(1, LIGHT_RECT_ARGS)],
};
pub const LIGHT_REC_BINDS: Group = Group {
    binds: &[
        bind(12, LIGHT_LIST),
        bind(9, NODE_CACHE),
        bind(16, LIGHT_RASTER_ARGS),
        bind(21, LIGHT_ARGS),
        bind(22, LIGHT_RECEIVER_TILES),
    ],
};
pub const LIGHT_MARK_BINDS: Group = Group {
    binds: &[
        bind(12, LIGHT_LIST),
        bind(21, LIGHT_ARGS),
        bind(22, LIGHT_RECEIVER_TILES),
        bind(23, BEAMS),
    ],
};
pub const LIGHT_MARK_DRAWS_OPEN_BINDS: Group = Group {
    binds: &[
        bind(12, LIGHT_LIST),
        bind(21, LIGHT_ARGS),
        bind(22, LIGHT_RECEIVER_TILES),
        bind(LIGHT_DRAWS_BINDING, LIGHT_DRAWS),
    ],
};
pub const LIGHT_MARK_DRAWS_BINDS: Group = Group {
    binds: &[
        bind(12, LIGHT_LIST),
        bind(21, LIGHT_ARGS),
        bind(22, LIGHT_RECEIVER_TILES),
    ],
};
pub const LIGHT_SLIDE_BINDS: Group = Group {
    binds: &[bind(12, LIGHT_LIST)],
};
pub const LIGHT_PROPAGATE_BINDS: Group = Group {
    binds: &[bind(12, LIGHT_LIST), bind(21, LIGHT_ARGS)],
};
pub const LIGHT_CORNERS_WALK_BINDS: Group = Group {
    binds: &[
        bind(8, LIGHT_LIST),
        bind(9, NODE_CACHE),
        queue_bind(ray::QUEUE_LIGHT_CORNERS),
    ],
};
pub const LIGHT_CORNERS_ASK_BINDS: Group = Group {
    binds: &[bind(9, NODE_CACHE), queue_bind(ray::QUEUE_LIGHT_CORNERS)],
};
pub const LIGHT_SAMPLES_WALK_BINDS: Group = Group {
    binds: &[
        bind(8, LIGHT_LIST),
        bind(9, NODE_CACHE),
        queue_bind(ray::QUEUE_LIGHT_SAMPLES),
    ],
};
pub const LIGHT_SAMPLES_ASK_BINDS: Group = Group {
    binds: &[bind(9, NODE_CACHE), queue_bind(ray::QUEUE_LIGHT_SAMPLES)],
};
pub const LIGHT_LATTICE_LAST_BINDS: Group = Group {
    binds: &[bind(8, LIGHT_LIST), bind(9, NODE_CACHE)],
};
pub const LIGHT_SAMPLE_HIT_BINDS: Group = Group {
    binds: &[
        bind(8, LIGHT_LIST),
        bind(9, NODE_CACHE),
        bind(16, LIGHT_RASTER_ARGS),
    ],
};
pub const LIGHT_SLOTS_BINDS: Group = Group {
    binds: &[
        bind(8, LIGHT_LIST),
        bind(9, NODE_CACHE),
        bind(17, LIGHT_MISSES),
    ],
};
pub const LIGHT_WALK_BINDS: Group = Group {
    binds: &[
        bind(8, LIGHT_LIST),
        bind(9, NODE_CACHE),
        queue_bind(ray::QUEUE_LIGHT_HOLES),
    ],
};
pub const LIGHT_WALK_ASK_BINDS: Group = Group {
    binds: &[bind(9, NODE_CACHE), queue_bind(ray::QUEUE_LIGHT_HOLES)],
};
pub const LIGHT_LAST_BINDS: Group = Group {
    binds: &[bind(8, LIGHT_LIST), bind(9, NODE_CACHE)],
};
pub const LIGHT_REC_HITS_BINDS: Group = Group {
    binds: &[
        bind(8, LIGHT_LIST),
        bind(9, NODE_CACHE),
        bind(16, LIGHT_RASTER_ARGS),
    ],
};
