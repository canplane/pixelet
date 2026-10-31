// pxl-render/gpu/ray/cast.rs

//! The GPU's cast of the eye's rays into the world surface's rays, the
//! world's depth from the light cast with them, and the decor generated over the
//! world (gpu/generated.rs): what each reads, made once, and their dispatches.

use pxl_spec_core::generated::{self, GROUPS};
use pxl_spec_core::window::{self, FRESH_CELL, FRESH_FIRST};
use pxl_spec_core::{beam, eye, light, observed, overwrite, ray};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use super::raster::Raster;
use super::tally::{self, Counted, Tally};
use super::{casts_px, RaysPass};
use crate::frame::shader::floor;
use crate::frame::shader::record::{
    compute, within, COMPUTE, SHADER_BEAM, SHADER_CAST, SHADER_CELL, SHADER_FORGET, SHADER_JOINED,
    SHADER_LIGHT, SHADER_LIGHT_BEAM, SHADER_LIGHT_DETAIL, SHADER_LIGHT_MARK,
    SHADER_LIGHT_MARK_DRAWN, SHADER_LIGHT_MISS, SHADER_LIGHT_REACH, SHADER_LIGHT_SAMPLE,
    SHADER_MISS, SHADER_MISS_LIST, SHADER_SAMPLE, UNIFORM,
};
use crate::gpu::api;
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, release, view_entry, word_bytes, write_buffer_bytes, Gpu,
    RenderContext,
};
use crate::gpu::generated::{cut_boxes, hold, Drawn, Generated, Generation};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::read::Read;
use crate::gpu::renderer::{Cast, Overwrites};
use crate::gpu::shadow;
use crate::gpu::span_spec::{FrameKind, GpuSpanName};
use pxl_spec_core::param::light::SAMPLE;

/// What the GPU's cast reads; it writes the world surface's rays.
pub(super) struct CastBuffers {
    built: Built,
    /// The beam rays (`beam_cs`, a thread a corner), where each cell's
    /// rays start off them (`cell_cs`, a thread a cell), the samples walked
    /// from those starts (`sample_cs`, a thread a sample): the eye's over
    /// the cast's group and the shadow map's over the light's (`light_groups`),
    /// each told by the head it binds (`eye_head`, `light_moved`); and the
    /// eye's buffer (pxl-spec-core `beam`): a line a corner, a start a
    /// cell, a stop a sample, as the eye's words lay the frame's `grid`, none
    /// a pixel's; room for `beam_room` words.
    beam_built: Built,
    cells_built: Built,
    sample_built: Built,
    beams: u32,
    /// The eye's head, all zero: a head naming no map (pxl-spec-core
    /// `light::HEAD_SIZE`), which tells a pass both take the eye's.
    eye_head: u32,
    grid: beam::Grid,
    beam_room: u32,
    /// The observed pyramid (pxl-spec-core `observed`): its buffer, no
    /// pixel's; the pixels the cast's reading walk missed walked again, a
    /// thread each of their list (`miss_list_cs`, pxl-spec-core
    /// `observed::MARKS_AT`), or where it was full a thread a drawn pixel
    /// (`miss_cs`), each told by the mark its own first ray word holds
    /// (pxl-spec-core `ray::MISS`), both dispatched off the words the
    /// reading walk counts them in (`marks`, pxl-spec-core
    /// `ray::MISS_ARGS_WORDS`); the pyramid forgotten (`forget_cs`) where
    /// what the world's query reads changed since the last cast: all of it
    /// (`forget`), or the nodes over the eye's windows' entries that moved
    /// (`moved`, a bit an entry, against the entries as `entries` last held them)
    /// and over the boxes of what is placed or cut that changed (`boxes`,
    /// `holding` of them, as decor's generation holds them, gpu/generated.rs
    /// `hold`), and those handed a slot of a family of what is placed that
    /// was written (`written`, `writes` of them); as this frame's cast does
    /// (`forgets`, pxl-spec-core `observed::FORGETS`), and what it sent, put
    /// back where its frame is dropped (`sent`). `frame` numbers the casts since it was last
    /// forgotten whole, and `cuts` are the preview's cuts it was asked with.
    miss_built: Built,
    miss_list_built: Built,
    marks: u32,
    forget_built: Built,
    seen: u32,
    frame: u32,
    forget: bool,
    moved: [u32; observed::MOVED_WORDS as usize],
    boxes: [[i32; 4]; overwrite::BOXES as usize],
    holding: usize,
    written: [u32; observed::WRITTEN as usize],
    writes: usize,
    sent: Option<Forgot>,
    entries: Vec<i32>,
    forgets: u32,
    cuts: [u32; eye::CUTS],
    /// The eye's windows folded over their bricks (`joined_cs`), over its
    /// group, the window buffer's rows past their entry table
    /// (`window::JOINED_AT`), written where the windows' entries moved
    /// (`refold`). The CPU folds its own over the bricks it holds
    /// (pxl-kernel domain/layer/terrain/window.rs `fold`).
    joined_built: Built,
    /// The world's depth from the light, a texel of the shadow map each
    /// (`light_cs`), cast with the eye's rays where the shadows are on.
    light_built: Built,
    /// The slots that took no visible sample walked apart (`light_miss_cs`).
    light_miss_built: Built,
    /// The tiles of the light's box its receivers stand on in a cast
    /// (pxl-spec-core `light::stands_at`): where the eye's rays meet the world
    /// (`light_mark_cs`), and where the decor and the actors drawn stand
    /// (`light_mark_drawn_cs`); then those they read (`light_reach_cs`).
    light_mark_built: Built,
    light_mark_drawn_built: Built,
    light_reach_built: Built,
    /// The tiles the box slid onto, or whose eye moved past the margin of
    /// what their walks asked, set back to be read anew (`light_detail_cs`),
    /// before the reach.
    light_detail_built: Built,
    /// The light's beam rays and samples, the eye's passes in entries of
    /// their own, which tell the margin of what their walks ask
    /// (`light_beam_cs`, `light_sample_cs`); its misses' walk tells its own.
    light_beam_built: Built,
    light_sample_built: Built,
    /// The light's cast's groups (`light_groups`), and the buffers they bind:
    /// made again exactly when one of them is another.
    light_groups: [u32; 10],
    light_bound: [u32; 12],
    /// A word bound where decor is off, the decor's buffers the drawn mark
    /// reads none (`light_mark_drawn_cs`).
    light_none: u32,
    /// The light's casts so far, the number its marks are made with
    /// (pxl-spec-core `light::HEAD_CAST`), never zero, which no tile holds
    /// marked.
    light_number: u32,
    /// The number of the cast since which only the eye's look moved
    /// (pxl-spec-core `light::HEAD_EPOCH`): a tile cast in it or after
    /// stands.
    light_epoch: u32,
    /// Whether all the light's cast is made (`light_made`): its list and its
    /// raster's buffers and targets; before then no shadow map is cast.
    light_ready: bool,
    /// The beam's lines, the samples and the misses (pxl-spec-core
    /// `light::list_words`), made where shadows are first cast.
    light_list: u32,
    light_list_whole: bool,
    /// The misses' walk's dispatch and count, as the eye's are
    /// (pxl-spec-core `ray::MISS_ARGS_WORDS`).
    light_misses: u32,
    /// The workgroups of the passes that cast its tiles (pxl-spec-core
    /// `light::ARGS_WORDS`): none, or each whole where a tile is claimed.
    light_args: u32,
    /// What a tile of the shadow map holds follows, as the light's cast last
    /// read it (`Read`): the light's frame (`Shadow::generation`), whether
    /// shadows are cast, the eye's zooms, and whether the world under it
    /// moved; one moving begins an epoch (`light_epoch`).
    light_content: Read,
    /// The eye's place and the light box, which follows it, as it last read
    /// them: the zooms the walks ask follow the one, and what each tile of
    /// the box holds moves with the other's corner. One moving alone moves
    /// the reads with the box and casts the tiles the box slid onto or whose
    /// eye moved past the margin of what their walks asked
    /// (`light_detail_cs`, pxl-spec-core `light::holds`), and those read
    /// anew; whether this frame's cast asks which first, and the corner the
    /// cast before was cast under (`light::HEAD_CORNER_WAS`).
    light_detail: Read,
    light_detail_moved: bool,
    light_corner: [i32; 2],
    light_corner_was: [i32; 2],
    /// The reads as the cast before left them, copied out where the box slid
    /// to be copied back moved with it.
    light_reads_was: u32,
    /// Which tiles the receivers read, as it last read it: the eye's look,
    /// the actors' places, decor placed again. One moving alone casts only
    /// the tiles read anew.
    light_receivers: Read,
    /// What it is handed beside the eye's words (pxl-spec-core
    /// `light::HEAD_WORDS`).
    light_moved: u32,
    light_cast: bool,
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
    bound: [u32; 11],
    eye: u32,
    pub(super) window: u32,
    shards: u32,
    /// Shards the buffer holds.
    shard_room: usize,
    /// The drawn pixels it casts, a ray each: the world surface's.
    size: [u32; 2],
    /// The overwrites' heap lane then their data lane, each `overwrite_room`
    /// words, the lanes' own room; the revision it holds, none at first.
    overwrites: u32,
    overwrite_room: usize,
    overwrite_revision: Option<u32>,
    generated: Generated,
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
    /// The samples' stops handed to the rasterizer (ray/raster.rs), whose
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
    /// The list, the light's visibility buffer and its boxes the light's groups
    /// were made over, made again where one is another.
    light_grouped: [u32; 3],
}

/// What a cast's head handed the pyramid's forgetting (pxl-spec-core
/// `observed::HEAD_WORDS`): whether all of it, the entries that moved, the
/// boxes and the families written; put back where its frame is dropped.
struct Forgot {
    all: bool,
    moved: [u32; observed::MOVED_WORDS as usize],
    boxes: [[i32; 4]; overwrite::BOXES as usize],
    holding: usize,
    written: [u32; observed::WRITTEN as usize],
    writes: usize,
}

/// Family `first` of what is placed noted written among the `writes` of
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

/// The binding of a pass's head (pxl-shader light.rs `Head`): the light's
/// map's, or the eye's, which names none.
pub(super) const HEAD_BINDING: u32 = 11;

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
    within(compute::JOINED_GROUP, WORLD_GROUP)
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
        let built = pipelines.build_compute(g, SHADER_CAST, &compute::CAST_GROUP);
        let beam_built = pipelines.build_compute(g, SHADER_BEAM, group);
        let cells_built = pipelines.build_compute(g, SHADER_CELL, group);
        let sample_built = pipelines.build_compute(g, SHADER_SAMPLE, group);
        let joined_built = pipelines.build_compute(g, SHADER_JOINED, group);
        let miss_built = pipelines.build_compute(g, SHADER_MISS, group);
        let miss_list_built = pipelines.build_compute(g, SHADER_MISS_LIST, group);
        let forget_built = pipelines.build_compute(g, SHADER_FORGET, group);
        let light_built = pipelines.build_compute(g, SHADER_LIGHT, &compute::LIGHT_GROUP);
        let light_miss_built =
            pipelines.build_compute(g, SHADER_LIGHT_MISS, &compute::LIGHT_MISS_GROUP);
        let light_mark_built =
            pipelines.build_compute(g, SHADER_LIGHT_MARK, &compute::LIGHT_MARK_GROUP);
        let light_mark_drawn_built =
            pipelines.build_compute(g, SHADER_LIGHT_MARK_DRAWN, &compute::LIGHT_MARK_DRAWN_GROUP);
        let light_reach_built =
            pipelines.build_compute(g, SHADER_LIGHT_REACH, &compute::LIGHT_REACH_GROUP);
        let light_detail_built =
            pipelines.build_compute(g, SHADER_LIGHT_DETAIL, &compute::LIGHT_DETAIL_GROUP);
        let light_beam_built =
            pipelines.build_compute(g, SHADER_LIGHT_BEAM, &compute::LIGHT_BEAM_GROUP);
        let light_sample_built =
            pipelines.build_compute(g, SHADER_LIGHT_SAMPLE, &compute::LIGHT_SAMPLE_GROUP);
        let raster = Raster::new(g, pipelines);
        let tally = Tally::new(g);
        let buf = |size: u32, usage: u32| g.buf(size, usage);
        let storage = buffer_usage::STORAGE | buffer_usage::COPY_DST;
        let c = CastBuffers {
            built,
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
            marks: buf(
                word_bytes(ray::MISS_ARGS_WORDS),
                storage | buffer_usage::INDIRECT | buffer_usage::COPY_SRC,
            ),
            forget_built,
            seen: buf(
                word_bytes(observed::WORDS as usize),
                storage | buffer_usage::COPY_SRC,
            ),
            frame: 0,
            forget: true,
            moved: [0; observed::MOVED_WORDS as usize],
            boxes: [[0; 4]; overwrite::BOXES as usize],
            holding: 0,
            written: [0; observed::WRITTEN as usize],
            writes: 0,
            sent: None,
            entries: Vec::new(),
            forgets: observed::FORGETS_NONE,
            cuts: [0; eye::CUTS],
            joined_built,
            light_built,
            light_miss_built,
            light_mark_built,
            light_mark_drawn_built,
            light_reach_built,
            light_detail_built,
            light_beam_built,
            light_sample_built,
            light_groups: [0; 10],
            light_bound: [0; 12],
            light_none: buf(word_bytes(2 * generated::drawn::REGIONS as usize), storage),
            light_number: 0,
            light_epoch: 0,
            light_ready: false,
            light_list: light_list(g, false),
            light_list_whole: false,
            light_misses: buf(
                word_bytes(ray::MISS_ARGS_WORDS),
                storage | buffer_usage::INDIRECT | buffer_usage::COPY_SRC,
            ),
            light_args: buf(
                word_bytes(light::ARGS_WORDS as usize),
                storage | buffer_usage::INDIRECT,
            ),
            light_content: Read::default(),
            light_detail: Read::default(),
            light_detail_moved: false,
            light_corner: [0; 2],
            light_corner_was: [0; 2],
            light_reads_was: buf(
                word_bytes((light::tiles(shadow::SIZE) * light::tiles(shadow::SIZE)) as usize),
                buffer_usage::COPY_SRC | buffer_usage::COPY_DST,
            ),
            light_receivers: Read::default(),
            light_moved: buf(
                word_bytes(light::HEAD_WORDS),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
            light_cast: false,
            light_world: false,
            light_footprint_owed: false,
            refold: false,
            group: 0,
            bound: [0; 11],
            eye: buf(
                word_bytes(eye::WORDS),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
            window: buf(
                word_bytes((4 * window::JOINED_AT + window::EYE_JOINED_WORDS) as usize),
                storage | buffer_usage::COPY_SRC,
            ),
            shards: 0,
            shard_room: 0,
            size: [0; 2],
            overwrites: 0,
            overwrite_room: 0,
            overwrite_revision: None,
            generated: Generated::new(g, pipelines),
            read: Read::default(),
            jitter: [0; 2],
            gated: false,
            raster,
            px_group: 0,
            px_bound: [0; 7],
            tally,
            light_grouped: [0; 3],
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
        light_map: (u32, u32, light::Starts),
        gated: bool,
    ) {
        // No cast, no world: no ray cast, none drawn and no decor generated.
        // What the cast reads stands for the next: the ground it binds is
        // made once.
        let (Some(cast), Some((ground, looks, pool))) = (
            cast,
            self.ground
                .as_ref()
                .map(|gb| (gb.ground, gb.looks, gb.pool)),
        ) else {
            self.kind = FrameKind::plain;
            if let Some(c) = self.cast.as_mut() {
                c.generated.off();
                c.read.forget();
                c.light_content.forget();
                c.light_cast = false;
            }
            return;
        };
        let c = self.cast.as_mut().or_bug(Errno::ENOTRECOVERABLE);
        // The cast reads exactly this, all of it here: a field the frame hands
        // the cast fails to destructure until it is read. The overwrites'
        // contents are their revision's. The decor it generates reads its own
        // (`Generated::prepare`), so a change to decor alone casts no ray again.
        let Cast {
            eye,
            window,
            shards,
            overwrites:
                Overwrites {
                    heap: _,
                    data: _,
                    room,
                    revision,
                    since: _,
                    written: _,
                },
            decor,
            // The light's alone, which reads them with the look (below).
            actors: _,
            ray_counts,
        } = &cast;
        let ray_counts = *ray_counts;
        let r = &mut c.read;
        r.begin();
        // The light's words are its cast's alone (below).
        let eye_moved = r.read(&eye[..eye::LIGHT]);
        let window_moved = r.read(window);
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
            cast.window.len() == (4 * window::JOINED_AT) as usize,
            ENOTRECOVERABLE,
            cast.window.len()
        );
        if window_moved {
            g.write_buf(c.window, 0, bytes(cast.window));
            c.refold = true;
        }
        c.size = rays.1;
        // The grid as the eye's words lay it (pxl-spec-core `eye::GRID`), the
        // dispatches' sizes, and room for its corners' lines, its cells'
        // starts and its samples' stops, made again only where they outgrow
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
            // Held at `since`: the families written after it alone, a record
            // each, where the stream has room for them all; else both lanes
            // whole, read where they live, two records however large.
            fn family(lane: &[u32], first: u32) -> &[u32] {
                &lane[first as usize..(first + overwrite::FAMILY) as usize]
            }
            // The families written since the revision the pyramid was asked
            // at, which the nodes handed one of their slots read again; past
            // the room, or where revisions went by unseen, all of it.
            if c.overwrite_revision == Some(o.since) {
                for &first in o.written {
                    c.forget |= !note(&mut c.written, &mut c.writes, first);
                }
            } else {
                c.forget = true;
            }
            let listed = |lane: &[u32]| {
                o.written
                    .iter()
                    .map(|&f| write_buffer_bytes(bytes(family(lane, f))))
                    .sum::<usize>()
            };
            if c.overwrite_revision == Some(o.since) && api::room(listed(o.heap) + listed(o.data)) {
                for &first in o.written {
                    g.write_buf(
                        c.overwrites,
                        word_bytes(first as usize),
                        bytes(family(o.heap, first)),
                    );
                    g.write_buf(
                        c.overwrites,
                        word_bytes(o.room + first as usize),
                        bytes(family(o.data, first)),
                    );
                }
            } else {
                g.upload_buf(c.overwrites, 0, bytes(o.heap));
                g.upload_buf(c.overwrites, word_bytes(o.room), bytes(o.data));
            }
            c.overwrite_revision = Some(o.revision);
        }
        // What the world's query reads moved: the windows, the bricks the
        // turn handed (their cells, looks and ways), the service area, what
        // is placed, a preview's cuts. What the observed pyramid kept is
        // forgotten; so it is once its frames would outrun a key's tag.
        let cuts: [u32; eye::CUTS] = cast.eye[eye::CARVE..eye::CARVE + eye::CUTS]
            .try_into()
            .or_bug(Errno::ENOTRECOVERABLE);
        let world_moved = window_moved
            || fresh != 0
            || shards_moved
            || shards_grow
            || overwrites_moved
            || cuts != c.cuts;
        // What is placed and a preview's cuts forget the nodes over the boxes
        // they changed, as decor's generation is asked over them (gpu/generated.rs):
        // every write of what is placed names its box (`DecorFrame::edits`);
        // one that names none, or the lanes made again, forgets all.
        let edits = decor.edits;
        for &b in edits {
            hold(&mut c.boxes, &mut c.holding, b);
        }
        cut_boxes(&c.cuts, &cuts, |b| hold(&mut c.boxes, &mut c.holding, b));
        c.forget |= shards_moved
            || shards_grow
            || overwrites_grow
            || (overwrites_moved && edits.is_empty())
            || c.frame >= observed::FRAMES;
        // The eye's windows' entries that hold another brick, or whose brick
        // the turn handed: what is over them is asked again.
        c.cuts = cuts;
        let eye_rows = (window::EYE_WINDOWS * window::ENTRIES) as usize;
        if window_moved {
            let rows = &cast.window[..4 * eye_rows];
            for k in 0..eye_rows {
                let row = &rows[4 * k..4 * k + 4];
                if c.entries.get(4 * k..4 * k + 4) != Some(row) {
                    c.moved[k / 32] |= 1 << (k % 32);
                }
            }
            c.entries.clear();
            c.entries.extend_from_slice(rows);
        }
        // A brick handed moves the eye's entry that wants it, whichever
        // window's slot it is filled into.
        if let Some(gb) = self.ground.as_ref() {
            for f in &gb.handed[..fresh as usize] {
                let (cell, q) = (
                    f[FRESH_CELL as usize],
                    window::fresh_brick([f[FRESH_FIRST as usize], f[FRESH_FIRST as usize + 1]]),
                );
                let w = cell.wrapping_sub(window::FIRST_CELL);
                let k = (w * window::ENTRIES + window::entry(q)) as usize;
                let row = &cast.window[4 * k..4 * k + 4];
                if w < window::EYE_WINDOWS && [row[0], row[1]] == q {
                    c.moved[k / 32] |= 1 << (k % 32);
                }
            }
        }
        // Decor generated first: where it stands is what the light's cast reads
        // of it, and its buffers what the light's groups bind.
        let world = [c.eye, c.window, ground, c.shards, c.overwrites, pool];
        let generation = Generation {
            window: cast.window,
            cuts: cast.eye[eye::CARVE..eye::CARVE + eye::CUTS]
                .try_into()
                .or_bug(Errno::ENOTRECOVERABLE),
            fresh: self
                .ground
                .as_ref()
                .map_or(&[][..], |gb| &gb.handed[..fresh as usize]),
            eye: &cast.eye,
        };
        c.generated
            .prepare(g, cast.decor, generation, meshes, world);
        // The shadow map is cast, every slot a receiver reads, as the eye's
        // rays are (pxl-spec-core `light`), where what it reads moved: the
        // light's frame or box, the eye's place or zooms, which the
        // zooms its rays ask follow, or the world under it; and where what
        // says which tiles are read moved, the look, the actors' places and
        // decor placed again (`light_mark_cs`, `light_mark_drawn_cs`). A
        // standing frame casts none. Where only the latter moved, a tile
        // cast since anything else last did stands, and only the tiles read
        // anew are cast (`light::HEAD_EPOCH`). None where no shadow is cast,
        // nor before all its cast is made (`light_made`), its read let go
        // until then.
        let shadows = cast.eye[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS]
            .iter()
            .any(|&w| w != 0);
        // All it reads but the look, then the look.
        let r = &mut c.light_content;
        r.begin();
        r.read(&[light_map.1, u32::from(shadows)]);
        r.read(&cast.eye[eye::RANGE..eye::RANGE + 2]);
        r.mark(world_moved);
        let held = r.end();
        // The eye's place, which the zooms the walks ask follow, and the box,
        // off the eye: a tile cast in the epoch holds while the one moved
        // less than the margin of what its walks asked and the other slid
        // off none of its slots (`light_detail_cs`).
        let d = &mut c.light_detail;
        d.begin();
        d.read(&cast.eye[eye::ORIGIN..eye::STEPS]);
        d.read(&cast.eye[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS]);
        let detail_held = d.end();
        // The look, where the actors stand and decor compacted again: which
        // tiles are read, not what a tile holds. Not the jitter: a pixel's
        // marks hold its footprint (`light_mark_cs`).
        let l = &mut c.light_receivers;
        l.begin();
        l.read(&cast.eye[eye::FORWARD..eye::TAN + 2]);
        l.read(cast.actors);
        l.mark(c.generated.counts());
        // The first frame the eye stands in after it moved marks again, with
        // the footprints the jitter's frames after it stand on.
        let footprint_owed = c.light_footprint_owed && kind != FrameKind::cast;
        let light_moved = !held || !detail_held || !l.end() || footprint_owed;
        c.light_detail_moved = held && !detail_held;
        if !c.light_ready {
            c.light_content.forget();
        }
        c.light_cast = shadows && light_moved && c.light_ready;
        // The world's receivers stand where the eye's rays met it: marked
        // again by the next cast after the rays were cast anew, or where an
        // epoch begins. Else what they stood on stands, marked in the epoch
        // already, so none of it is the cast's to cast anew
        // (`light_reach_cs`); the jitter's casts move a ray within its
        // footprint, marked already where the eye stood when they were.
        c.light_world |= kind == FrameKind::cast || !held || footprint_owed;
        if c.light_cast {
            // The head (pxl-spec-core `light::HEAD_WORDS`).
            let mut head = [0u32; light::HEAD_WORDS];
            c.light_number = c.light_number.wrapping_add(1).max(1);
            if !held {
                c.light_epoch = c.light_number;
            }
            head[light::HEAD_CAST] = c.light_number;
            head[light::HEAD_EPOCH] = c.light_epoch;
            head[light::HEAD_COUNTED] = u32::from(ray_counts);
            head[light::HEAD_SIZE] = shadow::SIZE;
            let corner = light::CORNER.map(|k| cast.eye[eye::LIGHT + k] as i32);
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
            // The actors drawn, receivers as decor is (`light_mark_drawn_cs`).
            let at = 4 * light::ACTORS_AT;
            let n = cast.actors.len().min(light::ACTORS);
            head[at] = n as u32;
            for (k, a) in cast.actors[..n].iter().enumerate() {
                let w = at + 4 * (1 + k);
                head[w..w + 3].copy_from_slice(&a.map(f32::to_bits));
            }
            g.write_buf(c.light_moved, 0, bytes(&head));
            g.write_buf(c.light_misses, 0, bytes(&ray::MISS_ARGS_EMPTY));
            g.write_buf(c.light_args, 0, bytes(&light_args()));
            c.raster.light_prepare(g, &cast.eye, shadow::SIZE);
        }
        if eye_moved || jittered || c.light_cast {
            g.write_buf(c.eye, 0, bytes(&cast.eye));
        }
        // The samples' raster through its gate where only the kernel's
        // bounds moved; the pixels' cast passes by the tiles not claimed
        // (`px_group`).
        c.gated = gated && kind == FrameKind::px;
        if casts {
            c.forgets = if c.forget {
                observed::FORGETS_ALL
            } else if c.moved.iter().any(|&m| m != 0) || c.holding != 0 || c.writes != 0 {
                observed::FORGETS_MOVED
            } else {
                observed::FORGETS_NONE
            };
            // The head before the frame's dispatches: what it forgets and over
            // which entries; forgotten whole, its first frame, else the next.
            let mut head = [0u32; observed::HEAD_WORDS as usize];
            c.frame = if c.forgets == observed::FORGETS_ALL {
                1
            } else {
                c.frame + 1
            };
            head[observed::FRAME as usize] = c.frame;
            head[observed::FORGETS as usize] = c.forgets;
            let boxes = observed::BOXES_AT as usize;
            head[observed::MOVED_AT as usize..boxes].copy_from_slice(&c.moved);
            head[boxes] = c.holding as u32;
            for (k, b) in c.boxes[..c.holding].iter().enumerate() {
                let at = boxes + 1 + 4 * k;
                head[at..at + 4].copy_from_slice(&b.map(|v| v as u32));
            }
            let written = observed::WRITTEN_AT as usize;
            head[written] = c.writes as u32;
            head[written + 1..written + 1 + c.writes].copy_from_slice(&c.written[..c.writes]);
            g.write_buf(c.seen, 0, bytes(&head));
            g.write_buf(c.marks, 0, bytes(&ray::MISS_ARGS_EMPTY));
            c.sent = Some(Forgot {
                all: c.forgets == observed::FORGETS_ALL,
                moved: c.moved,
                boxes: c.boxes,
                holding: c.holding,
                written: c.written,
                writes: c.writes,
            });
            (c.forget, c.moved) = (false, [0; observed::MOVED_WORDS as usize]);
            (c.holding, c.writes) = (0, 0);
        } else {
            c.sent = None;
        }
        let frame = tally::Frame {
            // A full cast's alone, so what a cast asks is not shared out
            // over the frames that cast only the pixels.
            casts: [self.kind == FrameKind::cast, c.light_cast],
            rays: [c.size[0] * c.size[1], shadow::SIZE * shadow::SIZE],
        };
        c.tally.begin(g, ray_counts, c.seen, frame);
        let bound = [
            c.eye,
            c.window,
            ground,
            rays.0,
            looks,
            c.shards,
            c.overwrites,
            pool,
            c.beams,
            c.seen,
            c.eye_head,
        ];
        if bound != c.bound {
            release(c.group);
            c.group = cast_group(g, &c.miss_built, bound);
            c.bound = bound;
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
                    c.window,
                    ground,
                    c.shards,
                    c.overwrites,
                    pool,
                ],
            );
            let px_bound = [
                c.eye,
                rays.0,
                c.beams,
                c.seen,
                c.raster.visibility_array,
                self.claimed.0,
                self.claimed.1,
            ];
            if px_bound != c.px_bound {
                release(c.px_group);
                c.px_group = px_group(g, &c.built, px_bound, c.marks);
                c.px_bound = px_bound;
            }
        }
        let [drawing, regions, instances] = c
            .generated
            .compacted()
            .map_or([c.light_none; 3], |p| [p.drawing, p.regions, p.instances]);
        let light_bound = [
            c.eye,
            c.window,
            ground,
            looks,
            c.shards,
            c.overwrites,
            pool,
            light_map.0,
            rays.0,
            drawing,
            regions,
            instances,
        ];
        let grouped = [
            c.light_list,
            c.raster.light_visibility(),
            c.raster.light.boxes,
        ];
        if light_bound != c.light_bound || grouped != c.light_grouped {
            for group in c.light_groups {
                release(group);
            }
            c.light_grouped = grouped;
            c.light_groups = light_groups(
                g,
                [
                    &c.light_built,
                    &c.miss_built,
                    &c.raster.boxes_built,
                    &c.light_miss_built,
                    &c.light_mark_built,
                    &c.light_mark_drawn_built,
                    &c.light_reach_built,
                    &c.light_detail_built,
                    &c.light_beam_built,
                    &c.light_sample_built,
                ],
                light_bound,
                [
                    c.seen,
                    c.light_moved,
                    c.light_list,
                    c.raster.light.boxes,
                    c.raster.light_visibility(),
                    c.raster.light.args,
                    c.light_misses,
                    c.light_args,
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

    /// The eye's windows folded over their bricks where their entries moved
    /// (`joined_cs`): ahead of the frame, as the fold outlives it.
    pub fn fold(&mut self, ctx: &mut RenderContext) -> bool {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(GpuSpanName::fill);
        let Some(c) = self.cast.as_mut().filter(|c| c.refold && c.group != 0) else {
            return false;
        };
        let pass = g.compute_pass(encoder, stamp.pass());
        g.set_bind_group(pass, 0, c.group, &[]);
        g.set_pipeline(pass, c.joined_built.pipeline);
        // A level a dispatch, each over the one under it, its level the
        // dispatch's depth (`joined_cs`).
        for l in 1..=window::JOINED_LEVELS {
            let blocks = (window::ACROSS >> l) * (window::ACROSS >> l);
            g.dispatch(
                pass,
                (blocks * window::EYE_WINDOWS).div_ceil(compute::JOINED_WORKGROUP[0]),
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
            // reading what the ones before wrote (pxl-spec-core `observed`).
            // A pass a dispatch, each timed in its own span, so the cast's
            // parts are read apart: `cast` holds the forgetting alone; the
            // samples' boxes drawn between the samples and the pixels, which
            // take the visible sample (ray/raster.rs, span `sraster`). Where
            // only the jitter moved (`FrameKind::px`), the beam rays, the
            // cells' starts and the samples stand, and the rest is cast again
            // off them.
            let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::cast).pass());
            g.set_bind_group(pass, 0, c.group, &[]);
            if c.forgets != observed::FORGETS_NONE {
                g.set_pipeline(pass, c.forget_built.pipeline);
                g.dispatch(
                    pass,
                    observed::ENTRIES.div_ceil(compute::FORGET_WORKGROUP[0]),
                    1,
                    1,
                );
            }
            g.end(pass);
            for (span, built, [x, y], w) in [
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
                (GpuSpanName::px, &c.built, c.size, compute::CAST_WORKGROUP),
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
                g.set_pipeline(pass, built.pipeline);
                g.dispatch(pass, x.div_ceil(w[0]), y.div_ceil(w[1]), 1);
                g.end(pass);
            }
            // The marked pixels' count where their walk reads it, then that
            // walk over their list, and over every pixel where it was full.
            g.copy_buf(
                encoder,
                (c.marks, word_bytes(ray::MISS_ARGS_CNT as usize)),
                (c.seen, word_bytes(observed::MARKS_CNT as usize)),
                4,
            );
            let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::miss).pass());
            g.set_bind_group(pass, 0, c.group, &[]);
            g.set_pipeline(pass, c.miss_list_built.pipeline);
            g.dispatch_indirect(pass, c.marks, word_bytes(ray::MISS_ARGS_LIST as usize));
            g.set_pipeline(pass, c.miss_built.pipeline);
            g.dispatch_indirect(pass, c.marks, word_bytes(ray::MISS_ARGS_WHOLE as usize));
            g.end(pass);
        }
        // The decor generated over the rays, before the light's cast, which
        // marks where it stands (`light_mark_drawn_cs`).
        if generates {
            let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::generate).pass());
            c.generated.dispatch(g, pass);
            g.end(pass);
        }
        if c.light_cast {
            // The reads moved with the box, a tile of it at a time: each
            // tile's word the word of the tile that held its world texels
            // under the corner before, copied out and back in, which the
            // detail's pass then sets to none where the box slid onto it
            // (`light_detail_cs`).
            if c.light_detail_moved && c.light_corner != c.light_corner_was {
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
            // The beam, the samples, their boxes drawn, the slots taking
            // their visible sample, a workgroup a tile (pxl-spec-core
            // `light::TILE`), and the rest walked off their list as the eye's
            // missed pixels are: each a pass timed in its own span.
            let stamp = ctx.stamp(GpuSpanName::light_beam).pass();
            let pass = g.compute_pass(encoder, stamp);
            // The tiles the box slid onto, and those whose eye moved past the
            // margin of what their walks asked, set back to be read anew
            // first, a thread a tile (`light_detail_cs`).
            if c.light_detail_moved {
                let n = light::tiles(shadow::SIZE);
                let w = compute::LIGHT_DETAIL_WORKGROUP;
                g.set_bind_group(pass, 0, c.light_groups[7], &[]);
                g.set_pipeline(pass, c.light_detail_built.pipeline);
                g.dispatch(pass, n.div_ceil(w[0]), n.div_ceil(w[1]), 1);
            }
            // The tiles its receivers read marked, timed with the beam:
            // where the eye's rays meet the world, a thread a brick of pixels
            // `light::MARK` a side (`light_mark_cs`), and where the decor and
            // the actors drawn stand, a thread each (`light_mark_drawn_cs`);
            // then those they read, a thread a tile (`light_reach_cs`).
            if c.light_world {
                c.light_world = false;
                g.set_bind_group(pass, 0, c.light_groups[4], &[]);
                g.set_pipeline(pass, c.light_mark_built.pipeline);
                let w = compute::LIGHT_MARK_WORKGROUP;
                let [x, y] = c.size.map(|v| v.div_ceil(light::MARK));
                g.dispatch(pass, x.div_ceil(w[0]), y.div_ceil(w[1]), 1);
            }
            g.set_bind_group(pass, 0, c.light_groups[5], &[]);
            g.set_pipeline(pass, c.light_mark_drawn_built.pipeline);
            let cap = c.generated.compacted().map_or(0, |p| p.cap);
            let w = compute::LIGHT_MARK_DRAWN_WORKGROUP;
            g.dispatch(pass, (cap + light::ACTORS as u32).div_ceil(w[0]), 1, 1);
            g.set_bind_group(pass, 0, c.light_groups[6], &[]);
            g.set_pipeline(pass, c.light_reach_built.pipeline);
            let w = compute::LIGHT_REACH_WORKGROUP;
            let n = light::tiles(shadow::SIZE);
            g.dispatch(pass, n.div_ceil(w[0]), n.div_ceil(w[1]), 1);
            // The beam by its entry of its own, which tells the margin of
            // what its walks ask (`light_beam_cs`), then the cells' starts by
            // the eye's pass over the light's world's group, which hands it
            // the light's head. Each as many workgroups as the reach left it
            // (`light::ARGS_WORDS`).
            let args = |w: u32| word_bytes(w as usize);
            g.set_bind_group(pass, 0, c.light_groups[8], &[]);
            g.set_pipeline(pass, c.light_beam_built.pipeline);
            g.dispatch_indirect(pass, c.light_args, args(light::ARGS_BEAM));
            g.set_bind_group(pass, 0, c.light_groups[1], &[]);
            g.set_pipeline(pass, c.cells_built.pipeline);
            g.dispatch_indirect(pass, c.light_args, args(light::ARGS_CELLS));
            g.end(pass);
            let stamp = ctx.stamp(GpuSpanName::light_sample).pass();
            let pass = g.compute_pass(encoder, stamp);
            g.set_bind_group(pass, 0, c.light_groups[9], &[]);
            g.set_pipeline(pass, c.light_sample_built.pipeline);
            g.dispatch_indirect(pass, c.light_args, args(light::ARGS_SAMPLE));
            g.end(pass);
            let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::light_raster).pass());
            g.set_bind_group(pass, 0, c.light_groups[2], &[]);
            g.set_pipeline(pass, c.raster.boxes_built.pipeline);
            g.dispatch_indirect(pass, c.light_args, args(light::ARGS_SAMPLE_BOX));
            g.end(pass);
            c.raster.light_draw(ctx);
            let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::light_slot).pass());
            g.set_bind_group(pass, 0, c.light_groups[0], &[]);
            g.set_pipeline(pass, c.light_built.pipeline);
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
            let pass = g.compute_pass(encoder, ctx.stamp(GpuSpanName::light_walk).pass());
            g.set_bind_group(pass, 0, c.light_groups[3], &[]);
            g.set_pipeline(pass, c.light_miss_built.pipeline);
            let list = word_bytes(ray::MISS_ARGS_LIST as usize);
            g.dispatch_indirect(pass, c.light_misses, list);
            g.end(pass);
        }
        let counted = Counted {
            seen: c.seen,
            args: [c.marks, c.light_misses],
            boxes: [c.raster.args, c.raster.light.args],
        };
        c.tally.copy(g, encoder, &counted);
    }

    /// The light's cast made where `wanted`, shadows first cast, before the
    /// frame: its list grown (`light::list_words`) and its raster's buffers
    /// and targets (`Raster::light_made`), what only a map cast needs. Its
    /// passes are made with the eye's (`make`), the beam's, the cells' and
    /// the samples' the eye's own; ready once its buffers are (`light_ready`).
    pub fn light_made(&mut self, g: &Gpu, wanted: bool) {
        let Some(c) = self.cast.as_mut() else { return };
        if !wanted || c.light_ready {
            return;
        }
        if !c.light_list_whole {
            release(c.light_list);
            c.light_list = light_list(g, true);
            c.light_list_whole = true;
        }
        c.light_ready = c.raster.light_made(g, shadow::SIZE);
    }

    /// The counts copied last frame mapped back (`Tally::back`).
    pub fn tally_back(&mut self, g: &Gpu) {
        if let Some(c) = self.cast.as_mut() {
            c.tally.back(g);
        }
    }

    /// The counts' sums since boot (ray/tally.rs `RAY_SUMS`).
    pub fn tally_sums(&self) -> [f64; tally::RAY_SUMS] {
        Tally::sums()
    }

    pub fn light_casts(&self) -> bool {
        self.cast.as_ref().is_some_and(|c| c.light_cast)
    }

    pub fn generates(&self) -> bool {
        self.cast.as_ref().is_some_and(|c| c.generated.works())
    }

    pub fn generated(&self) -> Option<Drawn> {
        self.cast.as_ref().and_then(|c| c.generated.drawn())
    }

    /// The frame was dropped with its cast and its image: the rays and the
    /// image hold older ones.
    pub fn on_dropped(&mut self) {
        if let Some(c) = self.cast.as_mut() {
            // What it was to forget, it did not: the next cast forgets it
            // too, every box and family as sent, as decor's generation holds
            // its edits again (`carry_edits`).
            if let Some(f) = c.sent.take() {
                c.forget |= f.all;
                for (m, was) in c.moved.iter_mut().zip(f.moved) {
                    *m |= was;
                }
                for &b in &f.boxes[..f.holding] {
                    hold(&mut c.boxes, &mut c.holding, b);
                }
                for &first in &f.written[..f.writes] {
                    c.forget |= !note(&mut c.written, &mut c.writes, first);
                }
            }
            c.read.forget();
            c.light_content.forget();
            c.generated.on_dropped();
            c.tally.on_dropped();
        }
    }

    /// The frame was drawn: what its decor marked by is done with.
    pub fn on_drawn(&mut self) {
        if let Some(c) = self.cast.as_mut() {
            c.generated.drawn_frame();
        }
    }

    /// A frame's cast handed and never drawn: the edits its decor named,
    /// held for the next that is.
    pub fn carry_edits(&mut self, edits: &[[i32; 4]]) {
        if let Some(c) = self.cast.as_mut() {
            for &b in edits {
                hold(&mut c.boxes, &mut c.holding, b);
            }
            c.generated.carry_edits(edits);
        }
    }
}

/// The cast's group 0 over `bound`: the eye, the window and its fold
/// (`joined_cs` writes it, the cast reads it), the tiles' frames, the
/// surface's rays, the bricks' looks, the service area's index, the
/// overwrites, the ways' pool, the beam rays' lines and the cells' starts,
/// the observed pyramid, and the head (`WORLD_GROUP`): the eye's, or, over
/// the light's list and head, the shadow map's (`light_groups`).
fn cast_group(g: &Gpu, built: &Built, bound: [u32; 11]) -> u32 {
    let [eye, window, ground, rays, looks, shards, overwrites, pool, beams, seen, head] = bound;
    let entries = [
        buf_entry(0, eye),
        buf_entry(1, window),
        buf_entry(2, ground),
        view_entry(3, rays),
        buf_entry(4, looks),
        buf_entry(5, shards),
        buf_entry(6, overwrites),
        buf_entry(7, pool),
        buf_entry(8, beams),
        buf_entry(9, seen),
        buf_entry(HEAD_BINDING, head),
    ];
    g.bind_group(built.layouts[0], &entries)
}

/// The pixels' group 0 over `bound`: the eye, the rays, the beam's words,
/// the pyramid, which the marked pixels are listed in, the visibility buffer,
/// a one-layer array, and the words the marked are counted in (`marks`).
/// No world: a pixel walks nothing of its own (`cast_cs`).
fn px_group(g: &Gpu, built: &Built, bound: [u32; 7], marks: u32) -> u32 {
    let [eye, rays, beams, seen, visibility, changed_at, tiles] = bound;
    let entries = [
        buf_entry(0, eye),
        view_entry(3, rays),
        buf_entry(8, beams),
        buf_entry(9, seen),
        view_entry(13, visibility),
        buf_entry(14, marks),
        buf_entry(30, changed_at),
        buf_entry(31, tiles),
    ];
    g.bind_group(built.layouts[0], &entries)
}

const _: () = assert!(
    light::tiles(shadow::SIZE).is_multiple_of(light::COARSE),
    "the map's tiles are a whole number of squares a side (`light::coarse_square`)"
);

/// The workgroups of the passes that cast the light's tiles as a cast starts:
/// none, then each whole (`light::ARGS_WORDS`).
fn light_args() -> [u32; light::ARGS_WORDS as usize] {
    let across = |n: u32, w: [u32; 3]| [n.div_ceil(w[0]), n.div_ceil(w[1]), 1];
    let samples = shadow::SIZE / SAMPLE;
    let tiles = shadow::SIZE / light::TILE;
    let mut args = [0; light::ARGS_WORDS as usize];
    for (w, whole) in [
        (
            light::ARGS_BEAM,
            across(light::beam_across(shadow::SIZE), compute::BEAM_WORKGROUP),
        ),
        (
            light::ARGS_CELLS,
            across(light::cells_across(shadow::SIZE), compute::CELL_WORKGROUP),
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

/// The light's list, a word before shadows are cast, or `whole`: the beam's
/// lines and the cells' starts, the raster's samples and the misses past
/// them (`light::list_words`).
fn light_list(g: &Gpu, whole: bool) -> u32 {
    let words = if whole {
        light::list_words(shadow::SIZE) as usize
    } else {
        4
    };
    g.buf(
        word_bytes(words),
        buffer_usage::STORAGE
            | buffer_usage::COPY_DST
            | buffer_usage::INDIRECT
            | buffer_usage::COPY_SRC,
    )
}

/// The light's cast's groups 0, over `bound`: the eye, the window, the
/// tiles' frames, the bricks' looks, the service area's index, the
/// overwrites and the ways' pool, the cast's alone; the observed pyramid,
/// which the light's walks read and write as the eye's do; the clipmap; its
/// head, the beam's lines, the samples and the misses (`list`),
/// the raster's boxes, visibility buffer and args, the misses' dispatch and
/// the casting passes' workgroups (`light::ARGS_WORDS`), which the reach
/// writes; the eye's rays, which the mark reads, and the decor's drawing
/// words, regions and instances, which the drawn mark reads. The slots',
/// its world's (the cells' starts, the eye's pass, over `WORLD_GROUP`, its
/// list where the eye's beam rays' buffer lies and its head where the
/// eye's), its boxes' (the eye's raster's pass), the misses' walk's, the two
/// marks', the reach's, the detail's, the beam's and the samples'; the
/// boxes' none before its raster's buffers are made.
fn light_groups(
    g: &Gpu,
    [walk, world, boxes_built, miss, mark, mark_drawn, reach, detail, beam, sample]: [&Built; 10],
    bound: [u32; 12],
    [seen, head, list, boxes, visibility, args, misses, light_args]: [u32; 8],
) -> [u32; 10] {
    let [eye, window, ground, looks, shards, overwrites, pool, light, rays, drawing, regions, instances] =
        bound;
    let entries = [
        (0, buf_entry(0, eye)),
        (1, buf_entry(1, window)),
        (2, buf_entry(2, ground)),
        (3, view_entry(3, rays)),
        (4, buf_entry(4, looks)),
        (5, buf_entry(5, shards)),
        (6, buf_entry(6, overwrites)),
        (7, buf_entry(7, pool)),
        (9, buf_entry(9, seen)),
        (10, view_entry(10, light)),
        (HEAD_BINDING, buf_entry(HEAD_BINDING, head)),
        (12, buf_entry(12, list)),
        (14, buf_entry(14, boxes)),
        (15, view_entry(15, visibility)),
        (16, buf_entry(16, args)),
        (17, buf_entry(17, misses)),
        (18, buf_entry(18, drawing)),
        (19, buf_entry(19, regions)),
        (20, buf_entry(20, instances)),
        (21, buf_entry(21, light_args)),
    ];
    // Each the entries its layout declares (build.rs `compute`).
    let of = |declared: &[u32]| {
        entries
            .iter()
            .filter(|(b, _)| declared.chunks(3).any(|d| d[0] == *b))
            .map(|&(_, e)| e)
            .collect::<Vec<_>>()
    };
    [
        g.bind_group(walk.layouts[0], &of(&compute::LIGHT_GROUP)),
        cast_group(
            g,
            world,
            [
                eye, window, ground, rays, looks, shards, overwrites, pool, list, seen, head,
            ],
        ),
        // The raster's once its buffers are made.
        if boxes == 0 {
            0
        } else {
            g.bind_group(
                boxes_built.layouts[0],
                &[
                    buf_entry(0, eye),
                    buf_entry(8, list),
                    buf_entry(HEAD_BINDING, head),
                    buf_entry(14, boxes),
                    buf_entry(16, args),
                ],
            )
        },
        g.bind_group(miss.layouts[0], &of(&compute::LIGHT_MISS_GROUP)),
        g.bind_group(mark.layouts[0], &of(&compute::LIGHT_MARK_GROUP)),
        g.bind_group(mark_drawn.layouts[0], &of(&compute::LIGHT_MARK_DRAWN_GROUP)),
        g.bind_group(reach.layouts[0], &of(&compute::LIGHT_REACH_GROUP)),
        g.bind_group(detail.layouts[0], &of(&compute::LIGHT_DETAIL_GROUP)),
        g.bind_group(beam.layouts[0], &of(&compute::LIGHT_BEAM_GROUP)),
        g.bind_group(sample.layouts[0], &of(&compute::LIGHT_SAMPLE_GROUP)),
    ]
}
