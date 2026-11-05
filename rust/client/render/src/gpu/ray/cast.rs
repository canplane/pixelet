// pxl-render/gpu/ray/cast.rs

//! The GPU's cast of the eye's rays into the world surface's rays, the
//! world's depth from the light cast with them, and the decor generated over the
//! world (gpu/generator.rs): what each reads, made once, and their dispatches.

use pxl_content::generation::{self, GROUPS};
use pxl_engine::clipmap::{self, FILL_CELL, FILL_FIRST};
use pxl_engine::{beam, eye, light, node_cache, overwrite, raster, ray, screen, LEAF_CELL};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use super::count::{self, Counter, Counts, RayCounts};
use super::raster::Raster;
use super::{casts_px, GroundPipelines, RaysPass};
use crate::frame::shader::floor;
use crate::frame::shader::record::{
    binds, COMPUTE, COMPUTE_BEAM, COMPUTE_BEAM_HIT, COMPUTE_CAST, COMPUTE_CAST_TILES, COMPUTE_CELL,
    COMPUTE_FOLD, COMPUTE_FORGET, COMPUTE_HIT_FACES, COMPUTE_HIT_LEAD, COMPUTE_LIGHT,
    COMPUTE_LIGHT_BEAM, COMPUTE_LIGHT_BEAM_HIT, COMPUTE_LIGHT_CELL, COMPUTE_LIGHT_MARK,
    COMPUTE_LIGHT_MARK_DRAWS, COMPUTE_LIGHT_MISS, COMPUTE_LIGHT_MISS_HIT, COMPUTE_LIGHT_PROPAGATE,
    COMPUTE_LIGHT_SAMPLE, COMPUTE_LIGHT_SAMPLE_HIT, COMPUTE_LIGHT_SLIDE, COMPUTE_MISS,
    COMPUTE_MISS_HIT, COMPUTE_MISS_LIST, COMPUTE_SAMPLE, COMPUTE_SAMPLE_HIT, STORAGE, STORAGE_RW,
    STORAGE_TEXTURE_RW, TEXTURE_UINT_ARRAY, UNIFORM,
};
use crate::frame::shader::word::{CELL_SIDE, FOLD_LANES, ROW_LANES, SQUARE_SIDE};
use crate::gpu::api;
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, release, view_entry, word_bytes, write_buffer_bytes, Entry,
    Gpu, RenderContext,
};
use crate::gpu::generator::{cut_boxes, hold, DecorFrame, Generation, Generator, IndirectDraw};
use crate::gpu::pipeline::{Pipeline, Pipelines};
use crate::gpu::read::Read;
use crate::gpu::shadow;
use crate::gpu::span_spec::{FrameKind, GpuSpanName, LightMoved};

/// The GPU's cast this frame (frame/shader/cast.wgsl `cast_cs`), a ray
/// a drawn pixel of the world surface: the eye, the terrain's tiles as the
/// GPU holds them, and what is placed.
pub struct Cast<'a> {
    /// The eye (pxl-engine `eye`; pxl-kernel domain/view/lens.rs `gpu_cast`,
    /// domain/scene/cast.rs `eye`).
    pub eye: [u32; eye::WORDS],
    /// The clipmap's table (pxl-engine `clipmap::Entry`; pxl-kernel
    /// domain/layer/terrain/clipmap.rs `table`), four words a row: every clip
    /// level's entries, the eye's first, from the top's, each the brick it
    /// wants, X and Y, the slot holding it plus one while present (`Entry::slot`)
    /// and the fill it is on its way from; then each clip level's first brick
    /// (`clipmap::ORIGIN_AT`).
    pub clipmap: &'a [i32],
    /// The service area's index (pxl-engine `area::Area`), its keys four a
    /// row (pxl-kernel domain/scene/cast.rs `shards`); handed where it lies,
    /// its writer asking `held` (gpu/api.rs) before it writes it again.
    pub shards: &'a [[i32; 4]],
    pub overwrites: Overwrites<'a>,
    /// The decor the GPU generates and draws over the same world (gpu/generator.rs).
    pub decor: DecorFrame<'a>,
    /// Where each actor drawn stands, units from the eye, as its draw is
    /// placed (frame/draw.rs `Lists::stands`): receivers of the shadow map as
    /// decor is (pxl-engine `light::ACTORS_AT`).
    pub actors: &'a [[f32; 3]],
    /// Whether what the eye's cast and the sun's walked and took is counted
    /// (gpu/ray/count.rs).
    pub ray_counts: bool,
}

/// The overwrites' tree (pxl-kernel domain/layer/object/store.rs) as it
/// lives: its child pointer and data lanes, handed where they lie, their
/// writer asking `held` (gpu/api.rs) before it writes them again; the room
/// either has before it grows, the revision they are at, and the child
/// groups written since `since`, the revision they were last handed at, by
/// their first slots (`GpuColumns`).
pub struct Overwrites<'a> {
    pub first_child: &'a [u32],
    pub data: &'a [u32],
    pub room: usize,
    pub revision: u32,
    pub since: u32,
    pub written_groups: &'a [u32],
}

/// What the GPU's cast reads; it writes the world surface's rays.
pub(super) struct CastBuffers {
    pipeline: Pipeline,
    /// The beam rays (`beam_cs`, a thread a corner left), where each cell's
    /// rays start off them (`cell_cs`, a thread a cell), the samples walked
    /// from those starts (`sample_cs`, a thread a sample left): the eye's over
    /// `ask_group` and the cast's group and the shadow map's over the light's (`light_groups`),
    /// each told by the head it binds (`eye_head`, `light_head`); and the
    /// eye's buffer (pxl-engine `beam`): a line a corner, a start a
    /// cell, a hit a sample, as the last cast's `grid` lays them, none
    /// a pixel's; room for `beam_room` words.
    beam_built: Pipeline,
    cells_built: Pipeline,
    sample_built: Pipeline,
    /// The eye's walks that ask the world, its beam rays' and its samples': over the world's
    /// buffers and the node cache written (`ASK_GROUP`).
    ask_group: u32,
    /// The samples' walk over the node cache alone, ahead of `sample_built`.
    sample_hit_built: Pipeline,
    sample_hit_group: u32,
    /// `ray::LEFT_EMPTY`'s words, as the pass leaves them.
    sample_left: u32,
    /// The pixels' walk over the node cache alone, ahead of `miss_list_built`.
    miss_hit_built: Pipeline,
    miss_hit_group: u32,
    /// `MISS_LEFT_EMPTY`'s words, as the pass leaves them.
    miss_left: u32,
    /// The beam rays' walk over the node cache alone, ahead of `beam_built`, its
    /// group as `sample_hit_group`'s, over `beam_left`.
    beam_hit_built: Pipeline,
    beam_hit_group: u32,
    beam_left: u32,
    /// The same passes for the shadow map, an entry each (beam.light.wgsl,
    /// light_cell.wgsl, sample.light.wgsl): the cells over the same group, the
    /// beam rays and the samples over `ASK_GROUP`.
    light_beam_built: Pipeline,
    light_cells_built: Pipeline,
    light_sample_built: Pipeline,
    /// The light's beam rays' walk over the node cache alone, ahead of `light_beam_built`.
    light_beam_hit_built: Pipeline,
    /// What it left: `light_beam_built`'s dispatch, then their count.
    light_beam_left: u32,
    /// The light's samples' walk over the node cache alone, ahead of `light_sample_built`.
    light_sample_hit_built: Pipeline,
    /// What it left: `light_sample_built`'s dispatch, then their count.
    light_sample_left: u32,
    beams: u32,
    /// The eye's head, all zero: a head naming no map (pxl-engine
    /// `light::HEAD_SIZE`), which tells a pass both take the eye's.
    eye_head: u32,
    grid: beam::Grid,
    beam_room: u32,
    /// The node cache (pxl-engine `node_cache`): its buffer, no
    /// pixel's; the pixels the cast's reading walk missed walked again, a
    /// thread each of their list (`miss_list_cs`, pxl-engine
    /// `node_cache::MARKS_AT`), or where it was full a thread a drawn pixel
    /// (`miss_cs`), each told by the mark its own first ray word holds
    /// (pxl-engine `ray::MISS`), both dispatched off the words the
    /// reading walk counts them in (`marks`, pxl-engine
    /// `ray::MISS_ARGS_WORDS`); the node cache forgotten (`forget_cs`) where
    /// what the world's query reads changed since the last cast: all of it
    /// (`forget`), or the nodes over the eye's clip levels' entries that moved
    /// (`moved`, a bit an entry, against the entries as `entries` last held them; `moved_bricks`,
    /// the box a clip level's lie in)
    /// and over the boxes of what is placed or cut that changed (`boxes`,
    /// `holding` of them, as decor's generation holds them, gpu/generator.rs
    /// `hold`), and those handed a slot of a child group of what is placed that
    /// was written (`written_groups`, `writes` of them); as this frame's cast does
    /// (`forgets`, pxl-engine `node_cache::FORGETS`), and what it sent, put
    /// back where its frame is dropped (`sent_forget`). `frame` numbers the casts since it was last
    /// forgotten whole, and `cuts` are the preview's cuts it was asked with.
    miss_built: Pipeline,
    miss_list_built: Pipeline,
    /// Their group 0 (`MISS_GROUP`), over the cast's group's buffers.
    miss_group: u32,
    marks: u32,
    /// What lies about the faces the walks again stopped on, asked once a
    /// square of their hits (pxl-engine `beam::HIT_WORDS`): the one of
    /// each square that asks listed (`hit_lead_cs`), the faces' pass over
    /// their asks (ray/raster.rs `ask_hits`), and the faces put at their
    /// pixels (`hit_faces_cs`), each dispatched off the walks' count in the
    /// node cache's head (`node_cache::HITS_ARGS`) or the leaders' among the
    /// walk again's words (`ray::MISS_ARGS_ROOM`); their groups, and the
    /// buffers they bind, made again exactly when one of them is another.
    hit_lead_built: Pipeline,
    hit_faces_built: Pipeline,
    hit_groups: [u32; 2],
    hit_bindings: [u32; 5],
    forget_built: Pipeline,
    node_cache: u32,
    frame: u32,
    forget: bool,
    moved: [u32; node_cache::MOVED_WORDS as usize],
    moved_bricks: [[i32; 4]; clipmap::EYE_CLIP_LEVELS as usize],
    boxes: [[i32; 4]; overwrite::BOXES as usize],
    holding: usize,
    written_groups: [u32; node_cache::WRITTEN_CAP as usize],
    writes: usize,
    sent_forget: Option<ForgetHead>,
    entries: Vec<i32>,
    forgets: u32,
    cuts: [u32; eye::CUTS],
    /// The eye's clip levels folded over their bricks (`fold_cs`), over its
    /// group, the clip level buffer's rows past their entry table
    /// (`clipmap::JOINED_AT`), written where the clip levels' entries moved
    /// (`refold`). The CPU folds its own over the bricks it holds
    /// (pxl-kernel domain/layer/terrain/clipmap.rs `fold`).
    fold_built: Pipeline,
    /// The world's depth from the light, a texel of the shadow map each
    /// (`light_cs`), cast with the eye's rays where the shadows are on.
    light_built: Pipeline,
    /// The slots that took no visible sample walked apart (`light_miss_cs`), first
    /// over the node cache alone (`light_miss_hit_cs`).
    light_miss_built: Pipeline,
    light_miss_hit_built: Pipeline,
    /// `ray::LEFT_EMPTY`'s words, as `light_miss_hit_cs` leaves them.
    light_left: u32,
    /// The tiles of the light's box its receivers stand on in a cast
    /// (pxl-engine `light::stands_at`): where the eye's rays meet the world
    /// (`light_mark_cs`), and where the decor and the actors drawn stand
    /// (`light_mark_draws_cs`); then those they read (`light_propagate_cs`).
    light_mark_built: Pipeline,
    light_mark_draws_built: Pipeline,
    light_propagate_built: Pipeline,
    /// The tiles the box slid onto, or slid into another band, set back to
    /// be read anew (`light_slide_cs`, pxl-engine `Light::keeps`), before
    /// the propagation, where the box slid.
    light_slide_built: Pipeline,
    /// The light's cast's groups (`light_groups`), and the buffers they bind:
    /// made again exactly when one of them is another.
    light_groups: [u32; 12],
    light_bound: [u32; 12],
    /// A word bound where decor is off, the decor's buffers the drawn mark
    /// reads none (`light_mark_draws_cs`).
    light_none: u32,
    /// The light's casts so far, the number its marks are made with
    /// (pxl-engine `light::HEAD_CAST`), never zero, which no tile holds
    /// marked.
    light_number: u32,
    /// The number of the cast since which only which tiles are read moved,
    /// and the box (pxl-engine `light::HEAD_EPOCH`): a tile cast in it or
    /// after stands where the box kept it.
    light_epoch: u32,
    /// Whether all the light's cast is made (`light_made`): its list, its
    /// raster's buffers and targets and its groups; while not, no shadow map
    /// is cast.
    light_ready: bool,
    /// The beam's lines, the samples and the misses (pxl-engine
    /// `light::list_words`), made while the shadows are on; 0 while off.
    light_list: u32,
    /// The misses' walk's dispatch and count, as the eye's are
    /// (pxl-engine `ray::MISS_ARGS_WORDS`).
    light_misses: u32,
    /// The workgroups of the passes that cast its tiles, none, or each whole
    /// where a tile is claimed, and what propagation claims and counts
    /// (pxl-engine `light::args_words`), made as `light_list` is.
    light_args: u32,
    /// What a tile of the shadow map holds follows, as the light's cast last
    /// read it (`Read`): the light's frame (`Shadow::generation`), whether
    /// shadows are cast, the eye's zooms, and whether the world under it
    /// moved; one moving begins an epoch (`light_epoch`).
    light_content: Read,
    /// Whether this frame's cast is in its epoch and the box slid, its
    /// corner `light_corner` where the cast before was cast under
    /// `light_corner_was` (`light::HEAD_CORNER_WAS`): the reads moved with the
    /// box, and the tiles it slid onto or into another band cast
    /// (`light_slide_cs`, pxl-engine `Light::keeps`), with those read
    /// anew. The box follows the eye a tile at a time, and a tile's band is
    /// its place in it alone (`Light::band`), so nothing else of the eye's
    /// place moves what a tile holds.
    light_slid: bool,
    light_corner: [i32; 2],
    light_corner_was: [i32; 2],
    /// The reads as the cast before left them, copied out where the box slid
    /// to be copied back moved with it.
    light_reads_was: u32,
    /// Which tiles the receivers read, as it last read it: the eye's place
    /// and look, the actors' places, decor placed again. One moving alone
    /// casts only the tiles read anew.
    light_receivers: Read,
    /// What it is handed beside the eye's words (pxl-engine
    /// `light::HEAD_WORDS`).
    light_head: u32,
    light_cast: bool,
    /// Why the map cast this frame, a bit each in LIGHT_MOVED's order
    /// (span.json); 0 where it did not.
    light_moved: u32,
    /// The world's columns that changed this frame, units from the eye (pxl-engine
    /// `light::column_of`): the cast sets again only the tiles whose rays may meet them
    /// (`Light::reaches`), where the epoch goes on.
    light_columns: Vec<[f32; 4]>,
    /// Whether this cast's columns set tiles again (`light_slide_cs`).
    light_reached: bool,
    /// Whether the next cast marks where the eye's rays met the world
    /// (`light_mark_cs`): held until one does; and whether the marks of the
    /// frame the eye stands in, its pixels' footprints, are owed, as the
    /// marks while it moved took none (`light::HEAD_FOOTPRINT`).
    light_world: bool,
    light_footprint_owed: bool,
    refold: bool,
    /// Its group 0, and the buffers it binds (`cast_group`): made again
    /// exactly when one of them is another.
    group: u32,
    bindings: [u32; 11],
    eye: u32,
    pub(super) clipmap: u32,
    shards: u32,
    /// Shards the buffer holds.
    shard_room: usize,
    /// The drawn pixels it casts, a ray each: the world surface's.
    size: [u32; 2],
    /// The overwrites' child pointer lane then their data lane, each `overwrite_room`
    /// words, the lanes' own room; the revision it holds, none at first.
    overwrites: u32,
    overwrite_room: usize,
    overwrite_revision: Option<u32>,
    generator: Generator,
    /// What the last cast read (`Read`) but the jitter and the grid, and
    /// whether this frame's is the same: the samples and the decor generated
    /// over them stand, and only the pixels are cast again where the jitter
    /// moved.
    read: Read,
    /// Where each pixel's ray went through it last (pxl-engine
    /// `eye::JITTER`). It moving alone, the beam rays, the cells' starts and
    /// the samples stand, and the pixels are cast again off them.
    jitter: [u32; 2],
    /// Whether the samples' raster is drawn through its gate, in the claimed
    /// tiles alone: where only the kernel's bounds moved.
    gated: bool,
    /// The samples' hits handed to the rasterizer (ray/raster.rs), whose
    /// visible sample at each pixel the pixels read (`cast_cs`), over a group
    /// of their own (`px_group`): the eye, the rays, the beam's words, the
    /// node cache, the visibility buffer and the claimed tiles (`px_bound`),
    /// made again exactly when one of them is another.
    raster: Raster,
    px_group: u32,
    px_bound: [u32; 7],
    /// The claimed tiles, listed ahead of the pixels in their pass, and the words their
    /// dispatch takes (cast_tiles.wgsl), made again with the tiles (`px_bound`).
    cast_tiles_built: Pipeline,
    cast_tiles: u32,
    cast_tiles_group: u32,
    /// What the light's cast and the eye's recast, counted where the setting
    /// asks (ray/count.rs).
    counter: Counter,
}

/// What a cast's head handed the node cache's forgetting (pxl-engine
/// `node_cache::HEAD_WORDS`): whether all of it, the entries that moved, the
/// boxes and the child groups written; put back where its frame is dropped.
struct ForgetHead {
    all: bool,
    moved: [u32; node_cache::MOVED_WORDS as usize],
    moved_bricks: [[i32; 4]; clipmap::EYE_CLIP_LEVELS as usize],
    boxes: [[i32; 4]; overwrite::BOXES as usize],
    holding: usize,
    written_groups: [u32; node_cache::WRITTEN_CAP as usize],
    writes: usize,
}

/// Child group `first` of what is placed noted written among the `writes` of
/// `written`, once; whether it found room.
fn note(written: &mut [u32], writes: &mut usize, first: u32) -> bool {
    if written[..*writes].contains(&first) {
        return true;
    }
    if *writes == written.len() {
        return false;
    }
    written[*writes] = first;
    *writes += 1;
    true
}

/// The binding of a pass's head (pxl-engine `light::HEAD_SIZE`): the light's
/// map's, or the eye's, which names none.
pub(super) const HEAD_BINDING: u32 = 11;

/// `sample_hit_cs`'s and `beam_hit_cs`'s (sample_hit.wgsl, beam_hit.wgsl): the eye, the
/// beam's words, the node cache and what they leave.
const SAMPLE_HIT_GROUP: [u32; 12] = [
    0, COMPUTE, UNIFORM, 8, COMPUTE, STORAGE_RW, 9, COMPUTE, STORAGE, 10, COMPUTE, STORAGE_RW,
];
/// `SAMPLE_HIT_GROUP` with the light's head: `sample_hit_cs`'s and `beam_hit_cs`'s for the
/// light's map.
const LIGHT_SAMPLE_HIT_GROUP: [u32; 15] = [
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
/// `sample_hit_cs`'s and `sample_cs`'s (sample_hit.wgsl, sample.wgsl).
const SAMPLE_WORKGROUP: [u32; 3] = [beam::SAMPLE_SIDE, beam::SAMPLE_SIDE, 1];
/// `beam_hit_cs`'s and `beam_cs`'s, either observer's (beam_hit.wgsl, beam.wgsl).
const BEAM_WORKGROUP: [u32; 3] = [beam::CORNER_SIDE, beam::CORNER_SIDE, 1];
/// The walks that ask the world (ask.wgsl): either observer's beam rays (`beam_cs`) and
/// samples (`sample_cs`): its world's, its words, the node cache it writes,
/// its head.
const ASK_GROUP: [u32; 30] = [
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
    floor::holds(&[&ASK_GROUP]),
    "the asking walks' group binds past WebGPU's floor"
);
/// The light's slots' (`light_cs`): the list, the map it writes, the head, the visibility
/// it reads and the missed slots' dispatch.
const LIGHT_GROUP: [u32; 18] = [
    0,
    COMPUTE,
    UNIFORM,
    8,
    COMPUTE,
    STORAGE_RW,
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
const MISS_HIT_GROUP: [u32; 15] = [
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
/// `cast_cs`'s (cast.wgsl): the eye, the rays, the beam's words, the node cache it lists the
/// marked pixels in, the visibility buffer, the walk again's words and the claimed tiles' list
/// (`px_group`).
const CAST_GROUP: [u32; 21] = [
    0,
    COMPUTE,
    UNIFORM,
    3,
    COMPUTE,
    STORAGE_TEXTURE_RW,
    8,
    COMPUTE,
    STORAGE,
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
const CAST_TILES_GROUP: [u32; 9] = [
    0, COMPUTE, STORAGE_RW, 30, COMPUTE, STORAGE, 31, COMPUTE, UNIFORM,
];
const CAST_WORKGROUP: [u32; 3] = [ray::CAST_SIDE, ray::CAST_SIDE, 1];
/// A tile's workgroups of `cast_cs`, across and down.
const TILE_CASTS: u32 = screen::TILE / ray::CAST_SIDE;
const _: () = assert!(
    screen::TILE.is_multiple_of(ray::CAST_SIDE),
    "a tile is a whole number of the cast's workgroups"
);
/// The claimed tiles' list as a frame's cast starts: `cast_cs`'s dispatch, no tile yet, each
/// one's workgroups across and down, and none counted (cast_tiles.wgsl).
const CAST_TILES_START: [u32; 4] = [0, TILE_CASTS, TILE_CASTS, 0];
/// The pixels `miss_hit_cs` left: `miss_list_cs`'s dispatch and their count
/// (`ray::LEFT_EMPTY`), then the hits it kept, as `node_cache::HITS_ARGS` lays them.
const MISS_LEFT_EMPTY: [u32; 8] = {
    let l = ray::LEFT_EMPTY;
    [l[0], l[1], l[2], l[3], 0, 1, 1, 0]
};
const LIGHT_MISS_HIT_GROUP: [u32; 18] = [
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
    13,
    COMPUTE,
    STORAGE_RW,
];
/// Its world's, the node cache it writes, the map and the list it reads.
const LIGHT_MISS_GROUP: [u32; 33] = [
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
    10,
    COMPUTE,
    STORAGE_TEXTURE_RW,
    HEAD_BINDING,
    COMPUTE,
    UNIFORM,
];
const _: () = assert!(
    floor::holds(&[&LIGHT_MISS_GROUP]),
    "the light's missed slots' group binds past WebGPU's floor"
);
/// The light's receivers' passes' groups: the eye, the head and the list,
/// and the rays the eye's pixels met the world by (light_mark.wgsl), the decor
/// placed (light_mark_draws.wgsl) or the passes' args (light_propagate.wgsl).
const LIGHT_MARK_GROUP: [u32; 12] = [
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
];
const LIGHT_MARK_DRAWS_GROUP: [u32; 18] = [
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
];
const LIGHT_PROPAGATE_GROUP: [u32; 12] = [
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
/// `light_slide_cs`'s (light_slide.wgsl): the eye, the head and the list.
const LIGHT_SLIDE_GROUP: [u32; 9] = [
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
/// The cast's world's group, over which `cell_cs`, `light_cell_cs`, `fold_cs` and
/// `forget_cs` run: the eye, the clip levels' table and the bricks' levels the fold reads
/// and writes, the beam rays' words the cells' starts go into, the node cache, and the
/// head the passes both observers take read their observer by. The shadow map's passes of
/// those bind it over the light's buffers (`light_groups`).
const WORLD_GROUP: [u32; 18] = [
    0,
    COMPUTE,
    UNIFORM,
    1,
    COMPUTE,
    STORAGE_RW,
    2,
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
/// The missed pixels' walks again (miss.wgsl): their world's, the rays, their words and the
/// node cache they write.
const MISS_GROUP: [u32; 30] = [
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
const _: () = assert!(
    floor::holds(&[&WORLD_GROUP]) && floor::holds(&[&MISS_GROUP]),
    "the cast's world's group or the missed pixels' binds past WebGPU's floor"
);

/// The missed pixels' hits' passes' groups (hit_lead.wgsl, hit_faces.wgsl,
/// `hit_groups`): the eye, the rays, the beam's words, the node cache's head, and
/// the walk again's words the leaders are listed in.
const HIT_LEAD_GROUP: [u32; 15] = [
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
    14,
    COMPUTE,
    STORAGE_RW,
];
const HIT_FACES_GROUP: [u32; 12] = [
    0,
    COMPUTE,
    UNIFORM,
    3,
    COMPUTE,
    STORAGE_TEXTURE_RW,
    8,
    COMPUTE,
    STORAGE,
    9,
    COMPUTE,
    STORAGE,
];
const _: () = assert!(
    floor::holds(&[&HIT_LEAD_GROUP]) && floor::holds(&[&HIT_FACES_GROUP]),
    "the hits' group binds past WebGPU's floor"
);

// Each compute group against what the shaders built over it read, as their declarations take
// it (build.rs `BINDS`).
const _: () = assert!(
    binds(&[COMPUTE_CAST], &CAST_GROUP)
        && binds(&[COMPUTE_CAST_TILES], &CAST_TILES_GROUP)
        && binds(
            &[
                COMPUTE_BEAM,
                COMPUTE_SAMPLE,
                COMPUTE_LIGHT_BEAM,
                COMPUTE_LIGHT_SAMPLE
            ],
            &ASK_GROUP
        )
        && binds(
            &[
                COMPUTE_CELL,
                COMPUTE_LIGHT_CELL,
                COMPUTE_FOLD,
                COMPUTE_FORGET
            ],
            &WORLD_GROUP
        )
        && binds(&[COMPUTE_SAMPLE_HIT, COMPUTE_BEAM_HIT], &SAMPLE_HIT_GROUP)
        && binds(
            &[COMPUTE_LIGHT_SAMPLE_HIT, COMPUTE_LIGHT_BEAM_HIT],
            &LIGHT_SAMPLE_HIT_GROUP
        )
        && binds(&[COMPUTE_MISS_HIT], &MISS_HIT_GROUP)
        && binds(&[COMPUTE_MISS, COMPUTE_MISS_LIST], &MISS_GROUP)
        && binds(&[COMPUTE_HIT_LEAD], &HIT_LEAD_GROUP)
        && binds(&[COMPUTE_HIT_FACES], &HIT_FACES_GROUP)
        && binds(&[COMPUTE_LIGHT], &LIGHT_GROUP)
        && binds(&[COMPUTE_LIGHT_MISS], &LIGHT_MISS_GROUP)
        && binds(&[COMPUTE_LIGHT_MISS_HIT], &LIGHT_MISS_HIT_GROUP)
        && binds(&[COMPUTE_LIGHT_MARK], &LIGHT_MARK_GROUP)
        && binds(&[COMPUTE_LIGHT_MARK_DRAWS], &LIGHT_MARK_DRAWS_GROUP)
        && binds(&[COMPUTE_LIGHT_PROPAGATE], &LIGHT_PROPAGATE_GROUP)
        && binds(&[COMPUTE_LIGHT_SLIDE], &LIGHT_SLIDE_GROUP),
    "a cast's compute group lays out other than its shaders read (record.rs `binds`)"
);

impl RaysPass {
    /// The cast pass's pipelines and buffers, and the ground's pipelines
    /// (`GroundPipelines`), made once with the renderer, before its first
    /// frame (gpu/pipeline.rs): no frame names a pipeline until every one
    /// is made.
    pub fn make(&mut self, g: &Gpu, pipelines: &mut Pipelines) {
        if self.cast.is_some() {
            return;
        }
        self.ground_built = Some(GroundPipelines::new(g, pipelines));
        // One group for every kernel over the cast's world but the pixels'.
        let group = &WORLD_GROUP;
        let pipeline = pipelines.build_compute(g, COMPUTE_CAST, &CAST_GROUP);
        let cast_tiles_built = pipelines.build_compute(g, COMPUTE_CAST_TILES, &CAST_TILES_GROUP);
        let beam_built = pipelines.build_compute(g, COMPUTE_BEAM, &ASK_GROUP);
        let cells_built = pipelines.build_compute(g, COMPUTE_CELL, group);
        let sample_built = pipelines.build_compute(g, COMPUTE_SAMPLE, &ASK_GROUP);
        let sample_hit_built = pipelines.build_compute(g, COMPUTE_SAMPLE_HIT, &SAMPLE_HIT_GROUP);
        let miss_hit_built = pipelines.build_compute(g, COMPUTE_MISS_HIT, &MISS_HIT_GROUP);
        let beam_hit_built = pipelines.build_compute(g, COMPUTE_BEAM_HIT, &SAMPLE_HIT_GROUP);
        let light_beam_built = pipelines.build_compute(g, COMPUTE_LIGHT_BEAM, &ASK_GROUP);
        let light_cells_built = pipelines.build_compute(g, COMPUTE_LIGHT_CELL, group);
        let light_sample_built = pipelines.build_compute(g, COMPUTE_LIGHT_SAMPLE, &ASK_GROUP);
        let light_sample_hit_built =
            pipelines.build_compute(g, COMPUTE_LIGHT_SAMPLE_HIT, &LIGHT_SAMPLE_HIT_GROUP);
        let light_beam_hit_built =
            pipelines.build_compute(g, COMPUTE_LIGHT_BEAM_HIT, &LIGHT_SAMPLE_HIT_GROUP);
        let fold_built = pipelines.build_compute(g, COMPUTE_FOLD, group);
        let miss_built = pipelines.build_compute(g, COMPUTE_MISS, &MISS_GROUP);
        let miss_list_built = pipelines.build_compute(g, COMPUTE_MISS_LIST, &MISS_GROUP);
        let hit_lead_built = pipelines.build_compute(g, COMPUTE_HIT_LEAD, &HIT_LEAD_GROUP);
        let hit_faces_built = pipelines.build_compute(g, COMPUTE_HIT_FACES, &HIT_FACES_GROUP);
        let forget_built = pipelines.build_compute(g, COMPUTE_FORGET, group);
        let light_built = pipelines.build_compute(g, COMPUTE_LIGHT, &LIGHT_GROUP);
        let light_miss_built = pipelines.build_compute(g, COMPUTE_LIGHT_MISS, &LIGHT_MISS_GROUP);
        let light_miss_hit_built =
            pipelines.build_compute(g, COMPUTE_LIGHT_MISS_HIT, &LIGHT_MISS_HIT_GROUP);
        let light_mark_built = pipelines.build_compute(g, COMPUTE_LIGHT_MARK, &LIGHT_MARK_GROUP);
        let light_mark_draws_built =
            pipelines.build_compute(g, COMPUTE_LIGHT_MARK_DRAWS, &LIGHT_MARK_DRAWS_GROUP);
        let light_propagate_built =
            pipelines.build_compute(g, COMPUTE_LIGHT_PROPAGATE, &LIGHT_PROPAGATE_GROUP);
        let light_slide_built = pipelines.build_compute(g, COMPUTE_LIGHT_SLIDE, &LIGHT_SLIDE_GROUP);
        let raster = Raster::new(g, pipelines);
        let counter = Counter::new(g);
        let buf = |size: u32, usage: u32| g.buf(size, usage);
        let storage = buffer_usage::STORAGE | buffer_usage::COPY_DST;
        let c = CastBuffers {
            pipeline,
            cast_tiles_built,
            cast_tiles: 0,
            cast_tiles_group: 0,
            beam_built,
            cells_built,
            sample_built,
            sample_hit_built,
            beam_hit_built,
            miss_hit_built,
            light_beam_built,
            light_cells_built,
            light_sample_built,
            light_sample_hit_built,
            light_beam_hit_built,
            beams: 0,
            // Never written: a buffer is made zeroed.
            eye_head: buf(word_bytes(light::HEAD_WORDS), buffer_usage::UNIFORM),
            grid: beam::Grid::of_words([0.0; 2], [0; eye::GRID_WORDS]),
            beam_room: 0,
            miss_built,
            miss_list_built,
            // The walk again's words, and the hits' faces' args past them.
            marks: buf(
                word_bytes((ray::MISS_ARGS_ROOM + raster::ARGS_WORDS) as usize),
                storage | buffer_usage::INDIRECT | buffer_usage::COPY_SRC,
            ),
            hit_lead_built,
            hit_faces_built,
            hit_groups: [0; 2],
            hit_bindings: [0; 5],
            forget_built,
            // Indirect too: the hits' passes are dispatched off its head.
            node_cache: buf(
                word_bytes(node_cache::WORDS as usize),
                storage | buffer_usage::COPY_SRC | buffer_usage::INDIRECT,
            ),
            frame: 0,
            forget: true,
            moved: [0; node_cache::MOVED_WORDS as usize],
            moved_bricks: [node_cache::NO_BRICKS; clipmap::EYE_CLIP_LEVELS as usize],
            boxes: [[0; 4]; overwrite::BOXES as usize],
            holding: 0,
            written_groups: [0; node_cache::WRITTEN_CAP as usize],
            writes: 0,
            sent_forget: None,
            entries: Vec::new(),
            forgets: node_cache::FORGETS_NONE,
            cuts: [0; eye::CUTS],
            fold_built,
            light_built,
            light_miss_built,
            light_miss_hit_built,
            light_mark_built,
            light_mark_draws_built,
            light_propagate_built,
            light_slide_built,
            light_groups: [0; 12],
            light_bound: [0; 12],
            light_none: buf(word_bytes(2 * generation::drawn::REGIONS as usize), storage),
            light_number: 0,
            light_epoch: 0,
            light_ready: false,
            light_list: 0,
            light_misses: buf(
                word_bytes(ray::MISS_ARGS_WORDS),
                storage | buffer_usage::INDIRECT | buffer_usage::COPY_SRC,
            ),
            light_left: buf(
                word_bytes(ray::LEFT_EMPTY.len()),
                storage | buffer_usage::INDIRECT | buffer_usage::COPY_DST | buffer_usage::COPY_SRC,
            ),
            light_args: 0,
            light_content: Read::default(),
            light_slid: false,
            light_corner: [0; 2],
            light_corner_was: [0; 2],
            light_reads_was: buf(
                word_bytes((light::tiles(shadow::SIZE) * light::tiles(shadow::SIZE)) as usize),
                buffer_usage::COPY_SRC | buffer_usage::COPY_DST,
            ),
            light_receivers: Read::default(),
            light_head: buf(
                word_bytes(light::HEAD_WORDS),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
            light_cast: false,
            light_moved: 0,
            light_columns: Vec::with_capacity(light::COLUMNS + 1),
            light_reached: false,
            light_world: false,
            light_footprint_owed: false,
            refold: false,
            group: 0,
            ask_group: 0,
            miss_group: 0,
            sample_hit_group: 0,
            miss_hit_group: 0,
            beam_hit_group: 0,
            beam_left: g.buf(
                word_bytes(ray::LEFT_EMPTY.len()),
                storage | buffer_usage::INDIRECT | buffer_usage::COPY_SRC,
            ),
            miss_left: g.buf(
                word_bytes(MISS_LEFT_EMPTY.len()),
                storage | buffer_usage::INDIRECT | buffer_usage::COPY_SRC,
            ),
            sample_left: g.buf(
                word_bytes(ray::LEFT_EMPTY.len()),
                storage | buffer_usage::INDIRECT | buffer_usage::COPY_SRC,
            ),
            light_sample_left: g.buf(
                word_bytes(ray::LEFT_EMPTY.len()),
                storage | buffer_usage::INDIRECT | buffer_usage::COPY_SRC,
            ),
            light_beam_left: g.buf(
                word_bytes(ray::LEFT_EMPTY.len()),
                storage | buffer_usage::INDIRECT | buffer_usage::COPY_SRC,
            ),
            bindings: [0; 11],
            eye: buf(
                word_bytes(eye::WORDS),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
            clipmap: buf(
                word_bytes((4 * clipmap::JOINED_AT + clipmap::EYE_JOINED_WORDS) as usize),
                storage | buffer_usage::COPY_SRC,
            ),
            shards: 0,
            shard_room: 0,
            size: [0; 2],
            overwrites: 0,
            overwrite_room: 0,
            overwrite_revision: None,
            generator: Generator::new(g, pipelines),
            read: Read::default(),
            jitter: [0; 2],
            gated: false,
            raster,
            px_group: 0,
            px_bound: [0; 7],
            counter,
        };
        self.cast = Some(c);
    }

    /// The rays for this frame into the world surface's (`rays`, its view,
    /// and its drawn pixels), and the world's depth from the light into `light`'s
    /// view: the GPU's cast where an eye casts, its eye handed over first;
    /// its samples' raster through its gate where the frame is `gated`;
    /// `frame` the renderer's number this frame takes, which its counts are
    /// named by (count.rs `Frame`).
    #[expect(
        clippy::too_many_arguments,
        reason = "the frame's values, each its own"
    )]
    pub fn prepare(
        &mut self,
        g: &Gpu,
        cast: Option<Cast>,
        meshes: &[u32; GROUPS as usize],
        rays: (u32, [u32; 2]),
        light_map: (u32, u32, light::Starts),
        gated: bool,
        frame: f64,
    ) {
        // No cast, no world: no ray cast, none drawn and no decor generated.
        // What the cast reads stands for the next: the ground it binds is
        // made once.
        let (Some(cast), Some((ground, palette_indices, pool))) = (
            cast,
            self.ground
                .as_ref()
                .map(|gb| (gb.ground, gb.palette_indices, gb.pool)),
        ) else {
            self.kind = FrameKind::plain;
            if let Some(c) = self.cast.as_mut() {
                c.generator.off();
                c.read.forget();
                c.light_content.forget();
                c.light_cast = false;
                c.light_moved = 0;
            }
            return;
        };
        let c = self.cast.as_mut().or_bug(Errno::ENOTRECOVERABLE);
        // The cast reads exactly this, all of it here: a field the frame hands
        // the cast fails to destructure until it is read. The overwrites'
        // contents are their revision's. The decor it generates reads its own
        // (`Generator::prepare`), so a change to decor alone casts no ray again.
        let Cast {
            eye,
            clipmap,
            shards,
            overwrites:
                Overwrites {
                    first_child: _,
                    data: _,
                    room,
                    revision,
                    since: _,
                    written_groups: _,
                },
            decor,
            // The light's alone, which reads them with the look (below).
            actors: _,
            ray_counts,
        } = &cast;
        let ray_counts = *ray_counts;
        let r = &mut c.read;
        r.begin();
        // The words a cast is from: not the light's, its cast's alone
        // (below), the jitter's or the grid's (pxl-engine `eye::GRID`).
        r.read(&eye[..eye::LIGHT]);
        let clipmap_moved = r.read(clipmap);
        let shards_moved = r.read(shards);
        // A surface made again holds no rays.
        r.read(&[*room as u32, *revision, rays.0]);
        // And the ground's bricks, written this frame or not.
        let fresh = self.ground.as_ref().map_or(0, |gb| gb.fresh_len);
        let stands = r.end() && fresh == 0 && c.group != 0;
        let jitter = [eye[eye::JITTER], eye[eye::JITTER + 1]];
        let jittered = jitter != c.jitter;
        c.jitter = jitter;
        // A cast lays the grid the eye's words hold (pxl-engine `eye::GRID`);
        // the frames that stand on its samples keep it, its dispatches'
        // sizes and the room for its corners' lines, its cells' starts and
        // its samples' hits.
        self.kind = match (stands, jittered) {
            (true, false) => FrameKind::standing,
            (true, true) => FrameKind::px,
            (false, _) => {
                let tan = [eye[eye::TAN], eye[eye::TAN + 1]].map(f32::from_bits);
                c.grid = beam::Grid::of_words(
                    tan,
                    eye[eye::GRID..eye::GRID + eye::GRID_WORDS]
                        .try_into()
                        .or_bug(Errno::ENOTRECOVERABLE),
                );
                FrameKind::cast
            }
        };
        let (kind, casts) = (self.kind, casts_px(self.kind));
        xpute_kit::ensure!(
            cast.clipmap.len() == (4 * clipmap::JOINED_AT) as usize,
            ENOTRECOVERABLE,
            cast.clipmap.len()
        );
        if clipmap_moved {
            g.write_buf(c.clipmap, 0, bytes(cast.clipmap));
            c.refold = true;
        }
        c.size = rays.1;
        // The room for the grid's words, made again only where they outgrow
        // it.
        let need = c.grid.size();
        if need > c.beam_room {
            if c.beams != 0 {
                release(c.beams);
            }
            let size = word_bytes(need as usize);
            xpute_kit::ensure!(floor::fits(size as u64, true), ENOTRECOVERABLE, need);
            (c.beams, c.beam_room) = (
                g.buf(size, buffer_usage::STORAGE | buffer_usage::COPY_DST),
                need,
            );
        }
        let shards_grow = cast.shards.len() > c.shard_room;
        if shards_grow {
            if c.shards != 0 {
                release(c.shards);
            }
            c.shard_room = cast.shards.len();
            c.shards = g.buf(
                word_bytes(4 * c.shard_room),
                buffer_usage::STORAGE | buffer_usage::COPY_DST,
            );
        }
        if shards_moved || shards_grow {
            // One record however large.
            g.upload_buf(c.shards, 0, bytes(cast.shards));
        }
        let o = &cast.overwrites;
        let overwrites_grow = o.room != c.overwrite_room;
        if overwrites_grow {
            if c.overwrites != 0 {
                release(c.overwrites);
            }
            c.overwrite_room = o.room;
            c.overwrites = g.buf(
                word_bytes(2 * o.room),
                buffer_usage::STORAGE | buffer_usage::COPY_DST,
            );
            c.overwrite_revision = None;
        }
        let overwrites_moved = c.overwrite_revision != Some(o.revision);
        if overwrites_moved {
            // Held at `since`: the child groups written after it alone, a record
            // each, where the stream has room for them all; else both lanes
            // whole, two records however large.
            fn child_group(lane: &[u32], first: u32) -> &[u32] {
                &lane[first as usize..(first + overwrite::CHILDREN) as usize]
            }
            // The child groups written since the revision the node cache was asked
            // at, which the nodes handed one of their slots read again; past
            // the room, or where revisions went by unseen, all of it.
            if c.overwrite_revision == Some(o.since) {
                for &first in o.written_groups {
                    c.forget |= !note(&mut c.written_groups, &mut c.writes, first);
                }
            } else {
                c.forget = true;
            }
            let listed = |lane: &[u32]| {
                o.written_groups
                    .iter()
                    .map(|&f| write_buffer_bytes(bytes(child_group(lane, f))))
                    .sum::<usize>()
            };
            if c.overwrite_revision == Some(o.since)
                && api::room(listed(o.first_child) + listed(o.data))
            {
                for &first in o.written_groups {
                    g.write_buf(
                        c.overwrites,
                        word_bytes(first as usize),
                        bytes(child_group(o.first_child, first)),
                    );
                    g.write_buf(
                        c.overwrites,
                        word_bytes(o.room + first as usize),
                        bytes(child_group(o.data, first)),
                    );
                }
            } else {
                g.upload_buf(c.overwrites, 0, bytes(o.first_child));
                g.upload_buf(c.overwrites, word_bytes(o.room), bytes(o.data));
            }
            c.overwrite_revision = Some(o.revision);
        }
        let cuts: [u32; eye::CUTS] = cast.eye[eye::CARVE..eye::CARVE + eye::CUTS]
            .try_into()
            .or_bug(Errno::ENOTRECOVERABLE);
        // What is placed and a preview's cuts forget the nodes over the boxes
        // they changed, as decor's generation is asked over them (gpu/generator.rs):
        // every write of what is placed names its box (`DecorFrame::edits`);
        // one that names none, or the lanes made again, forgets all.
        let edits = decor.edits;
        // The world's columns that changed, units from the eye, as the light's cast reads
        // them (`light_columns`).
        let origin = [0, 1, 2].map(|k| eye[eye::ORIGIN + k] as i32);
        let columns = &mut c.light_columns;
        columns.clear();
        let leaf = |b: [i32; 4]| light::column_of(origin, LEAF_CELL, [b[0], b[1]], [b[2], b[3]]);
        for &b in edits {
            hold(&mut c.boxes, &mut c.holding, b);
            columns.push(leaf(b));
        }
        cut_boxes(&c.cuts, &cuts, |b| {
            hold(&mut c.boxes, &mut c.holding, b);
            columns.push(leaf(b));
        });
        // What the node cache kept is forgotten where the service area
        // or what is placed moved past the boxes; so it is once its frames
        // would outrun a key's tag.
        c.forget |= shards_moved
            || shards_grow
            || overwrites_grow
            || (overwrites_moved && edits.is_empty())
            || c.frame >= node_cache::FRAMES;
        let cuts_moved = cuts != c.cuts;
        c.cuts = cuts;
        // The eye's clip levels' entries that hold another brick, or whose
        // brick the turn handed: what is over them is asked again.
        let eye_rows = (clipmap::EYE_CLIP_LEVELS * clipmap::ENTRIES) as usize;
        // Of the clip levels, a walk's hit reads the cells of the bricks
        // they hold present, each at its own place, every other read a
        // bound over them that holds whichever of them are: so they moved
        // where an entry holds a brick present it did not, or no longer
        // holds present the brick it holds. A clip level following its
        // center a brick moves none of it: the brick it lets go and the one
        // it takes, not present yet, each lie past what its zoom observes
        // about the eye (pxl-engine `clipmap`), and are read off the
        // coarser one alike.
        let mut present_moved = false;
        if clipmap_moved {
            let rows = &cast.clipmap[..4 * eye_rows];
            for k in 0..eye_rows {
                let row = &rows[4 * k..4 * k + 4];
                let was = c.entries.get(4 * k..4 * k + 4);
                let entry = |r: &[i32]| clipmap::Entry::of([r[0], r[1], r[2], r[3]]);
                if was.is_none_or(|w| entry(row).moved_from(entry(w))) {
                    c.moved[k / 32] |= 1 << (k % 32);
                    let level = &mut c.moved_bricks[k / clipmap::ENTRIES as usize];
                    for q in [Some(entry(row).brick), was.map(|w| entry(w).brick)]
                        .into_iter()
                        .flatten()
                        .filter(|&q| q != [i32::MIN; 2])
                    {
                        *level = node_cache::bricks_joined(*level, [q[0], q[1], q[0], q[1]]);
                    }
                }
                let held = |r: &[i32]| entry(r).brick();
                let (now, was) = (held(row), was.map(held));
                // The bricks a query reads otherwise: the one now held present where it was
                // not, and the one held present before where it no longer is.
                let (taken, gone) = match was {
                    Some((brick, present)) => (
                        now.1 && (!present || brick != now.0),
                        present && !(now.1 && brick == now.0),
                    ),
                    None => (now.1, false),
                };
                let cell = clipmap::FIRST_CELL + (k / clipmap::ENTRIES as usize) as u32;
                let brick = |q: [i32; 2]| {
                    let s = clipmap::BRICK_SHIFT;
                    light::column_of(
                        origin,
                        cell,
                        [q[0] << s, q[1] << s],
                        [(q[0] + 1) << s, (q[1] + 1) << s],
                    )
                };
                if taken {
                    c.light_columns.push(brick(now.0));
                }
                if let (true, Some((q, _))) = (gone, was) {
                    c.light_columns.push(brick(q));
                }
                present_moved |= match was {
                    Some((brick, present)) => {
                        now.1 && (!present || brick != now.0) || present && !now.1 && brick == now.0
                    }
                    None => now.1,
                };
            }
            c.entries.clear();
            c.entries.extend_from_slice(rows);
        }
        // What the world's query reads moved: the bricks the clip levels
        // hold present, the bricks the turn handed (their cells, palette
        // indices and ways), the service area, what is placed, a preview's
        // cuts.
        let world_moved = present_moved
            || fresh != 0
            || shards_moved
            || shards_grow
            || overwrites_moved
            || cuts_moved;
        // A brick handed moves the eye's entry that wants it, whichever
        // clip level's slot it is filled into.
        if let Some(gb) = self.ground.as_ref() {
            for f in &gb.handed[..fresh as usize] {
                let (cell, q) = (
                    f[FILL_CELL as usize],
                    clipmap::brick_of_first([f[FILL_FIRST as usize], f[FILL_FIRST as usize + 1]]),
                );
                let s = clipmap::BRICK_SHIFT;
                c.light_columns.push(light::column_of(
                    origin,
                    cell,
                    [q[0] << s, q[1] << s],
                    [(q[0] + 1) << s, (q[1] + 1) << s],
                ));
                let w = cell.wrapping_sub(clipmap::FIRST_CELL);
                let k = (w * clipmap::ENTRIES + clipmap::entry(q)) as usize;
                let row = &cast.clipmap[4 * k..4 * k + 4];
                if w < clipmap::EYE_CLIP_LEVELS && [row[0], row[1]] == q {
                    c.moved[k / 32] |= 1 << (k % 32);
                    let level = &mut c.moved_bricks[w as usize];
                    *level = node_cache::bricks_joined(*level, [q[0], q[1], q[0], q[1]]);
                }
            }
        }
        // Decor generated first: where it stands is what the light's cast reads
        // of it, and its buffers what the light's groups bind.
        let world = [c.eye, c.clipmap, ground, c.shards, c.overwrites, pool];
        let generation = Generation {
            clipmap: cast.clipmap,
            cuts: cast.eye[eye::CARVE..eye::CARVE + eye::CUTS]
                .try_into()
                .or_bug(Errno::ENOTRECOVERABLE),
            fresh: self
                .ground
                .as_ref()
                .map_or(&[][..], |gb| &gb.handed[..fresh as usize]),
            eye: &cast.eye,
        };
        c.generator
            .prepare(g, cast.decor, generation, meshes, world);
        // The shadow map is cast, every slot a receiver reads, as the eye's
        // rays are (pxl-engine `light`), where what it reads moved: the
        // light's frame, the eye's zooms, which every band is held to, or
        // the world under it; where the box slid; and where what says which
        // tiles are read moved, the eye's place and look, the actors' places
        // and decor placed again (`light_mark_cs`, `light_mark_draws_cs`). A
        // standing frame casts none. Where only the latter two moved, a tile
        // cast since anything else last did stands where the box kept it,
        // and only the tiles read anew are cast (`light::HEAD_EPOCH`). None
        // where no shadow is cast, nor while its cast is not made
        // (`light_made`), its read let go until then.
        let shadows = cast.eye[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS]
            .iter()
            .any(|&w| w != 0);
        // All it reads but the look, then the look.
        let r = &mut c.light_content;
        r.begin();
        r.read(&[light_map.1, u32::from(shadows)]);
        r.read(&cast.eye[eye::RANGE..eye::RANGE + 2]);
        // The world under it moved whole where the node cache forgets it whole, or where no
        // column or more than a cast is handed says what changed; else only the tiles its
        // columns reach are cast again.
        let n = c.light_columns.len();
        let world_whole = world_moved && (c.forget || n == 0 || n > light::COLUMNS);
        r.mark(world_whole);
        let held = r.end();
        let world_columns = world_moved && !world_whole;
        // The box, a tile at a time about the eye: what a tile holds moves
        // with its corner, kept where the slide kept its band
        // (`light_slide_cs`).
        let corner = light::CORNER.map(|k| cast.eye[eye::LIGHT + k] as i32);
        let slid = corner != c.light_corner;
        // The eye's place and look, where the actors stand and decor
        // compacted again: which tiles are read, not what a tile holds. Not
        // the jitter: a pixel's marks hold its footprint (`light_mark_cs`).
        let l = &mut c.light_receivers;
        l.begin();
        l.read(&cast.eye[eye::ORIGIN..eye::ORIGIN + 3]);
        l.read(&cast.eye[eye::FORWARD..eye::TAN + 2]);
        l.read(cast.actors);
        l.mark(c.generator.counts());
        // The first frame the eye stands in after it moved marks again, with
        // the footprints the jitter's frames after it stand on.
        let footprint_owed = c.light_footprint_owed && kind != FrameKind::cast;
        let receivers_held = l.end();
        let moved = LightMoved::ALL
            .into_iter()
            .filter(|m| match m {
                LightMoved::content => !held,
                LightMoved::slide => slid,
                LightMoved::receivers => !receivers_held,
                LightMoved::footprint => footprint_owed,
                LightMoved::world => world_columns,
            })
            .fold(0, |bits, m| bits | 1 << m as u32);
        c.light_slid = held && slid;
        c.light_reached = held && world_columns;
        if !c.light_ready {
            c.light_content.forget();
        }
        c.light_cast = shadows && moved != 0 && c.light_ready;
        c.light_moved = if c.light_cast { moved } else { 0 };
        // The world's receivers stand where the eye's rays met it: marked
        // again by the next cast after the rays were cast anew, or where an
        // epoch begins. Else what they stood on stands, marked in the epoch
        // already, so none of it is the cast's to cast anew
        // (`light_propagate_cs`); the jitter's casts move a ray within its
        // footprint, marked already where the eye stood when they were.
        c.light_world |= kind == FrameKind::cast || !held || footprint_owed;
        if c.light_cast {
            // The head (pxl-engine `light::HEAD_WORDS`).
            let mut head = [0u32; light::HEAD_WORDS];
            c.light_number = c.light_number.wrapping_add(1).max(1);
            if !held {
                c.light_epoch = c.light_number;
            }
            head[light::HEAD_CAST] = c.light_number;
            head[light::HEAD_EPOCH] = c.light_epoch;
            head[light::HEAD_COUNTED] = u32::from(ray_counts);
            for (k, v) in light::head_layout(shadow::SIZE) {
                head[k] = v;
            }
            c.light_corner_was = core::mem::replace(&mut c.light_corner, corner);
            head[light::HEAD_CORNER_WAS] = c.light_corner_was[0] as u32;
            head[light::HEAD_CORNER_WAS + 1] = c.light_corner_was[1] as u32;
            // The footprints taken where the eye stands; while it moves, the
            // next frame's rays mark again, and the frame it stands in owes
            // them.
            let footprint = kind != FrameKind::cast;
            head[light::HEAD_FOOTPRINT] = u32::from(footprint);
            if c.light_world {
                c.light_footprint_owed = !footprint;
            }
            let at = 4 * light::STARTS_AT;
            head[at..at + 4 * light::STARTS_VEC4S].copy_from_slice(&light_map.2.words());
            // The actors drawn, receivers as decor is (`light_mark_draws_cs`).
            let at = 4 * light::ACTORS_AT;
            let n = cast.actors.len().min(light::ACTORS);
            head[at] = n as u32;
            for (k, a) in cast.actors[..n].iter().enumerate() {
                let w = at + 4 * (1 + k);
                head[w..w + 3].copy_from_slice(&a.map(f32::to_bits));
            }
            // The world's columns that changed, where the epoch goes on (`light_slide_cs`).
            let at = 4 * light::COLUMNS_AT;
            if c.light_reached {
                head[at] = c.light_columns.len() as u32;
                for (k, col) in c.light_columns.iter().enumerate() {
                    let w = at + 4 * (1 + k);
                    head[w..w + 4].copy_from_slice(&col.map(f32::to_bits));
                }
            }
            g.write_buf(c.light_head, 0, bytes(&head));
            g.write_buf(c.light_misses, 0, bytes(&ray::MISS_ARGS_EMPTY));
            g.write_buf(c.light_args, 0, bytes(&light_args()));
            c.raster.light_prepare(g, &cast.eye, shadow::SIZE);
        }
        // The eye's words with the grid its cast laid (`grid`).
        if casts || c.light_cast {
            let mut words = cast.eye;
            words[eye::GRID..].copy_from_slice(&c.grid.words());
            g.write_buf(c.eye, 0, bytes(&words));
        }
        // The samples' raster through its gate where only the kernel's
        // bounds moved; the pixels' cast takes the tiles claimed alone
        // (`CAST_TILES_START`).
        c.gated = gated && kind == FrameKind::px;
        if casts {
            c.forgets = if c.forget {
                node_cache::FORGETS_ALL
            } else if c.moved.iter().any(|&m| m != 0) || c.holding != 0 || c.writes != 0 {
                node_cache::FORGETS_MOVED
            } else {
                node_cache::FORGETS_NONE
            };
            // The head before the frame's dispatches: what it forgets and over
            // which entries; forgotten whole, its first frame, else the next.
            let mut head = [0u32; node_cache::HEAD_WORDS as usize];
            c.frame = if c.forgets == node_cache::FORGETS_ALL {
                1
            } else {
                c.frame + 1
            };
            head[node_cache::FRAME as usize] = c.frame;
            head[node_cache::FORGETS as usize] = c.forgets;
            let boxes = node_cache::BOXES_AT as usize;
            let bricks = node_cache::MOVED_BRICKS_AT as usize;
            head[node_cache::MOVED_AT as usize..bricks].copy_from_slice(&c.moved);
            for (k, b) in c.moved_bricks.iter().enumerate() {
                let at = bricks + 4 * k;
                head[at..at + 4].copy_from_slice(&b.map(|v| v as u32));
            }
            head[boxes] = c.holding as u32;
            for (k, b) in c.boxes[..c.holding].iter().enumerate() {
                let at = boxes + 1 + 4 * k;
                head[at..at + 4].copy_from_slice(&b.map(|v| v as u32));
            }
            let written = node_cache::WRITTEN_AT as usize;
            head[written] = c.writes as u32;
            head[written + 1..written + 1 + c.writes]
                .copy_from_slice(&c.written_groups[..c.writes]);
            // The hits' dispatch none, a workgroup deep and high.
            head[node_cache::HITS_ARGS as usize + 1] = 1;
            head[node_cache::HITS_ARGS as usize + 2] = 1;
            g.write_buf(c.node_cache, 0, bytes(&head));
            g.write_buf(c.marks, 0, bytes(&ray::MISS_ARGS_EMPTY));
            // Their faces' args none, laid as the boxes' are, their asks
            // where the grid lays them.
            let mut asked = [0u32; raster::ARGS_WORDS as usize];
            asked[raster::ARGS_FACES as usize + 1] = 1;
            asked[raster::ARGS_FACES as usize + 2] = 1;
            asked[raster::ARGS_ASKS as usize] = c.grid.hit_asks_at;
            g.write_buf(
                c.marks,
                word_bytes(ray::MISS_ARGS_ROOM as usize),
                bytes(&asked),
            );
            c.sent_forget = Some(ForgetHead {
                all: c.forgets == node_cache::FORGETS_ALL,
                moved: c.moved,
                moved_bricks: c.moved_bricks,
                boxes: c.boxes,
                holding: c.holding,
                written_groups: c.written_groups,
                writes: c.writes,
            });
            (c.forget, c.moved) = (false, [0; node_cache::MOVED_WORDS as usize]);
            c.moved_bricks = [node_cache::NO_BRICKS; clipmap::EYE_CLIP_LEVELS as usize];
            (c.holding, c.writes) = (0, 0);
        } else {
            c.sent_forget = None;
        }
        let frame = count::Frame {
            number: frame,
            // A full cast's alone, so what a cast asks is not shared out
            // over the frames that cast only the pixels.
            casts: [self.kind == FrameKind::cast, c.light_cast],
            rays: [c.size[0] * c.size[1], shadow::SIZE * shadow::SIZE],
        };
        c.counter.begin(g, ray_counts, c.node_cache, frame);
        let bindings = [
            c.eye,
            c.clipmap,
            ground,
            rays.0,
            palette_indices,
            c.shards,
            c.overwrites,
            pool,
            c.beams,
            c.node_cache,
            c.eye_head,
        ];
        if bindings != c.bindings {
            release(c.group);
            let [eye, clipmap, ground, _, _, _, _, _, beams, node_cache, head] = bindings;
            c.group = cast_group(
                g,
                &c.cells_built,
                [eye, clipmap, ground, beams, node_cache, head],
            );
            release(c.miss_group);
            c.miss_group = g.bind_group(
                c.miss_built.layouts[0],
                &[
                    buf_entry(0, c.eye),
                    buf_entry(1, c.clipmap),
                    buf_entry(2, ground),
                    view_entry(3, bindings[3]),
                    buf_entry(4, palette_indices),
                    buf_entry(5, c.shards),
                    buf_entry(6, c.overwrites),
                    buf_entry(7, pool),
                    buf_entry(8, c.beams),
                    buf_entry(9, c.node_cache),
                ],
            );
            release(c.ask_group);
            c.ask_group = g.bind_group(
                c.sample_built.layouts[0],
                &[
                    buf_entry(0, c.eye),
                    buf_entry(1, c.clipmap),
                    buf_entry(2, ground),
                    buf_entry(4, palette_indices),
                    buf_entry(5, c.shards),
                    buf_entry(6, c.overwrites),
                    buf_entry(7, pool),
                    buf_entry(8, c.beams),
                    buf_entry(9, c.node_cache),
                    buf_entry(HEAD_BINDING, c.eye_head),
                ],
            );
            release(c.sample_hit_group);
            c.sample_hit_group = g.bind_group(
                c.sample_hit_built.layouts[0],
                &[
                    buf_entry(0, c.eye),
                    buf_entry(8, c.beams),
                    buf_entry(9, c.node_cache),
                    buf_entry(10, c.sample_left),
                ],
            );
            release(c.beam_hit_group);
            c.beam_hit_group = g.bind_group(
                c.beam_hit_built.layouts[0],
                &[
                    buf_entry(0, c.eye),
                    buf_entry(8, c.beams),
                    buf_entry(9, c.node_cache),
                    buf_entry(10, c.beam_left),
                ],
            );
            release(c.miss_hit_group);
            c.miss_hit_group = g.bind_group(
                c.miss_hit_built.layouts[0],
                &[
                    buf_entry(0, c.eye),
                    view_entry(3, bindings[3]),
                    buf_entry(8, c.beams),
                    buf_entry(9, c.node_cache),
                    buf_entry(10, c.miss_left),
                ],
            );
            c.bindings = bindings;
        }
        if casts {
            c.raster.prepare(
                g,
                &cast.eye,
                c.size,
                &c.grid,
                self.kind == FrameKind::cast,
                [
                    c.eye,
                    c.beams,
                    c.eye_head,
                    c.clipmap,
                    ground,
                    c.shards,
                    c.overwrites,
                    pool,
                    c.marks,
                ],
            );
            let hit_bound = [c.eye, rays.0, c.beams, c.node_cache, c.marks];
            if hit_bound != c.hit_bindings {
                for group in c.hit_groups {
                    release(group);
                }
                c.hit_groups = hit_groups(g, [&c.hit_lead_built, &c.hit_faces_built], hit_bound);
                c.hit_bindings = hit_bound;
            }
            let ((changed_at, uniform), _) = self.tiles;
            let px_bound = [
                c.eye,
                rays.0,
                c.beams,
                c.node_cache,
                c.raster.visibility_array,
                changed_at,
                uniform,
            ];
            if px_bound != c.px_bound {
                release(c.px_group);
                release(c.cast_tiles_group);
                release(c.cast_tiles);
                let [across, down] = c.size.map(|v| v.div_ceil(screen::TILE));
                c.cast_tiles = g.buf(
                    word_bytes(CAST_TILES_START.len() + (across * down) as usize),
                    buffer_usage::STORAGE | buffer_usage::INDIRECT | buffer_usage::COPY_DST,
                );
                c.cast_tiles_group = g.bind_group(
                    c.cast_tiles_built.layouts[0],
                    &[
                        buf_entry(0, c.cast_tiles),
                        buf_entry(30, changed_at),
                        buf_entry(31, uniform),
                    ],
                );
                c.px_group = px_group(g, &c.pipeline, px_bound, c.marks, c.cast_tiles);
                c.px_bound = px_bound;
            }
        }
        let [drawing, regions, instances] = c
            .generator
            .instances()
            .map_or([c.light_none; 3], |p| [p.drawing, p.regions, p.instances]);
        let light_bound = [
            c.eye,
            c.clipmap,
            ground,
            palette_indices,
            c.shards,
            c.overwrites,
            pool,
            light_map.0,
            rays.0,
            drawing,
            regions,
            instances,
        ];
        // Made once all the light's cast is, over its list and its raster's
        // buffers, which are let go with them (`light_made`).
        if c.light_ready && light_bound != c.light_bound {
            for group in c.light_groups {
                release(group);
            }
            c.light_groups = light_groups(
                g,
                [
                    &c.light_built,
                    &c.cells_built,
                    &c.raster.light_boxes_built,
                    &c.light_miss_built,
                    &c.light_mark_built,
                    &c.light_mark_draws_built,
                    &c.light_propagate_built,
                    &c.light_slide_built,
                    &c.light_miss_hit_built,
                    &c.light_sample_hit_built,
                    &c.light_sample_built,
                    &c.light_beam_hit_built,
                ],
                light_bound,
                [
                    c.node_cache,
                    c.light_head,
                    c.light_list,
                    c.raster.light.boxes,
                    c.raster.light.visibility_array,
                    c.raster.light.args,
                    c.light_misses,
                    c.light_args,
                    c.light_left,
                    c.light_sample_left,
                    c.light_beam_left,
                ],
            );
            c.light_bound = light_bound;
        }
        let eye = c.eye;
        self.regroup(g, [rays.0, eye]);
    }

    /// The frame draws whole after all: its samples' raster drawn whole.
    pub fn whole(&mut self) {
        if let Some(c) = self.cast.as_mut() {
            c.gated = false;
        }
    }

    /// The eye's clip levels folded over their bricks where their entries moved
    /// (`fold_cs`): ahead of the frame, as the fold outlives it.
    pub fn fold(&mut self, ctx: &mut RenderContext) -> bool {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(GpuSpanName::fill);
        let Some(c) = self.cast.as_mut().filter(|c| c.refold && c.group != 0) else {
            return false;
        };
        let pass = g.compute_pass(encoder, stamp.pass());
        g.set_bind_group(pass, 0, c.group, &[]);
        g.set_pipeline(pass, c.fold_built.id);
        // A level a dispatch, each over the one under it, its level the
        // dispatch's depth (`fold_cs`).
        for l in 1..=clipmap::JOINED_LEVELS {
            let blocks = (clipmap::ACROSS >> l) * (clipmap::ACROSS >> l);
            g.dispatch(
                pass,
                (blocks * clipmap::EYE_CLIP_LEVELS).div_ceil(FOLD_LANES),
                1,
                l,
            );
        }
        g.end(pass);
        c.refold = false;
        true
    }

    /// The cast where the frame casts, the world's depth from the light with
    /// it where `light`, then the decor generated over the world where it moves:
    /// each a pass of its own over the cast's group, timed in its own span,
    /// so what each costs is read apart; the light's in the shadow's, with
    /// the map's pass, which is not drawn at the world's pixels
    /// (span.json `per_px`).
    pub fn dispatch(&mut self, ctx: &mut RenderContext) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let (generates, casts, kind) = (self.generates(), self.casts_px(), self.kind);
        let claimed = self.tiles;
        let Some(c) = self.cast.as_mut() else { return };
        if casts {
            // The node cache forgotten first where it is; then the
            // beam rays writing it, a corner each; each cell's start off them;
            // the samples writing it, each from its cell's start; their
            // cells drawn as boxes, each pixel's nearest kept; the pixels
            // off that, or reading it from their cell's start, marking the
            // pixels that missed; and those again,
            // writing it, a thread a drawn pixel, each but the marked
            // returning at once, each reading what the ones before wrote
            // (pxl-engine `node_cache`).
            // A pass a dispatch, each timed in its own span, so the cast's
            // parts are read apart: `cast` holds the forgetting alone; the
            // samples' boxes written and drawn between the samples and the
            // pixels, which take the visible sample (ray/raster.rs, spans
            // `sample_box` and `sraster`). Where
            // only the jitter moved (`FrameKind::px`), the beam rays, the
            // cells' starts and the samples stand, and the rest is cast again
            // off them.
            let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::cast).pass());
            g.set_bind_group(pass, 0, c.group, &[]);
            if c.forgets != node_cache::FORGETS_NONE {
                g.set_pipeline(pass, c.forget_built.id);
                g.dispatch(pass, node_cache::ENTRIES.div_ceil(ROW_LANES), 1, 1);
            }
            g.end(pass);
            for (span, pipeline, [x, y], w) in [
                (
                    GpuSpanName::beam_hit,
                    &c.beam_hit_built,
                    c.grid.corners,
                    BEAM_WORKGROUP,
                ),
                (
                    GpuSpanName::beam,
                    &c.beam_built,
                    c.grid.corners,
                    BEAM_WORKGROUP,
                ),
                (
                    GpuSpanName::cell,
                    &c.cells_built,
                    c.grid.span,
                    [CELL_SIDE, CELL_SIDE, 1],
                ),
                (
                    GpuSpanName::sample_hit,
                    &c.sample_hit_built,
                    c.grid.samples,
                    SAMPLE_WORKGROUP,
                ),
                (
                    GpuSpanName::sample,
                    &c.sample_built,
                    c.grid.samples,
                    SAMPLE_WORKGROUP,
                ),
                (GpuSpanName::px, &c.pipeline, c.size, CAST_WORKGROUP),
            ] {
                if kind == FrameKind::px && span != GpuSpanName::px {
                    continue;
                }
                let mut group = c.group;
                let beam_left = span == GpuSpanName::beam;
                if span == GpuSpanName::beam_hit {
                    group = c.beam_hit_group;
                    g.write_buf(c.beam_left, 0, bytes(&ray::LEFT_EMPTY));
                }
                if beam_left {
                    group = c.ask_group;
                    g.copy_buf(
                        encoder,
                        (c.beam_left, word_bytes(ray::LEFT_CNT as usize)),
                        (c.beams, word_bytes(c.grid.corners_at as usize)),
                        4,
                    );
                }
                if span == GpuSpanName::sample_hit {
                    group = c.sample_hit_group;
                    g.write_buf(c.sample_left, 0, bytes(&ray::LEFT_EMPTY));
                }
                if span == GpuSpanName::sample {
                    group = c.ask_group;
                    g.copy_buf(
                        encoder,
                        (c.sample_left, word_bytes(ray::LEFT_CNT as usize)),
                        (c.beams, word_bytes(c.grid.lefts_at as usize)),
                        4,
                    );
                }
                if span == GpuSpanName::px {
                    let gated = c.gated.then_some(claimed);
                    c.raster.draw(ctx, &c.grid, kind == FrameKind::cast, gated);
                    group = c.px_group;
                    g.write_buf(c.cast_tiles, 0, bytes(&CAST_TILES_START));
                }
                let pass = g.compute_pass(encoder, ctx.stamp(span).pass());
                if span == GpuSpanName::px {
                    let tiles = c.size.map(|v| v.div_ceil(screen::TILE));
                    g.set_bind_group(pass, 0, c.cast_tiles_group, &[]);
                    g.set_pipeline(pass, c.cast_tiles_built.id);
                    g.dispatch(
                        pass,
                        (tiles[0] * tiles[1]).div_ceil(screen::CAST_TILES_LANES),
                        1,
                        1,
                    );
                }
                g.set_bind_group(pass, 0, group, &[]);
                g.set_pipeline(pass, pipeline.id);
                if span == GpuSpanName::px {
                    g.dispatch_indirect(pass, c.cast_tiles, 0);
                } else if span == GpuSpanName::sample {
                    g.dispatch_indirect(pass, c.sample_left, 0);
                } else if beam_left {
                    g.dispatch_indirect(pass, c.beam_left, 0);
                } else {
                    g.dispatch(pass, x.div_ceil(w[0]), y.div_ceil(w[1]), 1);
                }
                g.end(pass);
            }
            // The marked pixels' count where their walk reads it, then that
            // walk over their list, and over every pixel where it was full.
            g.copy_buf(
                encoder,
                (c.marks, word_bytes(ray::MISS_ARGS_CNT as usize)),
                (c.node_cache, word_bytes(node_cache::MARKS_CNT as usize)),
                4,
            );
            g.write_buf(c.miss_left, 0, bytes(&MISS_LEFT_EMPTY));
            let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::miss).pass());
            g.set_bind_group(pass, 0, c.miss_hit_group, &[]);
            g.set_pipeline(pass, c.miss_hit_built.id);
            g.dispatch_indirect(pass, c.marks, word_bytes(ray::MISS_ARGS_LIST as usize));
            g.end(pass);
            // How many it left where their walk reads them, and the hits it
            // kept where that goes on from.
            g.copy_buf(
                encoder,
                (c.miss_left, word_bytes(ray::LEFT_CNT as usize)),
                (c.beams, word_bytes(c.grid.misses_at as usize)),
                4,
            );
            g.copy_buf(
                encoder,
                (c.miss_left, word_bytes(4)),
                (c.node_cache, word_bytes(node_cache::HITS_ARGS as usize)),
                word_bytes(4),
            );
            let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::miss).pass());
            g.set_bind_group(pass, 0, c.miss_group, &[]);
            g.set_pipeline(pass, c.miss_list_built.id);
            g.dispatch_indirect(pass, c.miss_left, 0);
            g.set_pipeline(pass, c.miss_built.id);
            g.dispatch_indirect(pass, c.marks, word_bytes(ray::MISS_ARGS_WHOLE as usize));
            // What lies about the faces they stopped on, once a square of
            // hits: the one of each square that asks listed, the faces' pass
            // over their asks, and the faces put at their pixels.
            let hits = word_bytes(node_cache::HITS_ARGS as usize);
            g.set_pipeline(pass, c.hit_lead_built.id);
            g.set_bind_group(pass, 0, c.hit_groups[0], &[]);
            g.dispatch_indirect(pass, c.node_cache, hits);
            c.raster.ask_hits(g, pass, c.marks);
            g.set_pipeline(pass, c.hit_faces_built.id);
            g.set_bind_group(pass, 0, c.hit_groups[1], &[]);
            g.dispatch_indirect(pass, c.node_cache, hits);
            g.end(pass);
        }
        // The decor generated over the rays, before the light's cast, which
        // marks where it stands (`light_mark_draws_cs`).
        if generates {
            let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::generate).pass());
            c.generator.dispatch(g, pass);
            g.end(pass);
        }
        if c.light_cast {
            // The reads moved with the box, a tile of it at a time: each
            // tile's word the word of the tile that held its world texels
            // under the corner before, copied out and back in, which the
            // slide's pass then sets to none where the box slid onto it or
            // into another band (`light_slide_cs`).
            if c.light_slid {
                let n = light::tiles(shadow::SIZE) as i32;
                let t = light::TILE as i32;
                let dx = (c.light_corner[0] - c.light_corner_was[0]) / t;
                let dy = (c.light_corner[1] - c.light_corner_was[1]) / t;
                let shift = dy * n + dx;
                let all = n * n;
                if shift.abs() < all {
                    let reads = light::reads_at(shadow::SIZE) as i32;
                    let words = |w: i32| word_bytes(w as usize);
                    let out = (c.light_reads_was, 0);
                    g.copy_buf(encoder, (c.light_list, words(reads)), out, words(all));
                    let (from, to) = if shift >= 0 { (shift, 0) } else { (0, -shift) };
                    g.copy_buf(
                        encoder,
                        (c.light_reads_was, words(from)),
                        (c.light_list, words(reads + to)),
                        words(all - shift.abs()),
                    );
                }
            }
            // The tiles the world's receivers stand on marked where the eye's
            // rays meet it, a thread a drawn pixel (`light_mark_cs`), in a span
            // of its own (`light_receivers`, span.json `per_px`). It writes
            // the stands alone and the slide the reads alone
            // (`light::stands_at`, `light::reads_at`), so it goes first.
            if c.light_world {
                c.light_world = false;
                let stamp = ctx.stamp(GpuSpanName::light_receivers).pass();
                let pass = g.compute_pass(encoder, stamp);
                g.set_bind_group(pass, 0, c.light_groups[4], &[]);
                g.set_pipeline(pass, c.light_mark_built.id);
                let [x, y] = c.size;
                g.dispatch(pass, x.div_ceil(SQUARE_SIDE), y.div_ceil(SQUARE_SIDE), 1);
                g.end(pass);
            }
            // The beam, the samples, their boxes drawn, the slots taking
            // their visible sample, a workgroup a tile (pxl-engine
            // `light::TILE`), and the rest walked off their list as the eye's
            // missed pixels are: each a pass timed in its own span.
            let stamp = ctx.stamp(GpuSpanName::light_beam).pass();
            let pass = g.compute_pass(encoder, stamp);
            // The tiles the box slid onto or into another band, or whose rays
            // may meet a column of the world that changed, set back to be read
            // anew first, a thread a tile (`light_slide_cs`).
            if c.light_slid || c.light_reached {
                let n = light::tiles(shadow::SIZE);
                g.set_bind_group(pass, 0, c.light_groups[7], &[]);
                g.set_pipeline(pass, c.light_slide_built.id);
                g.dispatch(pass, n.div_ceil(SQUARE_SIDE), n.div_ceil(SQUARE_SIDE), 1);
            }
            // The tiles the drawn receivers stand on marked, timed with the
            // beam: where the decor and the actors drawn stand, a thread each
            // (`light_mark_draws_cs`); then those they read, a thread a tile
            // (`light_propagate_cs`).
            g.set_bind_group(pass, 0, c.light_groups[5], &[]);
            g.set_pipeline(pass, c.light_mark_draws_built.id);
            let cap = c.generator.instances().map_or(0, |p| p.cap);
            g.dispatch(pass, (cap + light::ACTORS as u32).div_ceil(ROW_LANES), 1, 1);
            g.set_bind_group(pass, 0, c.light_groups[6], &[]);
            g.set_pipeline(pass, c.light_propagate_built.id);
            let n = light::tiles(shadow::SIZE);
            g.dispatch(pass, n.div_ceil(SQUARE_SIDE), n.div_ceil(SQUARE_SIDE), 1);
            g.end(pass);
            // The lists' counts where the passes that take them read them.
            for which in 0..light::LISTS {
                g.copy_buf(
                    encoder,
                    (
                        c.light_args,
                        word_bytes((light::ARGS_COUNTS + which) as usize),
                    ),
                    (
                        c.light_list,
                        word_bytes(light::listed_at(shadow::SIZE, which) as usize),
                    ),
                    4,
                );
            }
            let stamp = ctx.stamp(GpuSpanName::light_beam).pass();
            let pass = g.compute_pass(encoder, stamp);
            // The beam over the node cache alone, as many workgroups as the
            // propagation left it (`light::ARGS_WORDS`); then what it left over
            // the samples' group, and the cells' starts over the light's
            // world's group, each handed the light's head.
            let args = |w: u32| word_bytes(w as usize);
            g.write_buf(c.light_beam_left, 0, bytes(&ray::LEFT_EMPTY));
            g.set_bind_group(pass, 0, c.light_groups[11], &[]);
            g.set_pipeline(pass, c.light_beam_hit_built.id);
            g.dispatch_indirect(pass, c.light_args, args(light::ARGS_BEAM));
            g.end(pass);
            g.copy_buf(
                encoder,
                (c.light_beam_left, word_bytes(ray::LEFT_CNT as usize)),
                (
                    c.light_list,
                    word_bytes(light::corners_left_at(shadow::SIZE) as usize),
                ),
                4,
            );
            let stamp = ctx.stamp(GpuSpanName::light_beam).pass();
            let pass = g.compute_pass(encoder, stamp);
            g.set_bind_group(pass, 0, c.light_groups[10], &[]);
            g.set_pipeline(pass, c.light_beam_built.id);
            g.dispatch_indirect(pass, c.light_beam_left, 0);
            g.set_bind_group(pass, 0, c.light_groups[1], &[]);
            g.set_pipeline(pass, c.light_cells_built.id);
            g.dispatch_indirect(pass, c.light_args, args(light::ARGS_CELLS));
            g.end(pass);
            g.write_buf(c.light_sample_left, 0, bytes(&ray::LEFT_EMPTY));
            // The samples, over the node cache alone first: one span.
            let stamp = ctx.stamp(GpuSpanName::light_sample).pass();
            let pass = g.compute_pass(encoder, stamp);
            g.set_bind_group(pass, 0, c.light_groups[9], &[]);
            g.set_pipeline(pass, c.light_sample_hit_built.id);
            g.dispatch_indirect(pass, c.light_args, args(light::ARGS_SAMPLE));
            g.end(pass);
            g.copy_buf(
                encoder,
                (c.light_sample_left, word_bytes(ray::LEFT_CNT as usize)),
                (
                    c.light_list,
                    word_bytes(light::lefts_at(shadow::SIZE) as usize),
                ),
                4,
            );
            let stamp = ctx.stamp(GpuSpanName::light_sample).pass();
            let pass = g.compute_pass(encoder, stamp);
            g.set_bind_group(pass, 0, c.light_groups[10], &[]);
            g.set_pipeline(pass, c.light_sample_built.id);
            g.dispatch_indirect(pass, c.light_sample_left, 0);
            g.end(pass);
            let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::light_raster).pass());
            g.set_bind_group(pass, 0, c.light_groups[2], &[]);
            g.set_pipeline(pass, c.raster.light_boxes_built.id);
            g.dispatch_indirect(pass, c.light_args, args(light::ARGS_SAMPLE_BOX));
            g.end(pass);
            c.raster.light_draw(ctx);
            let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::light_slot).pass());
            g.set_bind_group(pass, 0, c.light_groups[0], &[]);
            g.set_pipeline(pass, c.light_built.id);
            g.dispatch_indirect(pass, c.light_args, args(light::ARGS_SLOT));
            g.end(pass);
            // Their count where their walk reads it, then that walk.
            g.copy_buf(
                encoder,
                (c.light_misses, word_bytes(ray::MISS_ARGS_CNT as usize)),
                (
                    c.light_list,
                    word_bytes(light::misses_cnt(shadow::SIZE) as usize),
                ),
                4,
            );
            g.write_buf(c.light_left, 0, bytes(&ray::LEFT_EMPTY));
            let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::light_hit).pass());
            g.set_bind_group(pass, 0, c.light_groups[8], &[]);
            g.set_pipeline(pass, c.light_miss_hit_built.id);
            let list = word_bytes(ray::MISS_ARGS_LIST as usize);
            g.dispatch_indirect(pass, c.light_misses, list);
            g.end(pass);
            // How many it left where their walk reads them, then that walk over their list.
            g.copy_buf(
                encoder,
                (c.light_left, word_bytes(ray::LEFT_CNT as usize)),
                (
                    c.light_list,
                    word_bytes(light::misses_left_at(shadow::SIZE) as usize),
                ),
                4,
            );
            let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::light_walk).pass());
            g.set_bind_group(pass, 0, c.light_groups[3], &[]);
            g.set_pipeline(pass, c.light_miss_built.id);
            g.dispatch_indirect(pass, c.light_left, 0);
            g.end(pass);
        }
        let counted = Counts {
            node_cache: c.node_cache,
            args: [c.marks, c.light_misses],
            boxes: [c.raster.args, c.raster.light.args],
        };
        c.counter.copy(g, encoder, &counted);
    }

    /// The light's cast made where `on`, the shadows' setting
    /// (`STAGE_AT::SHADOWS`), before the frame: its list (`light::list_words`)
    /// and its raster's buffers and targets (`Raster::light_made`), what
    /// only a map cast needs; ready once they are (`light_ready`), and its
    /// groups made over them as the frame prepares. Let go where not on,
    /// its groups with them, so the first map cast after they are made again
    /// begins an epoch and casts whole, as the first one did. Its passes are
    /// made with the eye's (`make`), the beam's, the cells' and the samples'
    /// the eye's own.
    pub fn light_made(&mut self, g: &Gpu, on: bool) {
        let Some(c) = self.cast.as_mut() else { return };
        if on {
            if !c.light_ready {
                if c.light_list == 0 {
                    c.light_list = light_list(g);
                    c.light_args = light_args_buf(g);
                }
                c.light_ready = c.raster.light_made(g, shadow::SIZE);
            }
        } else if c.light_list != 0 || c.light_ready {
            for group in c.light_groups {
                release(group);
            }
            (c.light_groups, c.light_bound) = ([0; 12], [0; 12]);
            release(c.light_list);
            release(c.light_args);
            (c.light_list, c.light_args) = (0, 0);
            c.raster.light_release();
            c.light_ready = false;
        }
    }

    /// The counts copied last frame mapped back (`Counter::back`).
    pub fn counts_back(&mut self, g: &Gpu) {
        if let Some(c) = self.cast.as_mut() {
            c.counter.back(g);
        }
    }

    /// The counts that landed since last taken (`Counter::take`).
    pub fn take_counts(&mut self) -> Option<RayCounts> {
        self.cast.as_mut().and_then(|c| c.counter.take())
    }

    pub fn light_casts(&self) -> bool {
        self.cast.as_ref().is_some_and(|c| c.light_cast)
    }

    /// The light's list, whose reads tell the tiles its epoch cast, and the epoch's first
    /// cast (`light::reads_at`, `light::HEAD_EPOCH`), while the light's cast is made.
    pub fn light_reads(&self) -> Option<(u32, u32)> {
        self.cast
            .as_ref()
            .filter(|c| c.light_list != 0)
            .map(|c| (c.light_list, c.light_epoch))
    }

    /// Why the light's map cast this frame, a bit each in LIGHT_MOVED's
    /// order (span.json `LIGHT_MOVED`); 0 where it did not.
    pub fn light_moved(&self) -> u32 {
        self.cast.as_ref().map_or(0, |c| c.light_moved)
    }

    /// The boxes decor's slots are marked by to generate again this frame
    /// (gpu/generator.rs `marked_boxes`).
    pub fn decor_boxes(&self) -> u32 {
        self.cast.as_ref().map_or(0, |c| c.generator.marked_boxes())
    }

    pub fn generates(&self) -> bool {
        self.cast.as_ref().is_some_and(|c| c.generator.works())
    }

    pub fn generated(&self) -> Option<IndirectDraw> {
        self.cast.as_ref().and_then(|c| c.generator.indirect_draw())
    }

    /// What a frame through the gate draws of decor generated (`Generator::gate`).
    pub fn gate_generated(
        &self,
        ctx: &mut RenderContext,
        frame: u32,
        tiles: u32,
        claimed_box: u32,
    ) {
        if let Some(c) = self.cast.as_ref() {
            c.generator.gate(ctx, frame, tiles, claimed_box);
        }
    }

    /// The frame was dropped with its cast and its image: the rays and the
    /// image hold older ones.
    pub fn on_dropped(&mut self) {
        if let Some(c) = self.cast.as_mut() {
            // What it was to forget, it did not: the next cast forgets it
            // too, every box and child group as sent, as decor's generation holds
            // its edits again (`carry_edits`).
            if let Some(f) = c.sent_forget.take() {
                c.forget |= f.all;
                for (m, was) in c.moved.iter_mut().zip(f.moved) {
                    *m |= was;
                }
                for (b, was) in c.moved_bricks.iter_mut().zip(f.moved_bricks) {
                    *b = node_cache::bricks_joined(*b, was);
                }
                for &b in &f.boxes[..f.holding] {
                    hold(&mut c.boxes, &mut c.holding, b);
                }
                for &first in &f.written_groups[..f.writes] {
                    c.forget |= !note(&mut c.written_groups, &mut c.writes, first);
                }
            }
            c.read.forget();
            c.light_content.forget();
            c.generator.on_dropped();
            c.counter.on_dropped();
        }
    }

    /// The frame was drawn: what its decor marked by is done with.
    pub fn on_drawn(&mut self) {
        if let Some(c) = self.cast.as_mut() {
            c.generator.drawn_frame();
        }
    }

    /// A frame's cast handed and never drawn: the edits its decor named,
    /// held for the next that is.
    pub fn carry_edits(&mut self, edits: &[[i32; 4]]) {
        if let Some(c) = self.cast.as_mut() {
            for &b in edits {
                hold(&mut c.boxes, &mut c.holding, b);
            }
            c.generator.carry_edits(edits);
        }
    }
}

/// The cast's world's group 0 over `bindings` (`WORLD_GROUP`): the eye, the
/// clip levels' table, the tiles' frames, the beam rays' lines and the cells'
/// starts, the node cache, and the head: the eye's, or, over the light's
/// list and head, the shadow map's (`light_groups`).
fn cast_group(g: &Gpu, pipeline: &Pipeline, bindings: [u32; 6]) -> u32 {
    let [eye, clipmap, ground, beams, node_cache, head] = bindings;
    let entries = [
        buf_entry(0, eye),
        buf_entry(1, clipmap),
        buf_entry(2, ground),
        buf_entry(8, beams),
        buf_entry(9, node_cache),
        buf_entry(HEAD_BINDING, head),
    ];
    g.bind_group(pipeline.layouts[0], &entries)
}

/// The missed pixels' hits' passes' groups 0 (`hit_lead_cs`,
/// `hit_faces_cs`) over `bindings`: the eye, the rays, the beam's words, the
/// node cache, and the walk again's words, which the faces' leaders are
/// listed in and the faces' pass none of.
fn hit_groups(g: &Gpu, [lead, faces]: [&Pipeline; 2], bindings: [u32; 5]) -> [u32; 2] {
    let [eye, rays, beams, node_cache, marks] = bindings;
    [
        g.bind_group(
            lead.layouts[0],
            &[
                buf_entry(0, eye),
                view_entry(3, rays),
                buf_entry(8, beams),
                buf_entry(9, node_cache),
                buf_entry(14, marks),
            ],
        ),
        g.bind_group(
            faces.layouts[0],
            &[
                buf_entry(0, eye),
                view_entry(3, rays),
                buf_entry(8, beams),
                buf_entry(9, node_cache),
            ],
        ),
    ]
}

/// The pixels' group 0 over `bindings`: the eye, the rays, the beam's words,
/// the node cache, which the marked pixels are listed in, the visibility buffer,
/// a one-layer array, the words the marked are counted in (`marks`) and the claimed tiles' list.
/// No world: a pixel walks nothing of its own (`cast_cs`).
fn px_group(g: &Gpu, pipeline: &Pipeline, bindings: [u32; 7], marks: u32, list: u32) -> u32 {
    let [eye, rays, beams, node_cache, visibility, ..] = bindings;
    let entries = [
        buf_entry(0, eye),
        view_entry(3, rays),
        buf_entry(8, beams),
        buf_entry(9, node_cache),
        view_entry(13, visibility),
        buf_entry(14, marks),
        buf_entry(30, list),
    ];
    g.bind_group(pipeline.layouts[0], &entries)
}

const _: () = assert!(
    light::tiles(shadow::SIZE).is_multiple_of(light::COARSE),
    "the map's tiles are a whole number of squares a side (`light::coarse_square`)"
);

/// The workgroups of the passes that cast the light's tiles as a cast starts: none, a list's
/// row across and no row (`light::ARGS_WORDS`).
fn light_args() -> [u32; light::ARGS_WORDS as usize] {
    let mut args = [0; light::ARGS_WORDS as usize];
    for w in [
        light::ARGS_BEAM,
        light::ARGS_CELLS,
        light::ARGS_SAMPLE,
        light::ARGS_SAMPLE_BOX,
        light::ARGS_SLOT,
    ] {
        args[w as usize..w as usize + 3].copy_from_slice(&[light::LIST_ACROSS, 0, 1]);
    }
    args
}

/// The light's args (`light::args_words`).
fn light_args_buf(g: &Gpu) -> u32 {
    g.buf(
        word_bytes(light::args_words(shadow::SIZE) as usize),
        buffer_usage::STORAGE
            | buffer_usage::COPY_DST
            | buffer_usage::INDIRECT
            | buffer_usage::COPY_SRC,
    )
}

/// The light's list: the beam's lines and the cells' starts, the raster's
/// samples and the misses past them (`light::list_words`).
fn light_list(g: &Gpu) -> u32 {
    g.buf(
        word_bytes(light::list_words(shadow::SIZE) as usize),
        buffer_usage::STORAGE
            | buffer_usage::COPY_DST
            | buffer_usage::INDIRECT
            | buffer_usage::COPY_SRC,
    )
}

/// The light's cast's groups 0, over `bindings`: the eye, the clip level, the
/// tiles' frames, the bricks' palette indices, the service area's index, the
/// overwrites and the ways' pool, the cast's alone; the node cache,
/// which the light's walks read and write as the eye's do; the clipmap; its
/// head, the beam's lines, the samples and the misses (`list`),
/// the raster's boxes, visibility buffer and args, the misses' dispatch and
/// the casting passes' workgroups (`light::ARGS_WORDS`), which the
/// propagation writes; the eye's rays, which the mark reads, and the decor's drawing
/// words, regions and instances, which the drawn mark reads. The slots',
/// its world's (the beam, the cells' starts and the samples, the eye's
/// passes, over `WORLD_GROUP`, its list where the eye's beam rays' buffer
/// lies and its head where the eye's), its boxes' (the eye's raster's
/// pass), the misses' walk's, the two marks', the propagation's, the
/// slide's, the misses' and the samples' walks over the node cache alone, the
/// samples' and the beam rays' walks that ask, one group, and the beam rays'
/// walk over the node cache alone.
fn light_groups(
    g: &Gpu,
    [
        walk,
        world,
        boxes_built,
        miss,
        mark,
        mark_draws,
        propagate,
        slide,
        miss_hit,
        sample_hit,
        sample,
        beam_hit,
    ]: [&Pipeline; 12],
    bindings: [u32; 12],
    [node_cache, head, list, boxes, visibility, args, misses, light_args, left, sample_left, beam_left]: [u32;
        11],
) -> [u32; 12] {
    let [eye, clipmap, ground, palette_indices, shards, overwrites, pool, light, rays, drawing, regions, instances] =
        bindings;
    let entries = [
        buf_entry(0, eye),
        buf_entry(1, clipmap),
        buf_entry(2, ground),
        view_entry(3, rays),
        buf_entry(4, palette_indices),
        buf_entry(5, shards),
        buf_entry(6, overwrites),
        buf_entry(7, pool),
        buf_entry(8, list),
        buf_entry(9, node_cache),
        view_entry(10, light),
        buf_entry(HEAD_BINDING, head),
        buf_entry(12, list),
        buf_entry(13, left),
        buf_entry(14, boxes),
        view_entry(15, visibility),
        buf_entry(16, args),
        buf_entry(17, misses),
        buf_entry(18, drawing),
        buf_entry(19, regions),
        buf_entry(20, instances),
        buf_entry(21, light_args),
    ];
    // Each group takes the entries its layout binds; the samples' and the beam rays' walks
    // over the node cache alone, each its own list's atomics at 10.
    let hit = |p: &Pipeline, left: u32| {
        g.bind_group(
            p.layouts[0],
            &[
                buf_entry(0, eye),
                buf_entry(8, list),
                buf_entry(9, node_cache),
                buf_entry(10, left),
                buf_entry(HEAD_BINDING, head),
            ],
        )
    };
    let of = |p: &Pipeline| {
        let layout = p.layouts[0];
        let e: Vec<Entry> = entries
            .into_iter()
            .filter(|e| layout.binds(e.binding))
            .collect();
        g.bind_group(layout, &e)
    };
    [
        of(walk),
        cast_group(g, world, [eye, clipmap, ground, list, node_cache, head]),
        g.bind_group(
            boxes_built.layouts[0],
            &[
                buf_entry(0, eye),
                buf_entry(8, list),
                buf_entry(HEAD_BINDING, head),
                buf_entry(14, boxes),
                buf_entry(16, args),
            ],
        ),
        of(miss),
        of(mark),
        of(mark_draws),
        of(propagate),
        of(slide),
        of(miss_hit),
        hit(sample_hit, sample_left),
        of(sample),
        hit(beam_hit, beam_left),
    ]
}
