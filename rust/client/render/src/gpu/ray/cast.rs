// pxl-render/gpu/ray/cast.rs

//! The GPU's cast of the eye's rays into the world surface's rays, the
//! world's depth from the sun cast with them, and the decor grown over the
//! world (gpu/grown.rs): what each reads, made once, and their dispatches.

use pxl_spec_core::scatter::GROUPS;
use pxl_spec_core::window::{self, FRESH_CELL, FRESH_FIRST};
use pxl_spec_core::{beam, eye, observed, proxied, proxy, sun};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use super::{ProxyBindings, RaysPass};
use crate::frame::shader::record::{
    compute, within, SHADER_BEAM, SHADER_CAST, SHADER_CELLS, SHADER_FORGET, SHADER_JOINED,
    SHADER_MISS, SHADER_ORACLE, SHADER_PROXIED, SHADER_PROXIED_BEAM, SHADER_PROXIED_CHECK,
    SHADER_PROXIED_ENTRY, SHADER_PROXIED_MISS, SHADER_PROXIED_ROUTE, SHADER_PROXIED_WRITE,
    SHADER_SAMPLE, SHADER_SUN, SHADER_SUN_MARK,
};
use crate::gpu::api::{
    buffer_entry, buffer_usage, bytes, release, view_entry, word_bytes, write_buffer_bytes, Gpu,
    Stamp, DONE,
};
use crate::gpu::grown::{Drawn, Grown, Grows};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::read::Read;
use crate::gpu::renderer::{Cast, Overwrites};
use crate::gpu::shadow;
use crate::gpu::span_spec::{FrameKind, GpuSpanName};
use xpute_guest::mem::off_of;

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
    /// pixel's; the pixels the cast's reading walk missed walked again
    /// (`miss_cs`, a thread a drawn pixel), each told by the mark its own
    /// first ray word holds (pxl-spec-core `ray::MISS`); the pyramid forgotten (`forget_cs`) where
    /// what the world's query reads changed since the last cast: all of it
    /// (`forget`), or the nodes over the eye's windows' slots that moved
    /// (`moved`, a bit a slot, against the slots as `slots` last held them);
    /// as this frame's cast does (`forgets`, pxl-spec-core
    /// `observed::FORGETS`). `frame` numbers the casts since it was last
    /// forgotten whole, and `cuts` are the preview's cuts it was asked with.
    miss_built: Built,
    forget_built: Built,
    seen: u32,
    frame: u32,
    forget: bool,
    moved: [u32; observed::MOVED_WORDS as usize],
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
    sun_mark_built: Built,
    /// The sun's mark's group and its cast's (`sun_groups`), and the
    /// buffers they bind: made again exactly when one of them is another.
    sun_groups: [u32; 2],
    sun_bound: [u32; 7],
    /// The tiles the mark lists, and the cast's dispatch over them
    /// (pxl-spec-core `sun::LIST_AT`).
    sun_list: u32,
    /// What the sun's cast last read beside the eye (`Read`): the light's
    /// frame (`Shadow::frame`) and whether the world under it moved past
    /// the nodes it names, where its clipmap keeps nothing; the eye's
    /// place, quality and zooms and the light box, where it is cast keeping
    /// what stands (`sun_at`); the box's corner the clipmap holds the world at;
    /// and whether this frame casts it.
    sun_read: Read,
    sun_at: Read,
    sun_kept: Option<[i32; 2]>,
    /// The nodes the world's query answers otherwise since the sun's map was
    /// last cast, as its cast binds them (pxl-spec-core `sun::MOVED_WORDS`).
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
    oracle: Oracle,
    proxied: Proxied,
}

/// The cast a rasterized proxy starts (pxl-spec-core `proxied`, pxl-shader
/// proxied.rs), where the eye asks for it (`eye::PROXIED`) and the proxy's
/// raster ran this frame (ray/proxy.rs): each box's entry found, the
/// frame's route laid, and the proxied cast's or the cast as built's
/// dispatches run off `args` as the route laid them, each the workgroups
/// `sizes` gives it, the pixels read the observed pyramid and those it
/// could not answer walked again, or each walked once writing it where the
/// eye asks for that (`eye::PROXIED_WRITE`, `write`), or the cast as built
/// with no sample and each pixel from its cell's beam line no sooner than
/// its box (`eye::PROXIED_BEAM`, `beam`); and with the steps
/// shown, the check against the eye's walk, into the oracle's counts past
/// its own. The cut's counts copied into `header`, a uniform, ahead of the
/// pass, as its passes bind the most storage a device must give. Its
/// groups, over `bound`, made again exactly when one of them is another.
struct Proxied {
    entry: Built,
    route: Built,
    pixels: Built,
    miss: Built,
    check: Built,
    write: Built,
    beam: Built,
    groups: [u32; 7],
    bound: [u32; 15],
    sizes: u32,
    args: u32,
    header: u32,
    wanted: bool,
    writes: bool,
    beams: bool,
}

/// The counts `oracle_cs` keeps, by its words (pxl-shader cast.rs `STOPS`
/// on): the stops, how its walk again went, and the walk from the eye; then
/// the proxied cast's check's (pxl-spec-core `proxied::STOPS` on).
pub const ORACLE_COUNTS: usize = ORACLE_OWN + proxied::COUNTS as usize;
/// The oracle's own counts (pxl-shader cast.rs `ORACLE_BASE`).
const ORACLE_OWN: usize = 21;

/// The oracle's measurement, where the eye asks for it (pxl-spec-core
/// `eye::oracle`, pxl-shader cast.rs `oracle_cs`): its pipeline, its group
/// over the eye, the rays and the observed pyramid (`bound`) and its
/// counts, made again exactly when one of them is another. The counts are
/// cleared each frame it runs with the steps shown, copied into `read`
/// where no mapping of it is out (`copied`), mapped back into `landed`
/// (`mapped`), and summed into `sums` as they land: the frames read, then
/// each count, since boot.
struct Oracle {
    built: Built,
    group: u32,
    bound: [u32; 3],
    on: bool,
    steps: bool,
    counts: u32,
    read: u32,
    landed: Box<[u32; ORACLE_COUNTS]>,
    mapping: Box<u32>,
    copied: bool,
    mapped: bool,
    sums: [f64; ORACLE_COUNTS + 1],
}

/// The nodes the world's query answers otherwise this frame, as the sun's
/// cast binds them (pxl-spec-core `sun::MOVED_WORDS`): their count, then
/// each X, Y and its zoom; past `sun::MOVED`, or a slot with no square held
/// before, `whole`, and the world is cast again whole.
struct Nodes {
    words: [u32; sun::MOVED_WORDS],
    len: usize,
    whole: bool,
}

impl Default for Nodes {
    fn default() -> Nodes {
        Nodes {
            words: [0; sun::MOVED_WORDS],
            len: 0,
            whole: false,
        }
    }
}

impl Nodes {
    fn push(&mut self, [x, y]: [i32; 2], zoom: u32) {
        let node = [x as u32, y as u32, zoom, 0];
        if (0..self.len).any(|k| self.words[4 + 4 * k..8 + 4 * k] == node) {
            return;
        }
        if self.len == sun::MOVED {
            self.whole = true;
            return;
        }
        self.words[4 + 4 * self.len..8 + 4 * self.len].copy_from_slice(&node);
        self.len += 1;
        self.words[0] = self.len as u32;
    }
}

const _: () = assert!(
    within(compute::JOINED_GROUP, compute::MISS_GROUP)
        && within(compute::BEAM_GROUP, compute::MISS_GROUP)
        && within(compute::CELLS_GROUP, compute::MISS_GROUP)
        && within(compute::CAST_GROUP, compute::MISS_GROUP)
        && within(compute::FORGET_GROUP, compute::MISS_GROUP),
    "the fold, the beam, the cells, the reading cast and the forgetting bind the group the missed pixels' cast binds whole"
);
/// The cast's one group, every kernel's over the cast's world bar the
/// sun's: the one the missed pixels' cast binds whole. The sun's cast binds
/// its world and its clipmap (`SUN_GROUP`), past the floor's storage
/// buffers were they one group.
const CAST_GROUP: [u32; compute::MISS_GROUP.len()] = compute::MISS_GROUP;

impl CastBuffers {
    /// The world the cast binds, as the proxy's cut binds it (ray/proxy.rs):
    /// the eye, the windows, the ground, the service area, the overwrites
    /// and the ways' pool; none before the cast first binds it.
    pub(super) fn world(&self) -> Option<[u32; 6]> {
        let b = self.bound;
        (b[0] != 0).then_some([b[0], b[1], b[2], b[5], b[6], b[7]])
    }
}

impl RaysPass {
    /// The cast pass's pipelines and buffers, made once, where the frame
    /// first casts: before any of its uploads (`hand`), which take the
    /// stream only while `stream::RESERVE` is left after them, so the
    /// shaders made here are never left without room.
    pub fn make(&mut self, g: &Gpu, pipelines: &mut Pipelines) {
        if self.cast.is_some() {
            return;
        }
        // One group for every kernel over the cast's world.
        let group = &CAST_GROUP;
        let built = pipelines.build_compute(g, SHADER_CAST, group);
        let beam_built = pipelines.build_compute(g, SHADER_BEAM, group);
        let cells_built = pipelines.build_compute(g, SHADER_CELLS, group);
        let sample_built = pipelines.build_compute(g, SHADER_SAMPLE, group);
        let joined_built = pipelines.build_compute(g, SHADER_JOINED, group);
        let miss_built = pipelines.build_compute(g, SHADER_MISS, group);
        let forget_built = pipelines.build_compute(g, SHADER_FORGET, group);
        let sun_built = pipelines.build_compute(g, SHADER_SUN, &compute::SUN_GROUP);
        let sun_mark_built = pipelines.build_compute(g, SHADER_SUN_MARK, &compute::SUN_MARK_GROUP);
        let buffer = |size: u32, usage: u32| g.buffer(size, usage);
        let storage = buffer_usage::STORAGE | buffer_usage::COPY_DST;
        let oracle = Oracle {
            built: pipelines.build_compute(g, SHADER_ORACLE, &compute::ORACLE_GROUP),
            group: 0,
            bound: [0; 3],
            on: false,
            steps: false,
            counts: buffer(word_bytes(ORACLE_COUNTS), storage | buffer_usage::COPY_SRC),
            read: buffer(
                word_bytes(ORACLE_COUNTS),
                buffer_usage::MAP_READ | buffer_usage::COPY_DST,
            ),
            landed: Box::new([0; ORACLE_COUNTS]),
            mapping: Box::new(DONE),
            copied: false,
            mapped: false,
            sums: [0.0; ORACLE_COUNTS + 1],
        };
        let proxied = Proxied {
            entry: pipelines.build_compute(g, SHADER_PROXIED_ENTRY, &compute::PROXIED_ENTRY_GROUP),
            route: pipelines.build_compute(g, SHADER_PROXIED_ROUTE, &compute::PROXIED_ROUTE_GROUP),
            pixels: pipelines.build_compute(g, SHADER_PROXIED, &compute::PROXIED_GROUP),
            miss: pipelines.build_compute(g, SHADER_PROXIED_MISS, &compute::PROXIED_MISS_GROUP),
            check: pipelines.build_compute(g, SHADER_PROXIED_CHECK, &compute::PROXIED_CHECK_GROUP),
            write: pipelines.build_compute(g, SHADER_PROXIED_WRITE, &compute::PROXIED_WRITE_GROUP),
            beam: pipelines.build_compute(g, SHADER_PROXIED_BEAM, &compute::PROXIED_BEAM_GROUP),
            groups: [0; 7],
            bound: [0; 15],
            sizes: buffer(word_bytes(proxied::ROUTE_WORDS as usize), storage),
            args: buffer(
                word_bytes(proxied::ROUTE_WORDS as usize),
                buffer_usage::STORAGE | buffer_usage::INDIRECT,
            ),
            header: buffer(
                word_bytes(proxy::COUNTS as usize),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
            ),
            wanted: false,
            writes: false,
            beams: false,
        };
        let c = CastBuffers {
            built,
            beam_built,
            cells_built,
            sample_built,
            beams: 0,
            grid: beam::Grid::of_words([0.0; 2], [0; eye::GRID_WORDS]),
            beam_room: 0,
            miss_built,
            forget_built,
            seen: buffer(word_bytes(observed::WORDS as usize), storage),
            frame: 0,
            forget: true,
            moved: [0; observed::MOVED_WORDS as usize],
            slots: Vec::new(),
            forgets: observed::FORGETS_NONE,
            cuts: [0; eye::CUTS],
            joined_built,
            sun_built,
            sun_mark_built,
            sun_groups: [0; 2],
            sun_bound: [0; 7],
            sun_list: buffer(
                word_bytes(sun::LIST_AT + (shadow::SIZE / sun::TILE).pow(2) as usize),
                storage | buffer_usage::INDIRECT,
            ),
            sun_read: Read::default(),
            sun_at: Read::default(),
            sun_kept: None,
            sun_moved: buffer(
                word_bytes(sun::MOVED_WORDS),
                buffer_usage::UNIFORM | buffer_usage::COPY_DST,
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
            oracle,
            proxied,
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
        sun_map: (u32, u32),
    ) {
        // No cast, no world: no ray cast, none drawn and no decor grown.
        // What the cast reads stands for the next: the ground it binds is
        // made once.
        let (Some(mut cast), Some((ground, looks, pool))) = (
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
            decor: _,
            proxy: _,
        } = &cast;
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
                &lane[first as usize..first as usize + 8]
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
        c.forget |= shards_moved
            || shards_grow
            || overwrites_moved
            || cuts != c.cuts
            || c.frame >= observed::FRAMES;
        // The eye's windows' slots that hold another square, or whose square
        // the turn handed: what is over them is asked again; the sun's map
        // casts again what passes over the square they held and the one they
        // hold, and over a preview's cuts where they moved.
        let mut nodes = Nodes::default();
        for at in [0, eye::CUT_WORDS] {
            let (was, is) = (
                &c.cuts[at..at + eye::CUT_WORDS],
                &cuts[at..at + eye::CUT_WORDS],
            );
            if was != is {
                for cut in [was, is] {
                    if cut[3] != eye::NO_CUT {
                        nodes.push([cut[0] as i32, cut[1] as i32], cut[3]);
                    }
                }
            }
        }
        c.cuts = cuts;
        let eye_rows = (window::EYE_WINDOWS * window::SLOTS) as usize;
        let square = |row: &[i32], k: usize| {
            (
                [row[0], row[1]],
                window::squares_zoom(window::eye_cell((k / window::SLOTS as usize) as u32)),
            )
        };
        if window_moved {
            let rows = &cast.window[..4 * eye_rows];
            for k in 0..eye_rows {
                let row = &rows[4 * k..4 * k + 4];
                let was = c.slots.get(4 * k..4 * k + 4);
                if was != Some(row) {
                    c.moved[k / 32] |= 1 << (k % 32);
                    match was {
                        Some(was) => nodes.push(square(was, k).0, square(was, k).1),
                        None => nodes.whole = true,
                    }
                    nodes.push(square(row, k).0, square(row, k).1);
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
                if w < window::EYE_WINDOWS && square(&cast.window[4 * k..4 * k + 4], k).0 == q {
                    c.moved[k / 32] |= 1 << (k % 32);
                    nodes.push(q, window::squares_zoom(cell));
                }
            }
        }
        // The sun's cast reads the world as the eye's rays do, from the
        // eye's place at its zooms, but not its look: a turn of the eye
        // alone casts it no ray again. Its clipmap keeps what it holds while
        // the light's frame and the world under it read as they did
        // (`sun_read`); then the eye's place, quality or zooms or the box
        // moved (`sun_at`), or the nodes under it (`nodes`), casts only the
        // texels the box moved onto, those whose stop the eye now asks at
        // another zoom and those whose ray passes over a node moved, each
        // judged where it lies (pxl-spec-core `sun`). None where no shadow is
        // cast.
        let shadows = cast.eye[eye::SUN..eye::SUN + eye::SUN_WORDS]
            .iter()
            .any(|&w| w != 0);
        let r = &mut c.sun_read;
        r.begin();
        r.read(&[sun_map.1, u32::from(shadows)]);
        r.mark(shards_moved || shards_grow || overwrites_moved || nodes.whole);
        let whole = !r.end();
        let a = &mut c.sun_at;
        a.begin();
        a.read(&cast.eye[eye::ORIGIN..eye::STEPS]);
        a.read(&cast.eye[eye::TAU..eye::TAU + 1]);
        a.read(&cast.eye[eye::RANGE..eye::RANGE + 2]);
        a.read(&cast.eye[eye::SUN..eye::SUN + eye::SUN_WORDS]);
        let moved = !a.end();
        c.sun_cast = shadows && (whole || moved || nodes.len != 0);
        if c.sun_cast {
            // What the map holds, written over the kept corner's words, which
            // keep nothing as the box is aimed (`Sun::of`).
            if let (false, Some(kept)) = (whole, c.sun_kept) {
                for (k, w) in sun::KEPT.into_iter().zip(kept) {
                    cast.eye[eye::SUN + k] = w as u32;
                }
            }
            c.sun_kept = Some(sun::CORNER.map(|k| cast.eye[eye::SUN + k] as i32));
            let len = if whole { 0 } else { nodes.len };
            nodes.words[0] = len as u32;
            g.write_buffer(c.sun_moved, 0, bytes(&nodes.words[..4 + 4 * len]));
            g.write_buffer(c.sun_list, 0, bytes(&sun::LIST_EMPTY));
        }
        if eye_moved || c.sun_cast {
            g.write_buffer(c.eye, 0, bytes(&cast.eye));
        }
        if self.kind == FrameKind::cast {
            c.forgets = if c.forget {
                observed::FORGETS_ALL
            } else if c.moved.iter().any(|&m| m != 0) {
                observed::FORGETS_MOVED
            } else {
                observed::FORGETS_NONE
            };
            // The head before the frame's dispatches: what it forgets and over
            // which slots; forgotten whole, no tile taken and its first frame,
            // else the next frame.
            let mut head = [0u32; observed::HEAD_WORDS as usize];
            c.frame = if c.forgets == observed::FORGETS_ALL {
                1
            } else {
                c.frame + 1
            };
            head[observed::FRAME as usize] = c.frame;
            head[observed::FORGETS as usize] = c.forgets;
            head[observed::MOVED_AT as usize..].copy_from_slice(&c.moved);
            let from = if c.forgets == observed::FORGETS_ALL {
                0
            } else {
                observed::FRAME as usize
            };
            g.write_buffer(c.seen, word_bytes(from), bytes(&head[from..]));
            (c.forget, c.moved) = (false, [0; observed::MOVED_WORDS as usize]);
        }
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
            c.group = cast_group(g, &c.built, bound);
            c.bound = bound;
        }
        let o = &mut c.oracle;
        o.on = eye::oracle(cast.eye[eye::STEPS]) != eye::ORACLE_OFF;
        o.steps = cast.eye[eye::STEPS] & eye::SHOW_STEPS != 0;
        c.proxied.wanted = cast.eye[eye::STEPS] & eye::PROXIED != 0;
        c.proxied.writes = cast.eye[eye::STEPS] & eye::PROXIED_WRITE != 0;
        c.proxied.beams = cast.eye[eye::STEPS] & eye::PROXIED_BEAM != 0;
        if c.proxied.wanted && self.kind == FrameKind::cast {
            // Each route's workgroups, the cast as built's off the frame's
            // grid, the proxied cast's off the drawn pixels.
            let w = |[x, y]: [u32; 2], g: [u32; 3]| [x.div_ceil(g[0]), y.div_ceil(g[1]), 1];
            let mut sizes = [0u32; proxied::ROUTE_WORDS as usize];
            for (r, s) in [
                (proxied::BEAM, w(c.grid.corners, compute::BEAM_WORKGROUP)),
                (proxied::CELLS, w(c.grid.span, compute::CELLS_WORKGROUP)),
                (
                    proxied::SAMPLE,
                    w(c.grid.samples, compute::SAMPLE_WORKGROUP),
                ),
                (proxied::CAST, w(c.size, compute::CAST_WORKGROUP)),
                (proxied::MISS, w(c.size, compute::MISS_WORKGROUP)),
                (proxied::PIXELS, w(c.size, compute::PROXIED_WORKGROUP)),
                (
                    proxied::PIXELS_MISS,
                    w(c.size, compute::PROXIED_MISS_WORKGROUP),
                ),
            ] {
                sizes[3 * r as usize..3 * r as usize + 3].copy_from_slice(&s);
            }
            g.write_buffer(c.proxied.sizes, 0, bytes(&sizes));
        }
        let oracle_bound = [c.eye, rays.0, c.seen];
        if oracle_bound != o.bound {
            release(o.group);
            let [eye, rays, seen] = oracle_bound;
            o.group = g.bind_group(
                o.built.layouts[0],
                &[
                    buffer_entry(0, eye),
                    view_entry(3, rays),
                    buffer_entry(9, seen),
                    buffer_entry(10, o.counts),
                ],
            );
            o.bound = oracle_bound;
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
        if sun_bound != c.sun_bound {
            for group in c.sun_groups {
                release(group);
            }
            c.sun_groups = sun_groups(
                g,
                [&c.sun_mark_built, &c.sun_built],
                sun_bound,
                c.sun_moved,
                c.sun_list,
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
        let raster = self.proxy.bindings();
        let Some(c) = self.cast.as_mut() else { return };
        let proxied = c.proxied.wanted && raster.rastered && raster.index_array != 0;
        if proxied {
            c.proxied.regroup(g, c.bound, c.oracle.counts, raster);
        }
        let o = &c.oracle;
        let counting = self.kind == FrameKind::cast && o.steps && (o.on || proxied);
        if counting {
            g.write_buffer(o.counts, 0, bytes(&[0u32; ORACLE_COUNTS]));
        }
        if self.kind == FrameKind::cast && proxied {
            // The cut's counts into a uniform; then, in one pass, the
            // observed pyramid forgotten first where it is; each box's line
            // and node written into it, as the forgetting left it; the
            // frame's route off the cut's counts; and the dispatches it laid,
            // the cast as built's or the proxied cast's, the others none.
            let p = &c.proxied;
            let size = word_bytes(proxy::COUNTS as usize);
            g.copy_buffer(encoder, (raster.counts, 0), (p.header, 0), size);
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
            if p.beams {
                // The cast as built with no sample: the beam rays and the
                // cells always; the samples and the pixels off them where the
                // cut does not stand, else each pixel from its cell's line
                // no sooner than its box; those missed as built.
                g.set_bind_group(pass, 0, p.groups[1], &[]);
                g.set_pipeline(pass, p.route.pipeline);
                g.dispatch(pass, 1, 1, 1);
                g.set_bind_group(pass, 0, c.group, &[]);
                for (built, [x, y], w) in [
                    (&c.beam_built, c.grid.corners, compute::BEAM_WORKGROUP),
                    (&c.cells_built, c.grid.span, compute::CELLS_WORKGROUP),
                ] {
                    g.set_pipeline(pass, built.pipeline);
                    g.dispatch(pass, x.div_ceil(w[0]), y.div_ceil(w[1]), 1);
                }
                for (built, r) in [
                    (&c.sample_built, proxied::SAMPLE),
                    (&c.built, proxied::CAST),
                ] {
                    g.set_pipeline(pass, built.pipeline);
                    g.dispatch_indirect(pass, p.args, word_bytes(3 * r as usize));
                }
                g.set_bind_group(pass, 0, p.groups[6], &[]);
                g.set_pipeline(pass, p.beam.pipeline);
                g.dispatch_indirect(pass, p.args, word_bytes(3 * proxied::PIXELS as usize));
                g.set_bind_group(pass, 0, c.group, &[]);
                g.set_pipeline(pass, c.miss_built.pipeline);
                let w = compute::MISS_WORKGROUP;
                g.dispatch(pass, c.size[0].div_ceil(w[0]), c.size[1].div_ceil(w[1]), 1);
                g.end(pass);
            } else {
                g.set_bind_group(pass, 0, p.groups[0], &[]);
                g.set_pipeline(pass, p.entry.pipeline);
                g.dispatch(
                    pass,
                    proxy::BOXES.div_ceil(compute::PROXIED_ENTRY_WORKGROUP[0]),
                    1,
                    1,
                );
                g.set_bind_group(pass, 0, p.groups[1], &[]);
                g.set_pipeline(pass, p.route.pipeline);
                g.dispatch(pass, 1, 1, 1);
                g.set_bind_group(pass, 0, c.group, &[]);
                for (built, r) in [
                    (&c.beam_built, proxied::BEAM),
                    (&c.cells_built, proxied::CELLS),
                    (&c.sample_built, proxied::SAMPLE),
                    (&c.built, proxied::CAST),
                    (&c.miss_built, proxied::MISS),
                ] {
                    g.set_pipeline(pass, built.pipeline);
                    g.dispatch_indirect(pass, p.args, word_bytes(3 * r as usize));
                }
                let (pixels, group) = if p.writes {
                    (&p.write, p.groups[5])
                } else {
                    (&p.pixels, p.groups[2])
                };
                g.set_bind_group(pass, 0, group, &[]);
                g.set_pipeline(pass, pixels.pipeline);
                g.dispatch_indirect(pass, p.args, word_bytes(3 * proxied::PIXELS as usize));
                g.end(pass);
                // The pixels the pyramid could not answer, timed apart; none
                // where each pixel's one walk wrote it.
                if !p.writes {
                    let pass =
                        g.compute_pass(encoder, stamps[GpuSpanName::proxied_miss as usize].pass());
                    g.set_bind_group(pass, 0, p.groups[3], &[]);
                    g.set_pipeline(pass, p.miss.pipeline);
                    g.dispatch_indirect(
                        pass,
                        p.args,
                        word_bytes(3 * proxied::PIXELS_MISS as usize),
                    );
                    g.end(pass);
                }
            }
            if counting {
                // Untimed: the eye's walk it makes is no part of the cast.
                let pass = g.compute_pass(encoder, None);
                g.set_bind_group(pass, 0, p.groups[4], &[]);
                g.set_pipeline(pass, p.check.pipeline);
                let w = compute::PROXIED_CHECK_WORKGROUP;
                g.dispatch(pass, c.size[0].div_ceil(w[0]), c.size[1].div_ceil(w[1]), 1);
                g.end(pass);
            }
        } else if self.kind == FrameKind::cast {
            // The observed pyramid forgotten first where it is; then the
            // beam rays writing it, a corner each; each cell's start off them;
            // the samples writing it, each from its cell's start; the pixels
            // off the samples about them, or reading it from their cell's
            // start, marking the pixels that missed; and those again,
            // writing it, a thread a drawn pixel, each but the marked
            // returning at once. One pass, a dispatch each, each
            // reading what the ones before wrote (pxl-spec-core `observed`).
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
            for (built, [x, y], w) in [
                (&c.beam_built, c.grid.corners, compute::BEAM_WORKGROUP),
                (&c.cells_built, c.grid.span, compute::CELLS_WORKGROUP),
                (&c.sample_built, c.grid.samples, compute::SAMPLE_WORKGROUP),
                (&c.built, c.size, compute::CAST_WORKGROUP),
                (&c.miss_built, c.size, compute::MISS_WORKGROUP),
            ] {
                g.set_pipeline(pass, built.pipeline);
                g.dispatch(pass, x.div_ceil(w[0]), y.div_ceil(w[1]), 1);
            }
            g.end(pass);
        }
        if self.kind == FrameKind::cast {
            let o = &mut c.oracle;
            // Counted only with the steps shown, so the span the oracle is
            // timed in holds no atomic.
            if o.on {
                let pass = g.compute_pass(encoder, stamps[GpuSpanName::oracle as usize].pass());
                g.set_bind_group(pass, 0, o.group, &[]);
                g.set_pipeline(pass, o.built.pipeline);
                let w = compute::ORACLE_WORKGROUP;
                g.dispatch(pass, c.size[0].div_ceil(w[0]), c.size[1].div_ceil(w[1]), 1);
                g.end(pass);
            }
            if counting && !o.copied && !o.mapped {
                let size = word_bytes(ORACLE_COUNTS);
                g.copy_buffer(encoder, (o.counts, 0), (o.read, 0), size);
                o.copied = true;
            }
        }
        if c.sun_cast {
            // The tiles a slot of which walks again listed, a thread a tile;
            // then the walks, a workgroup a tile listed (pxl-spec-core
            // `sun::TILE`), so a slot that stands holds no lane of a walk's.
            let pass = g.compute_pass(encoder, stamps[GpuSpanName::shadow as usize].pass());
            g.set_bind_group(pass, 0, c.sun_groups[0], &[]);
            g.set_pipeline(pass, c.sun_mark_built.pipeline);
            let tiles = (shadow::SIZE / sun::TILE).pow(2);
            g.dispatch(pass, tiles.div_ceil(compute::SUN_MARK_WORKGROUP[0]), 1, 1);
            g.set_bind_group(pass, 0, c.sun_groups[1], &[]);
            g.set_pipeline(pass, c.sun_built.pipeline);
            g.dispatch_indirect(pass, c.sun_list, 0);
            g.end(pass);
        }
        if grows {
            let pass = g.compute_pass(encoder, stamps[GpuSpanName::grow as usize].pass());
            c.grown.dispatch(g, pass);
            g.end(pass);
        }
    }

    /// The oracle's counts copied last frame mapped back, and those that
    /// landed summed: after the frame that copied them was submitted.
    pub fn oracle_back(&mut self, g: &Gpu) {
        let Some(o) = self.cast.as_mut().map(|c| &mut c.oracle) else {
            return;
        };
        if o.mapped && *o.mapping & DONE != 0 {
            o.sums[0] += 1.0;
            for k in 0..ORACLE_COUNTS {
                o.sums[k + 1] += f64::from(o.landed[k]);
            }
            o.mapped = false;
        }
        if o.copied && *o.mapping & DONE != 0 {
            let address = off_of(o.landed.as_ptr()) as u32;
            let size = word_bytes(ORACLE_COUNTS);
            g.map_read(o.read, 0, size, address, &mut o.mapping, 1);
            (o.copied, o.mapped) = (false, true);
        }
    }

    /// The oracle's sums since boot (`Oracle`): the frames read, then each
    /// count.
    pub fn oracle_sums(&self) -> [f64; ORACLE_COUNTS + 1] {
        self.cast
            .as_ref()
            .map_or([0.0; ORACLE_COUNTS + 1], |c| c.oracle.sums)
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
            // Its copy was in the encoder dropped with it.
            c.oracle.copied = false;
            c.read.forget();
            c.sun_read.forget();
            c.grown.dropped();
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

/// The sun's mark's group 0 and its cast's, over `bound`: the eye, the
/// window, the tiles' frames, the service area's index, the overwrites and
/// the ways' pool, the cast's alone; the clipmap; the nodes moved under it
/// (`moved`) and the tiles listed (`list`).
fn sun_groups(
    g: &Gpu,
    [mark, walk]: [&Built; 2],
    bound: [u32; 7],
    moved: u32,
    list: u32,
) -> [u32; 2] {
    let [eye, window, ground, shards, overwrites, pool, sun] = bound;
    let entries = [
        (0, buffer_entry(0, eye)),
        (1, buffer_entry(1, window)),
        (2, buffer_entry(2, ground)),
        (5, buffer_entry(5, shards)),
        (6, buffer_entry(6, overwrites)),
        (7, buffer_entry(7, pool)),
        (10, view_entry(10, sun)),
        (11, buffer_entry(11, moved)),
        (12, buffer_entry(12, list)),
    ];
    // Each the entries its layout declares (build.rs `compute`): the mark
    // reads the clipmap and the nodes moved alone.
    let of = |declared: &[u32]| {
        entries
            .iter()
            .filter(|(b, _)| declared.chunks(3).any(|d| d[0] == *b))
            .map(|&(_, e)| e)
            .collect::<Vec<_>>()
    };
    [
        g.bind_group(mark.layouts[0], &of(&compute::SUN_MARK_GROUP)),
        g.bind_group(walk.layouts[0], &of(&compute::SUN_GROUP)),
    ]
}

impl Proxied {
    /// Its groups over the cast's bindings (`cast_group`'s `bound`), the
    /// oracle's counts and the proxy's (`ProxyBindings`), made again
    /// exactly when one of them is another.
    fn regroup(&mut self, g: &Gpu, cast: [u32; 10], checks: u32, raster: ProxyBindings) {
        let [eye, window, ground, rays, looks, shards, overwrites, pool, beams, seen] = cast;
        let bound = [
            eye,
            window,
            ground,
            rays,
            looks,
            shards,
            overwrites,
            pool,
            seen,
            checks,
            raster.counts,
            raster.data,
            raster.index_array,
            self.sizes,
            beams,
        ];
        if bound == self.bound {
            return;
        }
        for group in self.groups {
            release(group);
        }
        let (counts, data, index, header) =
            (raster.counts, raster.data, raster.index_array, self.header);
        self.groups = [
            g.bind_group(
                self.entry.layouts[0],
                &[
                    buffer_entry(0, eye),
                    buffer_entry(1, window),
                    buffer_entry(2, ground),
                    buffer_entry(4, looks),
                    buffer_entry(5, shards),
                    buffer_entry(6, overwrites),
                    buffer_entry(7, pool),
                    buffer_entry(9, seen),
                    buffer_entry(12, data),
                    buffer_entry(17, header),
                ],
            ),
            g.bind_group(
                self.route.layouts[0],
                &[
                    buffer_entry(11, counts),
                    buffer_entry(15, self.sizes),
                    buffer_entry(16, self.args),
                ],
            ),
            g.bind_group(
                self.pixels.layouts[0],
                &[
                    buffer_entry(0, eye),
                    buffer_entry(1, window),
                    buffer_entry(2, ground),
                    view_entry(3, rays),
                    buffer_entry(5, shards),
                    buffer_entry(6, overwrites),
                    buffer_entry(7, pool),
                    buffer_entry(9, seen),
                    buffer_entry(12, data),
                    view_entry(13, index),
                    buffer_entry(17, header),
                ],
            ),
            g.bind_group(
                self.miss.layouts[0],
                &[
                    buffer_entry(0, eye),
                    buffer_entry(1, window),
                    buffer_entry(2, ground),
                    view_entry(3, rays),
                    buffer_entry(4, looks),
                    buffer_entry(5, shards),
                    buffer_entry(6, overwrites),
                    buffer_entry(7, pool),
                    buffer_entry(9, seen),
                    buffer_entry(12, data),
                ],
            ),
            g.bind_group(
                self.check.layouts[0],
                &[
                    buffer_entry(0, eye),
                    buffer_entry(1, window),
                    buffer_entry(2, ground),
                    view_entry(3, rays),
                    buffer_entry(5, shards),
                    buffer_entry(6, overwrites),
                    buffer_entry(7, pool),
                    buffer_entry(10, checks),
                    buffer_entry(12, data),
                    view_entry(13, index),
                    buffer_entry(17, header),
                ],
            ),
            g.bind_group(
                self.write.layouts[0],
                &[
                    buffer_entry(0, eye),
                    buffer_entry(1, window),
                    buffer_entry(2, ground),
                    view_entry(3, rays),
                    buffer_entry(4, looks),
                    buffer_entry(5, shards),
                    buffer_entry(6, overwrites),
                    buffer_entry(7, pool),
                    buffer_entry(9, seen),
                    buffer_entry(12, data),
                    view_entry(13, index),
                    buffer_entry(17, header),
                ],
            ),
            g.bind_group(
                self.beam.layouts[0],
                &[
                    buffer_entry(0, eye),
                    buffer_entry(1, window),
                    buffer_entry(2, ground),
                    view_entry(3, rays),
                    buffer_entry(5, shards),
                    buffer_entry(6, overwrites),
                    buffer_entry(7, pool),
                    buffer_entry(8, beams),
                    buffer_entry(9, seen),
                    buffer_entry(12, data),
                    view_entry(13, index),
                ],
            ),
        ];
        self.bound = bound;
    }
}
