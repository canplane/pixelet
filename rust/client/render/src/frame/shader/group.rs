// pxl-render/frame/shader/group.rs

//! The compute groups and the buffers the frame's calls share. Each compute pipeline's group 0
//! is stated as its layout's (binding, stages, kind) words, and the crate's build holds each
//! layout to what its shaders read (record.rs `binds`) and to WebGPU's floor (floor.rs). Each
//! group the frame sets is stated by the frame's buffers it binds. The host makes the group
//! from that statement (gpu/api.rs `Gpu::frame_group`), and the frame's calls are checked
//! against the same statement (sequence.rs), so the two cannot disagree.

use crate::frame::shader::access::{Bind, Buffer, Group, ARGS, END, LIST, STATE};
use crate::frame::shader::floor;
use crate::frame::shader::record::*;
use pxl_engine::{raster, ray};

/// A pass of the light's map binds the light's head at this binding.
pub const HEADER_BINDING: u32 = 11;

/// The group of the walks over the node cache alone of the lattice's runs, which are the runs
/// of the samples and of the beam rays (queue_walk.wgsl over sample.wgsl or corner.wgsl): the
/// eye, the beam's words, the node cache and the queue's words.
pub const LATTICE_WALK_GROUP: [u32; 12] = [
    0, COMPUTE, UNIFORM, 8, COMPUTE, STORAGE_RW, 9, COMPUTE, STORAGE, 10, COMPUTE, STORAGE_RW,
];

/// `LATTICE_WALK_GROUP` with the light's head at `HEADER_BINDING`, for the light's map.
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
    HEADER_BINDING,
    COMPUTE,
    UNIFORM,
];

/// The group of the light's slots' pass (`light_cs`): the eye, the list, the node cache it
/// reads the visible voxel's entry from, the map it writes, the head, the visibility it reads,
/// and the dispatch of the missed slots.
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
    HEADER_BINDING,
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

/// The group of `cast_cs` (cast.wgsl, ray/cast.rs `px_group`): the eye, the rays, the
/// beam's words, the node cache it lists the marked pixels in and reads the visible voxel's
/// entry from, the visibility buffer, the words of the pixels to walk again, and the list of
/// the damaged tiles.
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

/// The group of `cast_tiles_cs` (cast_tile.wgsl): the list, the tiles' words and the tiles'
/// uniform.
pub const CAST_TILES_GROUP: [u32; 9] = [
    0, COMPUTE, STORAGE_RW, 30, COMPUTE, STORAGE, 31, COMPUTE, UNIFORM,
];

/// The groups of the light's receivers' passes. Each binds the eye, the head and the list.
/// The walked and the drawn receivers' passes also bind the stood list's args and the light's
/// args, whose marks they exchange atomically (light_receiver_tile.wgsl). The walked
/// receivers' pass binds the rays by which the eye's pixels met the world, and the eye's beam
/// rays' words, which list its walked pixels past the grid's (light_mark.wgsl). The drawn
/// receivers' pass binds the placed decor instead (light_mark_draws.wgsl). The propagation
/// binds the light's args alone (light_propagate.wgsl).
pub const LIGHT_MARK_GROUP: [u32; 21] = [
    0,
    COMPUTE,
    UNIFORM,
    3,
    COMPUTE,
    STORAGE_TEXTURE_RW,
    HEADER_BINDING,
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
    HEADER_BINDING,
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

/// The group of the pass that writes the drawn receivers' dispatch
/// (light_mark_draws_open.wgsl): the drawn receivers' group, and the dispatch their pass reads
/// its count from.
pub const LIGHT_MARK_DRAWS_OPEN_GROUP: [u32; 27] = [
    0,
    COMPUTE,
    UNIFORM,
    HEADER_BINDING,
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
    HEADER_BINDING,
    COMPUTE,
    UNIFORM,
    12,
    COMPUTE,
    STORAGE_RW,
    21,
    COMPUTE,
    STORAGE_RW,
];

/// The binding of the drawn receivers' dispatch (light_mark_draws_open.wgsl). The pass before
/// the drawn receivers' pass writes it there, and the drawn receivers' pass then reads its
/// count from it.
pub const DRAWS_BINDING: u32 = 24;

/// The group of `light_slide_cs` (light_slide.wgsl): the eye, the head and the list.
pub const LIGHT_SLIDE_GROUP: [u32; 9] = [
    0,
    COMPUTE,
    UNIFORM,
    HEADER_BINDING,
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

/// The cast's world group of the eye's passes over the world, `cell_cs` and `forget_cs`: the
/// eye, the beam rays' words that take the cells' starts, and the node cache.
pub const WORLD_GROUP: [u32; 9] = [
    0, COMPUTE, UNIFORM, 8, COMPUTE, STORAGE_RW, 9, COMPUTE, STORAGE_RW,
];

/// The group of the light's cells' pass, `light_cell_cs`: the eye, the light's list, which
/// takes the cells' starts, and the light's head (ray/cast.rs `light_groups`).
pub const LIGHT_WORLD_GROUP: [u32; 9] = [
    0,
    COMPUTE,
    UNIFORM,
    8,
    COMPUTE,
    STORAGE_RW,
    HEADER_BINDING,
    COMPUTE,
    UNIFORM,
];

/// The group of the eye's holes' queue's walks over the node cache alone (queue_walk.wgsl over
/// hole.eye.wgsl): the eye, the rays they write, the beam's words, the node cache they read and
/// the queue's words.
pub const QUEUE_GROUP: [u32; 15] = [
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
];

/// `QUEUE_GROUP` with the light's head at `HEADER_BINDING`, for the light's map, whose list
/// takes the beam's words' place and whose map the rays'.
pub const LIGHT_QUEUE_GROUP: [u32; 18] = [
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
    HEADER_BINDING,
    COMPUTE,
    UNIFORM,
];

/// The group of the eye's holes' queue's last walk (queue_last.wgsl over hole.eye.wgsl): the
/// eye, the world's buffers, the rays, the beam's words and the node cache it writes.
pub const QUEUE_LAST_GROUP: [u32; 30] = [
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
];

/// `QUEUE_LAST_GROUP` with the light's head at `HEADER_BINDING`, for the light's map.
pub const LIGHT_QUEUE_LAST_GROUP: [u32; 33] = [
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
    HEADER_BINDING,
    COMPUTE,
    UNIFORM,
];

/// The group of every run's asks (queue_ask.wgsl over ask_run.wgsl): the eye, the world's
/// buffers, the node cache they write and the queue's words.
pub const LATTICE_ASK_GROUP: [u32; 27] = [
    0, COMPUTE, UNIFORM, 1, COMPUTE, STORAGE, 2, COMPUTE, STORAGE, 4, COMPUTE, STORAGE, 5, COMPUTE,
    STORAGE, 6, COMPUTE, STORAGE, 7, COMPUTE, STORAGE, 9, COMPUTE, STORAGE_RW, 10, COMPUTE,
    STORAGE_RW,
];

/// The group of the last walk of the eye's lattice runs (queue_last.wgsl over sample.wgsl or
/// corner.wgsl): the eye, the world's buffers, the beam's words and the node cache it writes.
pub const LATTICE_LAST_GROUP: [u32; 27] = [
    0, COMPUTE, UNIFORM, 1, COMPUTE, STORAGE, 2, COMPUTE, STORAGE, 4, COMPUTE, STORAGE, 5, COMPUTE,
    STORAGE, 6, COMPUTE, STORAGE, 7, COMPUTE, STORAGE, 8, COMPUTE, STORAGE_RW, 9, COMPUTE,
    STORAGE_RW,
];

/// `LATTICE_LAST_GROUP` with the light's head at `HEADER_BINDING`, for the light's map.
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
    HEADER_BINDING,
    COMPUTE,
    UNIFORM,
];

const _: () = assert!(
    floor::holds(&[&LIGHT_WORLD_GROUP])
        && floor::holds(&[&QUEUE_GROUP])
        && floor::holds(&[&LATTICE_ASK_GROUP])
        && floor::holds(&[&LIGHT_LATTICE_LAST_GROUP])
        && floor::holds(&[&LIGHT_QUEUE_GROUP])
        && floor::holds(&[&LIGHT_QUEUE_LAST_GROUP]),
    "the cast's world's group or the holes' queue's binds past WebGPU's floor"
);

const _: () = assert!(
    binds(&[COMPUTE_CAST], &CAST_GROUP)
        && binds(&[COMPUTE_CAST_TILES], &CAST_TILES_GROUP)
        && binds(&[COMPUTE_CELL, COMPUTE_FORGET], &WORLD_GROUP)
        && binds(&[COMPUTE_LIGHT_CELL], &LIGHT_WORLD_GROUP)
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
            &[COMPUTE_QUEUE_LIGHT, COMPUTE_QUEUE_LIGHT_PARKED],
            &LIGHT_QUEUE_GROUP
        )
        && binds(
            &[COMPUTE_QUEUE_ASK, COMPUTE_QUEUE_ASK_PARKED],
            &LATTICE_ASK_GROUP
        )
        && binds(&[COMPUTE_QUEUE_LAST], &QUEUE_LAST_GROUP)
        && binds(&[COMPUTE_QUEUE_LAST_LIGHT], &LIGHT_QUEUE_LAST_GROUP)
        && binds(&[COMPUTE_LIGHT], &LIGHT_GROUP)
        && binds(&[COMPUTE_LIGHT_MARK_WALKED], &LIGHT_MARK_GROUP)
        && binds(&[COMPUTE_LIGHT_MARK_DRAWS], &LIGHT_MARK_DRAWS_GROUP)
        && binds(
            &[COMPUTE_LIGHT_MARK_DRAWS_OPEN],
            &LIGHT_MARK_DRAWS_OPEN_GROUP
        )
        && binds(
            &[COMPUTE_LIGHT_PROPAGATE, COMPUTE_LIGHT_DAMAGE],
            &LIGHT_PROPAGATE_GROUP
        )
        && binds(&[COMPUTE_LIGHT_SLIDE], &LIGHT_SLIDE_GROUP),
    "a cast's compute group lays out other than its shaders read (record.rs `binds`)"
);

/// The group of an observer's samples' stops (sample_hit.wgsl): the eye, the samples' words,
/// the node cache whose drawable entries they set, and the args of the faces' asks.
pub const SAMPLE_HIT_GROUP: [u32; 12] = [
    0, COMPUTE, UNIFORM, 8, COMPUTE, STORAGE_RW, 9, COMPUTE, STORAGE_RW, 16, COMPUTE, STORAGE_RW,
];

/// The group of the record's draw over every slot of the node cache (record_draw.wgsl): the
/// eye, the node cache, the boxes, the raster's args, and the light's buffers through which
/// the eye's boxes stand as receivers.
pub const REC_GROUP: [u32; 24] = [
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
    HEADER_BINDING,
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

/// The group of the holes' stops (record_hits.wgsl): the eye, the rays, the stops among the
/// beam rays' words, the node cache that holds their count and whose drawable entries they
/// set, and the args of the faces' asks. It does not bind the stops' dispatch: that is copied
/// out of the node cache, so that no pass reads its count from a buffer it binds writable
/// (ray/raster.rs `Raster::rec_hits`).
pub const REC_HITS_GROUP: [u32; 15] = [
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

/// The group of the light's holes' stops (record_hits.wgsl over record_hits.light.wgsl):
/// `REC_HITS_GROUP` with the light's map at 3 and its list at 8, and the light's head, by
/// which its slots' rays are aimed.
pub const LIGHT_REC_HITS_GROUP: [u32; 18] = [
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
    HEADER_BINDING,
    COMPUTE,
    UNIFORM,
    16,
    COMPUTE,
    STORAGE_RW,
];

/// The group of the light's samples' stops (sample_hit.wgsl over sample.light.wgsl): the eye,
/// the samples' words, the node cache, the light's head and its raster's args.
pub const LIGHT_SAMPLE_HIT_GROUP: [u32; 15] = [
    0,
    COMPUTE,
    UNIFORM,
    8,
    COMPUTE,
    STORAGE_RW,
    9,
    COMPUTE,
    STORAGE_RW,
    HEADER_BINDING,
    COMPUTE,
    UNIFORM,
    16,
    COMPUTE,
    STORAGE_RW,
];

const _: () = assert!(
    floor::holds(&[&SAMPLE_HIT_GROUP])
        && floor::holds(&[&LIGHT_SAMPLE_HIT_GROUP])
        && floor::holds(&[&REC_GROUP])
        && floor::holds(&[&REC_HITS_GROUP])
        && floor::holds(&[&LIGHT_REC_HITS_GROUP]),
    "the boxes' writing's group binds past WebGPU's floor"
);

/// The group of `face_cs` (face.wgsl): the eye, the buffers of the world's query except the
/// palette, which it does not read, the beam's words it asks from and leaves a hit's faces in,
/// the node cache it leaves a drawable entry's faces in, and the args it is bound to.
pub const FACE_GROUP: [u32; 27] = [
    0, COMPUTE, UNIFORM, 1, COMPUTE, STORAGE, 2, COMPUTE, STORAGE, 5, COMPUTE, STORAGE, 6, COMPUTE,
    STORAGE, 7, COMPUTE, STORAGE, 8, COMPUTE, STORAGE_RW, 9, COMPUTE, STORAGE_RW, 16, COMPUTE,
    STORAGE,
];

const _: () = assert!(
    binds(&[COMPUTE_SAMPLE_HIT], &SAMPLE_HIT_GROUP)
        && binds(&[COMPUTE_LIGHT_SAMPLE_HIT], &LIGHT_SAMPLE_HIT_GROUP)
        && binds(&[COMPUTE_FACE], &FACE_GROUP)
        && binds(&[COMPUTE_REC_DRAW, COMPUTE_LIGHT_REC_DRAW], &REC_GROUP)
        && binds(&[COMPUTE_REC_HITS, COMPUTE_REC_FACES], &REC_HITS_GROUP)
        && binds(&[COMPUTE_LIGHT_REC_HITS], &LIGHT_REC_HITS_GROUP),
    "a compute group lays out other than its shader reads (record.rs `binds`)"
);

const _: () = assert!(
    floor::holds(&[&FACE_GROUP]),
    "the faces' group binds past WebGPU's floor"
);

/// The group of `decor_pose_cs` (decor_pose.wgsl): the world's buffers at world.wgsl's binding
/// numbers with its eye, the records, the rows and the list.
pub const GENERATE_GROUP: [u32; 27] = [
    0, COMPUTE, UNIFORM, 1, COMPUTE, STORAGE, 2, COMPUTE, STORAGE, 5, COMPUTE, STORAGE, 6, COMPUTE,
    STORAGE, 7, COMPUTE, STORAGE, 9, COMPUTE, STORAGE_RW, 10, COMPUTE, STORAGE, 11, COMPUTE,
    STORAGE,
];

pub const MARK_GROUP: [u32; 12] = [
    1, COMPUTE, STORAGE, 9, COMPUTE, STORAGE_RW, 11, COMPUTE, STORAGE_RW, 12, COMPUTE, STORAGE,
];

pub const THIN_GROUP: [u32; 9] = [
    9, COMPUTE, STORAGE_RW, 10, COMPUTE, STORAGE, 11, COMPUTE, STORAGE,
];

pub const CNT_GROUP: [u32; 15] = [
    0, COMPUTE, UNIFORM, 1, COMPUTE, STORAGE, 9, COMPUTE, STORAGE_RW, 13, COMPUTE, STORAGE, 14,
    COMPUTE, STORAGE_RW,
];

pub const COMPACT_GROUP: [u32; 18] = [
    1, COMPUTE, STORAGE, 9, COMPUTE, STORAGE, 10, COMPUTE, STORAGE, 13, COMPUTE, STORAGE, 14,
    COMPUTE, STORAGE_RW, 15, COMPUTE, STORAGE_RW,
];

pub const ARGS_GROUP: [u32; 15] = [
    0, COMPUTE, STORAGE, 1, COMPUTE, STORAGE_RW, 2, COMPUTE, STORAGE_RW, 3, COMPUTE, STORAGE_RW,
    15, COMPUTE, STORAGE_RW,
];

/// The group of `decor_cull_cs` and `decor_cull_args_cs` (decor_cull.wgsl): the frame's
/// words, the regions' uniforms and args, the instances, the gated list and its args, the box
/// of the damaged tiles, the regions' counts and first slots, and the tiles' uniform.
pub const CULL_GROUP: [u32; 27] = [
    0, COMPUTE, UNIFORM, 1, COMPUTE, STORAGE, 2, COMPUTE, STORAGE, 3, COMPUTE, STORAGE, 4, COMPUTE,
    STORAGE_RW, 5, COMPUTE, STORAGE_RW, 6, COMPUTE, STORAGE, 7, COMPUTE, STORAGE, 31, COMPUTE,
    UNIFORM,
];

const _: () = assert!(
    binds(&[COMPUTE_DECOR_CULL, COMPUTE_DECOR_CULL_ARGS], &CULL_GROUP)
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
        && floor::holds(&[&CULL_GROUP]),
    "a decor pass's group binds past WebGPU's floor"
);

/// The group of the marks' pass (screen_mark.wgsl): the uniform that holds the mark and the
/// head, and the tiles' words.
pub const SCREEN_MARK_GROUP: [u32; 6] = [0, COMPUTE, UNIFORM, 1, COMPUTE, STORAGE_RW];

/// The group of the pass that writes the box's draw args (rect_args.wgsl): the box the marks
/// cover, the args, and the tiles' uniform.
pub const RECT_ARGS_GROUP: [u32; 9] = [
    0, COMPUTE, STORAGE, 1, COMPUTE, STORAGE_RW, 2, COMPUTE, UNIFORM,
];

/// The group of the pass that finds the damaged tiles' box (damage_rect.wgsl): the box, the
/// tiles' words and the tiles' uniform.
pub const DAMAGE_RECT_GROUP: [u32; 9] = [
    0, COMPUTE, STORAGE_RW, 30, COMPUTE, STORAGE, 31, COMPUTE, UNIFORM,
];

const _: () = assert!(
    binds(&[COMPUTE_RECT_ARGS], &RECT_ARGS_GROUP)
        && binds(&[COMPUTE_SCREEN_MARK], &SCREEN_MARK_GROUP)
        && binds(&[COMPUTE_DAMAGE_RECT], &DAMAGE_RECT_GROUP),
    "a compute group lays out other than its shader reads (record.rs `binds`)"
);

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

// The groups the host makes (gpu/api.rs `Gpu::frame_group`), each by its bindings of the
// frame's buffers. The host makes each group from this table, and the frame's calls are
// checked against the same table.
pub const CORNERS_WALK_BINDS: Group = Group {
    layout: &LATTICE_WALK_GROUP,
    binds: &[
        bind(8, BEAMS),
        bind(9, NODE_CACHE),
        queue_bind(ray::QUEUE_EYE_CORNERS),
    ],
};
pub const CORNERS_ASK_BINDS: Group = Group {
    layout: &LATTICE_ASK_GROUP,
    binds: &[bind(9, NODE_CACHE), queue_bind(ray::QUEUE_EYE_CORNERS)],
};
pub const SAMPLES_WALK_BINDS: Group = Group {
    layout: &LATTICE_WALK_GROUP,
    binds: &[
        bind(8, BEAMS),
        bind(9, NODE_CACHE),
        queue_bind(ray::QUEUE_EYE_SAMPLES),
    ],
};
pub const SAMPLES_ASK_BINDS: Group = Group {
    layout: &LATTICE_ASK_GROUP,
    binds: &[bind(9, NODE_CACHE), queue_bind(ray::QUEUE_EYE_SAMPLES)],
};
pub const LATTICE_LAST_BINDS: Group = Group {
    layout: &LATTICE_LAST_GROUP,
    binds: &[bind(8, BEAMS), bind(9, NODE_CACHE)],
};
pub const WORLD_BINDS: Group = Group {
    layout: &WORLD_GROUP,
    binds: &[bind(8, BEAMS), bind(9, NODE_CACHE)],
};
pub const SAMPLE_HIT_BINDS: Group = Group {
    layout: &SAMPLE_HIT_GROUP,
    binds: &[bind(8, BEAMS), bind(9, NODE_CACHE), bind(16, RASTER_ARGS)],
};
pub const REC_BINDS: Group = Group {
    layout: &REC_GROUP,
    binds: &[
        bind(12, LIGHT_LIST),
        bind(9, NODE_CACHE),
        bind(16, RASTER_ARGS),
        bind(21, LIGHT_ARGS),
        bind(22, LIGHT_RECEIVER_TILES),
    ],
};
pub const FACES_BINDS: Group = Group {
    layout: &FACE_GROUP,
    binds: &[
        bind(8, BEAMS),
        bind(9, NODE_CACHE),
        args_bind(raster::ASKED_AT),
    ],
};
pub const HIT_FACES_OF_BINDS: Group = Group {
    layout: &FACE_GROUP,
    binds: &[
        bind(8, BEAMS),
        bind(9, NODE_CACHE),
        args_bind(raster::HIT_ASKED_AT),
    ],
};
pub const CAST_TILES_OF_BINDS: Group = Group {
    layout: &CAST_TILES_GROUP,
    binds: &[bind(0, CAST_TILES)],
};
pub const PIXELS_BINDS: Group = Group {
    layout: &CAST_GROUP,
    binds: &[
        bind(8, BEAMS),
        bind(9, NODE_CACHE),
        bind(14, MARKS),
        bind(30, CAST_TILES),
    ],
};
pub const LIGHT_WORLD_BINDS: Group = Group {
    layout: &LIGHT_WORLD_GROUP,
    binds: &[bind(8, LIGHT_LIST)],
};
pub const WALK_BINDS: Group = Group {
    layout: &QUEUE_GROUP,
    binds: &[
        bind(8, BEAMS),
        bind(9, NODE_CACHE),
        queue_bind(ray::QUEUE_EYE_HOLES),
    ],
};
pub const WALK_ASK_BINDS: Group = Group {
    layout: &LATTICE_ASK_GROUP,
    binds: &[bind(9, NODE_CACHE), queue_bind(ray::QUEUE_EYE_HOLES)],
};
pub const LAST_BINDS: Group = Group {
    layout: &QUEUE_LAST_GROUP,
    binds: &[bind(8, BEAMS), bind(9, NODE_CACHE)],
};
pub const REC_HITS_OF_BINDS: Group = Group {
    layout: &REC_HITS_GROUP,
    binds: &[bind(8, BEAMS), bind(9, NODE_CACHE), bind(16, RASTER_ARGS)],
};
pub const GENERATION_MARK_BINDS: Group = Group {
    layout: &MARK_GROUP,
    binds: &[bind(11, GENERATION_LIST)],
};
pub const GENERATION_THIN_BINDS: Group = Group {
    layout: &THIN_GROUP,
    binds: &[bind(11, GENERATION_LIST)],
};
pub const GENERATION_BINDS: Group = Group {
    layout: &GENERATE_GROUP,
    binds: &[bind(11, GENERATION_LIST)],
};
pub const GENERATION_ARGS_OF_BINDS: Group = Group {
    layout: &ARGS_GROUP,
    binds: &[bind(2, GENERATION_ARGS)],
};
pub const CULL_SCREEN_BINDS: Group = Group {
    layout: &CULL_GROUP,
    binds: &[bind(2, GENERATION_ARGS), bind(5, CULLED_SCREEN_ARGS)],
};
pub const CULL_LIGHT_BINDS: Group = Group {
    layout: &CULL_GROUP,
    binds: &[bind(2, GENERATION_ARGS), bind(5, CULLED_LIGHT_ARGS)],
};
pub const LIGHT_RECT_BINDS: Group = Group {
    layout: &RECT_ARGS_GROUP,
    binds: &[bind(1, LIGHT_RECT_ARGS)],
};
pub const LIGHT_REC_BINDS: Group = Group {
    layout: &REC_GROUP,
    binds: &[
        bind(12, LIGHT_LIST),
        bind(9, NODE_CACHE),
        bind(16, LIGHT_RASTER_ARGS),
        bind(21, LIGHT_ARGS),
        bind(22, LIGHT_RECEIVER_TILES),
    ],
};
pub const LIGHT_MARK_BINDS: Group = Group {
    layout: &LIGHT_MARK_GROUP,
    binds: &[
        bind(12, LIGHT_LIST),
        bind(21, LIGHT_ARGS),
        bind(22, LIGHT_RECEIVER_TILES),
        bind(23, BEAMS),
    ],
};
pub const LIGHT_MARK_DRAWS_OPEN_BINDS: Group = Group {
    layout: &LIGHT_MARK_DRAWS_OPEN_GROUP,
    binds: &[
        bind(12, LIGHT_LIST),
        bind(21, LIGHT_ARGS),
        bind(22, LIGHT_RECEIVER_TILES),
        bind(DRAWS_BINDING, LIGHT_DRAWS),
    ],
};
pub const LIGHT_MARK_DRAWS_BINDS: Group = Group {
    layout: &LIGHT_MARK_DRAWS_GROUP,
    binds: &[
        bind(12, LIGHT_LIST),
        bind(21, LIGHT_ARGS),
        bind(22, LIGHT_RECEIVER_TILES),
    ],
};
pub const LIGHT_SLIDE_BINDS: Group = Group {
    layout: &LIGHT_SLIDE_GROUP,
    binds: &[bind(12, LIGHT_LIST)],
};
pub const LIGHT_PROPAGATE_BINDS: Group = Group {
    layout: &LIGHT_PROPAGATE_GROUP,
    binds: &[bind(12, LIGHT_LIST), bind(21, LIGHT_ARGS)],
};
pub const LIGHT_CORNERS_WALK_BINDS: Group = Group {
    layout: &LIGHT_LATTICE_WALK_GROUP,
    binds: &[
        bind(8, LIGHT_LIST),
        bind(9, NODE_CACHE),
        queue_bind(ray::QUEUE_LIGHT_CORNERS),
    ],
};
pub const LIGHT_CORNERS_ASK_BINDS: Group = Group {
    layout: &LATTICE_ASK_GROUP,
    binds: &[bind(9, NODE_CACHE), queue_bind(ray::QUEUE_LIGHT_CORNERS)],
};
pub const LIGHT_SAMPLES_WALK_BINDS: Group = Group {
    layout: &LIGHT_LATTICE_WALK_GROUP,
    binds: &[
        bind(8, LIGHT_LIST),
        bind(9, NODE_CACHE),
        queue_bind(ray::QUEUE_LIGHT_SAMPLES),
    ],
};
pub const LIGHT_SAMPLES_ASK_BINDS: Group = Group {
    layout: &LATTICE_ASK_GROUP,
    binds: &[bind(9, NODE_CACHE), queue_bind(ray::QUEUE_LIGHT_SAMPLES)],
};
pub const LIGHT_LATTICE_LAST_BINDS: Group = Group {
    layout: &LIGHT_LATTICE_LAST_GROUP,
    binds: &[bind(8, LIGHT_LIST), bind(9, NODE_CACHE)],
};
pub const LIGHT_SAMPLE_HIT_BINDS: Group = Group {
    layout: &LIGHT_SAMPLE_HIT_GROUP,
    binds: &[
        bind(8, LIGHT_LIST),
        bind(9, NODE_CACHE),
        bind(16, LIGHT_RASTER_ARGS),
    ],
};
pub const LIGHT_SLOTS_BINDS: Group = Group {
    layout: &LIGHT_GROUP,
    binds: &[
        bind(8, LIGHT_LIST),
        bind(9, NODE_CACHE),
        bind(17, LIGHT_MISSES),
    ],
};
pub const LIGHT_WALK_BINDS: Group = Group {
    layout: &LIGHT_QUEUE_GROUP,
    binds: &[
        bind(8, LIGHT_LIST),
        bind(9, NODE_CACHE),
        queue_bind(ray::QUEUE_LIGHT_HOLES),
    ],
};
pub const LIGHT_WALK_ASK_BINDS: Group = Group {
    layout: &LATTICE_ASK_GROUP,
    binds: &[bind(9, NODE_CACHE), queue_bind(ray::QUEUE_LIGHT_HOLES)],
};
pub const LIGHT_LAST_BINDS: Group = Group {
    layout: &LIGHT_QUEUE_LAST_GROUP,
    binds: &[bind(8, LIGHT_LIST), bind(9, NODE_CACHE)],
};
pub const LIGHT_REC_HITS_BINDS: Group = Group {
    layout: &LIGHT_REC_HITS_GROUP,
    binds: &[
        bind(8, LIGHT_LIST),
        bind(9, NODE_CACHE),
        bind(16, LIGHT_RASTER_ARGS),
    ],
};
