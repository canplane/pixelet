// pxl-render/gpu/ray/cast.rs

//! The GPU's cast of the eye's rays into the world surface's rays, the
//! world's depth from the light cast with them, and the decor generated over the
//! world (gpu/generator.rs): what each reads, made once, and their dispatches.

use pxl_spec_core::clipmap::{self, FILL_CELL, FILL_FIRST};
use pxl_spec_core::generation::{self, GROUPS};
use pxl_spec_core::{beam, eye, light, overwrite, pyramid, raster, ray};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use super::raster::Raster;
use super::tally::{self, Counts, RayCounts, Tally};
use super::{casts_px, RaysPass};
use crate::frame::shader::floor;
use crate::frame::shader::record::{
    compute, within, COMPUTE, SHADER_BEAM, SHADER_CAST, SHADER_CELL, SHADER_FOLD, SHADER_FORGET,
    SHADER_HIT_FACES, SHADER_HIT_LEAD, SHADER_LIGHT, SHADER_LIGHT_BEAM, SHADER_LIGHT_DETAIL,
    SHADER_LIGHT_MARGIN, SHADER_LIGHT_MARK, SHADER_LIGHT_MARK_DRAWS, SHADER_LIGHT_MISS,
    SHADER_LIGHT_MISS_MARGIN, SHADER_LIGHT_PROPAGATE, SHADER_LIGHT_SAMPLE, SHADER_MISS,
    SHADER_MISS_LIST, SHADER_SAMPLE, UNIFORM,
};
use crate::gpu::api;
use crate::gpu::api::{
    buf_entry, buf_range_entry, buffer_usage, bytes, release, view_entry, word_bytes,
    write_buffer_bytes, Gpu, RenderContext,
};
use crate::gpu::generator::{cut_boxes, hold, Generation, Generator, IndirectDraw};
use crate::gpu::pipeline::{Pipeline, Pipelines};
use crate::gpu::read::Read;
use crate::gpu::renderer::{next_frame, Cast, Overwrites};
use crate::gpu::shadow;
use crate::gpu::span_spec::{FrameKind, GpuSpanName, LightMoved};
use pxl_spec_core::param::light::SAMPLE;

/// What the GPU's cast reads; it writes the world surface's rays.
pub(super) struct CastBuffers {
    pipeline: Pipeline,
    /// The beam rays (`beam_cs`, a thread a corner), where each cell's
    /// rays start off them (`cell_cs`, a thread a cell), the samples walked
    /// from those starts (`sample_cs`, a thread a sample): the eye's over
    /// the cast's group and the shadow map's over the light's (`light_groups`),
    /// each told by the head it binds (`eye_head`, `light_head`); and the
    /// eye's buffer (pxl-spec-core `beam`): a line a corner, a start a
    /// cell, a hit a sample, as the eye's words lay the frame's `grid`, none
    /// a pixel's; room for `beam_room` words.
    beam_built: Pipeline,
    cells_built: Pipeline,
    sample_built: Pipeline,
    beams: u32,
    /// The eye's head, all zero: a head naming no map (pxl-spec-core
    /// `light::HEAD_SIZE`), which tells a pass both take the eye's.
    eye_head: u32,
    grid: beam::Grid,
    beam_room: u32,
    /// The observed pyramid (pxl-spec-core `pyramid`): its buffer, no
    /// pixel's; the pixels the cast's reading walk missed walked again, a
    /// thread each of their list (`miss_list_cs`, pxl-spec-core
    /// `pyramid::MARKS_AT`), or where it was full a thread a drawn pixel
    /// (`miss_cs`), each told by the mark its own first ray word holds
    /// (pxl-spec-core `ray::MISS`), both dispatched off the words the
    /// reading walk counts them in (`marks`, pxl-spec-core
    /// `ray::MISS_ARGS_WORDS`); the pyramid forgotten (`forget_cs`) where
    /// what the world's query reads changed since the last cast: all of it
    /// (`forget`), or the nodes over the eye's clip levels' entries that moved
    /// (`moved`, a bit an entry, against the entries as `entries` last held them)
    /// and over the boxes of what is placed or cut that changed (`boxes`,
    /// `holding` of them, as decor's generation holds them, gpu/generator.rs
    /// `hold`), and those handed a slot of a child group of what is placed that
    /// was written (`written_groups`, `writes` of them); as this frame's cast does
    /// (`forgets`, pxl-spec-core `pyramid::FORGETS`), and what it sent, put
    /// back where its frame is dropped (`sent_forget`). `frame` numbers the casts since it was last
    /// forgotten whole, and `cuts` are the preview's cuts it was asked with.
    miss_built: Pipeline,
    miss_list_built: Pipeline,
    marks: u32,
    /// What lies about the faces the walks again stopped on, asked once a
    /// square of their hits (pxl-spec-core `beam::HIT_WORDS`): the one of
    /// each square that asks listed (`hit_lead_cs`), the faces' pass over
    /// their asks (ray/raster.rs `ask_hits`), and the faces put at their
    /// pixels (`hit_faces_cs`), each dispatched off the walks' count in the
    /// pyramid's head (`pyramid::HITS_ARGS`) or the leaders' among the
    /// walk again's words (`ray::MISS_ARGS_ROOM`); their groups, and the
    /// buffers they bind, made again exactly when one of them is another.
    hit_lead_built: Pipeline,
    hit_faces_built: Pipeline,
    hit_groups: [u32; 2],
    hit_bindings: [u32; 5],
    forget_built: Pipeline,
    pyramid: u32,
    frame: u32,
    forget: bool,
    moved: [u32; pyramid::MOVED_WORDS as usize],
    boxes: [[i32; 4]; overwrite::BOXES as usize],
    holding: usize,
    written_groups: [u32; pyramid::WRITTEN_CAP as usize],
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
    /// The slots that took no visible sample walked apart (`light_miss_cs`).
    light_miss_built: Pipeline,
    /// The tiles of the light's box its receivers stand on in a cast
    /// (pxl-spec-core `light::stands_at`): where the eye's rays meet the world
    /// (`light_mark_cs`), and where the decor and the actors drawn stand
    /// (`light_mark_draws_cs`); then those they read (`light_propagate_cs`).
    light_mark_built: Pipeline,
    light_mark_draws_built: Pipeline,
    light_propagate_built: Pipeline,
    /// The tiles the box slid onto, or whose eye moved past the margin of
    /// what their walks asked, set back to be read anew (`light_detail_cs`),
    /// before the propagation.
    light_detail_built: Pipeline,
    /// The light's beam rays and samples, the eye's passes in entries of
    /// their own, which keep the margin of what their walks ask
    /// (`light_beam_cs`, `light_sample_cs`); its misses' walk keeps its own.
    light_beam_built: Pipeline,
    light_sample_built: Pipeline,
    /// What each tile cast holds by: gathered off its beam rays and samples
    /// (`light_margin_cs`), lowered by its listed slots' walks
    /// (`light_miss_margin_cs`).
    light_margin_built: Pipeline,
    light_miss_margin_built: Pipeline,
    /// A word bound where decor is off, the decor's buffers the drawn mark
    /// reads none (`light_mark_draws_cs`).
    light_none: u32,
    /// Whether all the light's cast is made (`light_made`): its list and its
    /// raster's buffers and targets; before then no shadow map is cast.
    light_ready: bool,
    /// The levels' beam lines, samples and misses, one list they share
    /// (pxl-spec-core `light::list_words`), each level cast whole in it
    /// before the next, then every level's state (`shadow::state_at`): one
    /// buffer, made where shadows are first cast (`light_made`), none
    /// before, which every level's passes bind whole.
    light_list: u32,
    /// The last cast's number, of any level (pxl-spec-core
    /// `light::HEAD_CAST`): each level's cast takes the next, so a word of
    /// the list another level's cast left names a number below it.
    light_cast: u32,
    /// The reads of a level whose box slid, as its cast before left them,
    /// copied out to be copied back moved with it, the levels one after
    /// another: the largest level's room.
    light_reads_was: u32,
    /// Every level's stands (pxl-spec-core `light::stands`), one buffer a
    /// level's at `shadow::stands_at`: the marks bind them all, a level's
    /// propagation its own.
    light_stands: u32,
    /// What every level is handed beside the eye's words (pxl-spec-core
    /// `light::HEAD_WORDS`), one buffer a level's head at
    /// `light::HEAD_STRIDE` apart: each level's passes bind its own, the
    /// marks all (pxl-shader light.rs `Heads`).
    light_heads: u32,
    /// Which tiles the receivers read, as it last read it: the eye's look,
    /// the actors' places, decor placed again. One moving alone casts only
    /// the tiles read anew.
    light_receivers: Read,
    /// The levels as their casts hold them (`LightLevel`), and how many the
    /// map has this frame.
    light_levels: Vec<LightLevel>,
    light_level_cnt: u32,
    /// Whether the marks run this frame (`light_mark_cs`), every level's
    /// receivers at once: where a level that casts owes them.
    light_marks: bool,
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
    /// What the last cast read (`Read`) but the jitter, and whether this
    /// frame's is the same: the samples and the decor generated over them
    /// stand, and only the pixels are cast again where the jitter moved.
    read: Read,
    /// Where each pixel's ray went through it last (pxl-spec-core
    /// `eye::JITTER`). It moving alone, the beam rays, the cells' starts and
    /// the samples stand, and the pixels are cast again off them.
    jitter: [u32; 2],
    /// Whether the samples' raster is drawn through its gate, in the claimed
    /// tiles alone: where only the kernel's bounds moved.
    gated: bool,
    /// The samples' hits handed to the rasterizer (ray/raster.rs), whose
    /// visible sample at each pixel the pixels read (`cast_cs`), over a group
    /// of their own (`px_group`): the eye, the rays, the beam's words, the
    /// pyramid, the visibility buffer and the claimed tiles (`px_bound`),
    /// made again exactly when one of them is another.
    raster: Raster,
    px_group: u32,
    px_bound: [u32; 7],
    /// What the light's cast and the eye's recast, counted where the setting
    /// asks (ray/tally.rs).
    tally: Tally,
}

/// A level of the light's map as its cast holds it (pxl-spec-core
/// `light::LEVELS`, pxl-render gpu/shadow.rs `Level`): each cast by the
/// same passes over its own head and state, in the list the levels share,
/// deciding apart whether to cast.
struct LightLevel {
    /// Its passes' groups (`light_groups`), and the buffers they bind:
    /// made again exactly when one of them is another.
    groups: [u32; 12],
    bound: [u32; 12],
    /// The list, the light's visibility buffer and boxes the groups were
    /// made over, made again where one is another.
    grouped: [u32; 3],
    /// Its last cast's number (`CastBuffers::light_cast`), the number its
    /// marks are made with (pxl-spec-core `light::HEAD_CAST`), never zero,
    /// which no tile holds marked.
    number: u32,
    /// The number of the cast since which only the eye's look moved
    /// (pxl-spec-core `light::HEAD_EPOCH`): a tile cast in it or after
    /// stands.
    epoch: u32,
    /// Its misses' walk's dispatch and count, as the eye's are
    /// (pxl-spec-core `ray::MISS_ARGS_WORDS`).
    misses: u32,
    /// The workgroups of the passes that cast its tiles (pxl-spec-core
    /// `light::ARGS_WORDS`): none, or each whole where a tile is claimed.
    args: u32,
    /// What a tile holds follows, as its cast last read it (`Read`): its
    /// light frame (`Level::generation`), whether shadows are cast, the
    /// eye's zooms, and whether the world under it moved; one moving begins
    /// an epoch (`epoch`).
    content: Read,
    /// The eye's place and its box, which follows it, as it last read
    /// them: the zooms the walks ask follow the one, and what each tile of
    /// the box holds moves with the other's corner. One moving alone moves
    /// the reads with the box and casts the tiles the box slid onto or whose
    /// eye moved past the margin of what their walks asked
    /// (`light_detail_cs`, pxl-spec-core `light::holds`), and those read
    /// anew; whether this frame's cast asks which first, and the corner the
    /// cast before was cast under (`light::HEAD_CORNER_WAS`).
    detail: Read,
    detail_moved: bool,
    corner: [i32; 2],
    corner_was: [i32; 2],
    cast: bool,
    /// Why it cast this frame, a bit each in LIGHT_MOVED's order
    /// (span.json); 0 where it did not.
    moved: u32,
    /// Whether its next cast marks where the eye's rays met the world
    /// (`light_mark_cs`): held until one does; and whether the marks of the
    /// frame the eye stands in, its pixels' footprints, are owed, as the
    /// marks while it moved took none (`light::HEAD_FOOTPRINT`).
    world: bool,
    footprint_owed: bool,
}

impl LightLevel {
    fn new(g: &Gpu) -> LightLevel {
        let storage = buffer_usage::STORAGE | buffer_usage::COPY_DST;
        LightLevel {
            groups: [0; 12],
            bound: [0; 12],
            grouped: [0; 3],
            number: 0,
            epoch: 0,
            misses: g.buf(
                word_bytes(ray::MISS_ARGS_WORDS),
                storage | buffer_usage::INDIRECT | buffer_usage::COPY_SRC,
            ),
            args: g.buf(
                word_bytes(light::ARGS_WORDS as usize),
                storage | buffer_usage::INDIRECT,
            ),
            content: Read::default(),
            detail: Read::default(),
            detail_moved: false,
            corner: [0; 2],
            corner_was: [0; 2],
            cast: false,
            moved: 0,
            world: false,
            footprint_owed: false,
        }
    }
}

/// A level of the light's map as the frame hands it to the cast
/// (gpu/shadow.rs `Level`): its layer of the world, its light frame's number,
/// where its rays start and its words.
pub struct CastLevel {
    pub world: u32,
    pub generation: u32,
    pub starts: light::Starts,
    pub words: [u32; light::WORDS],
}

/// Level `l`'s head among the levels' one buffer, in bytes: its first and
/// how many (`light::HEAD_STRIDE`).
fn head_range(l: u32) -> (u32, u32) {
    (
        word_bytes(4 * l as usize * light::HEAD_STRIDE),
        word_bytes(light::HEAD_WORDS),
    )
}

impl CastBuffers {
    /// The level whose casting the tally counts for the light: its first
    /// that casts this frame.
    fn counted_level(&self) -> Option<usize> {
        self.light_levels.iter().position(|v| v.cast)
    }
}

/// What a cast's head handed the pyramid's forgetting (pxl-spec-core
/// `pyramid::HEAD_WORDS`): whether all of it, the entries that moved, the
/// boxes and the child groups written; put back where its frame is dropped.
struct ForgetHead {
    all: bool,
    moved: [u32; pyramid::MOVED_WORDS as usize],
    boxes: [[i32; 4]; overwrite::BOXES as usize],
    holding: usize,
    written_groups: [u32; pyramid::WRITTEN_CAP as usize],
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

/// The binding of a pass's head (pxl-shader light.rs `Head`): a level's of
/// the light's map, or the eye's, which names none.
pub(super) const HEAD_BINDING: u32 = 11;

/// The binding of every level's head, which the marks read (pxl-shader
/// light.rs `Heads`).
const HEADS_BINDING: u32 = 22;

/// The binding of the stands (pxl-spec-core `light::stands`): every level's
/// for the marks, a level's own for its propagation.
const STANDS_BINDING: u32 = 23;

/// The cast's world's group, every kernel's over it bar the light's own and
/// the pixels': the one the missed pixels' cast binds, and the head the
/// passes both observers take read their observer by. The shadow map's
/// passes of those bind it over the light's buffers (`light_groups`); the
/// light's own bind its world and its clipmap (`LIGHT_GROUP`), past the floor's
/// storage buffers were they one group; the pixels' (`cast_cs`) read the
/// visibility buffer and no world (`px_group`).
const WORLD_GROUP: [u32; compute::MISS_GROUP.len() + 3] = {
    let mut g = [0; compute::MISS_GROUP.len() + 3];
    let mut k = 0;
    while k < compute::MISS_GROUP.len() {
        g[k] = compute::MISS_GROUP[k];
        k += 1;
    }
    g[k] = HEAD_BINDING;
    g[k + 1] = COMPUTE;
    g[k + 2] = UNIFORM;
    g
};

const _: () = assert!(
    within(compute::FOLD_GROUP, WORLD_GROUP)
        && within(compute::BEAM_GROUP, WORLD_GROUP)
        && within(compute::CELL_GROUP, WORLD_GROUP)
        && within(compute::SAMPLE_GROUP, WORLD_GROUP)
        && within(compute::FORGET_GROUP, WORLD_GROUP)
        && within(compute::MISS_LIST_GROUP, WORLD_GROUP),
    "the fold, the beam, the cells, the samples, the forgetting and the list's walk bind the cast's world's group"
);

impl RaysPass {
    /// The cast pass's pipelines and buffers, made once, where the frame
    /// first casts: before any of its uploads (`hand`), which take the
    /// stream only while `stream::RESERVE` is left after them, so the
    /// shaders made here are never left without room.
    pub fn make(&mut self, g: &Gpu, pipelines: &mut Pipelines) {
        if self.cast.is_some() {
            return;
        }
        // One group for every kernel over the cast's world but the pixels'.
        let group = &WORLD_GROUP;
        let pipeline = pipelines.build_compute(g, SHADER_CAST, &compute::CAST_GROUP);
        let beam_built = pipelines.build_compute(g, SHADER_BEAM, group);
        let cells_built = pipelines.build_compute(g, SHADER_CELL, group);
        let sample_built = pipelines.build_compute(g, SHADER_SAMPLE, group);
        let fold_built = pipelines.build_compute(g, SHADER_FOLD, group);
        let miss_built = pipelines.build_compute(g, SHADER_MISS, group);
        let miss_list_built = pipelines.build_compute(g, SHADER_MISS_LIST, group);
        let hit_lead_built = pipelines.build_compute(g, SHADER_HIT_LEAD, &compute::HIT_LEAD_GROUP);
        let hit_faces_built =
            pipelines.build_compute(g, SHADER_HIT_FACES, &compute::HIT_FACES_GROUP);
        let forget_built = pipelines.build_compute(g, SHADER_FORGET, group);
        let light_built = pipelines.build_compute(g, SHADER_LIGHT, &compute::LIGHT_GROUP);
        let light_miss_built =
            pipelines.build_compute(g, SHADER_LIGHT_MISS, &compute::LIGHT_MISS_GROUP);
        let light_mark_built =
            pipelines.build_compute(g, SHADER_LIGHT_MARK, &compute::LIGHT_MARK_GROUP);
        let light_mark_draws_built =
            pipelines.build_compute(g, SHADER_LIGHT_MARK_DRAWS, &compute::LIGHT_MARK_DRAWS_GROUP);
        let light_propagate_built =
            pipelines.build_compute(g, SHADER_LIGHT_PROPAGATE, &compute::LIGHT_PROPAGATE_GROUP);
        let light_detail_built =
            pipelines.build_compute(g, SHADER_LIGHT_DETAIL, &compute::LIGHT_DETAIL_GROUP);
        let light_beam_built =
            pipelines.build_compute(g, SHADER_LIGHT_BEAM, &compute::LIGHT_BEAM_GROUP);
        let light_sample_built =
            pipelines.build_compute(g, SHADER_LIGHT_SAMPLE, &compute::LIGHT_SAMPLE_GROUP);
        let light_margin_built =
            pipelines.build_compute(g, SHADER_LIGHT_MARGIN, &compute::LIGHT_MARGIN_GROUP);
        let light_miss_margin_built = pipelines.build_compute(
            g,
            SHADER_LIGHT_MISS_MARGIN,
            &compute::LIGHT_MISS_MARGIN_GROUP,
        );
        let raster = Raster::new(g, pipelines);
        let tally = Tally::new(g);
        let buf = |size: u32, usage: u32| g.buf(size, usage);
        let storage = buffer_usage::STORAGE | buffer_usage::COPY_DST;
        let c = CastBuffers {
            pipeline,
            beam_built,
            cells_built,
            sample_built,
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
            pyramid: buf(
                word_bytes(pyramid::WORDS as usize),
                storage | buffer_usage::COPY_SRC | buffer_usage::INDIRECT,
            ),
            frame: 0,
            forget: true,
            moved: [0; pyramid::MOVED_WORDS as usize],
            boxes: [[0; 4]; overwrite::BOXES as usize],
            holding: 0,
            written_groups: [0; pyramid::WRITTEN_CAP as usize],
            writes: 0,
            sent_forget: None,
            entries: Vec::new(),
            forgets: pyramid::FORGETS_NONE,
            cuts: [0; eye::CUTS],
            fold_built,
            light_built,
            light_miss_built,
            light_mark_built,
            light_mark_draws_built,
            light_propagate_built,
            light_detail_built,
            light_beam_built,
            light_sample_built,
            light_margin_built,
            light_miss_margin_built,
            light_none: buf(word_bytes(2 * generation::drawn::REGIONS as usize), storage),
            light_ready: false,
            light_list: 0,
            light_cast: 0,
            light_reads_was: {
                let n = (0..light::LEVELS)
                    .map(|l| light::tiles(shadow::size(l)))
                    .max();
                let n = n.unwrap_or(0) as usize;
                buf(
                    word_bytes(n * n),
                    buffer_usage::COPY_SRC | buffer_usage::COPY_DST,
                )
            },
            light_stands: buf(
                word_bytes(shadow::stands_at(light::LEVELS) as usize),
                storage,
            ),
            light_heads: buf(
                word_bytes(4 * light::HEAD_STRIDE * light::LEVELS as usize),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
            light_receivers: Read::default(),
            light_levels: (0..light::LEVELS).map(|_| LightLevel::new(g)).collect(),
            light_level_cnt: 0,
            light_marks: false,
            refold: false,
            group: 0,
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
            tally,
        };
        self.cast = Some(c);
    }

    /// The rays for this frame into the world surface's (`rays`, its view,
    /// and its drawn pixels), and the world's depth from the light into `light`'s
    /// view: the GPU's cast where an eye casts, its eye handed over first;
    /// its samples' raster through its gate where the frame is `gated`.
    pub fn prepare(
        &mut self,
        g: &Gpu,
        cast: Option<Cast>,
        meshes: &[u32; GROUPS as usize],
        rays: (u32, [u32; 2]),
        light_map: &[CastLevel],
        gated: bool,
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
                for v in &mut c.light_levels {
                    v.content.forget();
                    (v.cast, v.moved) = (false, 0);
                }
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
        // The jitter apart (below).
        let eye_moved = r.read(&eye[..eye::JITTER]);
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
        self.kind = match (stands, jittered) {
            (true, false) => FrameKind::standing,
            (true, true) => FrameKind::px,
            (false, _) => FrameKind::cast,
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
        // The grid as the eye's words lay it (pxl-spec-core `eye::GRID`), the
        // dispatches' sizes, and room for its corners' lines, its cells'
        // starts and its samples' hits, made again only where they outgrow
        // it.
        let tan = [
            f32::from_bits(cast.eye[eye::TAN]),
            f32::from_bits(cast.eye[eye::TAN + 1]),
        ];
        c.grid = beam::Grid::of_words(
            tan,
            cast.eye[eye::GRID..eye::GRID + eye::GRID_WORDS]
                .try_into()
                .or_bug(Errno::ENOTRECOVERABLE),
        );
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
            // Read where the kernel holds it, one record however large.
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
            // whole, read where they live, two records however large.
            fn child_group(lane: &[u32], first: u32) -> &[u32] {
                &lane[first as usize..(first + overwrite::CHILDREN) as usize]
            }
            // The child groups written since the revision the pyramid was asked
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
        for &b in edits {
            hold(&mut c.boxes, &mut c.holding, b);
        }
        cut_boxes(&c.cuts, &cuts, |b| hold(&mut c.boxes, &mut c.holding, b));
        // What the observed pyramid kept is forgotten where the service area
        // or what is placed moved past the boxes; so it is once its frames
        // would outrun a key's tag.
        c.forget |= shards_moved
            || shards_grow
            || overwrites_grow
            || (overwrites_moved && edits.is_empty())
            || c.frame >= pyramid::FRAMES;
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
        // about the eye (pxl-spec-core `clipmap`), and are read off the
        // coarser one alike.
        let mut present_moved = false;
        if clipmap_moved {
            let rows = &cast.clipmap[..4 * eye_rows];
            for k in 0..eye_rows {
                let row = &rows[4 * k..4 * k + 4];
                let was = c.entries.get(4 * k..4 * k + 4);
                if was != Some(row) {
                    c.moved[k / 32] |= 1 << (k % 32);
                }
                let held = |r: &[i32]| clipmap::Entry::of([r[0], r[1], r[2], r[3]]).brick();
                let (now, was) = (held(row), was.map(held));
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
                let w = cell.wrapping_sub(clipmap::FIRST_CELL);
                let k = (w * clipmap::ENTRIES + clipmap::entry(q)) as usize;
                let row = &cast.clipmap[4 * k..4 * k + 4];
                if w < clipmap::EYE_CLIP_LEVELS && [row[0], row[1]] == q {
                    c.moved[k / 32] |= 1 << (k % 32);
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
        // Each level of the shadow map is cast, every slot a receiver reads,
        // as the eye's rays are (pxl-spec-core `light`), where what it reads
        // moved: its light frame or box, the eye's place or zooms, which the
        // zooms its rays ask follow, or the world under it; and where what
        // says which tiles are read moved, the look, the actors' places and
        // decor placed again (`light_mark_cs`, `light_mark_draws_cs`). A
        // standing frame casts none. Where only the latter moved, a tile
        // cast since anything else last did stands, and only the tiles read
        // anew are cast (`light::HEAD_EPOCH`). None where no shadow is cast,
        // nor before all its cast is made (`light_made`), its read let go
        // until then. The levels past those the map has this frame cast
        // none.
        let shadows = !light_map.is_empty();
        // The look, where the actors stand and decor compacted again: which
        // tiles are read, not what a tile holds, every level's alike. Not the
        // jitter: a pixel's marks hold its footprint (`light_mark_cs`).
        let l = &mut c.light_receivers;
        l.begin();
        l.read(&cast.eye[eye::FORWARD..eye::TAN + 2]);
        l.read(cast.actors);
        l.mark(c.generator.counts());
        let receivers_held = l.end();
        // The footprints taken where the eye stands; while it moves, the
        // next frame's rays mark again, and the frame it stands in owes
        // them.
        let footprint = kind != FrameKind::cast;
        c.light_level_cnt = light_map.len() as u32;
        let ready = c.light_ready;
        let mut number = c.light_cast;
        for (k, v) in c.light_levels.iter_mut().enumerate() {
            let Some(m) = light_map.get(k) else {
                v.content.forget();
                (v.cast, v.moved) = (false, 0);
                continue;
            };
            // All it reads but the look.
            let r = &mut v.content;
            r.begin();
            r.read(&[m.generation, u32::from(shadows)]);
            r.read(&cast.eye[eye::RANGE..eye::RANGE + 2]);
            r.mark(world_moved);
            let held = r.end();
            // The eye's place, which the zooms the walks ask follow, and the
            // box, off the eye: a tile cast in the epoch holds while the one
            // moved less than the margin of what its walks asked and the
            // other slid off none of its slots (`light_detail_cs`).
            let d = &mut v.detail;
            d.begin();
            d.read(&cast.eye[eye::ORIGIN..eye::STEPS]);
            d.read(&m.words);
            let detail_held = d.end();
            // The first frame the eye stands in after it moved marks again,
            // with the footprints the jitter's frames after it stand on.
            let footprint_owed = v.footprint_owed && kind != FrameKind::cast;
            let moved = LightMoved::ALL
                .into_iter()
                .filter(|m| match m {
                    LightMoved::content => !held,
                    LightMoved::detail => !detail_held,
                    LightMoved::receivers => !receivers_held,
                    LightMoved::footprint => footprint_owed,
                })
                .fold(0, |bits, m| bits | 1 << m as u32);
            v.detail_moved = held && !detail_held;
            if !ready {
                v.content.forget();
            }
            v.cast = moved != 0 && ready;
            v.moved = if v.cast { moved } else { 0 };
            // The world's receivers stand where the eye's rays met it: marked
            // again by the next cast after the rays were cast anew, or where
            // an epoch begins. Else what they stood on stands, marked in the
            // epoch already, so none of it is the cast's to cast anew
            // (`light_propagate_cs`); the jitter's casts move a ray within its
            // footprint, marked already where the eye stood when they were.
            v.world |= kind == FrameKind::cast || !held || footprint_owed;
            if v.cast {
                number = number.wrapping_add(1).max(1);
                v.number = number;
                if !held {
                    v.epoch = v.number;
                }
                let corner = light::CORNER.map(|k| m.words[k] as i32);
                v.corner_was = core::mem::replace(&mut v.corner, corner);
                if v.world {
                    v.footprint_owed = !footprint;
                }
            }
        }
        c.light_cast = number;
        let casting = c.light_levels.iter().any(|v| v.cast);
        // The marks run, every level's receivers at once, where a level that
        // casts owes them; every level the map has its head written then, as
        // the marks read each, a level that casts none marking by a number
        // its next cast passes by.
        c.light_marks = casting && c.light_levels.iter().any(|v| v.cast && v.world);
        if casting {
            for (k, (v, m)) in c.light_levels.iter_mut().zip(light_map).enumerate() {
                // The head (pxl-spec-core `light::HEAD_WORDS`).
                let mut head = [0u32; light::HEAD_WORDS];
                head[light::HEAD_CAST] = v.number;
                head[light::HEAD_EPOCH] = v.epoch;
                head[light::HEAD_COUNTED] = u32::from(ray_counts);
                head[light::HEAD_SIZE] = shadow::size(k as u32);
                head[light::HEAD_CORNER_WAS] = v.corner_was[0] as u32;
                head[light::HEAD_CORNER_WAS + 1] = v.corner_was[1] as u32;
                head[light::HEAD_FOOTPRINT] = u32::from(footprint);
                head[light::HEAD_LEVEL] = k as u32;
                let at = 4 * light::HEAD_LIGHT;
                head[at..at + light::WORDS].copy_from_slice(&m.words);
                let at = 4 * light::STARTS_AT;
                head[at..at + 4 * light::STARTS_VEC4S].copy_from_slice(&m.starts.words());
                // The actors drawn, receivers as decor is
                // (`light_mark_draws_cs`), and the levels the map has.
                let at = 4 * light::ACTORS_AT;
                let n = cast.actors.len().min(light::ACTORS);
                head[at] = n as u32;
                head[light::HEAD_LEVELS] = light_map.len() as u32;
                head[light::HEAD_BASE] = shadow::stands_at(k as u32);
                head[light::HEAD_STATE] = shadow::state_at(k as u32);
                for (j, a) in cast.actors[..n].iter().enumerate() {
                    let w = at + 4 * (1 + j);
                    head[w..w + 3].copy_from_slice(&a.map(f32::to_bits));
                }
                g.write_buf(c.light_heads, head_range(k as u32).0, bytes(&head));
                if v.cast {
                    g.write_buf(v.misses, 0, bytes(&ray::MISS_ARGS_EMPTY));
                    let n = shadow::size(k as u32);
                    g.write_buf(v.args, 0, bytes(&light_args(n)));
                    c.raster.light_prepare(g, k as u32, &m.words, n);
                }
            }
        }
        if eye_moved || jittered {
            g.write_buf(c.eye, 0, bytes(&cast.eye));
        }
        // The samples' raster through its gate where only the kernel's
        // bounds moved; the pixels' cast passes by the tiles not claimed
        // (`px_group`).
        c.gated = gated && kind == FrameKind::px;
        if casts {
            c.forgets = if c.forget {
                pyramid::FORGETS_ALL
            } else if c.moved.iter().any(|&m| m != 0) || c.holding != 0 || c.writes != 0 {
                pyramid::FORGETS_MOVED
            } else {
                pyramid::FORGETS_NONE
            };
            // The head before the frame's dispatches: what it forgets and over
            // which entries; forgotten whole, its first frame, else the next.
            let mut head = [0u32; pyramid::HEAD_WORDS as usize];
            c.frame = if c.forgets == pyramid::FORGETS_ALL {
                1
            } else {
                c.frame + 1
            };
            head[pyramid::FRAME as usize] = c.frame;
            head[pyramid::FORGETS as usize] = c.forgets;
            let boxes = pyramid::BOXES_AT as usize;
            head[pyramid::MOVED_AT as usize..boxes].copy_from_slice(&c.moved);
            head[boxes] = c.holding as u32;
            for (k, b) in c.boxes[..c.holding].iter().enumerate() {
                let at = boxes + 1 + 4 * k;
                head[at..at + 4].copy_from_slice(&b.map(|v| v as u32));
            }
            let written = pyramid::WRITTEN_AT as usize;
            head[written] = c.writes as u32;
            head[written + 1..written + 1 + c.writes]
                .copy_from_slice(&c.written_groups[..c.writes]);
            // The hits' dispatch none, a workgroup deep and high.
            head[pyramid::HITS_ARGS as usize + 1] = 1;
            head[pyramid::HITS_ARGS as usize + 2] = 1;
            g.write_buf(c.pyramid, 0, bytes(&head));
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
                all: c.forgets == pyramid::FORGETS_ALL,
                moved: c.moved,
                boxes: c.boxes,
                holding: c.holding,
                written_groups: c.written_groups,
                writes: c.writes,
            });
            (c.forget, c.moved) = (false, [0; pyramid::MOVED_WORDS as usize]);
            (c.holding, c.writes) = (0, 0);
        } else {
            c.sent_forget = None;
        }
        let frame = tally::Frame {
            number: next_frame(),
            // A full cast's alone, so what a cast asks is not shared out
            // over the frames that cast only the pixels; the light's, its
            // first level that casts (`counted_level`).
            casts: [self.kind == FrameKind::cast, c.counted_level().is_some()],
            rays: [c.size[0] * c.size[1], {
                let n = shadow::size(c.counted_level().unwrap_or(0) as u32);
                n * n
            }],
        };
        c.tally.begin(g, ray_counts, c.pyramid, frame);
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
            c.pyramid,
            c.eye_head,
        ];
        if bindings != c.bindings {
            release(c.group);
            c.group = cast_group(g, &c.miss_built, bindings);
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
            let hit_bound = [c.eye, rays.0, c.beams, c.pyramid, c.marks];
            if hit_bound != c.hit_bindings {
                for group in c.hit_groups {
                    release(group);
                }
                c.hit_groups = hit_groups(g, [&c.hit_lead_built, &c.hit_faces_built], hit_bound);
                c.hit_bindings = hit_bound;
            }
            let px_bound = [
                c.eye,
                rays.0,
                c.beams,
                c.pyramid,
                c.raster.visibility_array,
                self.claimed.0,
                self.claimed.1,
            ];
            if px_bound != c.px_bound {
                release(c.px_group);
                c.px_group = px_group(g, &c.pipeline, px_bound, c.marks);
                c.px_bound = px_bound;
            }
        }
        let [drawing, regions, instances] = c
            .generator
            .instances()
            .map_or([c.light_none; 3], |p| [p.drawing, p.regions, p.instances]);
        for (k, m) in light_map.iter().enumerate().filter(|_| c.light_ready) {
            let l = k as u32;
            let grouped = [
                c.light_list,
                c.raster.light_visibility(l),
                c.raster.light.boxes,
            ];
            let v = &mut c.light_levels[k];
            let bound = [
                c.eye,
                c.clipmap,
                ground,
                palette_indices,
                c.shards,
                c.overwrites,
                pool,
                m.world,
                rays.0,
                drawing,
                regions,
                instances,
            ];
            if bound != v.bound || grouped != v.grouped {
                for group in v.groups {
                    release(group);
                }
                v.groups = light_groups(
                    g,
                    [
                        &c.light_built,
                        &c.miss_built,
                        &c.raster.boxes_built,
                        &c.light_miss_built,
                        &c.light_mark_built,
                        &c.light_mark_draws_built,
                        &c.light_propagate_built,
                        &c.light_detail_built,
                        &c.light_beam_built,
                        &c.light_sample_built,
                        &c.light_margin_built,
                        &c.light_miss_margin_built,
                    ],
                    bound,
                    LightBuffers {
                        level: l,
                        pyramid: c.pyramid,
                        heads: c.light_heads,
                        list: c.light_list,
                        stands: c.light_stands,
                        boxes: c.raster.light.boxes,
                        visibility: c.raster.light_visibility(l),
                        args: c.raster.light.levels.get(k).map_or(0, |r| r.args),
                        misses: v.misses,
                        light_args: v.args,
                    },
                );
                (v.bound, v.grouped) = (bound, grouped);
            }
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
                (blocks * clipmap::EYE_CLIP_LEVELS).div_ceil(compute::FOLD_WORKGROUP[0]),
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
    /// (`PER_PX`).
    pub fn dispatch(&mut self, ctx: &mut RenderContext) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let (generates, casts, kind) = (self.generates(), self.casts_px(), self.kind);
        let claimed = (self.claimed, self.tiles.1);
        let Some(c) = self.cast.as_mut() else { return };
        if casts {
            // The observed pyramid forgotten first where it is; then the
            // beam rays writing it, a corner each; each cell's start off them;
            // the samples writing it, each from its cell's start; their
            // cells drawn as boxes, each pixel's nearest kept; the pixels
            // off that, or reading it from their cell's start, marking the
            // pixels that missed; and those again,
            // writing it, a thread a drawn pixel, each but the marked
            // returning at once. One pass, a dispatch each, each
            // reading what the ones before wrote (pxl-spec-core `pyramid`).
            // A pass a dispatch, each timed in its own span, so the cast's
            // parts are read apart: `cast` holds the forgetting alone; the
            // samples' boxes drawn between the samples and the pixels, which
            // take the visible sample (ray/raster.rs, span `sraster`). Where
            // only the jitter moved (`FrameKind::px`), the beam rays, the
            // cells' starts and the samples stand, and the rest is cast again
            // off them.
            let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::cast).pass());
            g.set_bind_group(pass, 0, c.group, &[]);
            if c.forgets != pyramid::FORGETS_NONE {
                g.set_pipeline(pass, c.forget_built.id);
                g.dispatch(
                    pass,
                    pyramid::ENTRIES.div_ceil(compute::FORGET_WORKGROUP[0]),
                    1,
                    1,
                );
            }
            g.end(pass);
            for (span, pipeline, [x, y], w) in [
                (
                    GpuSpanName::beam,
                    &c.beam_built,
                    c.grid.corners,
                    compute::BEAM_WORKGROUP,
                ),
                (
                    GpuSpanName::cell,
                    &c.cells_built,
                    c.grid.span,
                    compute::CELL_WORKGROUP,
                ),
                (
                    GpuSpanName::sample,
                    &c.sample_built,
                    c.grid.samples,
                    compute::SAMPLE_WORKGROUP,
                ),
                (
                    GpuSpanName::px,
                    &c.pipeline,
                    c.size,
                    compute::CAST_WORKGROUP,
                ),
            ] {
                if kind == FrameKind::px && span != GpuSpanName::px {
                    continue;
                }
                let mut group = c.group;
                if span == GpuSpanName::px {
                    let gated = c.gated.then_some(claimed);
                    c.raster.draw(ctx, &c.grid, kind == FrameKind::cast, gated);
                    group = c.px_group;
                }
                let pass = g.compute_pass(encoder, ctx.stamp(span).pass());
                g.set_bind_group(pass, 0, group, &[]);
                g.set_pipeline(pass, pipeline.id);
                g.dispatch(pass, x.div_ceil(w[0]), y.div_ceil(w[1]), 1);
                g.end(pass);
            }
            // The marked pixels' count where their walk reads it, then that
            // walk over their list, and over every pixel where it was full.
            g.copy_buf(
                encoder,
                (c.marks, word_bytes(ray::MISS_ARGS_CNT as usize)),
                (c.pyramid, word_bytes(pyramid::MARKS_CNT as usize)),
                4,
            );
            let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::miss).pass());
            g.set_bind_group(pass, 0, c.group, &[]);
            g.set_pipeline(pass, c.miss_list_built.id);
            g.dispatch_indirect(pass, c.marks, word_bytes(ray::MISS_ARGS_LIST as usize));
            g.set_pipeline(pass, c.miss_built.id);
            g.dispatch_indirect(pass, c.marks, word_bytes(ray::MISS_ARGS_WHOLE as usize));
            // What lies about the faces they stopped on, once a square of
            // hits: the one of each square that asks listed, the faces' pass
            // over their asks, and the faces put at their pixels.
            let hits = word_bytes(pyramid::HITS_ARGS as usize);
            g.set_pipeline(pass, c.hit_lead_built.id);
            g.set_bind_group(pass, 0, c.hit_groups[0], &[]);
            g.dispatch_indirect(pass, c.pyramid, hits);
            c.raster.ask_hits(g, pass, c.marks);
            g.set_pipeline(pass, c.hit_faces_built.id);
            g.set_bind_group(pass, 0, c.hit_groups[1], &[]);
            g.dispatch_indirect(pass, c.pyramid, hits);
            g.end(pass);
        }
        // The decor generated over the rays, before the light's cast, which
        // marks where it stands (`light_mark_draws_cs`).
        if generates {
            let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::generate).pass());
            c.generator.dispatch(g, pass);
            g.end(pass);
        }
        if c.light_levels.iter().any(|v| v.cast) {
            // Each level's reads moved with its box, a tile of it at a time:
            // each tile's word the word of the tile that held its world
            // texels under the corner before, copied out and back in, which
            // the detail's pass then sets to none where the box slid onto it
            // (`light_detail_cs`).
            for (k, v) in c.light_levels.iter().enumerate() {
                if !(v.cast && v.detail_moved && v.corner != v.corner_was) {
                    continue;
                }
                let size = shadow::size(k as u32);
                let n = light::tiles(size) as i32;
                let t = light::TILE as i32;
                let dx = (v.corner[0] - v.corner_was[0]) / t;
                let dy = (v.corner[1] - v.corner_was[1]) / t;
                let shift = dy * n + dx;
                let all = n * n;
                if shift.abs() < all {
                    let list = c.light_list;
                    let reads = (shadow::state_at(k as u32) + light::reads_at(size)) as i32;
                    let words = |w: i32| word_bytes(w as usize);
                    let out = (c.light_reads_was, 0);
                    g.copy_buf(encoder, (list, words(reads)), out, words(all));
                    let (from, to) = if shift >= 0 { (shift, 0) } else { (0, -shift) };
                    g.copy_buf(
                        encoder,
                        (c.light_reads_was, words(from)),
                        (list, words(reads + to)),
                        words(all - shift.abs()),
                    );
                }
            }
            let stamp = ctx.stamp(GpuSpanName::light_beam).pass();
            let pass = g.compute_pass(encoder, stamp);
            // The tiles each box slid onto, and those whose eye moved past
            // the margin of what their walks asked, set back to be read anew
            // first, a thread a tile (`light_detail_cs`).
            for (k, v) in c.light_levels.iter().enumerate() {
                if !(v.cast && v.detail_moved) {
                    continue;
                }
                let n = light::tiles(shadow::size(k as u32));
                let w = compute::LIGHT_DETAIL_WORKGROUP;
                g.set_bind_group(pass, 0, v.groups[7], &[]);
                g.set_pipeline(pass, c.light_detail_built.id);
                g.dispatch(pass, n.div_ceil(w[0]), n.div_ceil(w[1]), 1);
            }
            // The tiles its receivers read marked, timed with the beam, every
            // level's at once, each receiver in the levels it is read at:
            // where the eye's rays meet the world, a thread a brick of pixels
            // `light::MARK` a side (`light_mark_cs`), and where the decor and
            // the actors drawn stand, a thread each (`light_mark_draws_cs`).
            if c.light_marks {
                for v in c.light_levels.iter_mut().filter(|v| v.cast) {
                    v.world = false;
                }
                let first = &c.light_levels[0];
                g.set_bind_group(pass, 0, first.groups[4], &[]);
                g.set_pipeline(pass, c.light_mark_built.id);
                let w = compute::LIGHT_MARK_WORKGROUP;
                let [x, y] = c.size;
                g.dispatch(pass, x.div_ceil(w[0]), y.div_ceil(w[1]), 1);
            }
            let first = &c.light_levels[0];
            g.set_bind_group(pass, 0, first.groups[5], &[]);
            g.set_pipeline(pass, c.light_mark_draws_built.id);
            let cap = c.generator.instances().map_or(0, |p| p.cap);
            let w = compute::LIGHT_MARK_DRAWS_WORKGROUP;
            g.dispatch(pass, (cap + light::ACTORS as u32).div_ceil(w[0]), 1, 1);
            g.end(pass);
            // Each level that casts, one after another, whole in the list
            // the levels share before the next takes it (pxl-spec-core
            // `light::list_words`): the lists of what feeds the tiles it
            // claims emptied (`light::listed_at`); the tiles it reads, a
            // thread a tile (`light_propagate_cs`); the beam, the samples,
            // their boxes drawn, the slots taking their visible sample, a
            // workgroup a tile (pxl-spec-core `light::TILE`), and the rest
            // walked off their list as the eye's missed pixels are. Each a
            // pass timed in its own span, which holds every level's.
            let args = |w: u32| word_bytes(w as usize);
            for (k, v) in c.light_levels.iter().enumerate().filter(|(_, v)| v.cast) {
                let n = shadow::size(k as u32);
                let tiles = light::tiles(n);
                for which in 0..light::LISTS {
                    let at = word_bytes(light::listed_at(n, which) as usize);
                    g.clear_buf(encoder, (c.light_list, at), word_bytes(1));
                }
                let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::light_beam).pass());
                g.set_bind_group(pass, 0, v.groups[6], &[]);
                g.set_pipeline(pass, c.light_propagate_built.id);
                let w = compute::LIGHT_PROPAGATE_WORKGROUP;
                g.dispatch(pass, tiles.div_ceil(w[0]), tiles.div_ceil(w[1]), 1);
                // The beam by its entry of its own, which tells the margin of
                // what its walks ask (`light_beam_cs`), then the cells' starts
                // by the eye's pass over the light's world's group, which
                // hands it the level's head. Each as many workgroups as the
                // propagation left it (`light::ARGS_WORDS`).
                g.set_bind_group(pass, 0, v.groups[8], &[]);
                g.set_pipeline(pass, c.light_beam_built.id);
                g.dispatch_indirect(pass, v.args, args(light::ARGS_BEAM));
                g.set_bind_group(pass, 0, v.groups[1], &[]);
                g.set_pipeline(pass, c.cells_built.id);
                g.dispatch_indirect(pass, v.args, args(light::ARGS_CELLS));
                g.end(pass);
                let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::light_sample).pass());
                g.set_bind_group(pass, 0, v.groups[9], &[]);
                g.set_pipeline(pass, c.light_sample_built.id);
                g.dispatch_indirect(pass, v.args, args(light::ARGS_SAMPLE));
                // What each tile cast holds by, gathered once off the beam
                // rays and the samples that feed it, a thread a tile
                // (`light_margin_cs`).
                g.set_bind_group(pass, 0, v.groups[10], &[]);
                g.set_pipeline(pass, c.light_margin_built.id);
                let w = compute::LIGHT_MARGIN_WORKGROUP;
                g.dispatch(pass, tiles.div_ceil(w[0]), tiles.div_ceil(w[1]), 1);
                g.end(pass);
                // Its boxes, drawn, and its slots, before the next level's
                // boxes take the raster's buffers and targets.
                let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::light_raster).pass());
                g.set_bind_group(pass, 0, v.groups[2], &[]);
                g.set_pipeline(pass, c.raster.boxes_built.id);
                g.dispatch_indirect(pass, v.args, args(light::ARGS_SAMPLE_BOX));
                g.end(pass);
                c.raster.light_draw(ctx, k as u32);
                let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::light_slot).pass());
                g.set_bind_group(pass, 0, v.groups[0], &[]);
                g.set_pipeline(pass, c.light_built.id);
                g.dispatch_indirect(pass, v.args, args(light::ARGS_SLOT));
                g.end(pass);
                // Their count where their walk reads it.
                g.copy_buf(
                    encoder,
                    (v.misses, word_bytes(ray::MISS_ARGS_CNT as usize)),
                    (c.light_list, word_bytes(light::misses_cnt(n) as usize)),
                    4,
                );
                let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::light_walk).pass());
                let list = word_bytes(ray::MISS_ARGS_LIST as usize);
                g.set_bind_group(pass, 0, v.groups[3], &[]);
                g.set_pipeline(pass, c.light_miss_built.id);
                g.dispatch_indirect(pass, v.misses, list);
                // Each walk's margin lowered into its tile's held words, over
                // the same list (`light_miss_margin_cs`).
                g.set_bind_group(pass, 0, v.groups[11], &[]);
                g.set_pipeline(pass, c.light_miss_margin_built.id);
                g.dispatch_indirect(pass, v.misses, list);
                g.end(pass);
            }
        }
        // The light's counted off its first level that casts
        // (`counted_level`).
        let counted_level = c.counted_level().unwrap_or(0);
        let counted = Counts {
            pyramid: c.pyramid,
            args: [c.marks, c.light_levels[counted_level].misses],
            boxes: [
                c.raster.args,
                c.raster
                    .light
                    .levels
                    .get(counted_level)
                    .map_or(0, |r| r.args),
            ],
        };
        c.tally.copy(g, encoder, &counted);
    }

    /// The light's cast made where `wanted`, shadows first cast, before the
    /// frame: its list (`shadow::LIST_WORDS`) and its raster's buffers
    /// and targets (`Raster::light_made`), what only a map cast needs. Its
    /// passes are made with the eye's (`make`), the beam's, the cells' and
    /// the samples' the eye's own; ready once its buffers are (`light_ready`).
    pub fn light_made(&mut self, g: &Gpu, wanted: bool) {
        let Some(c) = self.cast.as_mut() else { return };
        if !wanted || c.light_ready {
            return;
        }
        c.light_list = g.buf(
            word_bytes(shadow::LIST_WORDS as usize),
            buffer_usage::STORAGE | buffer_usage::COPY_DST | buffer_usage::COPY_SRC,
        );
        c.light_ready = c.raster.light_made(g, shadow::size);
    }

    /// The counts copied last frame mapped back (`Tally::back`).
    pub fn tally_back(&mut self, g: &Gpu) {
        if let Some(c) = self.cast.as_mut() {
            c.tally.back(g);
        }
    }

    /// The counts that landed since last taken (`Tally::take`).
    pub fn take_counts(&mut self) -> Option<RayCounts> {
        self.cast.as_mut().and_then(|c| c.tally.take())
    }

    pub fn light_casts(&self) -> bool {
        self.cast
            .as_ref()
            .is_some_and(|c| c.light_levels.iter().any(|v| v.cast))
    }

    /// Why the light's map cast this frame, a bit each in LIGHT_MOVED's
    /// order (span.json `LIGHT_MOVED`); 0 where it did not.
    pub fn light_moved(&self) -> u32 {
        self.cast.as_ref().map_or(0, |c| {
            c.light_levels.iter().fold(0, |bits, v| bits | v.moved)
        })
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
                for &b in &f.boxes[..f.holding] {
                    hold(&mut c.boxes, &mut c.holding, b);
                }
                for &first in &f.written_groups[..f.writes] {
                    c.forget |= !note(&mut c.written_groups, &mut c.writes, first);
                }
            }
            c.read.forget();
            for v in &mut c.light_levels {
                v.content.forget();
            }
            c.generator.on_dropped();
            c.tally.on_dropped();
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

/// The cast's group 0 over `bindings`: the eye, the clip level and its fold
/// (`fold_cs` writes it, the cast reads it), the tiles' frames, the
/// surface's rays, the bricks' palette indices, the service area's index, the
/// overwrites, the ways' pool, the beam rays' lines and the cells' starts,
/// the observed pyramid, and the head (`WORLD_GROUP`): the eye's, or, over
/// the light's list and head, the shadow map's (`light_groups`).
fn cast_group(g: &Gpu, pipeline: &Pipeline, bindings: [u32; 11]) -> u32 {
    let [eye, clipmap, ground, rays, palette_indices, shards, overwrites, pool, beams, pyramid, head] =
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
        buf_entry(8, beams),
        buf_entry(9, pyramid),
        buf_entry(HEAD_BINDING, head),
    ];
    g.bind_group(pipeline.layouts[0], &entries)
}

/// The missed pixels' hits' passes' groups 0 (`hit_lead_cs`,
/// `hit_faces_cs`) over `bindings`: the eye, the rays, the beam's words, the
/// pyramid, and the walk again's words, which the faces' leaders are
/// listed in and the faces' pass none of.
fn hit_groups(g: &Gpu, [lead, faces]: [&Pipeline; 2], bindings: [u32; 5]) -> [u32; 2] {
    let [eye, rays, beams, pyramid, marks] = bindings;
    [
        g.bind_group(
            lead.layouts[0],
            &[
                buf_entry(0, eye),
                view_entry(3, rays),
                buf_entry(8, beams),
                buf_entry(9, pyramid),
                buf_entry(14, marks),
            ],
        ),
        g.bind_group(
            faces.layouts[0],
            &[
                buf_entry(0, eye),
                view_entry(3, rays),
                buf_entry(8, beams),
                buf_entry(9, pyramid),
            ],
        ),
    ]
}

/// The pixels' group 0 over `bindings`: the eye, the rays, the beam's words,
/// the pyramid, which the marked pixels are listed in, the visibility buffer,
/// a one-layer array, and the words the marked are counted in (`marks`).
/// No world: a pixel walks nothing of its own (`cast_cs`).
fn px_group(g: &Gpu, pipeline: &Pipeline, bindings: [u32; 7], marks: u32) -> u32 {
    let [eye, rays, beams, pyramid, visibility, changed_at, tiles] = bindings;
    let entries = [
        buf_entry(0, eye),
        view_entry(3, rays),
        buf_entry(8, beams),
        buf_entry(9, pyramid),
        view_entry(13, visibility),
        buf_entry(14, marks),
        buf_entry(30, changed_at),
        buf_entry(31, tiles),
    ];
    g.bind_group(pipeline.layouts[0], &entries)
}

const _: () = assert!(
    light::tiles(shadow::FIRST_SIZE).is_multiple_of(light::COARSE)
        && light::tiles(shadow::SIZE).is_multiple_of(light::COARSE),
    "the map's tiles are a whole number of squares a side (`light::coarse_square`)"
);

/// The workgroups of the passes that cast the light's tiles as a cast starts:
/// none, a list's row across and no row, then each whole
/// (`light::ARGS_WORDS`), a level `size` texels a side.
fn light_args(size: u32) -> [u32; light::ARGS_WORDS as usize] {
    let across = |n: u32, w: [u32; 3]| [n.div_ceil(w[0]), n.div_ceil(w[1]), 1];
    let samples = size / SAMPLE;
    let tiles = size / light::TILE;
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
    for (w, whole) in [
        (
            light::ARGS_BEAM,
            across(light::beam_across(size), compute::BEAM_WORKGROUP),
        ),
        (
            light::ARGS_CELLS,
            across(light::cells_across(size), compute::CELL_WORKGROUP),
        ),
        (
            light::ARGS_SAMPLE,
            across(samples, compute::SAMPLE_WORKGROUP),
        ),
        (
            light::ARGS_SAMPLE_BOX,
            across(samples, compute::SAMPLE_BOX_WORKGROUP),
        ),
        (light::ARGS_SLOT, [tiles, tiles, 1]),
    ] {
        let at = (light::ARGS_WHOLE + w) as usize;
        args[at..at + 3].copy_from_slice(&whole);
    }
    args
}

/// What a level's groups bind beside the world (`light_groups`): its
/// number, the observed pyramid, the levels' heads and stands (each pass of
/// the level its own, the marks all) and the list's buffer, the raster's boxes and
/// visibility buffer, the level's raster args, its misses' dispatch and its
/// casting passes' workgroups.
struct LightBuffers {
    level: u32,
    pyramid: u32,
    heads: u32,
    list: u32,
    stands: u32,
    boxes: u32,
    visibility: u32,
    args: u32,
    misses: u32,
    light_args: u32,
}

/// A level's cast's groups 0, over `bound`: the eye, the clip level, the
/// tiles' frames, the bricks' palette indices, the service area's index, the
/// overwrites and the ways' pool, the cast's alone; the observed pyramid,
/// which the light's walks read and write as the eye's do; its layer of the
/// world; its head, the list's buffer, the beam's lines, the samples and
/// the misses the levels share, and its state past them (the list's
/// buffer, bound whole), the raster's boxes, visibility buffer and args, the misses'
/// dispatch and the casting passes' workgroups (`light::ARGS_WORDS`), which
/// the propagation writes; the eye's rays, which the mark reads, and the
/// decor's drawing words, regions and instances, which the drawn mark reads.
/// The slots', its world's (the cells' starts, the eye's pass, over
/// `WORLD_GROUP`, the list where the eye's beam rays' buffer lies and its
/// head where the eye's), its boxes' (the eye's raster's pass), the misses'
/// walk's, the two marks' (every level's heads and stands, `Heads`), the
/// propagation's, the detail's, the beam's, the samples', the held margins'
/// gather's and the misses' margins'; the boxes' none before its raster's
/// buffers are made.
fn light_groups(
    g: &Gpu,
    [walk, world, boxes_built, miss, mark, mark_draws, propagate, detail, beam, sample, margin, miss_margin]: [&Pipeline; 12],
    bound: [u32; 12],
    b: LightBuffers,
) -> [u32; 12] {
    let [eye, clipmap, ground, palette_indices, shards, overwrites, pool, light, rays, drawing, regions, instances] =
        bound;
    let (head_at, head_bytes) = head_range(b.level);
    let head = buf_range_entry(HEAD_BINDING, b.heads, head_at, head_bytes);
    let list = |binding: u32| buf_entry(binding, b.list);
    let n = shadow::size(b.level);
    let stands = buf_range_entry(
        STANDS_BINDING,
        b.stands,
        word_bytes(shadow::stands_at(b.level) as usize),
        word_bytes(light::stands(n) as usize),
    );
    let entries = [
        (0, buf_entry(0, eye)),
        (1, buf_entry(1, clipmap)),
        (2, buf_entry(2, ground)),
        (3, view_entry(3, rays)),
        (4, buf_entry(4, palette_indices)),
        (5, buf_entry(5, shards)),
        (6, buf_entry(6, overwrites)),
        (7, buf_entry(7, pool)),
        (9, buf_entry(9, b.pyramid)),
        (10, view_entry(10, light)),
        (HEAD_BINDING, head),
        (12, list(12)),
        (14, buf_entry(14, b.boxes)),
        (15, view_entry(15, b.visibility)),
        (16, buf_entry(16, b.args)),
        (17, buf_entry(17, b.misses)),
        (18, buf_entry(18, drawing)),
        (19, buf_entry(19, regions)),
        (20, buf_entry(20, instances)),
        (21, buf_entry(21, b.light_args)),
        (STANDS_BINDING, stands),
    ];
    // Each the entries its layout declares (build.rs `compute`).
    let of = |declared: &[u32]| {
        entries
            .iter()
            .filter(|(b, _)| declared.chunks(3).any(|d| d[0] == *b))
            .map(|&(_, e)| e)
            .collect::<Vec<_>>()
    };
    // The marks', over every level's heads and stands.
    let all = |declared: &[u32]| {
        let mut e = of(declared);
        for x in &mut e {
            if x.binding == STANDS_BINDING {
                *x = buf_entry(STANDS_BINDING, b.stands);
            }
        }
        if declared.chunks(3).any(|d| d[0] == HEADS_BINDING) {
            e.push(buf_entry(HEADS_BINDING, b.heads));
        }
        e
    };
    let world_entries = [
        buf_entry(0, eye),
        buf_entry(1, clipmap),
        buf_entry(2, ground),
        view_entry(3, rays),
        buf_entry(4, palette_indices),
        buf_entry(5, shards),
        buf_entry(6, overwrites),
        buf_entry(7, pool),
        list(8),
        buf_entry(9, b.pyramid),
        head,
    ];
    [
        g.bind_group(walk.layouts[0], &of(&compute::LIGHT_GROUP)),
        g.bind_group(world.layouts[0], &world_entries),
        // The raster's once its buffers are made.
        if b.boxes == 0 {
            0
        } else {
            g.bind_group(
                boxes_built.layouts[0],
                &[
                    buf_entry(0, eye),
                    list(8),
                    head,
                    buf_entry(14, b.boxes),
                    buf_entry(16, b.args),
                ],
            )
        },
        g.bind_group(miss.layouts[0], &of(&compute::LIGHT_MISS_GROUP)),
        g.bind_group(mark.layouts[0], &all(&compute::LIGHT_MARK_GROUP)),
        g.bind_group(
            mark_draws.layouts[0],
            &all(&compute::LIGHT_MARK_DRAWS_GROUP),
        ),
        g.bind_group(propagate.layouts[0], &of(&compute::LIGHT_PROPAGATE_GROUP)),
        g.bind_group(detail.layouts[0], &of(&compute::LIGHT_DETAIL_GROUP)),
        g.bind_group(beam.layouts[0], &of(&compute::LIGHT_BEAM_GROUP)),
        g.bind_group(sample.layouts[0], &of(&compute::LIGHT_SAMPLE_GROUP)),
        g.bind_group(margin.layouts[0], &of(&compute::LIGHT_MARGIN_GROUP)),
        g.bind_group(
            miss_margin.layouts[0],
            &of(&compute::LIGHT_MISS_MARGIN_GROUP),
        ),
    ]
}
