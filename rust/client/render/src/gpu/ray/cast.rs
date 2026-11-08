// pxl-render/gpu/ray/cast.rs

//! The GPU's cast of the eye's rays into the world surface's rays, the
//! world's depth from the light cast with them, and the decor generated over the
//! world (gpu/generator.rs): what each reads, made once, and their dispatches.

use crate::frame::shader::access::{Group, Step};
use crate::frame::shader::sequence;
use pxl_content::generation::{self, GROUPS};
use pxl_engine::clipmap;
use pxl_engine::{beam, eye, light, node_cache, object, ray, Zoom, LEAF_CELL_ZOOM};
use xpute_gpu::encode::Id;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use super::count::{self, Counter, Counts, RayCounts};
use super::raster::Raster;
use super::schedule::{Reads, Schedule};
use super::RaysPass;
use crate::frame::shader::floor;
use crate::frame::shader::group::{
    BEAMS, CAST_GROUP, CAST_TILES, CAST_TILES_GROUP, CAST_TILES_OF_BINDS, CORNERS_ASK_BINDS,
    CORNERS_WALK_BINDS, HEADER_BINDING, LAST_BINDS, LATTICE_ASK_GROUP, LATTICE_LAST_BINDS,
    LATTICE_LAST_GROUP, LATTICE_WALK_GROUP, LIGHT_ARGS, LIGHT_CORNERS_ASK_BINDS,
    LIGHT_CORNERS_WALK_BINDS, LIGHT_DRAWS, LIGHT_GROUP, LIGHT_LAST_BINDS, LIGHT_LATTICE_LAST_BINDS,
    LIGHT_LATTICE_LAST_GROUP, LIGHT_LATTICE_WALK_GROUP, LIGHT_LIST, LIGHT_MARK_BINDS,
    LIGHT_MARK_DRAWS_BINDS, LIGHT_MARK_DRAWS_GROUP, LIGHT_MARK_DRAWS_OPEN_BINDS,
    LIGHT_MARK_DRAWS_OPEN_GROUP, LIGHT_MARK_GROUP, LIGHT_MISSES, LIGHT_PROPAGATE_BINDS,
    LIGHT_PROPAGATE_GROUP, LIGHT_RASTER_ARGS, LIGHT_RECEIVER_TILES, LIGHT_SAMPLES_ASK_BINDS,
    LIGHT_SAMPLES_WALK_BINDS, LIGHT_SAMPLE_BOX_BINDS, LIGHT_SLIDE_BINDS, LIGHT_SLIDE_GROUP,
    LIGHT_SLOTS_BINDS, LIGHT_WALK_ASK_BINDS, LIGHT_WALK_BINDS, LIGHT_WORLD_BINDS, MARKS,
    NODE_CACHE, PIXELS_BINDS, QUEUES, QUEUE_ASK_GROUP, QUEUE_GROUP, QUEUE_LAST_GROUP,
    SAMPLES_ASK_BINDS, SAMPLES_WALK_BINDS, WALK_ASK_BINDS, WALK_BINDS, WORLD_BINDS, WORLD_GROUP,
};
use crate::frame::shader::record::{
    COMPUTE_CAST, COMPUTE_CAST_TILES, COMPUTE_CELL, COMPUTE_FORGET, COMPUTE_LIGHT,
    COMPUTE_LIGHT_CELL, COMPUTE_LIGHT_DAMAGE, COMPUTE_LIGHT_MARK_DRAWS,
    COMPUTE_LIGHT_MARK_DRAWS_OPEN, COMPUTE_LIGHT_MARK_WALKED, COMPUTE_LIGHT_PROPAGATE,
    COMPUTE_LIGHT_SLIDE, COMPUTE_QUEUE, COMPUTE_QUEUE_ASK, COMPUTE_QUEUE_ASK_CORNERS,
    COMPUTE_QUEUE_ASK_CORNERS_PARKED, COMPUTE_QUEUE_ASK_LIGHT_CORNERS,
    COMPUTE_QUEUE_ASK_LIGHT_CORNERS_PARKED, COMPUTE_QUEUE_ASK_LIGHT_SAMPLES,
    COMPUTE_QUEUE_ASK_LIGHT_SAMPLES_PARKED, COMPUTE_QUEUE_ASK_PARKED, COMPUTE_QUEUE_ASK_SAMPLES,
    COMPUTE_QUEUE_ASK_SAMPLES_PARKED, COMPUTE_QUEUE_CORNERS, COMPUTE_QUEUE_CORNERS_PARKED,
    COMPUTE_QUEUE_LAST, COMPUTE_QUEUE_LAST_CORNERS, COMPUTE_QUEUE_LAST_LIGHT_CORNERS,
    COMPUTE_QUEUE_LAST_LIGHT_SAMPLES, COMPUTE_QUEUE_LAST_SAMPLES, COMPUTE_QUEUE_LIGHT_CORNERS,
    COMPUTE_QUEUE_LIGHT_CORNERS_PARKED, COMPUTE_QUEUE_LIGHT_SAMPLES,
    COMPUTE_QUEUE_LIGHT_SAMPLES_PARKED, COMPUTE_QUEUE_PARKED, COMPUTE_QUEUE_SAMPLES,
    COMPUTE_QUEUE_SAMPLES_PARKED,
};
use crate::frame::shader::word::SHADOW_SIZE;
use crate::frame::shader::word::{screen, CELL_SIDE, ROW_LANES, SQUARE_SIDE};
use crate::gpu::api;
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, release, view_entry, word_bytes, write_buffer_bytes, Gpu,
    RenderContext,
};
use crate::gpu::draw::ReceiverBox;
use crate::gpu::generator::{
    cut_rects, hold, DecorFrame, Generation, Generator, IndirectDraw, Mask,
};
use crate::gpu::pipeline::{Pipeline, Pipelines};
use crate::gpu::read::Read;
use crate::gpu::span_spec::{FrameKind, GpuSpanName};
use crate::gpu::surface::DRAWN_CAP;
use crate::gpu::tiles::Tiles;

/// The GPU's cast this frame (frame/shader/cast.wgsl `cast_cs`), a ray
/// a drawn pixel of the world surface: the eye, the terrain's tiles as the
/// GPU holds them, and what is placed.
pub struct Cast<'a> {
    /// The eye (pxl-engine `eye`; pxl-core domain/view/lens.rs `gpu_cast`,
    /// domain/scene/cast.rs `eye`).
    pub eye: [u32; eye::WORDS],
    /// The clipmap's table (pxl-engine `clipmap::Entry`; pxl-core
    /// domain/layer/terrain/clipmap.rs `table`), four words a row: every clip
    /// level's entries, the eye's first, from the top's, each the brick it
    /// wants, X and Y, the slot holding it plus one while present (`Entry::slot`)
    /// and the fill it is on its way from; then each clip level's first brick
    /// (`clipmap::ORIGIN_AT`).
    pub clipmap: &'a [i32],
    /// The service area's index (pxl-engine `area::Area`), its keys four a
    /// row (pxl-core domain/scene/cast.rs `shards`); handed where it lies,
    /// its writer asking `held` (gpu/api.rs) before it writes it again.
    pub shards: &'a [[i32; 4]],
    pub objects: Objects<'a>,
    /// The decor the GPU generates and draws over the same world (gpu/generator.rs).
    pub decor: DecorFrame<'a>,
    /// The box each drawn thing that reads the shadow map stands within, in the world's Q3, as
    /// its draw is recorded (gpu/draw.rs `Lists::stands`): receivers of the map as decor is
    /// (pxl-engine `light::RECEIVER_BOXES_AT`), taken against the eye's `ORIGIN` as the head is written.
    pub stands: &'a [ReceiverBox],
    /// Whether what the eye's cast and the sun's walked and took is counted
    /// (gpu/ray/count.rs).
    pub ray_counts: bool,
}

/// The overwrites' tree (pxl-core domain/layer/object/store.rs) as it
/// lives: its child pointer and data lanes, handed where they lie, their
/// writer asking `held` (gpu/api.rs) before it writes them again; the room
/// either has before it grows, the revision they are at, and the child
/// groups written since `since`, the revision they were last handed at, by
/// their first slots (`GpuColumns`).
pub struct Objects<'a> {
    pub first_child: &'a [u32],
    pub data: &'a [u32],
    pub cap: usize,
    pub revision: u32,
    pub since: u32,
    pub written_groups: &'a [u32],
}

/// What the shadow map's pass reads of the light's cast (`RaysPass::light_reads`,
/// gpu/shadow.rs `Shadow::begin`): its list, the host's epoch's first cast, and its args.
#[derive(Clone, Copy)]
pub struct LightReads {
    pub list: u32,
    pub epoch: u32,
    pub args: u32,
}

/// What the GPU's cast reads; it writes the world surface's rays.
pub(super) struct CastBuffers {
    pipeline: Pipeline,
    /// The beam rays' run (`corners`), where each cell's rays start off them (`cell_cs`,
    /// a thread a cell), the samples' run from those starts (`samples`): the eye's over
    /// its groups and the cast's group and the shadow map's over the light's (`light_groups`),
    /// each told by the head it binds (`eye_header`, `light_header`); and the eye's buffer
    /// (pxl-engine `beam`): a line a corner, a start a cell, a hit a sample, as the last cast's
    /// `grid` lays them, none a pixel's; room for `beam_cap` words.
    cells: Pipeline,
    /// The eye's beam rays' run through the queue, as the samples': its pipelines and its
    /// groups.
    corners: [Pipeline; 5],
    corners_groups: [u32; 3],
    /// The eye's samples' run through the queue, its passes as a run's (`Run`): its walks over
    /// the node cache alone, its asks, its last walk; its groups.
    samples: [Pipeline; 5],
    samples_groups: [u32; 3],
    /// The same passes for the shadow map (beam.light.wgsl, light_cell.wgsl,
    /// sample.light.wgsl): the cells over the same group, the beam rays' and the samples' runs
    /// as the eye's, each over its own queue's head.
    light_cells: Pipeline,
    light_corners: [Pipeline; 5],
    light_samples: [Pipeline; 5],
    beams: u32,
    /// The eye's head, all zero: a head naming no map (pxl-engine
    /// `light::HEADER_SIZE`), which tells a pass both take the eye's.
    eye_header: u32,
    grid: beam::Grid,
    beam_cap: u32,
    /// The node cache (pxl-engine `node_cache`): its buffer, no
    /// pixel's; the holes' queue (pxl-engine `ray::QUEUE_LANES`): the pixels
    /// the cast's reading walk left to it, holding what the record drew there, counted in
    /// `marks` (pxl-engine `ray::MISS_ARGS_WORDS`) and marked a bit each (`ray::holes_words`),
    /// and the light's slots no voxel answered, walked off it by its passes
    /// (`queue_pipeline` to `queue_last`), the eye's over `queue_groups`, the light's over
    /// `light_groups`, each from its own head of `queues`; the node
    /// cache forgotten (`forget_cs`) where
    /// what the world's query reads changed since the last cast: all of it
    /// (`forget`), or the nodes over the eye's clip levels' entries that moved
    /// (`changed`, a bit an entry, against the entries as `entries` last held them; `changed_bricks`,
    /// the box a clip level's lie in)
    /// and over the boxes of what is placed or cut that changed (`boxes`,
    /// `holding` of them, as decor's generation holds them, gpu/generator.rs
    /// `hold`), and those handed a slot of a child group of what is placed that
    /// was written (`written_groups`, `writes` of them); as this frame's cast does
    /// (`forgets`, pxl-engine `node_cache::FORGETS`), and what it sent, put
    /// back where its frame is dropped (`sent_forget`). `frame` numbers the casts since it was last
    /// forgotten whole, and `cuts` are the preview's cuts it was asked with.
    queue_pipeline: Pipeline,
    queue_ask: Pipeline,
    queue_parked: Pipeline,
    queue_ask_parked: Pipeline,
    queue_last: Pipeline,
    /// The eye's groups of the queue's walks over the node cache alone, its asks and its last
    /// walk, over the cast's group's buffers.
    queue_groups: [u32; 3],
    /// Every run's queue, a head each and the asks' records they share (`ray::QUEUE_WORDS`,
    /// `ray::queue_at`).
    queues: u32,
    marks: u32,
    forget_pipeline: Pipeline,
    node_cache: u32,
    frame: u32,
    forget: bool,
    changed: [u32; node_cache::CHANGED_WORDS as usize],
    changed_bricks: [[i32; 4]; clipmap::EYE_CLIP_LEVELS as usize],
    rects: [[i32; 4]; object::RECTS as usize],
    holding: usize,
    written_groups: [u32; node_cache::WRITTEN_CAP as usize],
    writes: usize,
    sent_forget: Option<ForgetHeader>,
    entries: Vec<i32>,
    forgets: u32,
    cuts: [u32; eye::CUTS],
    /// The world's depth from the light, a texel of the shadow map each
    /// (`light_cs`), cast with the eye's rays where the shadows are on.
    light: Pipeline,
    /// The tiles of the light's box its receivers stand on (pxl-engine
    /// `light::LIST_RECEIVERS`): the eye's voxels' boxes, stood by its record's draw as it writes
    /// them (record_draw.wgsl `record_draw_cs`, `Raster::record_draw`), and the pixels it
    /// walked apart, off their list (`light_mark_walked_cs`); and where the decor and what
    /// the core draws that reads the map stand (`light_mark_draws_cs`), as many threads as its one thread opens
    /// (`light_mark_draws_open_cs`); then those they read (`light_propagate_cs`).
    light_mark_walked: Pipeline,
    light_mark_draws: Pipeline,
    light_mark_draws_open: Pipeline,
    light_propagate: Pipeline,
    /// The tiles read claimed, over the propagation's group (`light_damage_cs`).
    light_damage: Pipeline,
    /// The tiles the box slid onto, or slid into another band, set back to
    /// be read anew (`light_slide_cs`, pxl-engine `Light::keeps`), before
    /// the propagation, where the box slid.
    light_slide: Pipeline,
    /// The light's cast's groups (`light_groups`), and the buffers they bind:
    /// made again exactly when one of them is another.
    light_groups: [u32; LIGHT_GROUPS],
    light_bufs: [u32; 13],
    /// A word bound where decor is off, the decor's buffers the drawn mark
    /// reads none (`light_mark_draws_cs`).
    light_none: u32,
    /// The light's casts so far, the number its marks are made with
    /// (pxl-engine `light::HEADER_CAST`), never zero, which no tile holds
    /// marked.
    light_number: u32,
    /// The number of the cast since which only which tiles are read moved,
    /// and the box (pxl-engine `light::HEADER_EPOCH`): a tile cast in it or
    /// after stands where the box kept it.
    light_epoch: u32,
    /// Whether all the light's cast is made (`light_made`): its list, its
    /// raster's buffers and targets and its groups; while not, no shadow map
    /// is cast.
    light_ready: bool,
    /// The beam's lines, the samples and the misses (pxl-engine
    /// `light::list_words`), made while the shadows are on; 0 while off.
    light_list: u32,
    /// The holes' count and the counts' lanes, as the eye's are
    /// (pxl-engine `ray::MISS_ARGS_WORDS`).
    light_misses: u32,
    /// The workgroups of the passes that cast its tiles, none, or each whole
    /// where a tile is claimed, and what propagation claims and counts
    /// (pxl-engine `light::args_words`), made as `light_list` is.
    light_args: u32,
    /// The drawn receivers' dispatch (`light::DRAWS_DISPATCH_WORDS`), a buffer of its own, apart
    /// from the light's args the pass that reads it writes.
    light_draws_dispatch: u32,
    /// The stood list's args: the propagation's dispatch and the count, and the slide's
    /// (pxl-engine `light::RECEIVER_TILES_WORDS`), made as `light_list` is.
    light_receiver_tiles: u32,
    /// The number of the cast the world's receivers were last stood in
    /// (`light::HEADER_WORLD`), and of the last cast that owed a slide (`light::HEADER_SLIDE_CAST`).
    light_world_number: u32,
    light_slide_cast: u32,
    /// What a tile of the shadow map holds follows, as the light's cast last
    /// read it (`Read`): the light's frame (`Shadow::generation`), whether
    /// shadows are cast, and whether the world under it moved whole; one
    /// moving begins an epoch (`light_epoch`). The box follows the eye a tile at a
    /// time, and a tile's band is its place in it alone (`Light::band`), so nothing else of the
    /// eye's place moves what a tile holds: where the box slid within the epoch
    /// (`schedule::Light::slid`), the reads move with it, and the tiles it slid onto or into
    /// another band are cast (`light_slide_cs`, pxl-engine `Light::keeps`), with those read
    /// anew.
    light_content: Read,
    /// The reads as the cast before left them, copied out where the box slid
    /// to be copied back moved with it.
    light_reads_was: u32,
    /// Which tiles the receivers read, as it last read it: the eye's place
    /// and look, the boxes the host's draws stand in, decor placed again. One moving alone
    /// casts only the tiles read anew.
    light_receivers: Read,
    /// The host's draws' boxes within the effects' reach this frame (`ReceiverTiles`): what the
    /// receivers read of them and the head is handed.
    light_receiver_boxes: Vec<ReceiverTiles>,
    /// What it is handed beside the eye's words (pxl-engine
    /// `light::HEADER_WORDS`).
    light_header: u32,
    /// The world's columns that changed this frame, units from the eye (pxl-engine
    /// `light::column_of`): the cast sets again only the tiles whose rays may meet them
    /// (`Light::reaches`), where the epoch goes on.
    light_columns: Vec<light::Column>,
    /// What each frame runs of the eye's rays and the light's, and the state its plans step
    /// (ray/schedule.rs).
    schedule: Schedule,
    /// Whether the record's draw ran this frame, once at most (`Raster::record_draw`).
    draws_record: bool,
    /// Words the record's group binds in place of the light's list, args and stood list's
    /// args while the light's cast is not made, which no thread reads, the head saying no
    /// receiver stands (`Raster::record_group`).
    light_idle: [u32; 3],
    /// Whether the cast draws a receiver past the world's, a character or decor
    /// (`light_mark_draws_cs`).
    light_draws: bool,
    /// Its group 0, and the buffers it binds (`cast_group`): made again
    /// exactly when one of them is another.
    group: u32,
    bindings: [u32; 11],
    eye: u32,
    pub(super) clipmap: u32,
    shards: u32,
    /// Shards the buffer holds.
    shard_cap: usize,
    /// The drawn pixels it casts, a ray each: the world surface's.
    size: [u32; 2],
    /// The overwrites' child pointer lane then their data lane, each `object_cap`
    /// words, the lanes' own room; the revision it holds, none at first.
    objects: u32,
    object_cap: usize,
    object_revision: Option<u32>,
    generator: Generator,
    /// What the last cast read (`Read`) but the jitter and the grid, and
    /// whether this frame's is the same (`schedule::Reads::same`).
    read: Read,
    /// The samples' hits handed to the rasterizer (ray/raster.rs), whose
    /// visible voxel at each pixel the pixels read (`cast_cs`), over a group
    /// of their own (`px_group`): the eye, the rays, the beam's words, the
    /// node cache, the visibility buffer, the claimed tiles and the record (`px_bufs`),
    /// made again exactly when one of them is another.
    raster: Raster,
    px_group: u32,
    px_bufs: [u32; 7],
    /// The claimed tiles, listed ahead of the pixels in their pass, and the words their
    /// dispatch takes (cast_tiles.wgsl), made again with the tiles (`px_bufs`).
    cast_tiles_pipeline: Pipeline,
    cast_tiles: u32,
    cast_tiles_group: u32,
    /// What the light's cast and the eye's recast, counted where the setting
    /// asks (ray/count.rs).
    counter: Counter,
}

/// What a cast's head handed the node cache's forgetting (pxl-engine
/// `node_cache::HEADER_WORDS`): whether all of it, the entries that moved, the
/// boxes and the child groups written; put back where its frame is dropped.
struct ForgetHeader {
    all: bool,
    changed: [u32; node_cache::CHANGED_WORDS as usize],
    changed_bricks: [[i32; 4]; clipmap::EYE_CLIP_LEVELS as usize],
    rects: [[i32; 4]; object::RECTS as usize],
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

const CAST_WORKGROUP: [u32; 3] = [ray::CAST_SIDE, ray::CAST_SIDE, 1];
/// A tile's workgroups of `cast_cs`, across and down.
const TILE_CASTS: u32 = screen::TILE / ray::CAST_SIDE;
const _: () = assert!(
    screen::TILE.is_multiple_of(ray::CAST_SIDE),
    "a tile is a whole number of the cast's workgroups"
);
const _: () = assert!(
    floor::fits(
        word_bytes(
            CAST_TILES_START.len()
                + (DRAWN_CAP[0].div_ceil(screen::TILE) * DRAWN_CAP[1].div_ceil(screen::TILE))
                    as usize
        ) as u64,
        true
    ),
    "every buffer the drawn pixels size fits the floor at the most drawn"
);
/// A box a host's draw stands within, within the effects' reach: its corners in units from the
/// eye, as the light's passes read them (`ReceiverBox::from_eye`), and the first and last tiles of the
/// light's box it stands on (pxl-engine `Light::box_tiles`).
#[derive(Clone, Copy)]
struct ReceiverTiles {
    bounds: [[f32; 3]; 2],
    tiles: [[i32; 2]; 2],
}

/// The claimed tiles' list as a frame's cast starts: `cast_cs`'s dispatch, no tile yet, each
/// one's workgroups across and down, and none counted (cast_tiles.wgsl).
const CAST_TILES_START: [u32; 4] = [0, TILE_CASTS, TILE_CASTS, 0];
/// The light's cast's groups (`light_groups`).
const LIGHT_GROUPS: usize = 17;

impl CastBuffers {
    /// The frame's buffer `buf` (frame/shader/group.rs `BUFFERS`) the cast holds, 0 for
    /// another's.
    fn buf_of(&self, buf: u32) -> u32 {
        match buf {
            MARKS => self.marks,
            NODE_CACHE => self.node_cache,
            QUEUES => self.queues,
            CAST_TILES => self.cast_tiles,
            BEAMS => self.beams,
            LIGHT_ARGS => self.light_args,
            LIGHT_LIST => self.light_list,
            LIGHT_RECEIVER_TILES => self.light_receiver_tiles,
            LIGHT_DRAWS => self.light_draws_dispatch,
            LIGHT_MISSES => self.light_misses,
            LIGHT_RASTER_ARGS => self.raster.light.args,
            _ => 0,
        }
    }
}

impl RaysPass {
    /// The cast pass's pipelines and buffers, made once with the renderer,
    /// before its first frame (gpu/pipeline.rs): no frame names a pipeline
    /// until every one is made.
    pub fn make(&mut self, g: &Gpu, pipelines: &mut Pipelines) {
        if self.cast.is_some() {
            return;
        }
        // One group for every kernel over the cast's world but the pixels'.
        let group = &WORLD_GROUP;
        let pipeline = pipelines.build_compute(g, COMPUTE_CAST, &CAST_GROUP);
        let cast_tiles_pipeline = pipelines.build_compute(g, COMPUTE_CAST_TILES, &CAST_TILES_GROUP);
        let cells = pipelines.build_compute(g, COMPUTE_CELL, group);
        let mut run =
            |[walk, ask, walk_parked, ask_parked, last]: [u32; 5],
             [walk_group, ask_group, last_group]: [&'static [u32]; 3]| {
                [
                    pipelines.build_compute(g, walk, walk_group),
                    pipelines.build_compute(g, ask, ask_group),
                    pipelines.build_compute(g, walk_parked, walk_group),
                    pipelines.build_compute(g, ask_parked, ask_group),
                    pipelines.build_compute(g, last, last_group),
                ]
            };
        let samples = run(
            [
                COMPUTE_QUEUE_SAMPLES,
                COMPUTE_QUEUE_ASK_SAMPLES,
                COMPUTE_QUEUE_SAMPLES_PARKED,
                COMPUTE_QUEUE_ASK_SAMPLES_PARKED,
                COMPUTE_QUEUE_LAST_SAMPLES,
            ],
            [&LATTICE_WALK_GROUP, &LATTICE_ASK_GROUP, &LATTICE_LAST_GROUP],
        );
        let light_samples = run(
            [
                COMPUTE_QUEUE_LIGHT_SAMPLES,
                COMPUTE_QUEUE_ASK_LIGHT_SAMPLES,
                COMPUTE_QUEUE_LIGHT_SAMPLES_PARKED,
                COMPUTE_QUEUE_ASK_LIGHT_SAMPLES_PARKED,
                COMPUTE_QUEUE_LAST_LIGHT_SAMPLES,
            ],
            [
                &LIGHT_LATTICE_WALK_GROUP,
                &LATTICE_ASK_GROUP,
                &LIGHT_LATTICE_LAST_GROUP,
            ],
        );
        let corners = run(
            [
                COMPUTE_QUEUE_CORNERS,
                COMPUTE_QUEUE_ASK_CORNERS,
                COMPUTE_QUEUE_CORNERS_PARKED,
                COMPUTE_QUEUE_ASK_CORNERS_PARKED,
                COMPUTE_QUEUE_LAST_CORNERS,
            ],
            [&LATTICE_WALK_GROUP, &LATTICE_ASK_GROUP, &LATTICE_LAST_GROUP],
        );
        let light_corners = run(
            [
                COMPUTE_QUEUE_LIGHT_CORNERS,
                COMPUTE_QUEUE_ASK_LIGHT_CORNERS,
                COMPUTE_QUEUE_LIGHT_CORNERS_PARKED,
                COMPUTE_QUEUE_ASK_LIGHT_CORNERS_PARKED,
                COMPUTE_QUEUE_LAST_LIGHT_CORNERS,
            ],
            [
                &LIGHT_LATTICE_WALK_GROUP,
                &LATTICE_ASK_GROUP,
                &LIGHT_LATTICE_LAST_GROUP,
            ],
        );
        let light_cells = pipelines.build_compute(g, COMPUTE_LIGHT_CELL, group);
        let queue_pipeline = pipelines.build_compute(g, COMPUTE_QUEUE, &QUEUE_GROUP);
        let queue_parked = pipelines.build_compute(g, COMPUTE_QUEUE_PARKED, &QUEUE_GROUP);
        let queue_ask = pipelines.build_compute(g, COMPUTE_QUEUE_ASK, &QUEUE_ASK_GROUP);
        let queue_ask_parked =
            pipelines.build_compute(g, COMPUTE_QUEUE_ASK_PARKED, &QUEUE_ASK_GROUP);
        let queue_last = pipelines.build_compute(g, COMPUTE_QUEUE_LAST, &QUEUE_LAST_GROUP);
        let forget_pipeline = pipelines.build_compute(g, COMPUTE_FORGET, group);
        let light = pipelines.build_compute(g, COMPUTE_LIGHT, &LIGHT_GROUP);
        let light_mark_walked =
            pipelines.build_compute(g, COMPUTE_LIGHT_MARK_WALKED, &LIGHT_MARK_GROUP);
        let light_mark_draws =
            pipelines.build_compute(g, COMPUTE_LIGHT_MARK_DRAWS, &LIGHT_MARK_DRAWS_GROUP);
        let light_mark_draws_open = pipelines.build_compute(
            g,
            COMPUTE_LIGHT_MARK_DRAWS_OPEN,
            &LIGHT_MARK_DRAWS_OPEN_GROUP,
        );
        let light_propagate =
            pipelines.build_compute(g, COMPUTE_LIGHT_PROPAGATE, &LIGHT_PROPAGATE_GROUP);
        let light_damage = pipelines.build_compute(g, COMPUTE_LIGHT_DAMAGE, &LIGHT_PROPAGATE_GROUP);
        let light_slide = pipelines.build_compute(g, COMPUTE_LIGHT_SLIDE, &LIGHT_SLIDE_GROUP);
        let raster = Raster::new(g, pipelines);
        let counter = Counter::new(g);
        let buf = |size: u32, usage: u32| g.buf(size, usage);
        let storage = buffer_usage::STORAGE | buffer_usage::COPY_DST;
        let c = CastBuffers {
            pipeline,
            cast_tiles_pipeline,
            cast_tiles: 0,
            cast_tiles_group: 0,
            cells,
            samples,
            samples_groups: [0; 3],
            corners,
            corners_groups: [0; 3],
            light_corners,
            light_cells,
            light_samples,
            beams: 0,
            // Never written: a buffer is made zeroed.
            eye_header: buf(word_bytes(light::HEADER_WORDS), buffer_usage::UNIFORM),
            grid: beam::Grid::of_words([0.0; 2], [0; eye::GRID_WORDS]),
            beam_cap: 0,
            queue_pipeline,
            queue_ask,
            queue_parked,
            queue_ask_parked,
            queue_last,
            queue_groups: [0; 3],
            queues: buf(
                word_bytes(ray::QUEUE_WORDS as usize),
                storage | buffer_usage::COPY_SRC,
            ),
            // The holes' count and the counts' lanes.
            marks: buf(
                word_bytes(ray::MISS_ARGS_WORDS),
                storage | buffer_usage::COPY_SRC,
            ),
            forget_pipeline,
            node_cache: buf(
                word_bytes(node_cache::WORDS as usize),
                storage | buffer_usage::COPY_SRC,
            ),
            frame: 0,
            forget: true,
            changed: [0; node_cache::CHANGED_WORDS as usize],
            changed_bricks: [node_cache::NO_BRICKS; clipmap::EYE_CLIP_LEVELS as usize],
            rects: [[0; 4]; object::RECTS as usize],
            holding: 0,
            written_groups: [0; node_cache::WRITTEN_CAP as usize],
            writes: 0,
            sent_forget: None,
            entries: Vec::new(),
            forgets: node_cache::FORGETS_NONE,
            cuts: [0; eye::CUTS],
            light,
            light_mark_walked,
            light_mark_draws,
            light_mark_draws_open,
            light_propagate,
            light_damage,
            light_slide,
            light_groups: [0; LIGHT_GROUPS],
            light_bufs: [0; 13],
            light_none: buf(word_bytes(2 * generation::drawn::REGIONS as usize), storage),
            light_number: 0,
            light_epoch: 0,
            light_ready: false,
            light_list: 0,
            light_misses: buf(
                word_bytes(ray::MISS_ARGS_WORDS),
                storage | buffer_usage::COPY_SRC,
            ),
            light_args: 0,
            light_draws_dispatch: 0,
            light_receiver_tiles: 0,
            light_world_number: 0,
            light_slide_cast: 0,
            light_content: Read::default(),
            light_reads_was: buf(
                word_bytes((light::tiles(SHADOW_SIZE) * light::tiles(SHADOW_SIZE)) as usize),
                buffer_usage::COPY_SRC | buffer_usage::COPY_DST,
            ),
            light_receivers: Read::default(),
            light_receiver_boxes: Vec::with_capacity(light::RECEIVER_BOXES),
            light_header: buf(
                word_bytes(light::HEADER_WORDS),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
            light_columns: Vec::with_capacity(light::COLUMNS),
            schedule: Schedule::new(),
            draws_record: false,
            light_idle: [0; 3].map(|_| {
                buf(
                    word_bytes(light::RECEIVER_TILES_WORDS as usize),
                    storage | buffer_usage::COPY_SRC,
                )
            }),
            light_draws: false,
            group: 0,
            bindings: [0; 11],
            eye: buf(
                word_bytes(eye::WORDS),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
            clipmap: buf(
                word_bytes(clipmap::TABLE_WORDS as usize),
                storage | buffer_usage::COPY_SRC,
            ),
            shards: 0,
            shard_cap: 0,
            size: [0; 2],
            objects: 0,
            object_cap: 0,
            object_revision: None,
            generator: Generator::new(g, pipelines),
            read: Read::default(),
            raster,
            px_group: 0,
            px_bufs: [0; 7],
            counter,
        };
        self.cast = Some(c);
    }

    /// The rays for this frame into the world surface's (`rays`, its view,
    /// and its drawn pixels), and the world's depth from the light into `light`'s
    /// view: the GPU's cast where an eye casts, its eye handed over first;
    /// its samples' raster through its gate where the frame is `partial`;
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
        partial: bool,
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
            if let Some(c) = self.cast.as_mut() {
                c.generator.off();
                c.schedule.off();
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
            objects:
                Objects {
                    first_child: _,
                    data: _,
                    cap,
                    revision,
                    since: _,
                    written_groups: _,
                },
            decor,
            // The light's alone, which reads them with the look (below).
            stands: _,
            ray_counts,
        } = &cast;
        let ray_counts = *ray_counts;
        let r = &mut c.read;
        r.begin();
        // The words a cast is from: not the light's, its cast's alone
        // (below), the jitter's or the grid's (pxl-engine `eye::GRID`).
        r.read(&eye[..eye::LIGHT]);
        let clipmap_changed = r.read(clipmap);
        let shards_changed = r.read(shards);
        // A surface made again holds no rays.
        r.read(&[*cap as u32, *revision, rays.0]);
        let same = r.end() && c.group != 0;
        let stands = c.schedule.stands(same);
        xpute_kit::ensure!(
            cast.clipmap.len() == clipmap::TABLE_WORDS as usize,
            ENOTRECOVERABLE,
            cast.clipmap.len()
        );
        if clipmap_changed {
            g.write_buf(c.clipmap, 0, bytes(cast.clipmap));
        }
        c.size = rays.1;
        let shards_grow = cast.shards.len() > c.shard_cap;
        if shards_grow {
            if c.shards != 0 {
                release(c.shards);
            }
            c.shard_cap = cast.shards.len();
            c.shards = g.buf(
                word_bytes(4 * c.shard_cap),
                buffer_usage::STORAGE | buffer_usage::COPY_DST,
            );
        }
        if shards_changed || shards_grow {
            // One record however large.
            g.upload_buf(c.shards, 0, bytes(cast.shards));
        }
        let o = &cast.objects;
        let objects_grow = o.cap != c.object_cap;
        if objects_grow {
            if c.objects != 0 {
                release(c.objects);
            }
            c.object_cap = o.cap;
            c.objects = g.buf(
                word_bytes(2 * o.cap),
                buffer_usage::STORAGE | buffer_usage::COPY_DST,
            );
            c.object_revision = None;
        }
        let objects_changed = c.object_revision != Some(o.revision);
        if objects_changed {
            // Held at `since`: the child groups written after it alone, a record
            // each, where the stream has room for them all; else both lanes
            // whole, two records however large.
            fn child_group(lane: &[u32], first: u32) -> &[u32] {
                &lane[first as usize..(first + object::CHILDREN) as usize]
            }
            // The child groups written since the revision the node cache was asked
            // at, which the nodes handed one of their slots read again; past
            // the room, or where revisions went by unseen, all of it.
            if c.object_revision == Some(o.since) {
                for &first in o.written_groups {
                    c.forget |= !note(&mut c.written_groups, &mut c.writes, first);
                }
            } else {
                c.forget = true;
            }
            let list_of = |lane: &[u32]| {
                o.written_groups
                    .iter()
                    .map(|&f| write_buffer_bytes(bytes(child_group(lane, f))))
                    .sum::<usize>()
            };
            if c.object_revision == Some(o.since)
                && api::room(list_of(o.first_child) + list_of(o.data))
            {
                for &first in o.written_groups {
                    g.write_buf(
                        c.objects,
                        word_bytes(first as usize),
                        bytes(child_group(o.first_child, first)),
                    );
                    g.write_buf(
                        c.objects,
                        word_bytes(o.cap + first as usize),
                        bytes(child_group(o.data, first)),
                    );
                }
            } else {
                g.upload_buf(c.objects, 0, bytes(o.first_child));
                g.upload_buf(c.objects, word_bytes(o.cap), bytes(o.data));
            }
            c.object_revision = Some(o.revision);
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
        let leaf = |b: [i32; 4]| {
            light::Column::every_band(light::column_of(
                origin,
                LEAF_CELL_ZOOM,
                [b[0], b[1]],
                [b[2], b[3]],
            ))
        };
        for &b in edits {
            hold(&mut c.rects, &mut c.holding, b);
            put_column(columns, leaf(b));
        }
        cut_rects(&c.cuts, &cuts, |b| {
            hold(&mut c.rects, &mut c.holding, b);
            put_column(columns, leaf(b));
        });
        // What the node cache kept is forgotten where the service area
        // or what is placed moved past the boxes; so it is once its frames
        // would outrun a key's tag.
        c.forget |= shards_changed
            || shards_grow
            || objects_grow
            || (objects_changed && edits.is_empty())
            || c.frame >= node_cache::FRAMES;
        let cuts_changed = cuts != c.cuts;
        c.cuts = cuts;
        // The eye's clip levels' entries that hold another brick, or hold it
        // present or no longer: what is over them is asked again.
        let eye_rows = (clipmap::EYE_CLIP_LEVELS * clipmap::ENTRIES) as usize;
        let rows = &cast.clipmap[..4 * eye_rows];
        let entry = |r: &[i32]| clipmap::Entry::of([r[0], r[1], r[2], r[3]]);
        let present = |cell_zoom: Zoom, q: [i32; 2]| {
            let k = ((cell_zoom - clipmap::FIRST_CELL_ZOOM) * clipmap::ENTRIES + clipmap::entry(q))
                as usize;
            entry(&rows[4 * k..4 * k + 4]).holds(q)
        };
        // Of the clip levels, a walk's hit reads the cells of the bricks
        // they hold present, each at its own place, every other read a
        // bound over them that holds whichever of them are: so they moved
        // where an entry holds a brick present it did not, or no longer
        // holds present the brick it holds. A clip level following its
        // center a brick moves none of it: the brick it lets go and the one
        // it takes, not present yet, each lie past what its zoom observes
        // about the eye (pxl-engine `clipmap`), and are read off the
        // coarser one alike.
        let mut present_changed = false;
        if clipmap_changed {
            for k in 0..eye_rows {
                let row = &rows[4 * k..4 * k + 4];
                let was = c.entries.get(4 * k..4 * k + 4);
                if was.is_none_or(|w| entry(row).changed_from(entry(w))) {
                    c.changed[k / 32] |= 1 << (k % 32);
                }
                if was.is_none_or(|w| entry(row).held_changed_from(entry(w))) {
                    let level = &mut c.changed_bricks[k / clipmap::ENTRIES as usize];
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
                let cell_zoom = clipmap::FIRST_CELL_ZOOM + (k / clipmap::ENTRIES as usize) as u32;
                let mut changed = |q: [i32; 2]| {
                    let finer = |q: [i32; 2]| present(cell_zoom + 1, q);
                    light::brick_columns(origin, cell_zoom, q, finer, |v| {
                        put_column(&mut c.light_columns, v);
                    });
                };
                if taken {
                    changed(now.0);
                }
                if let (true, Some((q, _))) = (gone, was) {
                    changed(q);
                }
                present_changed |= match was {
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
        // hold present, the service area, what is placed, a preview's cuts.
        let world_changed =
            present_changed || shards_changed || shards_grow || objects_changed || cuts_changed;
        // Decor generated first: where it stands is what the light's cast reads
        // of it, and its buffers what the light's groups bind.
        let world = [c.eye, c.clipmap, ground, c.shards, c.objects, pool];
        let generation = Generation {
            clipmap: cast.clipmap,
            cuts: cast.eye[eye::CARVE..eye::CARVE + eye::CUTS]
                .try_into()
                .or_bug(Errno::ENOTRECOVERABLE),
            fresh: self
                .ground
                .as_ref()
                .map_or(&[][..], |gb| &gb.inheritance[..]),
            eye: &cast.eye,
        };
        c.generator
            .prepare(g, cast.decor, generation, meshes, world);
        // The shadow map is cast, every slot a receiver reads, as the eye's
        // rays are (pxl-engine `light`), where what it reads moved: the
        // light's frame or the world under it; where the box slid or the eye's
        // zooms, which every band is held to, moved, either keeping a tile
        // whose band stands (`Light::keeps`); and where what says which
        // tiles are read moved, the eye's place and look, the boxes the host's draws stand in
        // and decor placed again (`record_draw_cs`, `light_mark_walked_cs`,
        // `light_mark_draws_cs`). A
        // standing frame casts none. Where only the latter two moved, a tile
        // cast since anything else last did stands where the box kept it,
        // and only the tiles read anew are cast (`light::HEADER_EPOCH`). None
        // where no shadow is cast, nor while its cast is not made
        // (`light_made`), its read let go until then.
        let shadows = cast.eye[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS]
            .iter()
            .any(|&w| w != 0);
        // All it reads but the look, then the look.
        let r = &mut c.light_content;
        r.begin();
        r.read(&[light_map.1, u32::from(shadows)]);
        // The world under it moved whole where the node cache forgets it whole, or where no
        // column says what changed; else only the tiles its columns reach are cast again.
        let world_whole = world_changed && (c.forget || c.light_columns.is_empty());
        r.mark(world_whole);
        let content_same = r.end();
        let world_columns = world_changed && !world_whole;
        // The box, a tile at a time about the eye: what a tile holds moves
        // with its corner, kept where the slide kept its band
        // (`light_slide_cs`).
        let corner = light::CORNER.map(|k| cast.eye[eye::LIGHT + k] as i32);
        // Where the host's draws stand, each box units from the eye and the tiles it stands on
        // by the one rule every receiver stands by (pxl-engine `Light::box_tiles`, the
        // footprint where the eye stands, as the head asks it below), on the light the cast is
        // aimed by; a box past the effects' reach stands on none and is left out, so what moves
        // past it never casts the light. None where no shadow is cast.
        c.light_receiver_boxes.clear();
        if shadows {
            let words = cast.eye[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS]
                .try_into()
                .or_bug(Errno::ENOTRECOVERABLE);
            let lit = light::Light::of_words(words);
            let origin: [i32; 3] = core::array::from_fn(|k| cast.eye[eye::ORIGIN + k] as i32);
            for s in cast.stands.iter().take(light::RECEIVER_BOXES) {
                let [lo, hi] = s.from_eye(origin);
                if let Some(tiles) = lit.box_tiles(SHADOW_SIZE, lo, hi, stands) {
                    c.light_receiver_boxes.push(ReceiverTiles {
                        bounds: [lo, hi],
                        tiles,
                    });
                }
            }
        }
        // The eye's place and look, the tiles the host's draws stand on and decor compacted
        // again: which tiles are read, not what a tile holds. Not the jitter: a pixel's stands
        // hold its footprint (`light_mark_walked_cs`).
        let l = &mut c.light_receivers;
        l.begin();
        l.read(&cast.eye[eye::ORIGIN..eye::ORIGIN + 3]);
        l.read(&cast.eye[eye::FORWARD..eye::TAN + 2]);
        for s in &c.light_receiver_boxes {
            l.read(s.tiles.as_flattened());
        }
        l.mark(c.generator.counts());
        let receivers_same = l.end();
        // What the frame runs of the eye's rays and the light's (ray/schedule.rs).
        let plan = c.schedule.plan(Reads {
            same,
            jitter: [cast.eye[eye::JITTER], cast.eye[eye::JITTER + 1]],
            partial,
            shadows,
            light_made: c.light_ready,
            content_same,
            world_columns,
            corner,
            range: [cast.eye[eye::RANGE], cast.eye[eye::RANGE + 1]],
            receivers_same,
        });
        let (kind, casts) = (plan.kind, plan.casts());
        // A cast lays the grid the eye's words hold (pxl-engine `eye::GRID`).
        if kind == FrameKind::cast {
            let tan = [cast.eye[eye::TAN], cast.eye[eye::TAN + 1]].map(f32::from_bits);
            c.grid = beam::Grid::of_words(
                tan,
                cast.eye[eye::GRID..eye::GRID + eye::GRID_WORDS]
                    .try_into()
                    .or_bug(Errno::ENOTRECOVERABLE),
            );
        }
        // The room for the grid's words, made again only where they outgrow
        // it.
        let need = c.grid.walked_at() + ray::holes_words(c.size);
        if need > c.beam_cap {
            if c.beams != 0 {
                release(c.beams);
            }
            let size = word_bytes(need as usize);
            xpute_kit::ensure!(floor::fits(size as u64, true), ENOTRECOVERABLE, need);
            (c.beams, c.beam_cap) = (
                g.buf(size, buffer_usage::STORAGE | buffer_usage::COPY_DST),
                need,
            );
        }
        if let Some(due) = plan.light {
            // The head (pxl-engine `light::HEADER_WORDS`).
            let mut header = [0u32; light::HEADER_WORDS];
            c.light_number = c.light_number.wrapping_add(1).max(1);
            if due.epoch {
                c.light_epoch = c.light_number;
            }
            header[light::HEADER_CAST] = c.light_number;
            header[light::HEADER_EPOCH] = c.light_epoch;
            header[light::HEADER_COUNTED] = u32::from(ray_counts);
            for (k, v) in light::header_layout(SHADOW_SIZE) {
                header[k] = v;
            }
            header[light::HEADER_LAST_CORNER] = due.last_corner[0] as u32;
            header[light::HEADER_LAST_CORNER + 1] = due.last_corner[1] as u32;
            header[light::HEADER_LAST_RANGE] = due.last_range[0];
            header[light::HEADER_LAST_RANGE + 1] = due.last_range[1];
            // The world's receivers stood afresh in this cast where the walked bits
            // holds every walk; or added to those that stand, which hold the rest's, where it
            // holds the claimed tiles' alone; else on where they were last. And the last cast
            // before it that owed a slide (light_receiver_tiles.wgsl `receiver_tiles_first`).
            let afresh = due.world && !due.adds;
            if afresh {
                c.light_world_number = c.light_number;
            }
            header[light::HEADER_WORLD] = c.light_world_number;
            header[light::HEADER_SLIDE_CAST] = c.light_slide_cast;
            let owes = due.slid || due.reached;
            if owes {
                c.light_slide_cast = c.light_number;
            }
            // The footprints taken where the eye stands; while it moves, the next frame's rays
            // mark again, and the frame it stands in owes them.
            header[light::HEADER_FOOTPRINT] = u32::from(due.footprint);
            header[light::HEADER_WORLD_RECEIVERS] = u32::from(due.world);
            let at = 4 * light::STARTS_AT;
            header[at..at + 4 * light::STARTS_VEC4S].copy_from_slice(&light_map.2.words());
            // What the host draws that reads the map, receivers as decor is, a box each
            // (`light_mark_draws_cs`), in units from the eye's origin, the Q3 the
            // generated decor is placed against, rounded outward (`ReceiverBox::from_eye`).
            // Those within the effects' reach alone (`light_receiver_boxes`).
            let at = 4 * light::RECEIVER_BOXES_AT;
            let n = c.light_receiver_boxes.len();
            c.light_draws = n != 0 || c.generator.instances().is_some();
            header[at] = n as u32;
            for (
                k,
                &ReceiverTiles {
                    bounds: [lo, hi], ..
                },
            ) in c.light_receiver_boxes.iter().enumerate()
            {
                let w = at + 4 * (1 + 2 * k);
                header[w..w + 3].copy_from_slice(&lo.map(f32::to_bits));
                header[w + 4..w + 7].copy_from_slice(&hi.map(f32::to_bits));
            }
            // The world's columns that changed, where the epoch goes on (`light_slide_cs`).
            let at = 4 * light::COLUMNS_AT;
            if due.reached {
                header[at] = c.light_columns.len() as u32;
                for (k, col) in c.light_columns.iter().enumerate() {
                    let w = at + 4 * (1 + 2 * k);
                    header[w..w + 4].copy_from_slice(&col.at.map(f32::to_bits));
                    header[w + 4..w + 6].copy_from_slice(&col.bands);
                }
            }
            g.write_buf(c.light_header, 0, bytes(&header));
            let open = |s: &Step| s.to_at();
            g.write_buf(
                c.light_misses,
                open(&sequence::OPEN_LIGHT_MISSES),
                bytes(&ray::MISS_ARGS_EMPTY),
            );
            g.write_buf(
                c.queues,
                open(&sequence::OPEN_LIGHT_QUEUE),
                bytes(&ray::QUEUE_EMPTY),
            );
            g.write_buf(
                c.light_args,
                open(&sequence::OPEN_LIGHT_ARGS),
                bytes(&light_args()),
            );
            // The stood list empty where the world's receivers are stood afresh, and none of it
            // propagated, the read list empty with it (`light_args`); else what the casts since
            // left copied back in as the cast begins (`dispatch`); the slide's rows where it owes
            // one.
            let stood = light_receiver_tiles(owes);
            let open = if afresh {
                &sequence::OPEN_LIGHT_RECEIVER_TILES_AFRESH
            } else {
                &sequence::OPEN_LIGHT_RECEIVER_TILES
            };
            let from = open.to.at as usize;
            g.write_buf(c.light_receiver_tiles, open.to_at(), bytes(&stood[from..]));
            if afresh {
                g.write_buf(
                    c.light_list,
                    sequence::OPEN_RECEIVER_TILES_PROPAGATED.to_at(),
                    bytes(&[0u32]),
                );
            }
            c.raster
                .light_prepare(g, &cast.eye, SHADOW_SIZE, &light_map.2);
        }
        // No receiver stands where the light casts none: the head's flag the record's draw reads
        // every frame (`light::HEADER_WORLD_RECEIVERS`).
        if plan.light.is_none() {
            g.write_buf(
                c.light_header,
                word_bytes(light::HEADER_WORLD_RECEIVERS),
                bytes(&[0u32]),
            );
        }
        // The eye's words with the grid its cast laid (`grid`).
        if casts || plan.light.is_some() {
            let mut words = cast.eye;
            words[eye::GRID..].copy_from_slice(&c.grid.words());
            g.write_buf(c.eye, 0, bytes(&words));
        }
        if casts {
            c.forgets = if c.forget {
                node_cache::FORGETS_ALL
            } else if c.changed.iter().any(|&m| m != 0) || c.holding != 0 || c.writes != 0 {
                node_cache::FORGETS_CHANGED
            } else {
                node_cache::FORGETS_NONE
            };
            // The head before the frame's dispatches: what it forgets and over
            // which entries; forgotten whole, its first frame, else the next.
            let mut header = [0u32; node_cache::HEADER_WORDS as usize];
            c.frame = if c.forgets == node_cache::FORGETS_ALL {
                1
            } else {
                c.frame + 1
            };
            header[node_cache::FRAME as usize] = c.frame;
            header[node_cache::FORGETS as usize] = c.forgets;
            let rects = node_cache::RECTS_AT as usize;
            let bricks = node_cache::CHANGED_BRICKS_AT as usize;
            header[node_cache::CHANGED_AT as usize..bricks].copy_from_slice(&c.changed);
            for (k, b) in c.changed_bricks.iter().enumerate() {
                let at = bricks + 4 * k;
                header[at..at + 4].copy_from_slice(&b.map(|v| v as u32));
            }
            header[rects] = c.holding as u32;
            for (k, b) in c.rects[..c.holding].iter().enumerate() {
                let at = rects + 1 + 4 * k;
                header[at..at + 4].copy_from_slice(&b.map(|v| v as u32));
            }
            let written = node_cache::WRITTEN_AT as usize;
            header[written] = c.writes as u32;
            header[written + 1..written + 1 + c.writes]
                .copy_from_slice(&c.written_groups[..c.writes]);
            // The hits' dispatch none, a workgroup deep and high.
            header[node_cache::HITS_ARGS as usize + 1] = 1;
            header[node_cache::HITS_ARGS as usize + 2] = 1;
            g.write_buf(
                c.node_cache,
                sequence::OPEN_NODE_CACHE.to_at(),
                bytes(&header),
            );
            g.write_buf(
                c.marks,
                sequence::OPEN_MARKS.to_at(),
                bytes(&ray::MISS_ARGS_EMPTY),
            );
            g.write_buf(
                c.queues,
                sequence::OPEN_QUEUE.to_at(),
                bytes(&ray::QUEUE_EMPTY),
            );
            c.sent_forget = Some(ForgetHeader {
                all: c.forgets == node_cache::FORGETS_ALL,
                changed: c.changed,
                changed_bricks: c.changed_bricks,
                rects: c.rects,
                holding: c.holding,
                written_groups: c.written_groups,
                writes: c.writes,
            });
            (c.forget, c.changed) = (false, [0; node_cache::CHANGED_WORDS as usize]);
            c.changed_bricks = [node_cache::NO_BRICKS; clipmap::EYE_CLIP_LEVELS as usize];
            (c.holding, c.writes) = (0, 0);
        } else {
            c.sent_forget = None;
        }
        // Each run's queue words fresh, the eye's holes' with the head above where it casts.
        let light_casts = plan.light.is_some();
        if kind == FrameKind::cast {
            g.write_buf(
                c.queues,
                sequence::OPEN_CORNERS_QUEUE.to_at(),
                bytes(&ray::QUEUE_EMPTY),
            );
            g.write_buf(
                c.queues,
                sequence::OPEN_SAMPLES_QUEUE.to_at(),
                bytes(&ray::QUEUE_EMPTY),
            );
        }
        if light_casts {
            g.write_buf(
                c.queues,
                sequence::OPEN_LIGHT_CORNERS_QUEUE.to_at(),
                bytes(&ray::QUEUE_EMPTY),
            );
            g.write_buf(
                c.queues,
                sequence::OPEN_LIGHT_SAMPLES_QUEUE.to_at(),
                bytes(&ray::QUEUE_EMPTY),
            );
        }
        if !casts && light_casts {
            g.write_buf(
                c.node_cache,
                sequence::OPEN_NODE_CACHE_COUNTS.to_at(),
                bytes(&[0u32]),
            );
            // A frame of the node cache's own, as a cast's is: what the frames before kept is
            // then a slot a claim may take (ask.wgsl `claim`). Its tags held
            // within what a key tells apart (`node_cache::FRAMES`), past which the next cast
            // forgets all.
            if c.frame < node_cache::FRAMES {
                c.frame += 1;
                g.write_buf(
                    c.node_cache,
                    sequence::OPEN_NODE_CACHE_FRAME.to_at(),
                    bytes(&[c.frame]),
                );
            }
        }
        let frame = count::Frame {
            number: frame,
            // A full cast's alone, so what a cast asks is not shared out
            // over the frames that cast only the pixels.
            casts: [kind == FrameKind::cast, light_casts],
            rays: [c.size[0] * c.size[1], SHADOW_SIZE * SHADOW_SIZE],
        };
        c.counter.begin(g, ray_counts, c.node_cache, frame);
        let bindings = [
            c.eye,
            c.clipmap,
            ground,
            rays.0,
            palette_indices,
            c.shards,
            c.objects,
            pool,
            c.beams,
            c.node_cache,
            c.eye_header,
        ];
        if bindings != c.bindings {
            release(c.group);
            let [eye, clipmap, ground, _, _, _, _, _, _, _, header] = bindings;
            let buf = |b| c.buf_of(b);
            let observer = [buf_entry(0, eye), buf_entry(HEADER_BINDING, header)];
            let group = g.frame_group(c.cells.layouts[0], &WORLD_BINDS, buf, &observer);
            for group in c.queue_groups {
                release(group);
            }
            let world = [
                eye,
                clipmap,
                ground,
                palette_indices,
                c.shards,
                c.objects,
                pool,
            ];
            let run = (world, bindings[3], header);
            let queue_groups_of = queue_groups(
                g,
                [&c.queue_pipeline, &c.queue_ask, &c.queue_last],
                [&WALK_BINDS, &WALK_ASK_BINDS, &LAST_BINDS],
                buf,
                run,
            );
            for group in c.samples_groups {
                release(group);
            }
            let [walk, ask, _, _, last] = &c.samples;
            let samples_groups = queue_groups(
                g,
                [walk, ask, last],
                [&SAMPLES_WALK_BINDS, &SAMPLES_ASK_BINDS, &LATTICE_LAST_BINDS],
                buf,
                run,
            );
            for group in c.corners_groups {
                release(group);
            }
            let [walk, ask, _, _, last] = &c.corners;
            let corners_groups = queue_groups(
                g,
                [walk, ask, last],
                [&CORNERS_WALK_BINDS, &CORNERS_ASK_BINDS, &LATTICE_LAST_BINDS],
                buf,
                run,
            );
            c.group = group;
            c.queue_groups = queue_groups_of;
            c.samples_groups = samples_groups;
            c.corners_groups = corners_groups;
            c.bindings = bindings;
        }
        if casts {
            c.raster.prepare(
                g,
                &cast.eye,
                c.size,
                &c.grid,
                kind == FrameKind::cast,
                [
                    c.eye,
                    c.beams,
                    c.node_cache,
                    c.clipmap,
                    ground,
                    c.shards,
                    c.objects,
                    pool,
                    rays.0,
                ],
            );
            let ((changed_at, uniform), _) = self.tiles;
            let px_bufs = [
                c.eye,
                rays.0,
                c.beams,
                c.node_cache,
                c.raster.visibility_array,
                changed_at,
                uniform,
            ];
            if px_bufs != c.px_bufs {
                release(c.px_group);
                release(c.cast_tiles_group);
                release(c.cast_tiles);
                let [across, down] = c.size.map(|v| v.div_ceil(screen::TILE));
                c.cast_tiles = g.buf(
                    word_bytes(CAST_TILES_START.len() + (across * down) as usize),
                    buffer_usage::STORAGE | buffer_usage::INDIRECT | buffer_usage::COPY_DST,
                );
                let buf = |b| c.buf_of(b);
                let tiles = [buf_entry(30, changed_at), buf_entry(31, uniform)];
                let layout = c.cast_tiles_pipeline.layouts[0];
                let cast_tiles_group = g.frame_group(layout, &CAST_TILES_OF_BINDS, buf, &tiles);
                // No world: a pixel walks nothing of its own (`cast_cs`).
                let pixels = [
                    buf_entry(0, c.eye),
                    view_entry(3, rays.0),
                    view_entry(13, c.raster.visibility_array),
                ];
                let px_group = g.frame_group(c.pipeline.layouts[0], &PIXELS_BINDS, buf, &pixels);
                (c.cast_tiles_group, c.px_group) = (cast_tiles_group, px_group);
                c.px_bufs = px_bufs;
            }
        }
        let [drawing, regions, instances] = c
            .generator
            .instances()
            .map_or([c.light_none; 3], |p| [p.drawing, p.regions, p.instances]);
        let light_bufs = [
            c.eye,
            c.clipmap,
            ground,
            palette_indices,
            c.shards,
            c.objects,
            pool,
            light_map.0,
            rays.0,
            drawing,
            regions,
            instances,
            c.beams,
        ];
        // Made once all the light's cast is, over its list and its raster's
        // buffers, which are let go with them (`light_made`).
        if c.light_ready && light_bufs != c.light_bufs {
            for group in c.light_groups {
                release(group);
            }
            c.light_groups = light_groups(
                g,
                [
                    &c.light,
                    &c.cells,
                    &c.raster.light_boxes,
                    &c.queue_last,
                    &c.light_mark_walked,
                    &c.light_mark_draws,
                    &c.light_propagate,
                    &c.light_slide,
                    &c.queue_pipeline,
                    &c.light_samples[0],
                    &c.light_corners[0],
                    &c.light_corners[1],
                    &c.queue_ask,
                    &c.light_mark_draws_open,
                    &c.light_samples[1],
                    &c.light_samples[4],
                    &c.light_corners[4],
                ],
                |b| c.buf_of(b),
                light_bufs,
                [c.light_header, c.raster.light.visibility_array],
            );
            c.light_bufs = light_bufs;
        }
        // The record's group over the light's head, list, args and stood list's args, which its
        // draw stands the boxes it writes by; idle words where the light's cast is not made.
        let [list, args, stood] = if c.light_list != 0 {
            [c.light_list, c.light_args, c.light_receiver_tiles]
        } else {
            c.light_idle
        };
        c.raster
            .record_group(g, [c.light_header, list, args, stood]);
        c.raster
            .light_record_group(g, [c.light_header, list, args, stood, light_map.0]);
        let eye = c.eye;
        self.regroup(g, [rays.0, eye]);
    }

    /// Whether this frame casts every pixel whole, the world's receivers standing afresh off a
    /// walked bits that would hold the claimed tiles' alone (`prepare`): the frame draws whole.
    pub fn ungated(&self) -> bool {
        self.cast
            .as_ref()
            .is_some_and(|c| c.schedule.last().ungated)
    }

    /// What this frame does with the cast, as `prepare` decided it: nothing cast (`plain`), the
    /// rays standing (`standing`), the pixels cast again over the samples that stand (`px`), or
    /// a cast run (`cast`).
    pub fn kind(&self) -> FrameKind {
        self.cast
            .as_ref()
            .map_or(FrameKind::plain, |c| c.schedule.last().kind)
    }

    /// The frame draws whole after all: its samples' raster drawn whole.
    pub fn whole(&mut self) {
        if let Some(c) = self.cast.as_mut() {
            c.schedule.whole();
        }
    }

    /// The cast where the frame casts, the world's depth from the light with
    /// it where `light`, then the decor generated over the world where it moves:
    /// each a pass of its own over the cast's group, timed in its own span,
    /// so what each costs is read apart; the light's in the shadow's, with
    /// the map's pass, which is not drawn at the world's pixels
    /// (span.json `per_px`).
    pub fn dispatch(&mut self, ctx: &mut RenderContext) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let (generates, casts, kind) = (self.generates(), self.casts_px(), self.kind());
        let claimed = self.tiles;
        let Some(c) = self.cast.as_mut() else { return };
        let plan = *c.schedule.last();
        let lit = plan.light;
        // What the casts since the world's receivers were last stood afresh left in the stood
        // list's args, and the read list's count, copied back in, ahead of every pass that
        // stands a tile, the record's draw among the eye's, where this cast stands on them or
        // adds to them (`light::carry_at`).
        c.draws_record = false;
        if lit.is_some_and(|l| !l.world || l.adds) {
            let back = &sequence::RECEIVER_TILES_BACK;
            g.copy_buf(
                encoder,
                (c.light_list, back.from_at()),
                (c.light_receiver_tiles, back.to_at()),
                word_bytes(light::RECEIVER_TILES_CARRY as usize),
            );
            let back = &sequence::READ_COUNT_BACK;
            g.copy_buf(
                encoder,
                (c.light_list, back.from_at()),
                (c.light_args, back.to_at()),
                4,
            );
        }
        if casts {
            // The node cache forgotten first where it is, what draws its entries with it; then the
            // beam rays writing it, a corner each; each cell's start off them;
            // the samples writing it, each from its cell's start, and the record; its
            // voxels drawn as boxes, each pixel's nearest kept; the pixels
            // off that, or reading it from their cell's start, marking the
            // pixels that missed; and those again,
            // writing it, a thread a drawn pixel, each but the marked
            // returning at once, each reading what the ones before wrote
            // (pxl-engine `node_cache`).
            // A pass a dispatch, each timed in its own span, so the cast's
            // parts are read apart: `cast` holds the forgetting alone; the
            // record written and its boxes drawn between the samples and the
            // pixels, which take the visible voxel (ray/raster.rs, spans
            // `sample_box` and `sraster`). Where
            // only the jitter moved (`FrameKind::px`), the beam rays, the
            // cells' starts and the samples stand, and the rest is cast again
            // off them.
            let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::cast).pass());
            g.set_bind_group(pass, 0, c.group, &[]);
            if c.forgets != node_cache::FORGETS_NONE {
                g.set_pipeline(pass, c.forget_pipeline.id);
                g.dispatch(pass, node_cache::ENTRIES.div_ceil(ROW_LANES), 1, 1);
            }
            g.end(pass);
            // The beam rays' run, each from the root, where the samples are cast
            // (corners.wgsl).
            if kind == FrameKind::cast {
                let [walk, ask, walk_parked, ask_parked, last] = &c.corners;
                let pass = walk_run(
                    ctx,
                    [GpuSpanName::beam_hit, GpuSpanName::beam],
                    &Run {
                        pipelines: [walk, ask, walk_parked, ask_parked, last],
                        groups: c.corners_groups,
                        queue: c.queues,
                        node_cache: c.node_cache,
                        holes: None,
                    },
                );
                g.end(pass);
            }
            for (span, pipeline, [x, y], w) in [
                (
                    GpuSpanName::cell,
                    &c.cells,
                    c.grid.span,
                    [CELL_SIDE, CELL_SIDE, 1],
                ),
                (GpuSpanName::px, &c.pipeline, c.size, CAST_WORKGROUP),
            ] {
                if kind == FrameKind::px && span != GpuSpanName::px {
                    continue;
                }
                let mut group = c.group;
                if span == GpuSpanName::px {
                    // The samples' run, each from its cell's start (samples.wgsl).
                    if kind == FrameKind::cast {
                        let [walk, ask, walk_parked, ask_parked, last] = &c.samples;
                        let pass = walk_run(
                            ctx,
                            [GpuSpanName::sample_hit, GpuSpanName::sample],
                            &Run {
                                pipelines: [walk, ask, walk_parked, ask_parked, last],
                                groups: c.samples_groups,
                                queue: c.queues,
                                node_cache: c.node_cache,
                                holes: None,
                            },
                        );
                        g.end(pass);
                    }
                    let masked = c.schedule.masked().then_some(claimed);
                    let samples_cast = kind == FrameKind::cast;
                    c.draws_record = samples_cast || lit.is_some_and(|l| l.world);
                    c.raster
                        .draw(ctx, &c.grid, (samples_cast, c.draws_record), masked);
                    group = c.px_group;
                    g.write_buf(
                        c.cast_tiles,
                        sequence::OPEN_CAST_TILES.to_at(),
                        bytes(&CAST_TILES_START),
                    );
                    // The walked bits set back to none, so they hold this cast's holes alone, a
                    // gated cast's the claimed tiles' (cast.wgsl).
                    g.clear_buf(
                        encoder,
                        c.beams,
                        word_bytes(c.grid.walked_at() as usize),
                        word_bytes(ray::walked_words(c.size) as usize),
                    );
                }
                let pass = g.compute_pass(encoder, ctx.stamp(span).pass());
                if span == GpuSpanName::px {
                    let tiles = c.size.map(|v| v.div_ceil(screen::TILE));
                    g.set_bind_group(pass, 0, c.cast_tiles_group, &[]);
                    g.set_pipeline(pass, c.cast_tiles_pipeline.id);
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
                    g.dispatch_indirect(pass, c.cast_tiles, sequence::PX.at());
                } else {
                    g.dispatch(pass, x.div_ceil(w[0]), y.div_ceil(w[1]), 1);
                }
                g.end(pass);
            }
        }
        // The eye's holes walked; every stop into the record.
        if casts {
            g.copy_buf(
                encoder,
                (c.marks, sequence::EYE_HOLES_CNT.from_at()),
                (
                    c.beams,
                    word_bytes((c.grid.walked_at() + ray::holes_at(c.size)) as usize),
                ),
                4,
            );
            let pass = walk_run(
                ctx,
                [GpuSpanName::miss; 2],
                &Run {
                    pipelines: [
                        &c.queue_pipeline,
                        &c.queue_ask,
                        &c.queue_parked,
                        &c.queue_ask_parked,
                        &c.queue_last,
                    ],
                    groups: c.queue_groups,
                    queue: c.queues,
                    node_cache: c.node_cache,
                    holes: Some(Holes {
                        hits: &sequence::HITS,
                        pool: (
                            c.beams,
                            c.grid.walked_at() + ray::pool_at(c.size) + ray::POOL_CNT,
                            &sequence::EYE_POOL_CNT,
                        ),
                    }),
                },
            );
            g.end(pass);
            // Their stops, every one, their new faces asked and each stop's face's word put at
            // its pixel (ray/raster.rs `stops`): next frame draws them.
            c.raster.stops(ctx, c.node_cache, GpuSpanName::miss);
        }
        // The decor generated over the rays, before the light's cast, which
        // marks where it stands (`light_mark_draws_cs`).
        if generates {
            let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::generate).pass());
            c.generator.dispatch(g, pass);
            g.end(pass);
        }
        if let Some(due) = lit {
            // The reads moved with the box, a tile of it at a time: each
            // tile's word the word of the tile that held its world texels
            // under the corner before, copied out and back in, which the
            // slide's pass then sets to none where the box slid onto it or
            // into another band (`light_slide_cs`).
            if due.slid {
                let n = light::tiles(SHADOW_SIZE) as i32;
                let t = light::TILE as i32;
                let dx = (due.corner[0] - due.last_corner[0]) / t;
                let dy = (due.corner[1] - due.last_corner[1]) / t;
                let shift = dy * n + dx;
                let all = n * n;
                if shift.abs() < all {
                    let words = |w: i32| word_bytes(w as usize);
                    let out = (c.light_reads_was, 0);
                    let reads = (c.light_list, sequence::LIGHT_READS_OUT.from_at());
                    g.copy_buf(encoder, reads, out, words(all));
                    let (from, to) = if shift >= 0 { (shift, 0) } else { (0, -shift) };
                    g.copy_buf(
                        encoder,
                        (c.light_reads_was, words(from)),
                        (c.light_list, sequence::LIGHT_READS_IN.to_at() + words(to)),
                        words(all - shift.abs()),
                    );
                }
            }
            // The tiles the world's receivers stand on (`light::LIST_RECEIVERS`): every box the
            // eye's draw wrote, stood as it wrote it, by the eye's pixels' cast or here
            // where that ran none (`draws_record`); then every pixel the eye walked apart, a
            // thread a word of its walked bits, in a span of
            // their own (`light_receivers`, span.json `per_px`). Added to those that stand, theirs
            // copied back in before the frame's first pass (`dispatch`).
            if due.world {
                // The record's boxes stood, its draw run here where the eye's pixels' cast ran
                // none (`schedule::Light::world`).
                if !c.draws_record {
                    c.raster.record_draw(ctx);
                }
                let stamp = ctx.stamp(GpuSpanName::light_receivers).pass();
                let pass = g.compute_pass(encoder, stamp);
                g.set_bind_group(pass, 0, c.light_groups[4], &[]);
                g.set_pipeline(pass, c.light_mark_walked.id);
                let rows = ray::walked_words(c.size).div_ceil(ray::MISS_LANES * ray::MISS_ACROSS);
                g.dispatch(pass, ray::MISS_ACROSS, rows, 1);
                g.end(pass);
            }
            // The tiles the drawn receivers stand on, in a span of their own: where the decor
            // and what else the host draws that reads the map stand, as many threads as its one
            // thread opens (`light_mark_draws_open_cs`, `light_mark_draws_cs`), where any is
            // drawn.
            if c.light_draws {
                let stamp = ctx.stamp(GpuSpanName::light_draws).pass();
                let pass = g.compute_pass(encoder, stamp);
                g.set_bind_group(pass, 0, c.light_groups[13], &[]);
                g.set_pipeline(pass, c.light_mark_draws_open.id);
                g.dispatch(pass, 1, 1, 1);
                g.set_bind_group(pass, 0, c.light_groups[5], &[]);
                g.set_pipeline(pass, c.light_mark_draws.id);
                g.dispatch_indirect(
                    pass,
                    c.light_draws_dispatch,
                    sequence::LIGHT_MARK_DRAWS.at(),
                );
                g.end(pass);
            }
            // The stood tiles' count where the propagation reads it; then, once a tile is
            // stood, the tiles the box slid onto or into another band, or whose rays may meet
            // a column of the world that changed, set back to be read anew, a thread a tile,
            // where the cast owes it (`light_slide_cs`); then the tiles they read, a thread a
            // tile stood on since the casts before propagated theirs (`light_propagate_cs`). A
            // cast that stands on no tile runs neither. Each stage a pass timed in its own span.
            g.copy_buf(
                encoder,
                (
                    c.light_receiver_tiles,
                    sequence::RECEIVER_TILES_COUNT.from_at(),
                ),
                (c.light_list, sequence::RECEIVER_TILES_COUNT.to_at()),
                4,
            );
            let stamp = ctx.stamp(GpuSpanName::light_slide).pass();
            let pass = g.compute_pass(encoder, stamp);
            g.set_bind_group(pass, 0, c.light_groups[7], &[]);
            g.set_pipeline(pass, c.light_slide.id);
            g.dispatch_indirect(pass, c.light_receiver_tiles, sequence::LIGHT_SLIDE.at());
            g.end(pass);
            let stamp = ctx.stamp(GpuSpanName::light_propagate).pass();
            let pass = g.compute_pass(encoder, stamp);
            g.set_bind_group(pass, 0, c.light_groups[6], &[]);
            g.set_pipeline(pass, c.light_propagate.id);
            g.dispatch_indirect(pass, c.light_receiver_tiles, sequence::LIGHT_PROPAGATE.at());
            g.end(pass);
            // What the cast stood on and read kept for the next, the count propagated with them
            // (`light::carry_at`).
            let keep = &sequence::RECEIVER_TILES_KEEP;
            g.copy_buf(
                encoder,
                (c.light_receiver_tiles, keep.from_at()),
                (c.light_list, keep.to_at()),
                word_bytes(light::RECEIVER_TILES_CARRY as usize),
            );
            let keep = &sequence::READ_COUNT_KEEP;
            g.copy_buf(
                encoder,
                (c.light_args, keep.from_at()),
                (c.light_list, keep.to_at()),
                4,
            );
            // Then every tile read claimed where its epoch has not cast it, a thread a place of
            // the box (`light_damage_cs`).
            let stamp = ctx.stamp(GpuSpanName::light_claim).pass();
            let pass = g.compute_pass(encoder, stamp);
            g.set_bind_group(pass, 0, c.light_groups[6], &[]);
            g.set_pipeline(pass, c.light_damage.id);
            let n = light::tiles(SHADOW_SIZE);
            g.dispatch(pass, (n * n).div_ceil(light::LIST_LANES), 1, 1);
            g.end(pass);
            // The claimed lists' counts where the passes that take them read them.
            for step in [
                &sequence::CORNERS_COUNT,
                &sequence::SOURCES_COUNT,
                &sequence::DAMAGE_COUNT,
            ] {
                g.copy_buf(
                    encoder,
                    (c.light_args, step.from_at()),
                    (c.light_list, step.to_at()),
                    4,
                );
            }
            // The beam, the samples, their boxes drawn, the slots taking
            // their visible voxel, a workgroup a tile (pxl-engine
            // `light::TILE`), and the rest walked off their list as the eye's
            // missed pixels are: each a pass timed in its own span.
            // The beam rays' run, each from the root at its tile's band (corners.wgsl over
            // beam.light.wgsl), the corners listed its items; then the cells' starts over the
            // light's world's group, handed the light's head.
            let [walk, ask, walk_parked, ask_parked, last] = &c.light_corners;
            let pass = walk_run(
                ctx,
                [GpuSpanName::light_beam_hit, GpuSpanName::light_beam],
                &Run {
                    pipelines: [walk, ask, walk_parked, ask_parked, last],
                    groups: [c.light_groups[10], c.light_groups[11], c.light_groups[16]],
                    queue: c.queues,
                    node_cache: c.node_cache,
                    holes: None,
                },
            );
            g.set_bind_group(pass, 0, c.light_groups[1], &[]);
            g.set_pipeline(pass, c.light_cells.id);
            g.dispatch_indirect(pass, c.light_args, sequence::LIGHT_CELLS.at());
            g.end(pass);
            // The samples' run, each from its cell's start (samples.wgsl over
            // sample.light.wgsl), the tiles' fed samples its items.
            let [walk, ask, walk_parked, ask_parked, last] = &c.light_samples;
            let pass = walk_run(
                ctx,
                [GpuSpanName::light_sample; 2],
                &Run {
                    pipelines: [walk, ask, walk_parked, ask_parked, last],
                    groups: [c.light_groups[9], c.light_groups[14], c.light_groups[15]],
                    queue: c.queues,
                    node_cache: c.node_cache,
                    holes: None,
                },
            );
            g.end(pass);
            // The samples' stops, as the eye's (ray/raster.rs `draw`).
            let stamp = ctx.stamp(GpuSpanName::light_raster).pass();
            let pass = g.compute_pass(encoder, stamp);
            g.set_bind_group(pass, 0, c.light_groups[2], &[]);
            g.set_pipeline(pass, c.raster.light_boxes.id);
            g.dispatch_indirect(pass, c.light_args, sequence::LIGHT_SAMPLE_BOX.at());
            g.end(pass);
            c.raster.light_draw(ctx);
            let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::light_slot).pass());
            g.set_bind_group(pass, 0, c.light_groups[0], &[]);
            g.set_pipeline(pass, c.light.id);
            g.dispatch_indirect(pass, c.light_args, sequence::LIGHT_SLOTS.at());
            g.end(pass);
            // Their count where the holes' queue reads it, then the queue's passes over them.
            g.copy_buf(
                encoder,
                (c.light_misses, sequence::LIGHT_HOLES_CNT.from_at()),
                (c.light_list, sequence::LIGHT_HOLES_CNT.to_at()),
                4,
            );
            let pass = walk_run(
                ctx,
                [GpuSpanName::light_hit, GpuSpanName::light_walk],
                &Run {
                    pipelines: [
                        &c.queue_pipeline,
                        &c.queue_ask,
                        &c.queue_parked,
                        &c.queue_ask_parked,
                        &c.queue_last,
                    ],
                    groups: [c.light_groups[8], c.light_groups[12], c.light_groups[3]],
                    queue: c.queues,
                    node_cache: c.node_cache,
                    holes: Some(Holes {
                        hits: &sequence::LIGHT_HITS,
                        pool: (
                            c.light_list,
                            light::list_words(SHADOW_SIZE) + ray::POOL_CNT,
                            &sequence::LIGHT_POOL_CNT,
                        ),
                    }),
                },
            );
            g.end(pass);
            // Their stops, every one its queue listed, after the queue and before the light's
            // next cast's draw (ray/raster.rs `light_stops`, `light_draw`).
            c.raster
                .light_record_hits(ctx, c.node_cache, GpuSpanName::light_walk);
        }
        let counted = Counts {
            node_cache: c.node_cache,
            args: [c.marks, c.light_misses],
            boxes: [c.raster.args, c.raster.light.args],
        };
        c.counter.copy(g, encoder, &counted);
    }

    /// The light's cast made where `on`, the shadows' setting
    /// (`FrameInput::shadows`), before the frame: its list (`light::list_words`)
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
                    let dispatch = buffer_usage::STORAGE | buffer_usage::INDIRECT;
                    c.light_draws_dispatch =
                        g.buf(word_bytes(light::DRAWS_DISPATCH_WORDS as usize), dispatch);
                    c.light_receiver_tiles = g.buf(
                        word_bytes(light::RECEIVER_TILES_WORDS as usize),
                        buffer_usage::STORAGE
                            | buffer_usage::COPY_DST
                            | buffer_usage::INDIRECT
                            | buffer_usage::COPY_SRC,
                    );
                }
                c.light_ready = c.raster.light_made(g, SHADOW_SIZE);
            }
        } else if c.light_list != 0 || c.light_ready {
            for group in c.light_groups {
                release(group);
            }
            (c.light_groups, c.light_bufs) = ([0; LIGHT_GROUPS], [0; 13]);
            release(c.light_list);
            release(c.light_args);
            release(c.light_draws_dispatch);
            release(c.light_receiver_tiles);
            (c.light_list, c.light_args, c.light_receiver_tiles) = (0, 0, 0);
            c.light_draws_dispatch = 0;
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

    pub fn take_counts(&mut self) -> Option<RayCounts> {
        self.cast.as_mut().and_then(|c| c.counter.take())
    }

    pub fn light_casts(&self) -> bool {
        self.cast
            .as_ref()
            .is_some_and(|c| c.schedule.last().light.is_some())
    }

    /// The light's list, whose reads tell the tiles its epoch cast and which it lists read
    /// (`light::reads_at`, `light::LIST_READS`); the host's epoch's first cast
    /// (`light::HEADER_EPOCH`); and its args, whose lay draws the tiles read (`light::ARGS_LAY`),
    /// while the light's cast is made.
    pub fn light_reads(&self) -> Option<LightReads> {
        self.cast
            .as_ref()
            .filter(|c| c.light_list != 0)
            .map(|c| LightReads {
                list: c.light_list,
                epoch: c.light_epoch,
                args: c.light_args,
            })
    }

    /// Why the light's map cast this frame, a bit each in LIGHT_MOVED's
    /// order (span.json `LIGHT_MOVED`); 0 where it did not.
    pub fn light_moved(&self) -> u32 {
        self.cast
            .as_ref()
            .and_then(|c| c.schedule.last().light)
            .map_or(0, |l| l.moved)
    }

    /// The boxes decor's slots are marked by to generate again this frame
    /// (gpu/generator.rs `marked_rects`).
    pub fn decor_boxes(&self) -> u32 {
        self.cast.as_ref().map_or(0, |c| c.generator.marked_rects())
    }

    pub fn generates(&self) -> bool {
        self.cast.as_ref().is_some_and(|c| c.generator.works())
    }

    pub fn generated(&self) -> Option<IndirectDraw> {
        self.cast.as_ref().and_then(|c| c.generator.indirect_draw())
    }

    /// What a frame draws through `mask` of decor generated (`Generator::mask`).
    pub fn cull_decor(&self, ctx: &mut RenderContext, mask: Mask, frame: u32, tiles: &Tiles) {
        if let Some(c) = self.cast.as_ref() {
            c.generator.mask(ctx, mask, frame, tiles);
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
                for (m, was) in c.changed.iter_mut().zip(f.changed) {
                    *m |= was;
                }
                for (b, was) in c.changed_bricks.iter_mut().zip(f.changed_bricks) {
                    *b = node_cache::bricks_joined(*b, was);
                }
                for &b in &f.rects[..f.holding] {
                    hold(&mut c.rects, &mut c.holding, b);
                }
                for &first in &f.written_groups[..f.writes] {
                    c.forget |= !note(&mut c.written_groups, &mut c.writes, first);
                }
            }
            c.schedule.dropped();
            c.generator.on_dropped();
            c.counter.on_dropped();
        }
    }

    /// The frame was drawn: what its decor marked by is done with.
    pub fn on_drawn(&mut self) {
        if let Some(c) = self.cast.as_mut() {
            c.generator.on_drawn();
        }
    }

    /// A frame's cast handed and never drawn: the edits its decor named,
    /// held for the next that is.
    pub fn carry_edits(&mut self, edits: &[[i32; 4]]) {
        if let Some(c) = self.cast.as_mut() {
            for &b in edits {
                hold(&mut c.rects, &mut c.holding, b);
            }
            c.generator.carry_edits(edits);
        }
    }
}

/// The workgroups of the passes that cast the light's tiles as a cast starts: none, a list's
/// row across and no row; and the lay's tiles' vertices and no instance
/// (`light::ARGS_WORDS`).
fn light_args() -> [u32; light::ARGS_WORDS as usize] {
    let mut args = [0; light::ARGS_WORDS as usize];
    for w in [light::ARGS_CELLS, light::ARGS_SAMPLE_BOX, light::ARGS_SLOT] {
        args[w as usize..w as usize + 3].copy_from_slice(&[light::LIST_ACROSS, 0, 1]);
    }
    args[light::ARGS_LAY as usize] = light::LAY_VERTICES;
    args
}

/// The stood list's args as a cast starts (`light::RECEIVER_TILES_WORDS`): no stood tile, the
/// propagation's dispatch `ray::LEFT_EMPTY`'s, and the slide's none across and its tiles a
/// side down where the cast `owes` one, the box slid or a column changed.
fn light_receiver_tiles(owes: bool) -> [u32; light::RECEIVER_TILES_WORDS as usize] {
    let mut w = [0; light::RECEIVER_TILES_WORDS as usize];
    let list = light::RECEIVER_TILES_LIST as usize;
    w[list..list + ray::LEFT_EMPTY.len()].copy_from_slice(&ray::LEFT_EMPTY);
    let n = light::tiles(SHADOW_SIZE).div_ceil(SQUARE_SIDE);
    let slide = light::RECEIVER_TILES_SLIDE as usize;
    w[slide..slide + 3].copy_from_slice(&[0, if owes { n } else { 0 }, 1]);
    w
}

/// The light's args (`light::args_words`).
fn light_args_buf(g: &Gpu) -> u32 {
    g.buf(
        word_bytes(light::args_words(SHADOW_SIZE) as usize),
        buffer_usage::STORAGE
            | buffer_usage::COPY_DST
            | buffer_usage::INDIRECT
            | buffer_usage::COPY_SRC,
    )
}

/// The light's list: the beam's lines and the cells' starts, the raster's samples, the holes
/// and the rest of `light::list_words`, then its pool and its stops (`light::buffer_words`).
fn light_list(g: &Gpu) -> u32 {
    g.buf(
        word_bytes(light::buffer_words(SHADOW_SIZE) as usize),
        buffer_usage::STORAGE
            | buffer_usage::COPY_DST
            | buffer_usage::INDIRECT
            | buffer_usage::COPY_SRC,
    )
}

/// The light's cast's groups 0, each the group its table says (frame/shader/group.rs) over
/// `buf`, the frame's buffers, and `bindings`, what else they bind: the eye, the clip level,
/// the tiles' frames, the bricks' palette indices, the service area's index, the overwrites
/// and the ways' pool, the cast's alone; the light's map; the eye's rays, which the mark
/// reads; the decor's drawing words, regions and instances, which the drawn mark reads; and
/// its head and the raster's visibility buffer. The slots', its world's (the eye's passes over
/// `WORLD_GROUP`, its list where the eye's beam rays' buffer lies and its head where the
/// eye's), its boxes' (the eye's raster's pass), the misses' walk's, the two marks', the
/// propagation's, the slide's, the misses' and the samples' walks over the node cache alone,
/// the beam rays' walks over the node cache alone and their asks, the misses' asks, the drawn
/// receivers' opening, the samples' asks and last walk, and the beam rays' last walk.
fn light_groups(
    g: &Gpu,
    [
        walk,
        world,
        boxes,
        queue_last,
        mark,
        mark_draws,
        propagate,
        slide,
        queue_walk,
        samples_walk,
        corners_walk,
        corners_ask,
        queue_ask,
        mark_draws_open,
        samples_ask,
        samples_last,
        corners_last,
    ]: [&Pipeline; LIGHT_GROUPS],
    buf: impl Fn(u32) -> u32 + Copy,
    bindings: [u32; 13],
    [header, visibility]: [u32; 2],
) -> [u32; LIGHT_GROUPS] {
    let [eye, clipmap, ground, palette_indices, shards, objects, pool, light, rays, drawing, regions, instances, _] =
        bindings;
    let rest = [
        buf_entry(0, eye),
        view_entry(3, rays),
        view_entry(10, light),
        buf_entry(HEADER_BINDING, header),
        view_entry(15, visibility),
        buf_entry(18, drawing),
        buf_entry(19, regions),
        buf_entry(20, instances),
    ];
    let worlds = [eye, clipmap, ground, palette_indices, shards, objects, pool];
    let run = (worlds, light, header);
    let [queue_walk, ask, last] = queue_groups(
        g,
        [queue_walk, queue_ask, queue_last],
        [&LIGHT_WALK_BINDS, &LIGHT_WALK_ASK_BINDS, &LIGHT_LAST_BINDS],
        buf,
        run,
    );
    let [samples_walk, samples_ask, samples_last] = queue_groups(
        g,
        [samples_walk, samples_ask, samples_last],
        [
            &LIGHT_SAMPLES_WALK_BINDS,
            &LIGHT_SAMPLES_ASK_BINDS,
            &LIGHT_LATTICE_LAST_BINDS,
        ],
        buf,
        run,
    );
    let [corners_walk, corners_ask, corners_last] = queue_groups(
        g,
        [corners_walk, corners_ask, corners_last],
        [
            &LIGHT_CORNERS_WALK_BINDS,
            &LIGHT_CORNERS_ASK_BINDS,
            &LIGHT_LATTICE_LAST_BINDS,
        ],
        buf,
        run,
    );
    let of = |p: &Pipeline, table: &Group| g.frame_group(p.layouts[0], table, buf, &rest);
    [
        of(walk, &LIGHT_SLOTS_BINDS),
        of(world, &LIGHT_WORLD_BINDS),
        of(boxes, &LIGHT_SAMPLE_BOX_BINDS),
        last,
        of(mark, &LIGHT_MARK_BINDS),
        of(mark_draws, &LIGHT_MARK_DRAWS_BINDS),
        of(propagate, &LIGHT_PROPAGATE_BINDS),
        of(slide, &LIGHT_SLIDE_BINDS),
        queue_walk,
        samples_walk,
        corners_walk,
        corners_ask,
        ask,
        of(mark_draws_open, &LIGHT_MARK_DRAWS_OPEN_BINDS),
        samples_ask,
        samples_last,
        corners_last,
    ]
}

/// A run's groups 0, of either observer and any kind: its walks over the node cache alone, its
/// asks and its last walk, each the group its table says over `buf`, the frame's buffers, and
/// `world`, the eye and the world's buffers the asks read (the clip level, the tiles' frames,
/// the bricks' palette indices, the service area's index, the overwrites and the ways' pool),
/// the rays or the map its holes write, and its observer's head.
fn queue_groups(
    g: &Gpu,
    pipelines: [&Pipeline; 3],
    tables: [&Group; 3],
    buf: impl Fn(u32) -> u32 + Copy,
    (world, out, header): ([u32; 7], u32, u32),
) -> [u32; 3] {
    let [eye, clipmap, ground, palette_indices, shards, objects, pool] = world;
    let rest = [
        buf_entry(0, eye),
        buf_entry(1, clipmap),
        buf_entry(2, ground),
        view_entry(3, out),
        buf_entry(4, palette_indices),
        buf_entry(5, shards),
        buf_entry(6, objects),
        buf_entry(7, pool),
        buf_entry(HEADER_BINDING, header),
    ];
    core::array::from_fn(|k| g.frame_group(pipelines[k].layouts[0], tables[k], buf, &rest))
}

/// A run as its passes take it (pxl-engine `ray::QUEUE_LANES`): the pipelines of its first
/// walk, its asks, its second walk, their asks and its last walk; its groups
/// (`queue_groups`); the queues' words, its head among them; the node cache, whose head
/// holds its stops' count; and, for the holes, what is copied before the last walk (`Holes`).
struct Run<'a> {
    pipelines: [&'a Pipeline; 5],
    groups: [u32; 3],
    queue: u32,
    node_cache: u32,
    holes: Option<Holes<'a>>,
}

/// What a holes' run copies before its last walk: the step that copies its stops' count into
/// the node cache's head (`sequence::HITS`), and the buffer, the word and the step its pool's
/// count is copied by (`sequence::EYE_POOL_CNT`, pxl-engine `ray::POOL_CNT`).
struct Holes<'a> {
    hits: &'a Step,
    pool: (u32, u32, &'a Step),
}

/// A run's passes, each a fixed dispatch over what the one before wrote, no indirect call
/// among them: the two walks over the node cache alone, each followed by the asks of the nodes
/// it parked rays at, in the span `spans[0]`; then, for the holes, their stops' count and
/// dispatch into the node cache's head and their parked rays' count into their pool; and the
/// last walk, in a pass of the span `spans[1]` left open for what follows it.
fn walk_run(ctx: &mut RenderContext, spans: [GpuSpanName; 2], r: &Run) -> Id {
    let (g, encoder) = (ctx.g, ctx.encoder);
    let [walk, ask, walk_parked, ask_parked, last] = r.pipelines;
    let [walk_group, ask_group, last_group] = r.groups;
    let pass = g.compute_pass(encoder, ctx.stamp(spans[0]).pass());
    for (pipeline, group) in [
        (walk, walk_group),
        (ask, ask_group),
        (walk_parked, walk_group),
        (ask_parked, ask_group),
    ] {
        g.set_bind_group(pass, 0, group, &[]);
        g.set_pipeline(pass, pipeline.id);
        g.dispatch(pass, ray::QUEUE_GROUPS, 1, 1);
    }
    g.end(pass);
    if let Some(Holes {
        hits,
        pool: (buf, at, parked),
    }) = r.holes
    {
        g.copy_buf(
            encoder,
            (r.queue, hits.from_at()),
            (r.node_cache, hits.to_at()),
            word_bytes(4),
        );
        g.copy_buf(
            encoder,
            (r.queue, parked.from_at()),
            (buf, word_bytes(at as usize)),
            word_bytes(1),
        );
    }
    let pass = g.compute_pass(encoder, ctx.stamp(spans[1]).pass());
    g.set_bind_group(pass, 0, last_group, &[]);
    g.set_pipeline(pass, last.id);
    g.dispatch(pass, ray::QUEUE_GROUPS, 1, 1);
    pass
}

/// Column `c` put among `columns`, as many as the light's head holds (pxl-engine
/// `light::COLUMNS`): past them taken into the last, as the host's draws' stands are
/// (gpu/draw.rs `stand`).
fn put_column(columns: &mut Vec<light::Column>, c: light::Column) {
    let full = columns.len() == light::COLUMNS;
    match columns.last_mut() {
        Some(last) if full => last.hold(c),
        _ => columns.push(c),
    }
}
