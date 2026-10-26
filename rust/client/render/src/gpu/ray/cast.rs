// pxl-render/gpu/ray/cast.rs

//! The GPU's cast of the eye's rays into the world surface's rays, the
//! world's depth from the sun cast with them, and the decor grown over the
//! world (gpu/grown.rs): what each reads, made once, and their dispatches.

use pxl_spec_core::scatter::GROUPS;
use pxl_spec_core::window::{self, FRESH_CELL, FRESH_FIRST};
use pxl_spec_core::{beam, eye, observed, overwrite, ray, sun};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use super::raster::Raster;
use super::tally::{self, Tally};
use super::RaysPass;
use crate::frame::shader::record::{
    compute, source, within, SHADER_BEAM, SHADER_CAST, SHADER_CELLS, SHADER_FORGET, SHADER_JOINED,
    SHADER_MISS, SHADER_MISS_LIST, SHADER_SAMPLE, SHADER_SUN, SHADER_SUN_BEAM, SHADER_SUN_CELLS,
    SHADER_SUN_MISS,
};
use crate::gpu::api::{
    buffer_entry, buffer_usage, bytes, release, view_entry, word_bytes, write_buffer_bytes, Gpu,
    Stamp, DONE,
};
use crate::gpu::grown::{cut_boxes, hold, Drawn, Grown, Grows};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::read::Read;
use crate::gpu::renderer::{Cast, Overwrites};
use crate::gpu::shadow;
use crate::gpu::span_spec::{FrameKind, GpuSpanName};

/// What the GPU's cast reads; it writes the world surface's rays.
pub(super) struct CastBuffers {
    built: Built,
    /// The camera grid's beam rays (`beam_cs`, a thread a corner), where
    /// each cell's pixels start off them (`cells_cs`, a thread a cell of
    /// the screen), the samples walked from those starts (`sample_cs`, a
    /// thread a sample), and their buffer, over the cast's group
    /// (pxl-spec-core `beam`): a line a corner, a start a cell, a stop a
    /// sample, as the eye's words lay the frame's `grid`, none a pixel's;
    /// room for `beam_room` words
    /// (doc/minutes/20261022-a-cell-casts-a-beam.md).
    beam_built: Built,
    cells_built: Built,
    sample_built: Built,
    beams: u32,
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
    /// (`forget`), or the nodes over the eye's windows' slots that moved
    /// (`moved`, a bit a slot, against the slots as `slots` last held them)
    /// and over the boxes of what is placed or cut that changed (`boxes`,
    /// `holding` of them, as decor's growth holds them, gpu/grown.rs
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
    boxes: [[i32; 4]; observed::BOXES as usize],
    holding: usize,
    written: [u32; observed::WRITTEN as usize],
    writes: usize,
    sent: Option<Forgot>,
    slots: Vec<i32>,
    forgets: u32,
    cuts: [u32; eye::CUTS],
    /// The eye's windows folded over their squares (`joined_cs`), over its
    /// group, the window buffer's rows past their slot table
    /// (`window::JOINED_AT`), written where the windows' slots moved
    /// (`refold`); then copied into `joined_read` and mapped back
    /// (`read_back`), once a mapping is free, while `joined_back`; the
    /// mapping's own word where the guest's allocator puts it, as a
    /// square's is.
    joined_built: Built,
    /// The world's depth from the sun, a texel of the sun's map each
    /// (`sun_cs`), cast with the eye's rays where the shadows are on.
    sun_built: Built,
    /// The slots that took no pick walked apart (`sun_miss_cs`).
    sun_miss_built: Built,
    /// The sun's beam rays and its cells' starts (pxl-spec-core `sun::BEAM`),
    /// made where shadows are first cast, one a turn where the stream has
    /// room (`sun_made`).
    sun_beam_built: Built,
    sun_cells_built: Built,
    /// The sun's mark's group and its cast's (`sun_groups`), and the
    /// buffers they bind: made again exactly when one of them is another.
    sun_groups: [u32; 6],
    sun_bound: [u32; 7],
    /// Whether all the sun's cast is made (`sun_made`): its list, its beam's
    /// pipelines and its raster; before then no sun's map is cast.
    sun_ready: bool,
    /// The beam's lines, the samples and the misses (pxl-spec-core
    /// `sun::list_words`), made where shadows are first cast.
    sun_list: u32,
    sun_list_whole: bool,
    /// The misses' walk's dispatch and count, as the eye's are
    /// (pxl-spec-core `ray::MISS_ARGS_WORDS`).
    sun_misses: u32,
    /// What the sun's cast last read (`Read`): the light's frame
    /// (`Shadow::frame`), whether shadows are cast, the light box, the eye's
    /// place, quality and zooms, and whether the world under it moved; and
    /// whether this frame casts it, every slot.
    sun_read: Read,
    /// What it is handed beside the eye's words (pxl-spec-core
    /// `sun::MOVED_WORDS`).
    sun_moved: u32,
    sun_cast: bool,
    pub(super) joined_read: u32,
    pub(super) joined_mapping: Box<u32>,
    refold: bool,
    pub(super) joined_back: bool,
    /// Its group 0, and the buffers it binds (`cast_group`): made again
    /// exactly when one of them is another.
    group: u32,
    bound: [u32; 10],
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
    grown: Grown,
    /// What the last cast read (`Read`), and whether this frame's is the
    /// same: the rays and the decor grown over them stand, and nothing is
    /// cast.
    read: Read,
    /// The samples' stops handed to the rasterizer (ray/raster.rs), whose
    /// pick at each pixel the pixels read (`cast_cs`), over a group of
    /// their own (`pixel_group`): the world's group's bindings and the
    /// raster's picks (`pixel_bound`), made again exactly when one of them
    /// is another.
    raster: Raster,
    pixel_group: u32,
    pixel_bound: [u32; 11],
    /// What the sun's cast and the eye's recast, counted where the setting
    /// asks (ray/tally.rs).
    tally: Tally,
    /// The counts, the list and the beam's pipelines the sun's groups were
    /// made over, made again where one is another.
    sun_grouped: [u32; 6],
}

/// What a cast's head handed the pyramid's forgetting (pxl-spec-core
/// `observed::HEAD_WORDS`): whether all of it, the slots that moved, the
/// boxes and the families written; put back where its frame is dropped.
struct Forgot {
    all: bool,
    moved: [u32; observed::MOVED_WORDS as usize],
    boxes: [[i32; 4]; observed::BOXES as usize],
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

const _: () = assert!(
    within(compute::JOINED_GROUP, compute::MISS_GROUP)
        && within(compute::BEAM_GROUP, compute::MISS_GROUP)
        && within(compute::CELLS_GROUP, compute::MISS_GROUP)
        && within(compute::SAMPLE_GROUP, compute::MISS_GROUP)
        && within(compute::FORGET_GROUP, compute::MISS_GROUP)
        && within(compute::MISS_LIST_GROUP, compute::MISS_GROUP),
    "the fold, the beam, the cells, the samples, the forgetting and the list's walk bind the group the missed pixels' cast binds whole"
);
/// The cast's world's group, every kernel's over it bar the sun's and the
/// pixels': the one the missed pixels' cast binds whole. The sun's cast
/// binds its world and its clipmap (`SUN_GROUP`), past the floor's storage
/// buffers were they one group; the pixels' (`cast_cs`) read the raster's
/// picks beside the world, read-only (`pixel_group`).
const WORLD_GROUP: [u32; compute::MISS_GROUP.len()] = compute::MISS_GROUP;

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
        let cells_built = pipelines.build_compute(g, SHADER_CELLS, group);
        let sample_built = pipelines.build_compute(g, SHADER_SAMPLE, group);
        let joined_built = pipelines.build_compute(g, SHADER_JOINED, group);
        let miss_built = pipelines.build_compute(g, SHADER_MISS, group);
        let miss_list_built = pipelines.build_compute(g, SHADER_MISS_LIST, group);
        let forget_built = pipelines.build_compute(g, SHADER_FORGET, group);
        let sun_built = pipelines.build_compute(g, SHADER_SUN, &compute::SUN_GROUP);
        let sun_miss_built = pipelines.build_compute(g, SHADER_SUN_MISS, &compute::SUN_MISS_GROUP);
        let raster = Raster::new(g, pipelines);
        let tally = Tally::new(g);
        let buffer = |size: u32, usage: u32| g.buffer(size, usage);
        let storage = buffer_usage::STORAGE | buffer_usage::COPY_DST;
        let c = CastBuffers {
            built,
            beam_built,
            cells_built,
            sample_built,
            beams: 0,
            grid: beam::Grid::of_words([0.0; 2], [0; eye::GRID_WORDS]),
            beam_room: 0,
            miss_built,
            miss_list_built,
            marks: buffer(
                word_bytes(ray::MISS_ARGS_WORDS),
                storage | buffer_usage::INDIRECT | buffer_usage::COPY_SRC,
            ),
            forget_built,
            seen: buffer(word_bytes(observed::WORDS as usize), storage),
            frame: 0,
            forget: true,
            moved: [0; observed::MOVED_WORDS as usize],
            boxes: [[0; 4]; observed::BOXES as usize],
            holding: 0,
            written: [0; observed::WRITTEN as usize],
            writes: 0,
            sent: None,
            slots: Vec::new(),
            forgets: observed::FORGETS_NONE,
            cuts: [0; eye::CUTS],
            joined_built,
            sun_built,
            sun_miss_built,
            sun_beam_built: Built::default(),
            sun_cells_built: Built::default(),
            sun_groups: [0; 6],
            sun_bound: [0; 7],
            sun_ready: false,
            sun_list: sun_list(g, false),
            sun_list_whole: false,
            sun_misses: buffer(
                word_bytes(ray::MISS_ARGS_WORDS),
                storage | buffer_usage::INDIRECT | buffer_usage::COPY_SRC,
            ),
            sun_read: Read::default(),
            sun_moved: buffer(
                word_bytes(sun::MOVED_WORDS),
                buffer_usage::STORAGE | buffer_usage::COPY_DST,
            ),
            sun_cast: false,
            joined_read: buffer(
                word_bytes(window::EYE_JOINED_WORDS as usize),
                buffer_usage::MAP_READ | buffer_usage::COPY_DST,
            ),
            joined_mapping: Box::new(DONE),
            refold: false,
            joined_back: false,
            group: 0,
            bound: [0; 10],
            eye: buffer(
                word_bytes(eye::WORDS),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
            window: buffer(
                word_bytes((4 * window::JOINED_AT + window::EYE_JOINED_WORDS) as usize),
                storage | buffer_usage::COPY_SRC,
            ),
            shards: 0,
            shard_room: 0,
            size: [0; 2],
            overwrites: 0,
            overwrite_room: 0,
            overwrite_revision: None,
            grown: Grown::new(g, pipelines),
            read: Read::default(),
            raster,
            pixel_group: 0,
            pixel_bound: [0; 11],
            tally,
            sun_grouped: [0; 6],
        };
        self.cast = Some(c);
    }

    /// The rays for this frame into the world surface's (`rays`, its view,
    /// and its drawn pixels), and the world's depth from the sun into `sun`'s
    /// view: the GPU's cast where an eye casts, its eye handed over first.
    pub fn prepare(
        &mut self,
        g: &Gpu,
        cast: Option<Cast>,
        meshes: &[u32; GROUPS as usize],
        rays: (u32, [u32; 2]),
        sun_map: (u32, u32, sun::Starts),
    ) {
        // No cast, no world: no ray cast, none drawn and no decor grown.
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
                c.grown.off();
                c.read.forget();
                c.sun_read.forget();
                c.sun_cast = false;
            }
            return;
        };
        let c = self.cast.as_mut().or_bug(Errno::ENOTRECOVERABLE);
        // The cast reads exactly this, all of it here: a field the frame hands
        // the cast fails to destructure until it is read. The overwrites'
        // contents are their revision's. The decor it grows reads its own
        // (`Grown::prepare`), so a change to decor alone casts no ray again.
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
            sun_counts,
        } = &cast;
        let sun_counts = *sun_counts;
        let r = &mut c.read;
        r.begin();
        // The sun's words are its cast's alone (below).
        let eye_moved = r.read(&eye[..eye::SUN]);
        let window_moved = r.read(window);
        let shards_moved = r.read(shards);
        // A surface made again holds no rays.
        r.read(&[*room as u32, *revision, rays.0]);
        // And the ground's squares, written this frame or not.
        let fresh = self.ground.as_ref().map_or(0, |gb| gb.fresh_len);
        self.kind = if r.end() && fresh == 0 && c.group != 0 {
            FrameKind::standing
        } else {
            FrameKind::cast
        };
        xpute_kit::ensure!(
            cast.window.len() == (4 * window::JOINED_AT) as usize,
            ENOTRECOVERABLE,
            cast.window.len()
        );
        if window_moved {
            g.write_buffer(c.window, 0, bytes(cast.window));
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
            xpute_kit::ensure!(
                crate::frame::shader::floor::fits(size as u64, true),
                ENOTRECOVERABLE,
                need
            );
            (c.beams, c.beam_room) = (
                g.buffer(size, buffer_usage::STORAGE | buffer_usage::COPY_DST),
                need,
            );
        }
        let shards_grow = cast.shards.len() > c.shard_room;
        if shards_grow {
            if c.shards != 0 {
                release(c.shards);
            }
            c.shard_room = cast.shards.len();
            c.shards = g.buffer(
                word_bytes(4 * c.shard_room),
                buffer_usage::STORAGE | buffer_usage::COPY_DST,
            );
        }
        if shards_moved || shards_grow {
            // Read where the kernel holds it, one record however large.
            g.upload_buffer(c.shards, 0, bytes(cast.shards));
        }
        let o = &cast.overwrites;
        let overwrites_grow = o.room != c.overwrite_room;
        if overwrites_grow {
            if c.overwrites != 0 {
                release(c.overwrites);
            }
            c.overwrite_room = o.room;
            c.overwrites = g.buffer(
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
            if c.overwrite_revision == Some(o.since)
                && crate::gpu::api::room(listed(o.heap) + listed(o.data))
            {
                for &first in o.written {
                    g.write_buffer(
                        c.overwrites,
                        word_bytes(first as usize),
                        bytes(family(o.heap, first)),
                    );
                    g.write_buffer(
                        c.overwrites,
                        word_bytes(o.room + first as usize),
                        bytes(family(o.data, first)),
                    );
                }
            } else {
                g.upload_buffer(c.overwrites, 0, bytes(o.heap));
                g.upload_buffer(c.overwrites, word_bytes(o.room), bytes(o.data));
            }
            c.overwrite_revision = Some(o.revision);
        }
        // What the world's query reads moved: the windows, the squares the
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
        // they changed, as decor's growth is asked over them (gpu/grown.rs):
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
        // The eye's windows' slots that hold another square, or whose square
        // the turn handed: what is over them is asked again.
        c.cuts = cuts;
        let eye_rows = (window::EYE_WINDOWS * window::SLOTS) as usize;
        if window_moved {
            let rows = &cast.window[..4 * eye_rows];
            for k in 0..eye_rows {
                let row = &rows[4 * k..4 * k + 4];
                if c.slots.get(4 * k..4 * k + 4) != Some(row) {
                    c.moved[k / 32] |= 1 << (k % 32);
                }
            }
            c.slots.clear();
            c.slots.extend_from_slice(rows);
        }
        // A square handed moves the eye's slot that wants it, whichever
        // window's berth it is filled into.
        if let Some(gb) = self.ground.as_ref() {
            for f in &gb.handed[..fresh as usize] {
                let (cell, q) = (
                    f[FRESH_CELL as usize],
                    window::fresh_square([f[FRESH_FIRST as usize], f[FRESH_FIRST as usize + 1]]),
                );
                let w = cell.wrapping_sub(window::FIRST_CELL);
                let k = (w * window::SLOTS + window::slot(q)) as usize;
                let row = &cast.window[4 * k..4 * k + 4];
                if w < window::EYE_WINDOWS && [row[0], row[1]] == q {
                    c.moved[k / 32] |= 1 << (k % 32);
                }
            }
        }
        // The sun's map is cast whole, every slot, as the eye's rays are
        // (pxl-spec-core `sun`), where what it reads moved: the light's frame
        // or box, the eye's place, quality or zooms, which the zooms its rays
        // ask follow, or the world under it. The eye's look alone casts it
        // no ray, and a standing frame none. Nothing it cast is kept for
        // the next. None where no shadow is cast, nor before all its cast is
        // made (`sun_made`), its read let go until then.
        let shadows = cast.eye[eye::SUN..eye::SUN + eye::SUN_WORDS]
            .iter()
            .any(|&w| w != 0);
        let r = &mut c.sun_read;
        r.begin();
        r.read(&[sun_map.1, u32::from(shadows)]);
        r.read(&cast.eye[eye::SUN..eye::SUN + eye::SUN_WORDS]);
        r.read(&cast.eye[eye::ORIGIN..eye::STEPS]);
        r.read(&cast.eye[eye::TAU..eye::TAU + 1]);
        r.read(&cast.eye[eye::RANGE..eye::RANGE + 2]);
        r.mark(world_moved);
        let sun_moved = !r.end();
        if !c.sun_ready {
            c.sun_read.forget();
        }
        c.sun_cast = shadows && sun_moved && c.sun_ready;
        if c.sun_cast {
            // The head (pxl-spec-core `sun::MOVED_WORDS`).
            let mut head = [0u32; sun::MOVED_WORDS];
            head[sun::MOVED_COUNTED] = u32::from(sun_counts);
            let at = 4 * sun::STARTS_AT;
            head[at..at + 4 * sun::STARTS_VEC4S].copy_from_slice(&sun_map.2.words());
            g.write_buffer(c.sun_moved, 0, bytes(&head));
            g.write_buffer(c.sun_misses, 0, bytes(&ray::MISS_ARGS_EMPTY));
            c.raster.sun_prepare(g, &cast.eye, shadow::SIZE);
        }
        if eye_moved || c.sun_cast {
            g.write_buffer(c.eye, 0, bytes(&cast.eye));
        }
        if self.kind == FrameKind::cast {
            c.forgets = if c.forget {
                observed::FORGETS_ALL
            } else if c.moved.iter().any(|&m| m != 0) || c.holding != 0 || c.writes != 0 {
                observed::FORGETS_MOVED
            } else {
                observed::FORGETS_NONE
            };
            // The head before the frame's dispatches: what it forgets and over
            // which slots; forgotten whole, its first frame, else the next.
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
            g.write_buffer(c.seen, 0, bytes(&head));
            g.write_buffer(c.marks, 0, bytes(&ray::MISS_ARGS_EMPTY));
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
            sun: c.sun_cast,
            texels: shadow::SIZE * shadow::SIZE,
        };
        c.tally.begin(g, sun_counts, frame);
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
        ];
        if bound != c.bound {
            release(c.group);
            c.group = cast_group(g, &c.miss_built, bound);
            c.bound = bound;
        }
        if self.kind == FrameKind::cast {
            c.raster.prepare(
                g,
                &cast.eye,
                c.size,
                &c.grid,
                [c.eye, c.beams],
                [c.window, ground, c.shards, c.overwrites, pool],
            );
            let mut pixel_bound = [0; 11];
            pixel_bound[..10].copy_from_slice(&bound);
            pixel_bound[10] = c.raster.picks_array;
            if pixel_bound != c.pixel_bound {
                release(c.pixel_group);
                c.pixel_group = pixel_group(g, &c.built, pixel_bound, c.marks);
                c.pixel_bound = pixel_bound;
            }
        }
        let sun_bound = [
            c.eye,
            c.window,
            ground,
            c.shards,
            c.overwrites,
            pool,
            sun_map.0,
        ];
        let grouped = [
            c.tally.counts,
            c.sun_list,
            c.sun_beam_built.pipeline,
            c.sun_cells_built.pipeline,
            c.raster.sun_picks(),
            c.raster.sun.boxes,
        ];
        if sun_bound != c.sun_bound || grouped != c.sun_grouped {
            for group in c.sun_groups {
                release(group);
            }
            c.sun_grouped = grouped;
            c.sun_groups = sun_groups(
                g,
                [
                    &c.sun_built,
                    &c.sun_beam_built,
                    &c.sun_cells_built,
                    &c.raster.sun.sample_built,
                    &c.raster.sun.boxes_built,
                    &c.sun_miss_built,
                ],
                sun_bound,
                [
                    c.seen,
                    c.sun_moved,
                    c.sun_list,
                    c.tally.counts,
                    c.raster.sun.boxes,
                    c.raster.sun_picks(),
                    c.raster.sun.args,
                    c.sun_misses,
                ],
            );
            c.sun_bound = sun_bound;
        }
        let fresh_squares = self.ground.as_ref().map_or(0, |gb| gb.fresh);
        let world = [
            c.eye,
            c.window,
            ground,
            c.shards,
            c.overwrites,
            pool,
            fresh_squares,
        ];
        let grows = Grows {
            window: cast.window,
            cuts: cast.eye[eye::CARVE..eye::CARVE + eye::CUTS]
                .try_into()
                .or_bug(Errno::ENOTRECOVERABLE),
            filled: fresh,
            eye: &cast.eye,
        };
        c.grown.prepare(g, cast.decor, grows, meshes, world);
        let eye = c.eye;
        self.regroup(g, [rays.0, eye]);
    }

    /// The eye's windows folded over their squares where their slots moved
    /// (`joined_cs`): ahead of the frame, as the fold outlives it, and then
    /// mapped back (`read_back`).
    pub fn fold(&mut self, g: &Gpu, encoder: u32, stamp: &mut Stamp) -> bool {
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
        (c.refold, c.joined_back) = (false, true);
        true
    }

    /// The cast where the frame casts, the world's depth from the sun with
    /// it where `sun`, then the decor grown over the world where it moves:
    /// each a pass of its own over the cast's group, timed in its own span,
    /// so what each costs is read apart; the sun's in the shadow's, with
    /// the map's pass, which is not drawn at the world's pixels
    /// (`PER_PIXEL`).
    pub fn dispatch(&mut self, g: &Gpu, encoder: u32, stamps: &mut [Stamp]) {
        let grows = self.grows();
        let Some(c) = self.cast.as_mut() else { return };
        if self.kind == FrameKind::cast {
            // The observed pyramid forgotten first where it is; then the
            // beam rays writing it, a corner each; each cell's start off them;
            // the samples writing it, each from its cell's start; their
            // blocks drawn as boxes, each pixel's nearest kept; the pixels
            // off that, or reading it from their cell's start, marking the
            // pixels that missed; and those again,
            // writing it, a thread a drawn pixel, each but the marked
            // returning at once. One pass, a dispatch each, each
            // reading what the ones before wrote (pxl-spec-core `observed`).
            // A pass a dispatch, each timed in its own span, so the cast's
            // parts are read apart: `cast` holds the forgetting alone; the
            // samples' boxes drawn between the samples and the pixels, which
            // take the raster's pick (ray/raster.rs, span `sraster`).
            let pass = g.compute_pass(encoder, stamps[GpuSpanName::cast as usize].pass());
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
                    GpuSpanName::cells,
                    &c.cells_built,
                    c.grid.span,
                    compute::CELLS_WORKGROUP,
                ),
                (
                    GpuSpanName::sample,
                    &c.sample_built,
                    c.grid.samples,
                    compute::SAMPLE_WORKGROUP,
                ),
                (
                    GpuSpanName::pixel,
                    &c.built,
                    c.size,
                    compute::CAST_WORKGROUP,
                ),
            ] {
                let mut group = c.group;
                if span == GpuSpanName::pixel {
                    c.raster.draw(g, encoder, stamps, &c.grid);
                    group = c.pixel_group;
                }
                let pass = g.compute_pass(encoder, stamps[span as usize].pass());
                g.set_bind_group(pass, 0, group, &[]);
                g.set_pipeline(pass, built.pipeline);
                g.dispatch(pass, x.div_ceil(w[0]), y.div_ceil(w[1]), 1);
                g.end(pass);
            }
            // The marked pixels' count where their walk reads it, then that
            // walk over their list, and over every pixel where it was full.
            g.copy_buffer(
                encoder,
                (c.marks, word_bytes(ray::MISS_ARGS_COUNT as usize)),
                (c.seen, word_bytes(observed::MARKS_COUNT as usize)),
                4,
            );
            let pass = g.compute_pass(encoder, stamps[GpuSpanName::miss as usize].pass());
            g.set_bind_group(pass, 0, c.group, &[]);
            g.set_pipeline(pass, c.miss_list_built.pipeline);
            g.dispatch_indirect(pass, c.marks, word_bytes(ray::MISS_ARGS_LIST as usize));
            g.set_pipeline(pass, c.miss_built.pipeline);
            g.dispatch_indirect(pass, c.marks, word_bytes(ray::MISS_ARGS_WHOLE as usize));
            g.end(pass);
        }
        if c.sun_cast {
            // The beam, the samples, their boxes drawn, the slots taking
            // their pick, a workgroup a tile (pxl-spec-core `sun::TILE`),
            // and the rest walked off their list as the eye's missed pixels
            // are: each a pass timed in its own span.
            let stamp = stamps[GpuSpanName::sun_beam as usize].pass();
            let pass = g.compute_pass(encoder, stamp);
            g.set_bind_group(pass, 0, c.sun_groups[1], &[]);
            g.set_pipeline(pass, c.sun_beam_built.pipeline);
            let n = sun::beam_across(shadow::SIZE);
            let w = compute::SUN_BEAM_WORKGROUP;
            g.dispatch(pass, n.div_ceil(w[0]), n.div_ceil(w[1]), 1);
            g.set_bind_group(pass, 0, c.sun_groups[2], &[]);
            g.set_pipeline(pass, c.sun_cells_built.pipeline);
            let n = sun::cells_across(shadow::SIZE);
            let w = compute::SUN_CELLS_WORKGROUP;
            g.dispatch(pass, n.div_ceil(w[0]), n.div_ceil(w[1]), 1);
            g.end(pass);
            let n = shadow::SIZE / sun::SAMPLE;
            let stamp = stamps[GpuSpanName::sun_sample as usize].pass();
            let pass = g.compute_pass(encoder, stamp);
            g.set_bind_group(pass, 0, c.sun_groups[3], &[]);
            g.set_pipeline(pass, c.raster.sun.sample_built.pipeline);
            let w = compute::SUN_SAMPLE_WORKGROUP;
            g.dispatch(pass, n.div_ceil(w[0]), n.div_ceil(w[1]), 1);
            g.end(pass);
            let stamp = &mut stamps[GpuSpanName::sun_raster as usize];
            let pass = g.compute_pass(encoder, stamp.pass());
            g.set_bind_group(pass, 0, c.sun_groups[4], &[]);
            g.set_pipeline(pass, c.raster.sun.boxes_built.pipeline);
            let w = compute::SUN_BOXES_WORKGROUP;
            g.dispatch(pass, n.div_ceil(w[0]), n.div_ceil(w[1]), 1);
            g.end(pass);
            c.raster.sun_draw(g, encoder, stamp);
            let pass = g.compute_pass(encoder, stamps[GpuSpanName::sun_pick as usize].pass());
            g.set_bind_group(pass, 0, c.sun_groups[0], &[]);
            g.set_pipeline(pass, c.sun_built.pipeline);
            let tiles = shadow::SIZE / sun::TILE;
            g.dispatch(pass, tiles, tiles, 1);
            g.end(pass);
            // Their count where their walk reads it, then that walk.
            g.copy_buffer(
                encoder,
                (c.sun_misses, word_bytes(ray::MISS_ARGS_COUNT as usize)),
                (
                    c.sun_list,
                    word_bytes(sun::misses_count(shadow::SIZE) as usize),
                ),
                4,
            );
            let pass = g.compute_pass(encoder, stamps[GpuSpanName::sun_walk as usize].pass());
            g.set_bind_group(pass, 0, c.sun_groups[5], &[]);
            g.set_pipeline(pass, c.sun_miss_built.pipeline);
            let list = word_bytes(ray::MISS_ARGS_LIST as usize);
            g.dispatch_indirect(pass, c.sun_misses, list);
            g.end(pass);
        }
        c.tally.copy(g, encoder);
        if grows {
            let pass = g.compute_pass(encoder, stamps[GpuSpanName::grow as usize].pass());
            c.grown.dispatch(g, pass);
            g.end(pass);
        }
    }

    /// The sun's cast made where `wanted`, shadows first cast, before the
    /// frame: its list grown (`sun::list_words`), then its beam's pipelines
    /// and its raster's (`Raster::sun_made`), one a turn where the stream
    /// has room for its module; ready once all are (`sun_ready`).
    pub fn sun_made(&mut self, g: &Gpu, pipelines: &mut Pipelines, wanted: bool) {
        let Some(c) = self.cast.as_mut() else { return };
        if !wanted || c.sun_ready {
            return;
        }
        if !c.sun_list_whole {
            release(c.sun_list);
            c.sun_list = sun_list(g, true);
            c.sun_list_whole = true;
        }
        let room = |kind: u32| crate::gpu::api::room(source(kind).len() + 4096);
        if c.sun_beam_built.pipeline == 0 {
            if room(SHADER_SUN_BEAM) {
                c.sun_beam_built =
                    pipelines.build_compute(g, SHADER_SUN_BEAM, &compute::SUN_BEAM_GROUP);
            }
        } else if c.sun_cells_built.pipeline == 0 {
            if room(SHADER_SUN_CELLS) {
                c.sun_cells_built =
                    pipelines.build_compute(g, SHADER_SUN_CELLS, &compute::SUN_CELLS_GROUP);
            }
        } else {
            c.sun_ready = c.raster.sun_made(g, pipelines, true, shadow::SIZE);
        }
    }

    /// The counts copied last frame mapped back (`Tally::back`).
    pub fn tally_back(&mut self, g: &Gpu) {
        if let Some(c) = self.cast.as_mut() {
            c.tally.back(g);
        }
    }

    /// The counts' sums since boot (ray/tally.rs `SUN_SUMS`).
    pub fn tally_sums(&self) -> [f64; tally::SUN_SUMS] {
        self.cast
            .as_ref()
            .map_or([0.0; tally::SUN_SUMS], |c| c.tally.sums())
    }

    pub fn sun_casts(&self) -> bool {
        self.cast.as_ref().is_some_and(|c| c.sun_cast)
    }

    pub fn grows(&self) -> bool {
        self.cast.as_ref().is_some_and(|c| c.grown.works())
    }

    pub fn grown(&self) -> Option<Drawn> {
        self.cast.as_ref().and_then(|c| c.grown.drawn())
    }

    /// The frame was dropped with its cast and its image: the rays and the
    /// image hold older ones.
    pub fn dropped(&mut self) {
        if let Some(c) = self.cast.as_mut() {
            // What it was to forget, it did not: the next cast forgets it
            // too, every box and family as sent, as decor's growth holds
            // its edits again (`unconsumed`).
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
            c.sun_read.forget();
            c.grown.dropped();
            c.tally.dropped();
        }
    }

    /// The frame was drawn: what its decor marked by is done with.
    pub fn drawn(&mut self) {
        if let Some(c) = self.cast.as_mut() {
            c.grown.drawn_frame();
        }
    }

    /// A frame's cast handed and never drawn: the edits its decor named,
    /// held for the next that is.
    pub fn unconsumed(&mut self, edits: &[[i32; 4]]) {
        if let Some(c) = self.cast.as_mut() {
            for &b in edits {
                hold(&mut c.boxes, &mut c.holding, b);
            }
            c.grown.unconsumed(edits);
        }
    }
}

/// The cast's group 0 over `bound`: the eye, the window and its fold
/// (`joined_cs` writes it, the cast reads it), the tiles' frames, the
/// surface's rays, the squares' looks, the service area's index, the
/// overwrites, the ways' pool, the beam rays' lines and the cells' starts,
/// and the observed pyramid.
fn cast_group(g: &Gpu, built: &Built, bound: [u32; 10]) -> u32 {
    let [eye, window, ground, rays, looks, shards, overwrites, pool, beams, seen] = bound;
    let entries = [
        buffer_entry(0, eye),
        buffer_entry(1, window),
        buffer_entry(2, ground),
        view_entry(3, rays),
        buffer_entry(4, looks),
        buffer_entry(5, shards),
        buffer_entry(6, overwrites),
        buffer_entry(7, pool),
        buffer_entry(8, beams),
        buffer_entry(9, seen),
    ];
    g.bind_group(built.layouts[0], &entries)
}

/// The pixels' group 0 over `bound`: the world's group's (`cast_group`),
/// read-only bar the pyramid, which the marked pixels are listed in, the
/// raster's picks, a one-layer array, and the words the marked are counted
/// in (`marks`).
fn pixel_group(g: &Gpu, built: &Built, bound: [u32; 11], marks: u32) -> u32 {
    let [eye, window, ground, rays, _looks, shards, overwrites, pool, beams, seen, picks] = bound;
    let entries = [
        buffer_entry(0, eye),
        buffer_entry(1, window),
        buffer_entry(2, ground),
        view_entry(3, rays),
        buffer_entry(5, shards),
        buffer_entry(6, overwrites),
        buffer_entry(7, pool),
        buffer_entry(8, beams),
        buffer_entry(9, seen),
        view_entry(13, picks),
        buffer_entry(14, marks),
    ];
    g.bind_group(built.layouts[0], &entries)
}

/// The sun's list, a word before shadows are cast, or `whole`: the beam's
/// lines and the cells' starts, the raster's samples and the misses past
/// them (`sun::list_words`).
fn sun_list(g: &Gpu, whole: bool) -> u32 {
    let words = if whole {
        sun::list_words(shadow::SIZE) as usize
    } else {
        4
    };
    g.buffer(
        word_bytes(words),
        buffer_usage::STORAGE | buffer_usage::COPY_DST | buffer_usage::INDIRECT,
    )
}

/// The sun's cast's groups 0, over `bound`: the eye, the window, the
/// tiles' frames, the service area's index, the overwrites and the ways'
/// pool, the cast's alone; the observed pyramid, which the sun's walks read
/// and write as the eye's do; the clipmap; its head (`moved`), the beam's
/// lines, the samples and the misses (`list`), the counts (ray/tally.rs),
/// the raster's boxes, picks and args, and the misses' dispatch; the
/// slots', the beam's, the cells', the samples', the boxes' and the
/// misses' walk's, each none before its pipeline and buffers are made.
fn sun_groups(
    g: &Gpu,
    [walk, beam, cells, sample, boxes_built, miss]: [&Built; 6],
    bound: [u32; 7],
    [seen, moved, list, counts, boxes, picks, args, misses]: [u32; 8],
) -> [u32; 6] {
    let [eye, window, ground, shards, overwrites, pool, sun] = bound;
    let entries = [
        (0, buffer_entry(0, eye)),
        (1, buffer_entry(1, window)),
        (2, buffer_entry(2, ground)),
        (5, buffer_entry(5, shards)),
        (6, buffer_entry(6, overwrites)),
        (7, buffer_entry(7, pool)),
        (9, buffer_entry(9, seen)),
        (10, view_entry(10, sun)),
        (11, buffer_entry(11, moved)),
        (12, buffer_entry(12, list)),
        (13, buffer_entry(13, counts)),
        (14, buffer_entry(14, boxes)),
        (15, view_entry(15, picks)),
        (16, buffer_entry(16, args)),
        (17, buffer_entry(17, misses)),
    ];
    // Each the entries its layout declares (build.rs `compute`).
    let of = |declared: &[u32]| {
        entries
            .iter()
            .filter(|(b, _)| declared.chunks(3).any(|d| d[0] == *b))
            .map(|&(_, e)| e)
            .collect::<Vec<_>>()
    };
    let made = |b: &Built, declared: &[u32]| {
        if b.pipeline == 0 {
            0
        } else {
            g.bind_group(b.layouts[0], &of(declared))
        }
    };
    [
        g.bind_group(walk.layouts[0], &of(&compute::SUN_GROUP)),
        made(beam, &compute::SUN_BEAM_GROUP),
        made(cells, &compute::SUN_CELLS_GROUP),
        // The raster's once its buffers are made, a turn after its pipelines.
        if boxes == 0 {
            0
        } else {
            made(sample, &compute::SUN_SAMPLE_GROUP)
        },
        if boxes == 0 {
            0
        } else {
            made(boxes_built, &compute::SUN_BOXES_GROUP)
        },
        g.bind_group(miss.layouts[0], &of(&compute::SUN_MISS_GROUP)),
    ]
}
