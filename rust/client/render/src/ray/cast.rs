// pxl-render/ray/cast.rs

//! The GPU's cast: the eye's rays into the world surface's `rays`, the world's depth from the
//! light into the shadow map, and decor generated over the same world (gpu/generator.rs).
//! `prepare` reads what the frame hands, uploads what changed, asks the schedule
//! (ray/schedule.rs) what to cast and writes the heads the passes read; `dispatch` encodes
//! the passes.
//!
//! The eye's cast runs in stages over the camera grid (pxl-engine `beam`): beam rays from the
//! grid's corners, each cell's start off them, and samples from those starts, whose stops go
//! into the record. The raster draws the record as boxes and keeps each pixel's nearest voxel,
//! which the pixels' cast takes. The pixels the raster missed, the holes, are walked apart by
//! the holes' queue, and the walked bits mark them, a bit a pixel (pxl-engine
//! `ray::SQUARE_WORDS`). The light's cast runs the same stages over the tiles of its box it
//! claims. Both observers walk the world through one node cache (pxl-engine `node_cache`),
//! whose nodes the cast forgets wherever the world beneath them changed.

use crate::frame::shader::access::{Group, Step};
use crate::frame::shader::sequence;
use pxl_content::generation::{self, GROUPS};
use pxl_engine::clipmap;
use pxl_engine::{beam, eye, light, node_cache, object, ray};
use pxl_spec::space::coord::zoom::{Zoom, LEAF_CELL_ZOOM};
use xpute_gpu::encode::Id;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use super::count::{self, CountBuffer, Counter, RayCount};
use super::raster::Raster;
use super::schedule::{CastRead, Schedule};
use super::RayPass;
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
    NODE_CACHE, PIXELS_BINDS, QUEUES, QUEUE_GROUP, QUEUE_LAST_GROUP, SAMPLES_ASK_BINDS,
    SAMPLES_WALK_BINDS, WALK_ASK_BINDS, WALK_BINDS, WORLD_BINDS, WORLD_GROUP,
};
use crate::frame::shader::record::{
    COMPUTE_CAST, COMPUTE_CAST_TILES, COMPUTE_CELL, COMPUTE_FORGET, COMPUTE_LIGHT,
    COMPUTE_LIGHT_CELL, COMPUTE_LIGHT_DAMAGE, COMPUTE_LIGHT_MARK_DRAWS,
    COMPUTE_LIGHT_MARK_DRAWS_OPEN, COMPUTE_LIGHT_MARK_WALKED, COMPUTE_LIGHT_PROPAGATE,
    COMPUTE_LIGHT_SLIDE, COMPUTE_QUEUE, COMPUTE_QUEUE_ASK, COMPUTE_QUEUE_ASK_PARKED,
    COMPUTE_QUEUE_CORNERS, COMPUTE_QUEUE_CORNERS_PARKED, COMPUTE_QUEUE_LAST,
    COMPUTE_QUEUE_LAST_CORNERS, COMPUTE_QUEUE_LAST_LIGHT_CORNERS, COMPUTE_QUEUE_LAST_LIGHT_SAMPLES,
    COMPUTE_QUEUE_LAST_SAMPLES, COMPUTE_QUEUE_LIGHT_CORNERS, COMPUTE_QUEUE_LIGHT_CORNERS_PARKED,
    COMPUTE_QUEUE_LIGHT_SAMPLES, COMPUTE_QUEUE_LIGHT_SAMPLES_PARKED, COMPUTE_QUEUE_PARKED,
    COMPUTE_QUEUE_SAMPLES, COMPUTE_QUEUE_SAMPLES_PARKED,
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
use crate::gpu::pipeline::{Pipeline, PipelineCache};
use crate::gpu::read::Read;
use crate::gpu::span_spec::{FrameKind, GpuSpanName};
use crate::gpu::surface::DRAWN_CAP;
use crate::gpu::tile::TileGrid;

/// What the core hands the GPU's cast each frame.
pub struct Cast<'a> {
    /// The eye's words (pxl-engine `eye`), as pxl-core writes them (domain/view/lens.rs
    /// `gpu_cast`, domain/scene/cast.rs `eye`).
    pub eye: [u32; eye::WORDS],
    /// The clip levels' table (pxl-engine `clipmap::TABLE_WORDS`; pxl-core
    /// domain/layer/terrain/clipmap.rs `table`), in rows of four words. Every clip level's
    /// entries come first, the eye's clip levels leading from the coarsest, then each clip
    /// level's first brick (`clipmap::ORIGIN_AT`). An entry's row holds the brick its clip
    /// level wants there, X and Y, and the slot holding it plus one while it is present
    /// (`clipmap::Entry`); its fourth word holds nothing.
    pub clipmap: &'a [i32],
    /// The service area's index (pxl-spec `area::Area`), four keys a row (pxl-core
    /// domain/scene/cast.rs `shards`). The runtime reads it where it lies, so its writer asks
    /// `in_flight` (gpu/api.rs) before it writes it again.
    pub shards: &'a [[i32; 4]],
    pub objects: ObjectTable<'a>,
    /// The input of the decor the GPU generates over the same world (gpu/generator.rs).
    pub decor: DecorFrame<'a>,
    /// The box each of the host's draws that reads the shadow map stands within, in the
    /// world's Q3, as gpu/draw.rs `DrawUpdate::receivers` records it. These draws receive the map as
    /// decor does (pxl-engine `light::RECEIVER_BOXES_AT`), and their boxes are taken from the
    /// eye's `ORIGIN` when the light's head is written.
    pub receivers: &'a [ReceiverBox],
    /// Whether the cast counts what the eye's rays and the light's walked and reused
    /// (ray/count.rs).
    pub ray_cnts: bool,
}

/// The overwrites' tree (pxl-core domain/layer/object/store.rs `objects`): its child
/// pointer lane and its data lane, with room for `cap` words each before they grow, at
/// `revision`. `written_groups` names, by their first slots, the child groups written since
/// revision `since`, the last the core uploaded (pxl-core domain/layer/object/store.rs `Upload`). The
/// runtime reads the lanes where they lie, so their writer asks `in_flight` (gpu/api.rs)
/// before it writes them again.
pub struct ObjectTable<'a> {
    pub first_child: &'a [u32],
    pub data: &'a [u32],
    pub cap: usize,
    pub revision: u32,
    pub since: u32,
    pub written_groups: &'a [u32],
}

/// What the shadow map's pass reads of the light's cast (`RayPass::light_read`,
/// pass/shadow.rs `Shadow::begin`).
#[derive(Clone, Copy)]
pub struct LightRead {
    pub list: u32,
    pub epoch: u32,
    pub args: u32,
}

/// What the GPU's cast holds from frame to frame: its pipelines, its buffers and groups, and
/// what it last read.
pub(super) struct CastBuffer {
    pipeline: Pipeline,
    /// Finds where each cell's rays start off the beam rays, a thread a cell (`cell_cs`). It
    /// binds the cast's world group, and the head it binds tells the eye's run from the
    /// light's (`eye_header`, `light_header`).
    cell: Pipeline,
    /// The eye's beam rays' run through the queue (`Run`), and its groups.
    corners: [Pipeline; 5],
    corners_groups: [u32; 3],
    /// The eye's samples' run through the queue, and its groups.
    samples: [Pipeline; 5],
    samples_groups: [u32; 3],
    /// The shadow map's cells, beam rays' run and samples' run (light_cell.wgsl,
    /// beam.light.wgsl, sample.light.wgsl). They run as the eye's do, over the light's groups,
    /// each run from its own queue's head.
    light_cell: Pipeline,
    light_corners: [Pipeline; 5],
    light_samples: [Pipeline; 5],
    /// The eye's beam rays' buffer (pxl-engine `beam`): a line a corner, a start a cell and a
    /// hit a sample, as the last full cast's `grid` lays them, then the eye's holes
    /// (`ray::holes_words`). It has room for `beam_cap` words.
    beams: u32,
    /// The head bound in the light's head's place for the eye's passes. It stays all zero,
    /// and a head that names no map size tells a pass both observers share that it runs for
    /// the eye (pxl-engine `light::HEADER_SIZE`).
    eye_header: u32,
    grid: beam::Grid,
    beam_cap: u32,
    /// The holes' queue's passes (pxl-engine `ray::QUEUE_LANES`). They walk the eye's holes,
    /// which its pixels' cast left, and the light's slots no voxel answered. They run as a
    /// `Run`, the eye's over `queue_groups` and the light's over `light_groups`, each from its
    /// own head in `queues`.
    queue_pipeline: Pipeline,
    queue_ask: Pipeline,
    queue_parked: Pipeline,
    queue_ask_parked: Pipeline,
    queue_last: Pipeline,
    /// The eye's holes' groups: the walks over the node cache alone, the asks, and the last
    /// walk.
    queue_groups: [u32; 3],
    /// Every run's queue: a head for each run, and the asks' records they share
    /// (`ray::QUEUE_WORDS`).
    queues: u32,
    /// The eye's holes' count and the counts' lanes (pxl-engine `ray::MISS_ARGS_WORDS`).
    marks: u32,
    /// Forgets the nodes over what changed since the last cast (`forget_cs`).
    forget_pipeline: Pipeline,
    node_cache: u32,
    /// The node cache's frame: how many frames walked it since it was last forgotten whole. A
    /// key's tag tells apart only `node_cache::FRAMES` of them, so the next cast forgets all
    /// once it reaches that.
    frame: u32,
    /// Whether the next cast forgets the whole node cache.
    forget: bool,
    /// The eye's clip levels' entries that moved since the last cast, a bit an entry, against
    /// the entries as `entries` last held them.
    changed: [u32; node_cache::CHANGED_WORDS as usize],
    /// For each eye clip level, the box its moved bricks lie in
    /// (`node_cache::CHANGED_BRICKS_AT`).
    changed_bricks: [[i32; 4]; clipmap::EYE_CLIP_LEVELS as usize],
    /// The boxes of what is placed or cut that changed, `holding` of them, held as decor's
    /// generation holds them (gpu/generator.rs `hold`).
    rects: [[i32; 4]; object::RECTS as usize],
    holding: usize,
    /// The child groups of what is placed that were written, `write_cnt` of them. The nodes
    /// handed a slot of one are forgotten.
    written_groups: [u32; node_cache::WRITTEN_CAP as usize],
    write_cnt: usize,
    /// What the last cast's head told the node cache to forget, put back if its frame is
    /// dropped.
    sent_forget: Option<ForgetHeader>,
    /// The eye's clip levels' entries as the last cast read them.
    entries: Vec<i32>,
    /// What this frame's cast forgets (pxl-engine `node_cache::FORGETS`).
    forget_kind: u32,
    /// The preview's cuts the last cast was asked with.
    cuts: [u32; eye::CUTS],
    /// Writes the world's depth from the light into each claimed slot of the shadow map, a
    /// thread a slot (`light_cs`).
    light: Pipeline,
    /// The passes that list the tiles of the light's box the receivers stand on (pxl-engine
    /// `light::LIST_RECEIVERS`), and then the tiles those read. The record's draw marks the
    /// tiles under the eye's voxels' boxes as it writes them (`Raster::record_draw`), and
    /// `light_mark_walked_cs` those under the hits of the pixels the eye walked apart.
    /// `light_mark_draws_cs` marks where decor and the host's other draws that read the map
    /// stand, over as many threads as the single thread of `light_mark_draws_open_cs` opens.
    /// Then `light_propagate_cs` lists the tiles the stood tiles read.
    light_mark_walked: Pipeline,
    light_mark_draw: Pipeline,
    light_mark_draw_open: Pipeline,
    light_propagate: Pipeline,
    /// Claims the tiles read that the epoch has not cast, over the propagation's group
    /// (`light_damage_cs`).
    light_damage: Pipeline,
    /// Sets back to be cast anew the tiles the box slid onto or into another band, and those
    /// whose rays may meet a column of the world that changed, before the propagation
    /// (`light_slide_cs`, pxl-engine `Light::keeps`).
    light_slide: Pipeline,
    /// The light's cast's groups, in the order `light_groups` makes them, and the buffers they
    /// bind. They are made again when any of those changes.
    light_groups: [u32; LIGHT_GROUPS],
    light_bufs: [u32; 13],
    /// Words bound in place of decor's buffers while decor is off. The drawn receivers' pass
    /// then reads none of them (`light_mark_draws_cs`).
    light_none: u32,
    /// The light's casts so far, which name its marks (pxl-engine `light::HEADER_CAST`). It is
    /// never zero, as a tile no cast marked holds zero.
    light_number: u32,
    /// The number of the cast that began the epoch (pxl-engine `light::HEADER_EPOCH`). An epoch
    /// is a run of light casts over which what decides a tile's content stands, so a tile cast
    /// since it began stands where the box kept it.
    light_epoch: u32,
    /// Whether the light's cast has its list and its raster's buffers and targets
    /// (`light_made`). No shadow map is cast while it does not.
    light_ready: bool,
    /// The light's list (pxl-engine `light::buf_words`), which exists while shadows are on
    /// and is 0 otherwise.
    light_list: u32,
    /// The light's holes' count and the counts' lanes, laid out as the eye's `marks` are.
    light_misses: u32,
    /// The dispatches of the passes that cast the light's tiles, which open as tiles are
    /// claimed, and what the propagation claims and counts (pxl-engine `light::args_words`).
    /// It exists while `light_list` does.
    light_args: u32,
    /// The drawn receivers' dispatch (`light::DRAWS_DISPATCH_WORDS`). It takes a buffer of its
    /// own, as the pass it dispatches writes the light's args.
    light_draws_dispatch: u32,
    /// The stood list's args: the propagation's dispatch over the stood tiles, their count,
    /// and the slide's dispatch (pxl-engine `light::RECEIVER_TILES_WORDS`). The stood list is
    /// the tiles the receivers stand on. It exists while `light_list` does.
    light_receiver_tiles: u32,
    /// The number of the cast that last marked the world's receivers afresh
    /// (`light::HEADER_WORLD`).
    light_world_number: u32,
    /// The number of the last cast that owed a slide (`light::HEADER_SLIDE_CAST`).
    light_slide_cast: u32,
    /// What decides a shadow map tile's content, as the light's cast last read it: the light's
    /// frame (`Shadow::generation`), whether shadows are cast, and whether the world under the
    /// map moved whole. Any of them moving begins an epoch (`light_epoch`). Nothing else of the
    /// eye's place moves a tile's content, as the box follows the eye a tile at a time and a
    /// tile's band depends on its place in the box alone (`Light::band`). Where the box slid
    /// within the epoch (`schedule::Light::slid`), the reads move with it, and the tiles it slid
    /// onto or into another band are cast again (`light_slide_cs`, pxl-engine `Light::keeps`).
    light_content: Read,
    /// Where the box slid, the reads as the cast before left them are copied out here, and
    /// back in shifted by the slide.
    light_read_was: u32,
    /// What decides which tiles the receivers read, as the light's cast last read it: the
    /// eye's place and look, the tiles the host's draws stand on, and decor placed again. Where
    /// only this moved, the cast casts only the tiles read anew.
    light_receiver_read: Read,
    /// The host's draws' boxes within the effects' reach this frame. `light_receiver_read` reads
    /// their tiles, and the light's head is handed their bounds.
    light_receiver_boxes: Vec<Receiver>,
    /// The light's head, which its passes read beside the eye's words (pxl-engine
    /// `light::HEADER_WORDS`).
    light_header: u32,
    /// The world's columns that changed this frame, in units from the eye (pxl-engine
    /// `light::column_of`). Where the epoch goes on, the cast casts again only the tiles whose
    /// rays may meet them (`Light::reaches`).
    light_columns: Vec<light::Column>,
    /// Decides what each frame casts (ray/schedule.rs).
    schedule: Schedule,
    /// Whether the record's draw ran this frame, which it does once at most
    /// (`Raster::record_draw`).
    draws_record: bool,
    /// Buffers the record's group binds in place of the light's list, args and stood list's
    /// args while the light's cast has none. No thread reads them, as the head then says no
    /// receiver stands (`Raster::record_group`).
    light_idle: [u32; 3],
    /// Whether a receiver other than the world's is drawn this frame, a host's draw or decor,
    /// so the drawn receivers' pass runs (`light_mark_draws_cs`).
    light_draws: bool,
    /// The group over the cast's world, which every kernel but the pixels' binds, and the
    /// buffers it binds. It is made again when any of them changes.
    group: u32,
    bindings: [u32; 11],
    eye: u32,
    pub(super) clipmap: u32,
    shards: u32,
    /// The shards `shards` has room for.
    shard_cap: usize,
    /// The world surface's drawn pixels, across and down. The cast casts a ray at each.
    size: [u32; 2],
    /// The overwrites' child pointer lane then their data lane, `object_cap` words each, and
    /// the revision they hold, none before the first upload.
    objects: u32,
    object_cap: usize,
    object_revision: Option<u32>,
    generator: Generator,
    /// What the last cast read, without the jitter and the grid. Comparing this frame's with
    /// it gives `schedule::CastRead::same`.
    read: Read,
    /// The raster of the record (ray/raster.rs). The pixels' cast reads the visible voxel it
    /// keeps at each pixel (`cast_cs`).
    raster: Raster,
    /// The pixels' cast's own group, and the buffers it binds: the eye, the rays, the beam
    /// rays' words, the node cache, the visibility buffer and the claimed tiles. It is made
    /// again when any of them changes.
    px_group: u32,
    px_bufs: [u32; 7],
    /// Lists the claimed tiles ahead of the pixels' cast, in the same pass, into `cast_tiles`,
    /// whose words give the pixels' indirect dispatch (cast_tile.wgsl). `cast_tiles` is made
    /// again with `px_bufs`.
    cast_tiles_pipeline: Pipeline,
    cast_tiles: u32,
    cast_tiles_group: u32,
    /// Counts what the eye's and the light's casts walked and reused, while the setting asks
    /// (ray/count.rs).
    counter: Counter,
}

/// What a cast's head told the node cache to forget (pxl-engine `node_cache::HEADER_WORDS`),
/// kept so that a dropped frame's forgetting passes to the next cast.
struct ForgetHeader {
    all: bool,
    changed: [u32; node_cache::CHANGED_WORDS as usize],
    changed_bricks: [[i32; 4]; clipmap::EYE_CLIP_LEVELS as usize],
    rects: [[i32; 4]; object::RECTS as usize],
    holding: usize,
    written_groups: [u32; node_cache::WRITTEN_CAP as usize],
    write_cnt: usize,
}

/// Notes child group `first` of what is placed as written, once, among the first `write_cnt` of
/// `written`, and says whether there was room. Past the room, the caller forgets the whole
/// node cache.
fn note(written: &mut [u32], write_cnt: &mut usize, first: u32) -> bool {
    if written[..*write_cnt].contains(&first) {
        return true;
    }
    if *write_cnt == written.len() {
        return false;
    }
    written[*write_cnt] = first;
    *write_cnt += 1;
    true
}

const CAST_WORKGROUP: [u32; 3] = [ray::CAST_SIDE, ray::CAST_SIDE, 1];
/// The workgroups of `cast_cs` a screen tile takes, across and down alike.
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
/// A box a host's draw stands within, inside the effects' reach: its corners in units from the
/// eye, as the light's passes read them (`ReceiverBox::from_eye`), and the first and last tiles
/// of the light's box it stands on (pxl-engine `Light::box_tiles`).
#[derive(Clone, Copy)]
struct Receiver {
    bounds: [[f32; 3]; 2],
    tiles: [[i32; 2]; 2],
}

/// The claimed tiles' list as a frame's cast starts it (cast_tile.wgsl): `cast_cs`'s
/// dispatch with no tile yet and a tile's workgroups across and down, then no tile counted.
const CAST_TILES_START: [u32; 4] = [0, TILE_CASTS, TILE_CASTS, 0];
/// How many groups the light's cast has (`light_groups`).
const LIGHT_GROUPS: usize = 17;

impl CastBuffer {
    /// The cast's id for the frame buffer named `buf` (frame/shader/group.rs `BUFS`), or 0
    /// for a buffer the cast does not hold.
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

impl RayPass {
    /// Makes the cast's pipelines and buffers, once, with the renderer and before its first
    /// frame (gpu/pipeline.rs). No frame names a pipeline until every one is made.
    pub fn make(&mut self, g: &Gpu, pipelines: &mut PipelineCache) {
        if self.cast.is_some() {
            return;
        }
        // Every kernel over the cast's world but the pixels' binds one group.
        let group = &WORLD_GROUP;
        let pipeline = pipelines.build_compute(g, COMPUTE_CAST, &CAST_GROUP);
        let cast_tiles_pipeline = pipelines.build_compute(g, COMPUTE_CAST_TILES, &CAST_TILES_GROUP);
        let cell = pipelines.build_compute(g, COMPUTE_CELL, group);
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
                COMPUTE_QUEUE_ASK,
                COMPUTE_QUEUE_SAMPLES_PARKED,
                COMPUTE_QUEUE_ASK_PARKED,
                COMPUTE_QUEUE_LAST_SAMPLES,
            ],
            [&LATTICE_WALK_GROUP, &LATTICE_ASK_GROUP, &LATTICE_LAST_GROUP],
        );
        let light_samples = run(
            [
                COMPUTE_QUEUE_LIGHT_SAMPLES,
                COMPUTE_QUEUE_ASK,
                COMPUTE_QUEUE_LIGHT_SAMPLES_PARKED,
                COMPUTE_QUEUE_ASK_PARKED,
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
                COMPUTE_QUEUE_ASK,
                COMPUTE_QUEUE_CORNERS_PARKED,
                COMPUTE_QUEUE_ASK_PARKED,
                COMPUTE_QUEUE_LAST_CORNERS,
            ],
            [&LATTICE_WALK_GROUP, &LATTICE_ASK_GROUP, &LATTICE_LAST_GROUP],
        );
        let light_corners = run(
            [
                COMPUTE_QUEUE_LIGHT_CORNERS,
                COMPUTE_QUEUE_ASK,
                COMPUTE_QUEUE_LIGHT_CORNERS_PARKED,
                COMPUTE_QUEUE_ASK_PARKED,
                COMPUTE_QUEUE_LAST_LIGHT_CORNERS,
            ],
            [
                &LIGHT_LATTICE_WALK_GROUP,
                &LATTICE_ASK_GROUP,
                &LIGHT_LATTICE_LAST_GROUP,
            ],
        );
        let light_cell = pipelines.build_compute(g, COMPUTE_LIGHT_CELL, group);
        let queue_pipeline = pipelines.build_compute(g, COMPUTE_QUEUE, &QUEUE_GROUP);
        let queue_parked = pipelines.build_compute(g, COMPUTE_QUEUE_PARKED, &QUEUE_GROUP);
        let queue_ask = pipelines.build_compute(g, COMPUTE_QUEUE_ASK, &LATTICE_ASK_GROUP);
        let queue_ask_parked =
            pipelines.build_compute(g, COMPUTE_QUEUE_ASK_PARKED, &LATTICE_ASK_GROUP);
        let queue_last = pipelines.build_compute(g, COMPUTE_QUEUE_LAST, &QUEUE_LAST_GROUP);
        let forget_pipeline = pipelines.build_compute(g, COMPUTE_FORGET, group);
        let light = pipelines.build_compute(g, COMPUTE_LIGHT, &LIGHT_GROUP);
        let light_mark_walked =
            pipelines.build_compute(g, COMPUTE_LIGHT_MARK_WALKED, &LIGHT_MARK_GROUP);
        let light_mark_draw =
            pipelines.build_compute(g, COMPUTE_LIGHT_MARK_DRAWS, &LIGHT_MARK_DRAWS_GROUP);
        let light_mark_draw_open = pipelines.build_compute(
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
        let c = CastBuffer {
            pipeline,
            cast_tiles_pipeline,
            cast_tiles: 0,
            cast_tiles_group: 0,
            cell,
            samples,
            samples_groups: [0; 3],
            corners,
            corners_groups: [0; 3],
            light_corners,
            light_cell,
            light_samples,
            beams: 0,
            // It is never written, as a buffer is made zeroed.
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
            write_cnt: 0,
            sent_forget: None,
            entries: Vec::new(),
            forget_kind: node_cache::FORGETS_NONE,
            cuts: [0; eye::CUTS],
            light,
            light_mark_walked,
            light_mark_draw,
            light_mark_draw_open,
            light_propagate,
            light_damage,
            light_slide,
            light_groups: [0; LIGHT_GROUPS],
            light_bufs: [0; 13],
            light_none: buf(
                word_bytes(2 * generation::drawing::REGIONS as usize),
                storage,
            ),
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
            light_read_was: buf(
                word_bytes((light::tiles(SHADOW_SIZE) * light::tiles(SHADOW_SIZE)) as usize),
                buffer_usage::COPY_SRC | buffer_usage::COPY_DST,
            ),
            light_receiver_read: Read::default(),
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

    /// Readies this frame's cast: it reads what the frame hands, uploads what changed, asks the
    /// schedule what to cast, and writes the heads and groups the passes read. `rays` is the
    /// world surface's rays view and its drawn pixels; `light_map` is the shadow map's world
    /// view, the light's frame and the rays' starts. `partial` says the frame may be drawn
    /// through the gate, and `frame` is the number the renderer gives this frame, which names
    /// its counts (count.rs `Frame`).
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
        light_map: (u32, u32, light::StartGrid),
        partial: bool,
        frame: f64,
    ) {
        // Without a cast, or before the terrain's buffers are made, there is no world: no ray is
        // cast or drawn, and no decor is generated.
        let (Some(cast), Some((ground, folds, pool))) = (
            cast,
            self.ground
                .as_ref()
                .map(|gb| (gb.ground, gb.folds, gb.pool)),
        ) else {
            if let Some(c) = self.cast.as_mut() {
                c.generator.off();
                c.schedule.off();
            }
            return;
        };
        let c = self.cast.as_mut().or_bug(Errno::ENOTRECOVERABLE);
        // The pattern names every field the frame hands, so a new field fails to compile until
        // the cast reads it. The overwrites' contents are read by their revision. Decor's
        // generation reads its own input (`Generator::prepare`), so a change to decor alone
        // casts no ray again.
        let Cast {
            eye,
            clipmap,
            shards,
            objects:
                ObjectTable {
                    first_child: _,
                    data: _,
                    cap,
                    revision,
                    since: _,
                    written_groups: _,
                },
            decor,
            // Only the light's cast reads these, with the look (below).
            receivers: _,
            ray_cnts,
        } = &cast;
        let ray_cnts = *ray_cnts;
        let r = &mut c.read;
        r.begin();
        // The eye's words up to the light's. This leaves out the light's words, which the
        // light's cast alone reads (below), and the jitter's and the grid's (pxl-engine
        // `eye::GRID`).
        r.read(&eye[..eye::LIGHT]);
        let clipmap_changed = r.read(clipmap);
        let shards_changed = r.read(shards);
        // A world surface made again holds no rays, so its id is read too.
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
            // One record however large, read where it lies.
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
            // Where the GPU holds revision `since`, only the child groups written after it go
            // up, a record each, if the stream has room for them all. Otherwise both lanes go up
            // whole, two records however large.
            fn child_group(lane: &[u32], first: u32) -> &[u32] {
                &lane[first as usize..(first + object::CHILDREN) as usize]
            }
            // The nodes handed a slot of a child group written since `since` are forgotten.
            // Past the room for them, or where revisions went by that the GPU never held, the
            // whole node cache is.
            if c.object_revision == Some(o.since) {
                for &first in o.written_groups {
                    c.forget |= !note(&mut c.written_groups, &mut c.write_cnt, first);
                }
            } else {
                c.forget = true;
            }
            let written_bytes = |lane: &[u32]| {
                o.written_groups
                    .iter()
                    .map(|&f| write_buffer_bytes(bytes(child_group(lane, f))))
                    .sum::<usize>()
            };
            if c.object_revision == Some(o.since)
                && api::room(written_bytes(o.first_child) + written_bytes(o.data))
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
        // What is placed and a preview's cuts forget the nodes over the boxes they changed, as
        // decor is generated again over them (gpu/generator.rs). Every write of what is placed
        // names its box (`DecorFrame::edits`); a write that names none, or the lanes made
        // again, forgets all.
        let edits = decor.edits;
        // The world's columns that changed, in units from the eye, for the light's cast
        // (`light_columns`).
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
        // The whole node cache is forgotten where the service area changed or what is placed
        // moved past what the boxes name, and once its frames would outrun what a key's tag
        // tells apart.
        c.forget |= shards_changed
            || shards_grow
            || objects_grow
            || (objects_changed && edits.is_empty())
            || c.frame >= node_cache::FRAMES;
        let cuts_changed = cuts != c.cuts;
        c.cuts = cuts;
        // The eye's clip levels' entries that hold another brick, or that hold their brick
        // present where they did not or no longer do. The nodes over them are asked again.
        let eye_rows = (clipmap::EYE_CLIP_LEVELS * clipmap::ENTRIES) as usize;
        let rows = &cast.clipmap[..4 * eye_rows];
        let entry = |r: &[i32]| clipmap::Entry::of([r[0], r[1], r[2], r[3]]);
        let present = |cell_zoom: Zoom, q: [i32; 2]| {
            let k = ((cell_zoom - clipmap::FIRST_CELL_ZOOM) * clipmap::ENTRIES + clipmap::entry(q))
                as usize;
            entry(&rows[4 * k..4 * k + 4]).holds(q)
        };
        // A walk's hit reads the cells of the bricks the clip levels hold present, each at its
        // own place, and every other read takes a bound over whichever of them are present. So
        // what the world's query reads moved only where an entry holds a brick present that it
        // did not, or no longer holds present the brick it holds. A clip level that follows its
        // center by a brick moves none of it: the brick it lets go and the one it takes, not
        // present yet, both lie past what its zoom observes about the eye (pxl-engine
        // `clipmap`), and both are read off the coarser clip level alike.
        let mut present_changed = false;
        if clipmap_changed {
            for k in 0..eye_rows {
                let row = &rows[4 * k..4 * k + 4];
                let was = c.entries.get(4 * k..4 * k + 4);
                if was.is_none_or(|w| entry(row).changed_from(entry(w))) {
                    c.changed[k / 32] |= 1 << (k % 32);
                }
                if was.is_none_or(|w| entry(row).present_changed_from(entry(w))) {
                    let level = &mut c.changed_bricks[k / clipmap::ENTRIES as usize];
                    for q in [Some(entry(row).brick), was.map(|w| entry(w).brick)]
                        .into_iter()
                        .flatten()
                        .filter(|&q| q != [i32::MIN; 2])
                    {
                        *level = node_cache::bricks_joined(*level, [q[0], q[1], q[0], q[1]]);
                    }
                }
                let brick_of = |r: &[i32]| entry(r).brick();
                let (now, was) = (brick_of(row), was.map(brick_of));
                // The bricks a query reads differently now: the one held present that was not,
                // and the one held present before that no longer is.
                let (added, dropped) = match was {
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
                if added {
                    changed(now.0);
                }
                if let (true, Some((q, _))) = (dropped, was) {
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
        // Whether what the world's query reads moved: the bricks the clip levels hold present,
        // the service area, what is placed, or a preview's cuts.
        let world_changed =
            present_changed || shards_changed || shards_grow || objects_changed || cuts_changed;
        // Decor is prepared first, as the light's cast reads where it stands and the light's
        // groups bind its buffers.
        let world = [c.eye, c.clipmap, ground, c.shards, c.objects, pool];
        let generation = Generation {
            clipmap: cast.clipmap,
            cuts: cast.eye[eye::CARVE..eye::CARVE + eye::CUTS]
                .try_into()
                .or_bug(Errno::ENOTRECOVERABLE),
            fresh: self.ground.as_ref().map_or(&[][..], |gb| &gb.bricks[..]),
            eye: &cast.eye,
        };
        c.generator
            .prepare(g, cast.decor, generation, meshes, world);
        // The shadow map is cast in every slot a receiver reads, by the same walk as the eye's
        // rays (pxl-engine `light`), where what it reads moved. That is the light's frame or the
        // world under the map; the box's slide or a step of the eye's zooms, which bound every
        // band, each keeping a tile whose band stands (`Light::keeps`); and what decides which
        // tiles are read: the eye's place and look, the boxes the host's draws stand in, and
        // decor placed again (`record_draw_cs`, `light_mark_walked_cs`, `light_mark_draws_cs`).
        // Where only the slide or which tiles are read moved, a tile cast within the epoch
        // stands where the box kept it, and only the tiles read anew are cast
        // (`light::HEADER_EPOCH`). Nothing is cast where no shadow is, or while the light's cast
        // has no buffers (`light_made`).
        let shadows = cast.eye[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS]
            .iter()
            .any(|&w| w != 0);
        // First what decides a tile's content; what decides which tiles are read follows.
        let r = &mut c.light_content;
        r.begin();
        r.read(&[light_map.1, u32::from(shadows)]);
        // The world under the map moved whole where the node cache forgets it whole, or where
        // no column says what changed. Otherwise only the tiles its columns reach are cast again.
        let world_whole = world_changed && (c.forget || c.light_columns.is_empty());
        r.mark(world_whole);
        let content_same = r.end();
        let world_columns = world_changed && !world_whole;
        // The box moves about the eye a tile at a time. What a tile holds moves with the
        // corner, and is kept where the slide kept its band (`light_slide_cs`).
        let corner = light::CORNER.map(|k| cast.eye[eye::LIGHT + k] as i32);
        // Where the host's draws stand: each box in units from the eye, and the tiles it stands
        // on under the light the cast is aimed by, by the rule every receiver follows
        // (pxl-engine `Light::box_tiles`), with the footprint where the eye stands, as the head
        // asks below. A box past the effects' reach stands on no tile and is left out, so what
        // moves there never casts the light. There are none where no shadow is cast.
        c.light_receiver_boxes.clear();
        if shadows {
            let words = cast.eye[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS]
                .try_into()
                .or_bug(Errno::ENOTRECOVERABLE);
            let lit = light::Light::of_words(words);
            let origin: [i32; 3] = core::array::from_fn(|k| cast.eye[eye::ORIGIN + k] as i32);
            for s in cast.receivers.iter().take(light::RECEIVER_BOXES) {
                let [lo, hi] = s.from_eye(origin);
                if let Some(tiles) = lit.box_tiles(SHADOW_SIZE, lo, hi, stands) {
                    c.light_receiver_boxes.push(Receiver {
                        bounds: [lo, hi],
                        tiles,
                    });
                }
            }
        }
        // What decides which tiles are read, not what a tile holds: the eye's place and look,
        // the tiles the host's draws stand on, and decor compacted again. The jitter is left
        // out, as a pixel's marks take its footprint (`light_mark_walked_cs`).
        let l = &mut c.light_receiver_read;
        l.begin();
        l.read(&cast.eye[eye::ORIGIN..eye::ORIGIN + 3]);
        l.read(&cast.eye[eye::FORWARD..eye::TAN + 2]);
        for s in &c.light_receiver_boxes {
            l.read(s.tiles.as_flattened());
        }
        l.mark(c.generator.counts());
        let receivers_same = l.end();
        let plan = c.schedule.plan(CastRead {
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
        // A full cast lays out the camera grid the eye's words hold (pxl-engine `eye::GRID`); a
        // `px` frame keeps the last cast's.
        if kind == FrameKind::cast {
            let tan = [cast.eye[eye::TAN], cast.eye[eye::TAN + 1]].map(f32::from_bits);
            c.grid = beam::Grid::of_words(
                tan,
                cast.eye[eye::GRID..eye::GRID + eye::GRID_WORDS]
                    .try_into()
                    .or_bug(Errno::ENOTRECOVERABLE),
            );
        }
        // The beam rays' buffer is made again only where the grid's words and the holes
        // outgrow it.
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
            let mut header = [0u32; light::HEADER_WORDS];
            c.light_number = c.light_number.wrapping_add(1).max(1);
            if due.epoch {
                c.light_epoch = c.light_number;
            }
            header[light::HEADER_CAST] = c.light_number;
            header[light::HEADER_EPOCH] = c.light_epoch;
            header[light::HEADER_COUNTED] = u32::from(ray_cnts);
            for (k, v) in light::header_layout(SHADOW_SIZE) {
                header[k] = v;
            }
            header[light::HEADER_LAST_CORNER] = due.last_corner[0] as u32;
            header[light::HEADER_LAST_CORNER + 1] = due.last_corner[1] as u32;
            header[light::HEADER_LAST_RANGE] = due.last_range[0];
            header[light::HEADER_LAST_RANGE + 1] = due.last_range[1];
            // The world's receivers are marked afresh in this cast where the walked bits cover
            // every pixel, and added to the marks already made, which hold the other pixels',
            // where they cover the claimed tiles alone. Otherwise they stand where they were
            // last marked. The head also names the last cast before this one that owed a slide
            // (light_receiver_tile.wgsl `receiver_tiles_first`).
            let afresh = due.world && !due.additive;
            if afresh {
                c.light_world_number = c.light_number;
            }
            header[light::HEADER_WORLD] = c.light_world_number;
            header[light::HEADER_SLIDE_CAST] = c.light_slide_cast;
            let owes = due.slid || due.columns_changed;
            if owes {
                c.light_slide_cast = c.light_number;
            }
            // The marks take the footprints where the eye stands. While it moves, the next
            // frame's rays mark again, and the first frame it stands in owes the footprints.
            header[light::HEADER_FOOTPRINT] = u32::from(due.footprint);
            header[light::HEADER_WORLD_RECEIVERS] = u32::from(due.world);
            let at = 4 * light::STARTS_AT;
            header[at..at + 4 * light::STARTS_VEC4S].copy_from_slice(&light_map.2.words());
            // The host's draws that read the map receive it as decor does, a box each
            // (`light_mark_draws_cs`). Each box is in units from the eye's origin, the Q3 that
            // generated decor is placed against, rounded outward (`ReceiverBox::from_eye`).
            // Only those within the effects' reach are handed (`light_receiver_boxes`).
            let at = 4 * light::RECEIVER_BOXES_AT;
            let n = c.light_receiver_boxes.len();
            c.light_draws = n != 0 || c.generator.decor_buf().is_some();
            header[at] = n as u32;
            for (
                k,
                &Receiver {
                    bounds: [lo, hi], ..
                },
            ) in c.light_receiver_boxes.iter().enumerate()
            {
                let w = at + 4 * (1 + 2 * k);
                header[w..w + 3].copy_from_slice(&lo.map(f32::to_bits));
                header[w + 4..w + 7].copy_from_slice(&hi.map(f32::to_bits));
            }
            // The world's columns that changed, where the epoch goes on and the slide casts the
            // tiles they reach again (`light_slide_cs`).
            let at = 4 * light::COLUMNS_AT;
            if due.columns_changed {
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
            // Where the world's receivers are marked afresh, the stood list starts empty with
            // none of it propagated, and the read list empties with it (`light_args`).
            // Otherwise `dispatch` copies back in what the casts since then left. The slide's
            // rows open where the cast owes one.
            let receiver_tiles = light_receiver_tiles(owes);
            let open = if afresh {
                &sequence::OPEN_LIGHT_RECEIVER_TILES_AFRESH
            } else {
                &sequence::OPEN_LIGHT_RECEIVER_TILES
            };
            let from = open.to.at as usize;
            g.write_buf(
                c.light_receiver_tiles,
                open.to_at(),
                bytes(&receiver_tiles[from..]),
            );
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
        // The record's draw reads this flag of the light's head every frame
        // (`light::HEADER_WORLD_RECEIVERS`), so a frame the light does not cast clears it, and
        // no receiver stands.
        if plan.light.is_none() {
            g.write_buf(
                c.light_header,
                word_bytes(light::HEADER_WORLD_RECEIVERS),
                bytes(&[0u32]),
            );
        }
        // The eye's words go up with the grid the last full cast laid (`grid`) in place of the
        // frame's own.
        if casts || plan.light.is_some() {
            let mut words = cast.eye;
            words[eye::GRID..].copy_from_slice(&c.grid.words());
            g.write_buf(c.eye, 0, bytes(&words));
        }
        if casts {
            c.forget_kind = if c.forget {
                node_cache::FORGETS_ALL
            } else if c.changed.iter().any(|&m| m != 0) || c.holding != 0 || c.write_cnt != 0 {
                node_cache::FORGETS_CHANGED
            } else {
                node_cache::FORGETS_NONE
            };
            // The node cache's head before the frame's dispatches: what it forgets, and over
            // which entries. Its frame number restarts at 1 where the cache is forgotten whole.
            let mut header = [0u32; node_cache::HEADER_WORDS as usize];
            c.frame = if c.forget_kind == node_cache::FORGETS_ALL {
                1
            } else {
                c.frame + 1
            };
            header[node_cache::FRAME as usize] = c.frame;
            header[node_cache::FORGETS as usize] = c.forget_kind;
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
            header[written] = c.write_cnt as u32;
            header[written + 1..written + 1 + c.write_cnt]
                .copy_from_slice(&c.written_groups[..c.write_cnt]);
            // The holes' stops' dispatch starts with no workgroup across, one deep and one high.
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
                all: c.forget_kind == node_cache::FORGETS_ALL,
                changed: c.changed,
                changed_bricks: c.changed_bricks,
                rects: c.rects,
                holding: c.holding,
                written_groups: c.written_groups,
                write_cnt: c.write_cnt,
            });
            (c.forget, c.changed) = (false, [0; node_cache::CHANGED_WORDS as usize]);
            c.changed_bricks = [node_cache::NO_BRICKS; clipmap::EYE_CLIP_LEVELS as usize];
            (c.holding, c.write_cnt) = (0, 0);
        } else {
            c.sent_forget = None;
        }
        // Each run that runs starts its queue empty. The eye's holes' queue was emptied above,
        // with the node cache's head.
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
                sequence::OPEN_NODE_CACHE_CNTS.to_at(),
                bytes(&[0u32]),
            );
            // A frame that casts only the light still steps the node cache's frame, as a cast
            // does, so what the frames before kept becomes a slot a claim may take (ask.wgsl
            // `claim`). The frame stops at `node_cache::FRAMES`, what a key's tag tells apart,
            // and the next cast then forgets all.
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
            // The eye counts only in a full cast, so what a cast asks is not spread over the
            // frames that cast only the pixels.
            casts: [kind == FrameKind::cast, light_casts],
            rays: [c.size[0] * c.size[1], SHADOW_SIZE * SHADOW_SIZE],
        };
        c.counter.begin(g, ray_cnts, c.node_cache, frame);
        let bindings = [
            c.eye,
            c.clipmap,
            ground,
            rays.0,
            folds,
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
            let group = g.frame_group(c.cell.layouts[0], &WORLD_BINDS, buf, &observer);
            for group in c.queue_groups {
                release(group);
            }
            let world = [eye, clipmap, ground, folds, c.shards, c.objects, pool];
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
                // No world buffers: a pixel walks nothing itself (`cast_cs`).
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
            .decor_buf()
            .map_or([c.light_none; 3], |p| [p.drawing, p.regions, p.instances]);
        let light_bufs = [
            c.eye,
            c.clipmap,
            ground,
            folds,
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
        // The light's groups exist once the light's cast has its buffers, and are released
        // with them (`light_made`).
        if c.light_ready && light_bufs != c.light_bufs {
            for group in c.light_groups {
                release(group);
            }
            c.light_groups = light_groups(
                g,
                [
                    &c.light,
                    &c.cell,
                    &c.raster.light_sample_box,
                    &c.queue_last,
                    &c.light_mark_walked,
                    &c.light_mark_draw,
                    &c.light_propagate,
                    &c.light_slide,
                    &c.queue_pipeline,
                    &c.light_samples[0],
                    &c.light_corners[0],
                    &c.light_corners[1],
                    &c.queue_ask,
                    &c.light_mark_draw_open,
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
        // The record's draw marks the tiles its boxes stand on through the light's head, list,
        // args and stood list's args, so its group binds them, or idle words where the light's
        // cast has no buffers.
        let [list, args, receiver_tiles] = if c.light_list != 0 {
            [c.light_list, c.light_args, c.light_receiver_tiles]
        } else {
            c.light_idle
        };
        c.raster
            .record_group(g, [c.light_header, list, args, receiver_tiles]);
        c.raster
            .light_record_group(g, [c.light_header, list, args, receiver_tiles, light_map.0]);
        let eye = c.eye;
        self.regroup(g, [rays.0, eye]);
    }

    /// Whether this frame casts every pixel again though only the claimed tiles moved, as the
    /// light marks the world's receivers afresh and needs walked bits that cover every pixel.
    /// The frame is then drawn whole.
    pub fn unmasked(&self) -> bool {
        self.cast
            .as_ref()
            .is_some_and(|c| c.schedule.last().unmasked)
    }

    /// What this frame casts of the eye's rays, as `prepare` decided it
    /// (`schedule::Plan::kind`).
    pub fn kind(&self) -> FrameKind {
        self.cast
            .as_ref()
            .map_or(FrameKind::plain, |c| c.schedule.last().kind)
    }

    /// The renderer draws the frame whole after all, so its samples' raster is drawn whole.
    pub fn whole(&mut self) {
        if let Some(c) = self.cast.as_mut() {
            c.schedule.whole();
        }
    }

    /// Encodes the frame's cast: the eye's rays and their holes where they are cast, the decor
    /// generated over the world where it moves, then the light's cast where it casts. Each step
    /// is a pass of its own timed in its own span, so what each costs is read apart, and the
    /// spans whose threads run over the drawn pixels are told from the rest (span.json
    /// `per_px`).
    pub fn dispatch(&mut self, ctx: &mut RenderContext) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let (generates, casts, kind) = (self.generates(), self.casts_px(), self.kind());
        let claimed = self.tiles;
        let Some(c) = self.cast.as_mut() else { return };
        let plan = *c.schedule.last();
        let lit = plan.light;
        // Where this light cast keeps the receivers' marks or adds to them, what the casts since
        // they were last marked afresh left in the stood list's args, and the read list's
        // count, are copied back in (`light::carry_at`). This runs ahead of every pass that
        // marks a tile, the record's draw among the eye's passes.
        c.draws_record = false;
        if lit.is_some_and(|l| !l.world || l.additive) {
            let back = &sequence::RECEIVER_TILES_BACK;
            g.copy_buf(
                encoder,
                (c.light_list, back.from_at()),
                (c.light_receiver_tiles, back.to_at()),
                word_bytes(light::RECEIVER_TILES_CARRY as usize),
            );
            let back = &sequence::READ_CNT_BACK;
            g.copy_buf(
                encoder,
                (c.light_list, back.from_at()),
                (c.light_args, back.to_at()),
                4,
            );
        }
        if casts {
            // The eye's cast, in order: the node cache forgets what it must, and what draws its
            // entries goes with them; the beam rays walk it, a corner each; each cell's start is
            // found off them; the samples walk it from their cells' starts and set their stops
            // in the record; the raster draws the record's voxels as boxes and keeps each
            // pixel's nearest (ray/raster.rs); the pixels take their voxel off that and mark the
            // pixels it missed; and the holes' queue walks those (below). Each step reads what
            // the steps before wrote (pxl-engine `node_cache`). Each is timed in its own span, so
            // the cast's parts are read apart, and `cast` times the forgetting alone. Where only
            // the jitter moved (`FrameKind::px`), the beam rays, the cells' starts and the
            // samples stand, and the rest is cast again off them.
            let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::cast).pass());
            g.set_bind_group(pass, 0, c.group, &[]);
            if c.forget_kind != node_cache::FORGETS_NONE {
                g.set_pipeline(pass, c.forget_pipeline.id);
                g.dispatch(pass, node_cache::ENTRIES.div_ceil(ROW_LANES), 1, 1);
            }
            g.end(pass);
            // The beam rays' run, each ray from the root, in a frame that casts the samples
            // (corner.wgsl).
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
                        hits: None,
                    },
                );
                g.end(pass);
            }
            for (span, pipeline, [x, y], w) in [
                (
                    GpuSpanName::grid_cell,
                    &c.cell,
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
                    // The samples' run, each from its cell's start (sample.wgsl).
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
                                hits: None,
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
                    // The walked bits are cleared, so they hold this cast's holes alone, and a
                    // gated cast's in the claimed tiles alone (cast.wgsl).
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
        // The holes' queue walks the eye's holes, and every stop goes into the record.
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
                [GpuSpanName::hole; 2],
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
                    hits: Some(&sequence::HITS),
                },
            );
            g.end(pass);
            // Every stop is recorded, its new faces are asked, and its face word is put at its
            // pixel (ray/raster.rs `record_hits`). The next frame's raster draws them.
            c.raster.record_hits(ctx, c.node_cache, GpuSpanName::hole);
        }
        // Decor is generated after the rays and before the light's cast, which marks where
        // decor stands (`light_mark_draws_cs`).
        if generates {
            let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::generate).pass());
            c.generator.dispatch(g, pass);
            g.end(pass);
        }
        if let Some(due) = lit {
            // The reads move with the box, a tile at a time. Each tile takes the word of the
            // tile that held its world texels under the last corner, by a copy out and back in;
            // the slide's pass then clears it where the box slid onto it or into another band
            // (`light_slide_cs`).
            if due.slid {
                let n = light::tiles(SHADOW_SIZE) as i32;
                let t = light::TILE as i32;
                let dx = (due.corner[0] - due.last_corner[0]) / t;
                let dy = (due.corner[1] - due.last_corner[1]) / t;
                let shift = dy * n + dx;
                let all = n * n;
                if shift.abs() < all {
                    let words = |w: i32| word_bytes(w as usize);
                    let out = (c.light_read_was, 0);
                    let reads = (c.light_list, sequence::LIGHT_READS_OUT.from_at());
                    g.copy_buf(encoder, reads, out, words(all));
                    let (from, to) = if shift >= 0 { (shift, 0) } else { (0, -shift) };
                    g.copy_buf(
                        encoder,
                        (c.light_read_was, words(from)),
                        (c.light_list, sequence::LIGHT_READS_IN.to_at() + words(to)),
                        words(all - shift.abs()),
                    );
                }
            }
            // The tiles the world's receivers stand on (`light::LIST_RECEIVERS`). The record's
            // draw marks every box it writes, in the eye's pixels' cast, or here where that did
            // not run (`draws_record`). Then every pixel the eye walked apart marks its hit, a
            // thread a word of the walked bits, in a span of their own (`light_world_receivers`,
            // span.json `per_px`). Where these marks are added to those already made, the
            // earlier ones were copied back in before the frame's first pass (above).
            if due.world {
                if !c.draws_record {
                    c.raster.record_draw(ctx);
                }
                let stamp = ctx.stamp(GpuSpanName::light_world_receivers).pass();
                let pass = g.compute_pass(encoder, stamp);
                g.set_bind_group(pass, 0, c.light_groups[4], &[]);
                g.set_pipeline(pass, c.light_mark_walked.id);
                let rows = ray::walked_words(c.size).div_ceil(ray::MISS_LANES * ray::MISS_ACROSS);
                g.dispatch(pass, ray::MISS_ACROSS, rows, 1);
                g.end(pass);
            }
            // The tiles the drawn receivers stand on, where any is drawn: decor, and the host's
            // other draws that read the map. One thread opens the dispatch
            // (`light_mark_draws_open_cs`), and the marking runs over as many threads as it
            // opened (`light_mark_draws_cs`).
            if c.light_draws {
                let stamp = ctx.stamp(GpuSpanName::light_draws).pass();
                let pass = g.compute_pass(encoder, stamp);
                g.set_bind_group(pass, 0, c.light_groups[13], &[]);
                g.set_pipeline(pass, c.light_mark_draw_open.id);
                g.dispatch(pass, 1, 1, 1);
                g.set_bind_group(pass, 0, c.light_groups[5], &[]);
                g.set_pipeline(pass, c.light_mark_draw.id);
                g.dispatch_indirect(
                    pass,
                    c.light_draws_dispatch,
                    sequence::LIGHT_MARK_DRAWS.at(),
                );
                g.end(pass);
            }
            // The stood tiles' count is copied where the propagation reads it. Then, where the
            // cast owes a slide and a tile is stood, the slide sets back to be cast anew the
            // tiles the box slid onto or into another band, and those whose rays may meet a
            // column of the world that changed, a thread a tile (`light_slide_cs`). Then the
            // propagation lists the tiles they read, a thread for each tile stood since the
            // casts before propagated theirs (`light_propagate_cs`). A cast that stands on no
            // tile runs neither.
            g.copy_buf(
                encoder,
                (
                    c.light_receiver_tiles,
                    sequence::RECEIVER_TILES_CNT.from_at(),
                ),
                (c.light_list, sequence::RECEIVER_TILES_CNT.to_at()),
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
            // What this cast stood on and read is kept for the next, with the count it
            // propagated (`light::carry_at`).
            let keep = &sequence::RECEIVER_TILES_KEEP;
            g.copy_buf(
                encoder,
                (c.light_receiver_tiles, keep.from_at()),
                (c.light_list, keep.to_at()),
                word_bytes(light::RECEIVER_TILES_CARRY as usize),
            );
            let keep = &sequence::READ_CNT_KEEP;
            g.copy_buf(
                encoder,
                (c.light_args, keep.from_at()),
                (c.light_list, keep.to_at()),
                4,
            );
            // Then every tile read that its epoch has not cast is claimed, a thread a tile of the
            // box (`light_damage_cs`).
            let stamp = ctx.stamp(GpuSpanName::light_damage).pass();
            let pass = g.compute_pass(encoder, stamp);
            g.set_bind_group(pass, 0, c.light_groups[6], &[]);
            g.set_pipeline(pass, c.light_damage.id);
            let n = light::tiles(SHADOW_SIZE);
            g.dispatch(pass, (n * n).div_ceil(light::LIST_LANES), 1, 1);
            g.end(pass);
            // The claimed lists' counts are copied where the passes that take them read them.
            for step in [
                &sequence::CORNERS_CNT,
                &sequence::SOURCES_CNT,
                &sequence::DAMAGE_CNT,
            ] {
                g.copy_buf(
                    encoder,
                    (c.light_args, step.from_at()),
                    (c.light_list, step.to_at()),
                    4,
                );
            }
            // The light's cast, a workgroup a tile (pxl-engine `light::TILE`): the beam rays,
            // the samples, their boxes drawn, the slots taking their visible voxel, and the rest
            // walked off their list as the eye's holes are. First the beam rays' run over the
            // listed corners, each ray from the root at its tile's band (corner.wgsl over
            // beam.light.wgsl); then the cells' starts, over the light's world group with the
            // light's head.
            let [walk, ask, walk_parked, ask_parked, last] = &c.light_corners;
            let pass = walk_run(
                ctx,
                [GpuSpanName::light_beam_hit, GpuSpanName::light_beam],
                &Run {
                    pipelines: [walk, ask, walk_parked, ask_parked, last],
                    groups: [c.light_groups[10], c.light_groups[11], c.light_groups[16]],
                    queue: c.queues,
                    node_cache: c.node_cache,
                    hits: None,
                },
            );
            g.set_bind_group(pass, 0, c.light_groups[1], &[]);
            g.set_pipeline(pass, c.light_cell.id);
            g.dispatch_indirect(pass, c.light_args, sequence::LIGHT_CELLS.at());
            g.end(pass);
            // The samples' run over the samples of the tiles they feed, each from its cell's
            // start (sample.wgsl over sample.light.wgsl).
            let [walk, ask, walk_parked, ask_parked, last] = &c.light_samples;
            let pass = walk_run(
                ctx,
                [GpuSpanName::light_sample; 2],
                &Run {
                    pipelines: [walk, ask, walk_parked, ask_parked, last],
                    groups: [c.light_groups[9], c.light_groups[14], c.light_groups[15]],
                    queue: c.queues,
                    node_cache: c.node_cache,
                    hits: None,
                },
            );
            g.end(pass);
            // The samples' stops are set and drawn as the eye's are (ray/raster.rs `draw`).
            let stamp = ctx.stamp(GpuSpanName::light_raster).pass();
            let pass = g.compute_pass(encoder, stamp);
            g.set_bind_group(pass, 0, c.light_groups[2], &[]);
            g.set_pipeline(pass, c.raster.light_sample_box.id);
            g.dispatch_indirect(pass, c.light_args, sequence::LIGHT_SAMPLE_BOX.at());
            g.end(pass);
            c.raster.light_draw(ctx);
            let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::light_slot).pass());
            g.set_bind_group(pass, 0, c.light_groups[0], &[]);
            g.set_pipeline(pass, c.light.id);
            g.dispatch_indirect(pass, c.light_args, sequence::LIGHT_SLOTS.at());
            g.end(pass);
            // The light's holes' count is copied where the holes' queue reads it, then the queue
            // walks them.
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
                    hits: Some(&sequence::LIGHT_HITS),
                },
            );
            g.end(pass);
            // Every stop the queue listed is recorded after the queue and before the light's
            // next draw (ray/raster.rs `light_record_hits`, `light_draw`).
            c.raster
                .light_record_hits(ctx, c.node_cache, GpuSpanName::light_walk);
        }
        let counted = CountBuffer {
            node_cache: c.node_cache,
            args: [c.marks, c.light_misses],
            boxes: [c.raster.args, c.raster.light.args],
        };
        c.counter.copy(g, encoder, &counted);
    }

    /// Makes the buffers only a map cast needs, before the frame, where `on`, the shadows'
    /// setting (`FrameInput::shadows`), is set: the light's list (`light::buf_words`) and
    /// its raster's buffers and targets (`Raster::light_made`). The cast is ready once they are
    /// made (`light_ready`), and `prepare` makes its groups over them. Where shadows are off,
    /// they are released with the groups, so the first map cast after they are made again
    /// begins an epoch and casts whole. The light's passes are made with the eye's (`make`).
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

    /// Maps back the counts the last frame copied (`Counter::back`).
    pub fn cnts_back(&mut self, g: &Gpu) {
        if let Some(c) = self.cast.as_mut() {
            c.counter.back(g);
        }
    }

    pub fn take_cnts(&mut self) -> Option<RayCount> {
        self.cast.as_mut().and_then(|c| c.counter.take())
    }

    pub fn light_casts(&self) -> bool {
        self.cast
            .as_ref()
            .is_some_and(|c| c.schedule.last().light.is_some())
    }

    /// What the shadow map's pass reads of the light's cast, while the light's list exists: the
    /// list, whose reads tell the tiles its epoch cast and which tiles are read
    /// (`light::reads_at`, `light::LIST_READS`); the cast that began the host's epoch
    /// (`light::HEADER_EPOCH`); and the args, whose lay draws the tiles read (`light::ARGS_LAY`).
    pub fn light_read(&self) -> Option<LightRead> {
        self.cast
            .as_ref()
            .filter(|c| c.light_list != 0)
            .map(|c| LightRead {
                list: c.light_list,
                epoch: c.light_epoch,
                args: c.light_args,
            })
    }

    /// What changed that cast the light's map this frame, a bit for each `LightChanged`
    /// (span.json `LIGHT_CHANGED`), or 0 where it did not cast.
    pub fn light_changed(&self) -> u32 {
        self.cast
            .as_ref()
            .and_then(|c| c.schedule.last().light)
            .map_or(0, |l| l.changed)
    }

    /// How many rects mark decor's slots to generate again this frame (gpu/generator.rs
    /// `marked_rects`).
    pub fn decor_rects(&self) -> u32 {
        self.cast.as_ref().map_or(0, |c| c.generator.marked_rects())
    }

    pub fn generates(&self) -> bool {
        self.cast.as_ref().is_some_and(|c| c.generator.works())
    }

    pub fn generated(&self) -> Option<IndirectDraw> {
        self.cast.as_ref().and_then(|c| c.generator.indirect_draw())
    }

    /// Culls the decor generated to the instances a frame draws through `mask`
    /// (`Generator::cull`).
    pub fn cull_decor(&self, ctx: &mut RenderContext, mask: Mask, frame: u32, tiles: &TileGrid) {
        if let Some(c) = self.cast.as_ref() {
            c.generator.cull(ctx, mask, frame, tiles);
        }
    }

    /// The frame was dropped with its cast and its image, so the rays and the image hold
    /// older ones.
    pub fn on_dropped(&mut self) {
        if let Some(c) = self.cast.as_mut() {
            // The dropped cast never forgot what its head named, so the next cast forgets it
            // too, every box and child group as sent, as decor's generation holds its edits
            // again (`carry_edits`).
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
                for &first in &f.written_groups[..f.write_cnt] {
                    c.forget |= !note(&mut c.written_groups, &mut c.write_cnt, first);
                }
            }
            c.schedule.dropped();
            c.generator.on_dropped();
            c.counter.on_dropped();
        }
    }

    /// The frame was drawn, so the marks its decor generated by are done with.
    pub fn on_drawn(&mut self) {
        if let Some(c) = self.cast.as_mut() {
            c.generator.on_drawn();
        }
    }

    /// A frame's cast was handed but never drawn. The edits its decor named are held for the
    /// next frame that is drawn.
    pub fn carry_edits(&mut self, edits: &[[i32; 4]]) {
        if let Some(c) = self.cast.as_mut() {
            for &b in edits {
                hold(&mut c.rects, &mut c.holding, b);
            }
            c.generator.carry_edits(edits);
        }
    }
}

/// The light's args as a cast starts (`light::ARGS_WORDS`). Each pass that casts the light's
/// tiles has a list's row of workgroups across and no row, and the lay has a tile's vertices
/// and no instance.
fn light_args() -> [u32; light::ARGS_WORDS as usize] {
    let mut args = [0; light::ARGS_WORDS as usize];
    for w in [light::ARGS_CELLS, light::ARGS_SAMPLE_BOX, light::ARGS_SLOT] {
        args[w as usize..w as usize + 3].copy_from_slice(&[light::LIST_ACROSS, 0, 1]);
    }
    args[light::ARGS_LAY as usize] = light::LAY_VERTICES;
    args
}

/// The stood list's args as a cast starts (`light::RECEIVER_TILES_WORDS`): no stood tile, and
/// the propagation's dispatch as `ray::LEFT_EMPTY` lays it. The slide's dispatch has no
/// workgroup across, and as many rows as cover a side of the box's tiles where the cast `owes`
/// a slide, as the box slid or a column changed.
fn light_receiver_tiles(owes: bool) -> [u32; light::RECEIVER_TILES_WORDS as usize] {
    let mut w = [0; light::RECEIVER_TILES_WORDS as usize];
    let list = light::RECEIVER_TILES_LIST as usize;
    w[list..list + ray::LEFT_EMPTY.len()].copy_from_slice(&ray::LEFT_EMPTY);
    let n = light::tiles(SHADOW_SIZE).div_ceil(SQUARE_SIDE);
    let slide = light::RECEIVER_TILES_SLIDE as usize;
    w[slide..slide + 3].copy_from_slice(&[0, if owes { n } else { 0 }, 1]);
    w
}

/// Makes the light's args buffer (`light::args_words`).
fn light_args_buf(g: &Gpu) -> u32 {
    g.buf(
        word_bytes(light::args_words(SHADOW_SIZE) as usize),
        buffer_usage::STORAGE
            | buffer_usage::COPY_DST
            | buffer_usage::INDIRECT
            | buffer_usage::COPY_SRC,
    )
}

/// Makes the light's list buffer: the words of `light::list_words`, then the pool and the
/// stops (`light::buf_words`).
fn light_list(g: &Gpu) -> u32 {
    g.buf(
        word_bytes(light::buf_words(SHADOW_SIZE) as usize),
        buffer_usage::STORAGE
            | buffer_usage::COPY_DST
            | buffer_usage::INDIRECT
            | buffer_usage::COPY_SRC,
    )
}

/// Makes the light's cast's groups 0, each as its table says (frame/shader/group.rs), over the
/// frame's buffers (`buf`) and `bindings`: the world's buffers, which the cast alone holds; the
/// light's map; the eye's rays, which the walked mark reads; the decor's drawing words,
/// regions and instances, which the drawn mark reads; and the head and the raster's visibility
/// buffer. The light's world group has the layout of the eye's passes over `WORLD_GROUP`, and
/// binds the light's list where the eye's binds the beam rays' buffer, and the light's head
/// where the eye's binds its own.
///
/// The groups, by index: the slots'; the world's; the boxes'; the holes' last walk; the walked
/// mark; the drawn mark; the propagation; the slide; the holes' walks over the node cache
/// alone; the samples' walks over the node cache alone; the beam rays' walks over the node
/// cache alone and their asks; the holes' asks; the drawn receivers' opening; the samples'
/// asks and last walk; and the beam rays' last walk.
fn light_groups(
    g: &Gpu,
    [
        walk,
        world,
        sample_box,
        queue_last,
        mark,
        mark_draw,
        propagate,
        slide,
        queue_walk,
        samples_walk,
        corners_walk,
        corners_ask,
        queue_ask,
        mark_draw_open,
        samples_ask,
        samples_last,
        corners_last,
    ]: [&Pipeline; LIGHT_GROUPS],
    buf: impl Fn(u32) -> u32 + Copy,
    bindings: [u32; 13],
    [header, visibility]: [u32; 2],
) -> [u32; LIGHT_GROUPS] {
    let [eye, clipmap, ground, folds, shards, objects, pool, light, rays, drawing, regions, instances, _] =
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
    let worlds = [eye, clipmap, ground, folds, shards, objects, pool];
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
        of(sample_box, &LIGHT_SAMPLE_BOX_BINDS),
        last,
        of(mark, &LIGHT_MARK_BINDS),
        of(mark_draw, &LIGHT_MARK_DRAWS_BINDS),
        of(propagate, &LIGHT_PROPAGATE_BINDS),
        of(slide, &LIGHT_SLIDE_BINDS),
        queue_walk,
        samples_walk,
        corners_walk,
        corners_ask,
        ask,
        of(mark_draw_open, &LIGHT_MARK_DRAWS_OPEN_BINDS),
        samples_ask,
        samples_last,
        corners_last,
    ]
}

/// Makes a run's groups 0, for either observer and any run: its walks over the node cache
/// alone, its asks, and its last walk, each as its table says, over the frame's buffers
/// (`buf`) and what else it binds. That is `world`, the eye and the world's buffers the asks
/// read; `out`, the rays or the map its holes write; and `header`, its observer's head.
fn queue_groups(
    g: &Gpu,
    pipelines: [&Pipeline; 3],
    tables: [&Group; 3],
    buf: impl Fn(u32) -> u32 + Copy,
    (world, out, header): ([u32; 7], u32, u32),
) -> [u32; 3] {
    let [eye, clipmap, ground, folds, shards, objects, pool] = world;
    let rest = [
        buf_entry(0, eye),
        buf_entry(1, clipmap),
        buf_entry(2, ground),
        view_entry(3, out),
        buf_entry(4, folds),
        buf_entry(5, shards),
        buf_entry(6, objects),
        buf_entry(7, pool),
        buf_entry(HEADER_BINDING, header),
    ];
    core::array::from_fn(|k| g.frame_group(pipelines[k].layouts[0], tables[k], buf, &rest))
}

/// A run of the queue's passes (pxl-engine `ray::QUEUE_LANES`): the pipelines of its first
/// walk, its asks, its second walk, their asks and its last walk; its groups
/// (`queue_groups`); the queues' buffer, which holds its head; the node cache, whose head takes
/// its stops' count; and, for a holes' run, the step by which that count is copied before the
/// last walk (`sequence::HITS`).
struct Run<'a> {
    pipelines: [&'a Pipeline; 5],
    groups: [u32; 3],
    queue: u32,
    node_cache: u32,
    hits: Option<&'a Step>,
}

/// Encodes a run's passes, each a fixed dispatch over what the one before wrote. The two walks
/// over the node cache alone, each followed by the asks of the nodes it parked rays at, are
/// timed in `spans[0]`. A holes' run then copies its stops' count and dispatch into the node
/// cache's head. The last walk runs in a pass timed in `spans[1]`, which is returned open for
/// what follows it.
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
    if let Some(hits) = r.hits {
        g.copy_buf(
            encoder,
            (r.queue, hits.from_at()),
            (r.node_cache, hits.to_at()),
            word_bytes(4),
        );
    }
    let pass = g.compute_pass(encoder, ctx.stamp(spans[1]).pass());
    g.set_bind_group(pass, 0, last_group, &[]);
    g.set_pipeline(pass, last.id);
    g.dispatch(pass, ray::QUEUE_GROUPS, 1, 1);
    pass
}

/// Puts column `c` among `columns`. The light's head holds `light::COLUMNS` of them, and a
/// column past them is taken into the last, as the host's draws' boxes are (gpu/draw.rs
/// `stand`).
fn put_column(columns: &mut Vec<light::Column>, c: light::Column) {
    let full = columns.len() == light::COLUMNS;
    match columns.last_mut() {
        Some(last) if full => last.hold(c),
        _ => columns.push(c),
    }
}
