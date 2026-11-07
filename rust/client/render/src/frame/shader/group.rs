// pxl-render/frame/shader/group.rs

//! Each compute pipeline's group 0 as its layout's (binding, stages, kind) words: what the
//! frame's calls bind (sequence.rs) and the host makes its pipelines over (gpu/ray/cast.rs,
//! gpu/ray/raster.rs, gpu/generator.rs, gpu/tiles.rs), each held to what its shaders read and
//! to WebGPU's floor as the crate is built.

use crate::frame::shader::floor;
use crate::frame::shader::record::*;

/// The binding of a pass's head (pxl-engine `light::HEAD_SIZE`): the light's
/// map's, or the eye's, which names none.
pub const HEAD_BINDING: u32 = 11;

/// The lattice's runs' walks over the node cache alone (queue_walk.wgsl over samples.wgsl or
/// corners.wgsl): the eye, the beam's words, the node cache and the queue's words.
pub const LATTICE_WALK_GROUP: [u32; 12] = [
    0, COMPUTE, UNIFORM, 8, COMPUTE, STORAGE_RW, 9, COMPUTE, STORAGE, 10, COMPUTE, STORAGE_RW,
];

/// `LATTICE_WALK_GROUP` with the light's head, for the light's map.
pub const LIGHT_LATTICE_WALK_GROUP: [u32; 15] = [
    0,
    COMPUTE,
    UNIFORM,
    8,
    COMPUTE,
    STORAGE_RW,
    9,
    COMPUTE,
    STORAGE,
    10,
    COMPUTE,
    STORAGE_RW,
    HEAD_BINDING,
    COMPUTE,
    UNIFORM,
];

/// The light's slots' (`light_cs`): the list, the node cache it reads the visible voxel's
/// entry off, the map it writes, the head, the visibility it reads and the missed slots'
/// dispatch.
pub const LIGHT_GROUP: [u32; 21] = [
    0,
    COMPUTE,
    UNIFORM,
    8,
    COMPUTE,
    STORAGE_RW,
    9,
    COMPUTE,
    STORAGE,
    10,
    COMPUTE,
    STORAGE_TEXTURE_RW,
    HEAD_BINDING,
    COMPUTE,
    UNIFORM,
    15,
    COMPUTE,
    TEXTURE_UINT_ARRAY,
    17,
    COMPUTE,
    STORAGE_RW,
];

const _: () = assert!(
    floor::holds(&[&LIGHT_GROUP]),
    "the light's slots' group binds past WebGPU's floor"
);

/// `cast_cs`'s (cast.wgsl): the eye, the rays, the beam's words, the node cache it lists the
/// marked pixels in and reads the visible voxel's entry off, the visibility buffer, the walk
/// again's words and the claimed tiles' list (`px_group`).
pub const CAST_GROUP: [u32; 21] = [
    0,
    COMPUTE,
    UNIFORM,
    3,
    COMPUTE,
    STORAGE_TEXTURE_RW,
    8,
    COMPUTE,
    STORAGE_RW,
    9,
    COMPUTE,
    STORAGE_RW,
    13,
    COMPUTE,
    TEXTURE_UINT_ARRAY,
    14,
    COMPUTE,
    STORAGE_RW,
    30,
    COMPUTE,
    STORAGE,
];

/// `cast_tiles_cs`'s (cast_tiles.wgsl): the list, the tiles' words and uniform.
pub const CAST_TILES_GROUP: [u32; 9] = [
    0, COMPUTE, STORAGE_RW, 30, COMPUTE, STORAGE, 31, COMPUTE, UNIFORM,
];

/// The light's receivers' passes' groups: the eye, the head and the list; the stood list's
/// args and the light's args, whose marks they swap (light_stood.wgsl); and the rays the eye's
/// pixels met the world by and its beam rays' words, its walked pixels listed past the grid's
/// (light_mark.wgsl), or the decor placed
/// (light_mark_draws.wgsl); or the passes' args alone (light_propagate.wgsl).
pub const LIGHT_MARK_GROUP: [u32; 21] = [
    0,
    COMPUTE,
    UNIFORM,
    3,
    COMPUTE,
    STORAGE_TEXTURE_RW,
    HEAD_BINDING,
    COMPUTE,
    UNIFORM,
    12,
    COMPUTE,
    STORAGE_RW,
    21,
    COMPUTE,
    STORAGE_RW,
    22,
    COMPUTE,
    STORAGE_RW,
    23,
    COMPUTE,
    STORAGE,
];

pub const LIGHT_MARK_DRAWS_GROUP: [u32; 24] = [
    0,
    COMPUTE,
    UNIFORM,
    HEAD_BINDING,
    COMPUTE,
    UNIFORM,
    12,
    COMPUTE,
    STORAGE_RW,
    18,
    COMPUTE,
    STORAGE,
    19,
    COMPUTE,
    STORAGE,
    20,
    COMPUTE,
    STORAGE,
    21,
    COMPUTE,
    STORAGE_RW,
    22,
    COMPUTE,
    STORAGE_RW,
];

/// The drawn receivers' dispatch opened (light_mark_draws_open.wgsl): their pass's group and
/// the dispatch it reads its count from.
pub const LIGHT_MARK_DRAWS_OPEN_GROUP: [u32; 27] = [
    0,
    COMPUTE,
    UNIFORM,
    HEAD_BINDING,
    COMPUTE,
    UNIFORM,
    12,
    COMPUTE,
    STORAGE_RW,
    18,
    COMPUTE,
    STORAGE,
    19,
    COMPUTE,
    STORAGE,
    20,
    COMPUTE,
    STORAGE,
    21,
    COMPUTE,
    STORAGE_RW,
    22,
    COMPUTE,
    STORAGE_RW,
    DRAWS_BINDING,
    COMPUTE,
    STORAGE_RW,
];

pub const LIGHT_PROPAGATE_GROUP: [u32; 12] = [
    0,
    COMPUTE,
    UNIFORM,
    HEAD_BINDING,
    COMPUTE,
    UNIFORM,
    12,
    COMPUTE,
    STORAGE_RW,
    21,
    COMPUTE,
    STORAGE_RW,
];

/// The binding of the drawn receivers' dispatch (light_mark_draws_open.wgsl), written by the pass
/// before the one that reads its count from it.
pub const DRAWS_BINDING: u32 = 24;

/// `light_slide_cs`'s (light_slide.wgsl): the eye, the head and the list.
pub const LIGHT_SLIDE_GROUP: [u32; 9] = [
    0,
    COMPUTE,
    UNIFORM,
    HEAD_BINDING,
    COMPUTE,
    UNIFORM,
    12,
    COMPUTE,
    STORAGE_RW,
];

const _: () = assert!(
    floor::holds(&[&LIGHT_MARK_GROUP])
        && floor::holds(&[&LIGHT_MARK_DRAWS_GROUP])
        && floor::holds(&[&LIGHT_PROPAGATE_GROUP])
        && floor::holds(&[&LIGHT_SLIDE_GROUP]),
    "a light receivers' pass's group binds past WebGPU's floor"
);

/// The cast's world's group, over which `cell_cs`, `light_cell_cs` and `forget_cs` run:
/// the eye, the beam rays' words the cells' starts go into, the node cache, and the head
/// the passes both observers take read their observer by. The shadow map's passes of those
/// bind it over the light's buffers (`light_groups`).
pub const WORLD_GROUP: [u32; 12] = [
    0,
    COMPUTE,
    UNIFORM,
    8,
    COMPUTE,
    STORAGE_RW,
    9,
    COMPUTE,
    STORAGE_RW,
    HEAD_BINDING,
    COMPUTE,
    UNIFORM,
];

/// The holes' queue's walks over the node cache alone (queue_walk.wgsl), either observer's:
/// the eye, the rays or the map they write, the observer's list, the node cache they read,
/// the queue's words and the observer's head.
pub const QUEUE_GROUP: [u32; 18] = [
    0,
    COMPUTE,
    UNIFORM,
    3,
    COMPUTE,
    STORAGE_TEXTURE_RW,
    8,
    COMPUTE,
    STORAGE_RW,
    9,
    COMPUTE,
    STORAGE,
    10,
    COMPUTE,
    STORAGE_RW,
    HEAD_BINDING,
    COMPUTE,
    UNIFORM,
];

/// Its asks (queue_ask.wgsl): the eye, the world's buffers, the node cache they write, the
/// queue's words and the observer's head.
pub const QUEUE_ASK_GROUP: [u32; 30] = [
    0,
    COMPUTE,
    UNIFORM,
    1,
    COMPUTE,
    STORAGE,
    2,
    COMPUTE,
    STORAGE,
    4,
    COMPUTE,
    STORAGE,
    5,
    COMPUTE,
    STORAGE,
    6,
    COMPUTE,
    STORAGE,
    7,
    COMPUTE,
    STORAGE,
    9,
    COMPUTE,
    STORAGE_RW,
    10,
    COMPUTE,
    STORAGE_RW,
    HEAD_BINDING,
    COMPUTE,
    UNIFORM,
];

/// Its last walk (queue_last.wgsl): the eye, the world's buffers, the rays or the map, the
/// observer's list, the node cache it writes and the observer's head.
pub const QUEUE_LAST_GROUP: [u32; 33] = [
    0,
    COMPUTE,
    UNIFORM,
    1,
    COMPUTE,
    STORAGE,
    2,
    COMPUTE,
    STORAGE,
    3,
    COMPUTE,
    STORAGE_TEXTURE_RW,
    4,
    COMPUTE,
    STORAGE,
    5,
    COMPUTE,
    STORAGE,
    6,
    COMPUTE,
    STORAGE,
    7,
    COMPUTE,
    STORAGE,
    8,
    COMPUTE,
    STORAGE_RW,
    9,
    COMPUTE,
    STORAGE_RW,
    HEAD_BINDING,
    COMPUTE,
    UNIFORM,
];

/// The lattice's asks (queue_ask.wgsl over samples_run.eye.wgsl or samples_run.light.wgsl):
/// `QUEUE_ASK_GROUP` but the head, which no lattice's run reads there.
pub const LATTICE_ASK_GROUP: [u32; 27] = [
    0, COMPUTE, UNIFORM, 1, COMPUTE, STORAGE, 2, COMPUTE, STORAGE, 4, COMPUTE, STORAGE, 5, COMPUTE,
    STORAGE, 6, COMPUTE, STORAGE, 7, COMPUTE, STORAGE, 9, COMPUTE, STORAGE_RW, 10, COMPUTE,
    STORAGE_RW,
];

/// The eye's lattice's last walk (queue_last.wgsl over samples.wgsl): the eye, the world's
/// buffers, the beam's words and the node cache it writes.
pub const LATTICE_LAST_GROUP: [u32; 27] = [
    0, COMPUTE, UNIFORM, 1, COMPUTE, STORAGE, 2, COMPUTE, STORAGE, 4, COMPUTE, STORAGE, 5, COMPUTE,
    STORAGE, 6, COMPUTE, STORAGE, 7, COMPUTE, STORAGE, 8, COMPUTE, STORAGE_RW, 9, COMPUTE,
    STORAGE_RW,
];

/// `LATTICE_LAST_GROUP` with the light's head, for the light's map.
pub const LIGHT_LATTICE_LAST_GROUP: [u32; 30] = [
    0,
    COMPUTE,
    UNIFORM,
    1,
    COMPUTE,
    STORAGE,
    2,
    COMPUTE,
    STORAGE,
    4,
    COMPUTE,
    STORAGE,
    5,
    COMPUTE,
    STORAGE,
    6,
    COMPUTE,
    STORAGE,
    7,
    COMPUTE,
    STORAGE,
    8,
    COMPUTE,
    STORAGE_RW,
    9,
    COMPUTE,
    STORAGE_RW,
    HEAD_BINDING,
    COMPUTE,
    UNIFORM,
];

const _: () = assert!(
    floor::holds(&[&WORLD_GROUP])
        && floor::holds(&[&QUEUE_GROUP])
        && floor::holds(&[&LATTICE_ASK_GROUP])
        && floor::holds(&[&LIGHT_LATTICE_LAST_GROUP])
        && floor::holds(&[&QUEUE_ASK_GROUP])
        && floor::holds(&[&QUEUE_LAST_GROUP]),
    "the cast's world's group or the holes' queue's binds past WebGPU's floor"
);

// Each compute group against what the shaders built over it read, as their declarations take
// it (build.rs `BINDS`).
const _: () = assert!(
    binds(&[COMPUTE_CAST], &CAST_GROUP)
        && binds(&[COMPUTE_CAST_TILES], &CAST_TILES_GROUP)
        && binds(
            &[COMPUTE_CELL, COMPUTE_LIGHT_CELL, COMPUTE_FORGET],
            &WORLD_GROUP
        )
        && binds(
            &[
                COMPUTE_QUEUE_SAMPLES,
                COMPUTE_QUEUE_SAMPLES_PARKED,
                COMPUTE_QUEUE_CORNERS,
                COMPUTE_QUEUE_CORNERS_PARKED
            ],
            &LATTICE_WALK_GROUP
        )
        && binds(
            &[
                COMPUTE_QUEUE_LIGHT_SAMPLES,
                COMPUTE_QUEUE_LIGHT_SAMPLES_PARKED,
                COMPUTE_QUEUE_LIGHT_CORNERS,
                COMPUTE_QUEUE_LIGHT_CORNERS_PARKED
            ],
            &LIGHT_LATTICE_WALK_GROUP
        )
        && binds(
            &[
                COMPUTE_QUEUE_ASK_SAMPLES,
                COMPUTE_QUEUE_ASK_SAMPLES_PARKED,
                COMPUTE_QUEUE_ASK_LIGHT_SAMPLES,
                COMPUTE_QUEUE_ASK_LIGHT_SAMPLES_PARKED,
                COMPUTE_QUEUE_ASK_CORNERS,
                COMPUTE_QUEUE_ASK_CORNERS_PARKED,
                COMPUTE_QUEUE_ASK_LIGHT_CORNERS,
                COMPUTE_QUEUE_ASK_LIGHT_CORNERS_PARKED
            ],
            &LATTICE_ASK_GROUP
        )
        && binds(
            &[COMPUTE_QUEUE_LAST_SAMPLES, COMPUTE_QUEUE_LAST_CORNERS],
            &LATTICE_LAST_GROUP
        )
        && binds(
            &[
                COMPUTE_QUEUE_LAST_LIGHT_SAMPLES,
                COMPUTE_QUEUE_LAST_LIGHT_CORNERS
            ],
            &LIGHT_LATTICE_LAST_GROUP
        )
        && binds(&[COMPUTE_QUEUE, COMPUTE_QUEUE_PARKED], &QUEUE_GROUP)
        && binds(
            &[COMPUTE_QUEUE_ASK, COMPUTE_QUEUE_ASK_PARKED],
            &QUEUE_ASK_GROUP
        )
        && binds(&[COMPUTE_QUEUE_LAST], &QUEUE_LAST_GROUP)
        && binds(&[COMPUTE_LIGHT], &LIGHT_GROUP)
        && binds(&[COMPUTE_LIGHT_MARK_WALKED], &LIGHT_MARK_GROUP)
        && binds(&[COMPUTE_LIGHT_MARK_DRAWS], &LIGHT_MARK_DRAWS_GROUP)
        && binds(
            &[COMPUTE_LIGHT_MARK_DRAWS_OPEN],
            &LIGHT_MARK_DRAWS_OPEN_GROUP
        )
        && binds(
            &[COMPUTE_LIGHT_PROPAGATE, COMPUTE_LIGHT_CLAIM],
            &LIGHT_PROPAGATE_GROUP
        )
        && binds(&[COMPUTE_LIGHT_SLIDE], &LIGHT_SLIDE_GROUP),
    "a cast's compute group lays out other than its shaders read (record.rs `binds`)"
);

/// An observer's samples' stops (sample_box.wgsl): the eye, the samples' words, the node
/// cache whose drawable entries they set, and the faces' asks' args.
pub const SAMPLE_BOX_GROUP: [u32; 12] = [
    0, COMPUTE, UNIFORM, 8, COMPUTE, STORAGE_RW, 9, COMPUTE, STORAGE_RW, 16, COMPUTE, STORAGE_RW,
];

/// The node cache over every slot, its drawable entries drawn (record_draw.wgsl): the eye,
/// the node cache, the boxes, the raster's args, and the light's buffers its receivers stand
/// the eye's boxes by.
pub const RECORD_GROUP: [u32; 24] = [
    0,
    COMPUTE,
    UNIFORM,
    9,
    COMPUTE,
    STORAGE_RW,
    14,
    COMPUTE,
    STORAGE_RW,
    16,
    COMPUTE,
    STORAGE_RW,
    HEAD_BINDING,
    COMPUTE,
    UNIFORM,
    12,
    COMPUTE,
    STORAGE_RW,
    21,
    COMPUTE,
    STORAGE_RW,
    22,
    COMPUTE,
    STORAGE_RW,
];

/// The holes' stops (record_stops.wgsl): the eye, the rays, the stops among the beam rays'
/// words, the node cache their count lies in and whose drawable entries they set, and the
/// faces' asks' args; not the stops' dispatch, which is copied out of the node cache so that
/// no pass reads its count from a buffer it binds writable (`Raster::stops`).
pub const STOPS_GROUP: [u32; 15] = [
    0,
    COMPUTE,
    UNIFORM,
    3,
    COMPUTE,
    STORAGE_TEXTURE_RW,
    8,
    COMPUTE,
    STORAGE_RW,
    9,
    COMPUTE,
    STORAGE_RW,
    16,
    COMPUTE,
    STORAGE_RW,
];

/// The light's holes' stops (record_stops.wgsl over record_stops.light.wgsl): `STOPS_GROUP`,
/// its map at 3 and its list at 8, and the light's head its slots' rays are aimed by.
pub const LIGHT_STOPS_GROUP: [u32; 18] = [
    0,
    COMPUTE,
    UNIFORM,
    3,
    COMPUTE,
    STORAGE_TEXTURE_RW,
    8,
    COMPUTE,
    STORAGE_RW,
    9,
    COMPUTE,
    STORAGE_RW,
    HEAD_BINDING,
    COMPUTE,
    UNIFORM,
    16,
    COMPUTE,
    STORAGE_RW,
];

/// The light's samples' stops (sample_box.wgsl over sample.light.wgsl): the eye, the samples'
/// words, the node cache, the light's head and its raster's args.
pub const LIGHT_SAMPLE_BOX_GROUP: [u32; 15] = [
    0,
    COMPUTE,
    UNIFORM,
    8,
    COMPUTE,
    STORAGE_RW,
    9,
    COMPUTE,
    STORAGE_RW,
    HEAD_BINDING,
    COMPUTE,
    UNIFORM,
    16,
    COMPUTE,
    STORAGE_RW,
];

const _: () = assert!(
    floor::holds(&[&SAMPLE_BOX_GROUP])
        && floor::holds(&[&LIGHT_SAMPLE_BOX_GROUP])
        && floor::holds(&[&RECORD_GROUP])
        && floor::holds(&[&STOPS_GROUP])
        && floor::holds(&[&LIGHT_STOPS_GROUP]),
    "the boxes' writing's group binds past WebGPU's floor"
);

/// `face_cs`'s (face.wgsl): the eye, the world's query's buffers but the palette, which it
/// reads none of, the beam's words it asks off and leaves a hit's faces in, the node cache it
/// leaves a drawable entry's faces in, and the args it is bound to.
pub const FACE_GROUP: [u32; 27] = [
    0, COMPUTE, UNIFORM, 1, COMPUTE, STORAGE, 2, COMPUTE, STORAGE, 5, COMPUTE, STORAGE, 6, COMPUTE,
    STORAGE, 7, COMPUTE, STORAGE, 8, COMPUTE, STORAGE_RW, 9, COMPUTE, STORAGE_RW, 16, COMPUTE,
    STORAGE,
];

// Each compute group here against what its shader reads, as its declarations take it
// (build.rs `BINDS`).
const _: () = assert!(
    binds(&[COMPUTE_SAMPLE_BOX], &SAMPLE_BOX_GROUP)
        && binds(&[COMPUTE_LIGHT_SAMPLE_BOX], &LIGHT_SAMPLE_BOX_GROUP)
        && binds(&[COMPUTE_FACE], &FACE_GROUP)
        && binds(
            &[COMPUTE_RECORD_DRAW, COMPUTE_LIGHT_RECORD_DRAW],
            &RECORD_GROUP
        )
        && binds(&[COMPUTE_RECORD_STOPS, COMPUTE_RECORD_FACES], &STOPS_GROUP)
        && binds(&[COMPUTE_LIGHT_RECORD_STOPS], &LIGHT_STOPS_GROUP),
    "a compute group lays out other than its shader reads (record.rs `binds`)"
);

const _: () = assert!(
    floor::holds(&[&FACE_GROUP]),
    "the faces' group binds past WebGPU's floor"
);

/// `decor_cs`'s group (decor_pose.wgsl): the world's, at world.wgsl's numbers, its eye, the
/// records, the rows and the list.
pub const GENERATE_GROUP: [u32; 27] = [
    0, COMPUTE, UNIFORM, 1, COMPUTE, STORAGE, 2, COMPUTE, STORAGE, 5, COMPUTE, STORAGE, 6, COMPUTE,
    STORAGE, 7, COMPUTE, STORAGE, 9, COMPUTE, STORAGE_RW, 10, COMPUTE, STORAGE, 11, COMPUTE,
    STORAGE,
];

/// `decor_mark_cs`'s (decor_mark.wgsl).
pub const MARK_GROUP: [u32; 12] = [
    1, COMPUTE, STORAGE, 9, COMPUTE, STORAGE_RW, 11, COMPUTE, STORAGE_RW, 12, COMPUTE, STORAGE,
];

/// `decor_thin_cs`'s (decor_thin.wgsl).
pub const THIN_GROUP: [u32; 9] = [
    9, COMPUTE, STORAGE_RW, 10, COMPUTE, STORAGE, 11, COMPUTE, STORAGE,
];

/// `decor_cnt_cs`'s (decor_cnt.wgsl).
pub const CNT_GROUP: [u32; 15] = [
    0, COMPUTE, UNIFORM, 1, COMPUTE, STORAGE, 9, COMPUTE, STORAGE_RW, 13, COMPUTE, STORAGE, 14,
    COMPUTE, STORAGE_RW,
];

/// `decor_compact_cs`'s (decor_compact.wgsl).
pub const COMPACT_GROUP: [u32; 18] = [
    1, COMPUTE, STORAGE, 9, COMPUTE, STORAGE, 10, COMPUTE, STORAGE, 13, COMPUTE, STORAGE, 14,
    COMPUTE, STORAGE_RW, 15, COMPUTE, STORAGE_RW,
];

/// `decor_args_cs`'s (decor_args.wgsl).
pub const ARGS_GROUP: [u32; 15] = [
    0, COMPUTE, STORAGE, 1, COMPUTE, STORAGE_RW, 2, COMPUTE, STORAGE_RW, 3, COMPUTE, STORAGE_RW,
    15, COMPUTE, STORAGE_RW,
];

/// `decor_gate_cs`'s and `decor_gate_args_cs`'s (decor_gate.wgsl): the frame's words, the
/// regions' uniforms and args, the instances, the gated list and args, the claimed tiles' box,
/// the regions' counts and first slots, and the tiles' uniform.
pub const GATE_GROUP: [u32; 27] = [
    0, COMPUTE, UNIFORM, 1, COMPUTE, STORAGE, 2, COMPUTE, STORAGE, 3, COMPUTE, STORAGE, 4, COMPUTE,
    STORAGE_RW, 5, COMPUTE, STORAGE_RW, 6, COMPUTE, STORAGE, 7, COMPUTE, STORAGE, 31, COMPUTE,
    UNIFORM,
];

// Each compute group here against what its shader reads, as its declarations take it
// (build.rs `BINDS`).
const _: () = assert!(
    binds(&[COMPUTE_DECOR_GATE, COMPUTE_DECOR_GATE_ARGS], &GATE_GROUP)
        && binds(&[COMPUTE_GENERATION_MARK], &MARK_GROUP)
        && binds(&[COMPUTE_GENERATION_THIN], &THIN_GROUP)
        && binds(&[COMPUTE_GENERATION], &GENERATE_GROUP)
        && binds(&[COMPUTE_GENERATION_CNT], &CNT_GROUP)
        && binds(&[COMPUTE_GENERATION_ARGS], &ARGS_GROUP)
        && binds(&[COMPUTE_GENERATION_COMPACT], &COMPACT_GROUP),
    "a compute group lays out other than its shader reads (record.rs `binds`)"
);

const _: () = assert!(
    floor::holds(&[&GENERATE_GROUP])
        && floor::holds(&[&MARK_GROUP])
        && floor::holds(&[&THIN_GROUP])
        && floor::holds(&[&CNT_GROUP])
        && floor::holds(&[&COMPACT_GROUP])
        && floor::holds(&[&ARGS_GROUP])
        && floor::holds(&[&GATE_GROUP]),
    "a decor pass's group binds past WebGPU's floor"
);

/// What the marks' pass binds (screen_mark.wgsl): its mark and head, and
/// the tiles' words.
pub const SCREEN_MARK_GROUP: [u32; 6] = [0, COMPUTE, UNIFORM, 1, COMPUTE, STORAGE_RW];

/// What the box's args' pass binds (box_args.wgsl): the marks' box, the
/// args and the tiles' words.
pub const BOX_ARGS_GROUP: [u32; 9] = [
    0, COMPUTE, STORAGE, 1, COMPUTE, STORAGE_RW, 2, COMPUTE, UNIFORM,
];

/// What the claimed tiles' box's pass binds (claimed_box.wgsl): the box, the tiles' words
/// and uniform.
pub const CLAIMED_BOX_GROUP: [u32; 9] = [
    0, COMPUTE, STORAGE_RW, 30, COMPUTE, STORAGE, 31, COMPUTE, UNIFORM,
];

// Each compute group here against what its shader reads, as its declarations take it
// (build.rs `BINDS`).
const _: () = assert!(
    binds(&[COMPUTE_BOX_ARGS], &BOX_ARGS_GROUP)
        && binds(&[COMPUTE_SCREEN_MARK], &SCREEN_MARK_GROUP)
        && binds(&[COMPUTE_CLAIMED_BOX], &CLAIMED_BOX_GROUP),
    "a compute group lays out other than its shader reads (record.rs `binds`)"
);
