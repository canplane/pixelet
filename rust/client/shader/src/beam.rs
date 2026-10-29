// pxl-shader/beam.rs

//! The beam rays (pxl-spec-core `beam`): the eye's line asked once ahead of
//! them (`origin_cs`), each corner's walk and line (`beam_cs`), where each
//! cell's rays start off them (`cell_cs`), and the
//! samples walked from those starts (`sample_cs`), each its pass's one
//! entry, cast for the eye's camera grid or the sun's map as the head it is
//! handed names (observer.rs `Either`); and the words a line, a node and a
//! sample are held in, every observer's.

use pxl_spec_core::cast::{self, Detail, Ends, Met, Query, Until};
use pxl_spec_core::world::At;
use pxl_spec_core::{beam, observed, tally, ways};
use spirv_std::glam::{IVec4, UVec3};
use spirv_std::spirv;

use crate::eye::{eye_origin, eye_ray, grid, Eye};
use crate::observed::{seen_at, seen_put, tallied, Seen, Writ, Writing};
use crate::world::World;

/// The beam rays' buffer, the cast's own for a frame (pxl-spec-core
/// `beam`): the lines, a corner each, the cells' starts and the samples'
/// stops, as the frame's `Grid` lays them (`Grid::size`), or as the sun's
/// list does (`sun::list_words`) where the pass casts for the map; a
/// runtime array as `Seen` is. A pixel's own ray words say whether it
/// missed (`ray::MISS`), so nothing here is a pixel's.
pub(crate) type Beams = spirv_std::RuntimeArray<u32>;

/// A beam ray's line (pxl-spec-core `beam::LINE_WORDS`, at `base` of the
/// words `W` hold: the eye's beam rays' buffer or the sun's list) as a walk
/// resumes from one: what was handed each node of it (`world::At::words`),
/// its own at its zoom. A node's entry is its key's (`observed::entry_of`),
/// so none is read off the line.
pub(crate) struct Beamed<'a, W: ?Sized>(pub(crate) &'a W, pub(crate) u32);

impl<W: ?Sized> Clone for Beamed<'_, W> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<W: ?Sized> Copy for Beamed<'_, W> {}

impl<W: Words + ?Sized> Beamed<'_, W> {
    /// The node the beam ray stopped at.
    pub(crate) fn node(self) -> cast::Node {
        node_at(self.0, self.1 + beam::LINE_NODE)
    }

    /// The nodes the beam ray's walk visited (`beam::LINE_VISITS`), the
    /// eye's: the start of every walk resumed from it, in the steps view.
    pub(crate) fn visits(self) -> u32 {
        self.0.word(self.1 + beam::LINE_VISITS)
    }

    /// What was handed the line's node of `zoom`.
    fn at(self, zoom: u32) -> At {
        let (w, k) = (self.0, self.1 + beam::LINE_AT + beam::LINE_STEP * zoom);
        At::of_words([
            w.word(k),
            w.word(k + 1),
            w.word(k + 2),
            w.word(k + 3),
            w.word(k + 4),
        ])
    }
}

impl<W: Words + ?Sized> cast::Over<u32> for Beamed<'_, W> {
    fn over(&self, _: u32) -> u32 {
        observed::NONE
    }
}

impl<W: Words + ?Sized> cast::Over<Writ> for Beamed<'_, W> {
    fn over(&self, zoom: u32) -> Writ {
        Writ {
            entry: observed::NONE,
            at: self.at(zoom),
        }
    }
}

/// Where lines, nodes and samples' words lie (pxl-spec-core
/// `beam::LINE_WORDS`, `beam::NODE_WORDS`, `beam::SAMPLE_WORDS`): the eye's
/// in the beam rays' buffer; the sun's in its list's (pxl-spec-core
/// `sun::line`, `sun::sample`).
pub(crate) trait Words {
    fn word(&self, k: u32) -> u32;
    fn set(&mut self, k: u32, v: u32);
}

impl Words for Beams {
    fn word(&self, k: u32) -> u32 {
        seen_at(self, k)
    }

    fn set(&mut self, k: u32, v: u32) {
        seen_put(self, k, v)
    }
}

impl Words for [u32] {
    fn word(&self, k: u32) -> u32 {
        pxl_spec_core::at(self, k)
    }

    fn set(&mut self, k: u32, v: u32) {
        pxl_spec_core::fill::put(self, k, v)
    }
}

/// The node at word `k`, as a line and a sample each hold one
/// (pxl-spec-core `beam::NODE_WORDS`).
pub(crate) fn node_at<W: Words + ?Sized>(w: &W, k: u32) -> cast::Node {
    cast::Node {
        corner: [
            w.word(k + beam::NODE_CORNER),
            w.word(k + beam::NODE_CORNER + 1),
            w.word(k + beam::NODE_CORNER + 2),
        ],
        zoom: w.word(k + beam::NODE_ZOOM),
        roots: [
            w.word(k + beam::NODE_ROOTS) as i32,
            w.word(k + beam::NODE_ROOTS + 1) as i32,
            w.word(k + beam::NODE_ROOTS + 2) as i32,
        ],
    }
}

/// Node `n` put at word `k`, as `node_at` reads it.
pub(crate) fn node_put<W: Words + ?Sized>(w: &mut W, k: u32, n: &cast::Node) {
    w.set(k + beam::NODE_ZOOM, n.zoom);
    w.set(k + beam::NODE_CORNER, n.corner[0]);
    w.set(k + beam::NODE_CORNER + 1, n.corner[1]);
    w.set(k + beam::NODE_CORNER + 2, n.corner[2]);
    w.set(k + beam::NODE_ROOTS, n.roots[0] as u32);
    w.set(k + beam::NODE_ROOTS + 1, n.roots[1] as u32);
    w.set(k + beam::NODE_ROOTS + 2, n.roots[2] as u32);
}

/// A sample's stop `met` put at word `base` (pxl-spec-core
/// `beam::SAMPLE_WORDS`), as the raster reads it (raster.rs): the node and
/// block it stopped at, `beam::SAMPLE_NONE` where it went on to its reach,
/// whether what it stopped at lacks, and the axis it entered across; what
/// lies about that face is the eye's box's, asked once a box (raster.rs
/// `faces_cs`).
pub(crate) fn sample_put<W: Words + ?Sized, A>(w: &mut W, base: u32, met: &Met<A>) {
    let stop = &met.stop;
    node_put(w, base + beam::SAMPLE_NODE, &stop.node);
    if met.ends == Ends::On {
        w.set(
            base + beam::SAMPLE_NODE + beam::NODE_ZOOM,
            beam::SAMPLE_NONE,
        );
    }
    w.set(base + beam::SAMPLE_LACKS, (met.ends == Ends::Lacks) as u32);
    w.set(base + beam::SAMPLE_BLOCK, stop.block[0]);
    w.set(base + beam::SAMPLE_BLOCK + 1, stop.block[1]);
    w.set(base + beam::SAMPLE_AXIS, stop.axis);
    w.set(base + beam::SAMPLE_VISITS, met.visits);
}

/// `ray`'s walk to the block that fills, resumed at `t` from the deepest
/// node of `line` holding where it is there, or, where that lies in another
/// root, from the root at it (doc/minutes/20261022-a-cell-casts-a-beam.md,
/// "Where a pixel resumes"); through `q`, reading or writing, its line left
/// where it ends to `keep`.
pub(crate) fn resumed<'a, Q: Query, W: Words + ?Sized>(
    q: &mut Q,
    origin: [i32; 3],
    ray: &cast::Ray,
    detail: Detail,
    t: f32,
    line: &Beamed<'a, W>,
    node: cast::Node,
    keep: &mut impl cast::Keep<Q::At>,
) -> Met<Q::At>
where
    Beamed<'a, W>: cast::Over<Q::At>,
{
    let (h, held) = node.over(cast::point_at(origin, ray, t));
    let h = cast::holding(origin, ray, h, t);
    let from = if held {
        (
            h,
            <Beamed<'a, W> as cast::Over<Q::At>>::over(line, h.zoom),
            t,
        )
    } else {
        (cast::Node::ROOT, q.root(cast::Node::ROOT.idx(origin)), t)
    };
    cast::walk(q, origin, ray, detail, Until::Opaque, from, line, keep)
}

/// The eye's line, asked once ahead of its beam rays (eye.rs
/// `origin`): one thread, the frame's shared descent paid once.
#[spirv(compute(threads(1, 1, 1), entry_point_name = "origin_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn origin_cs(
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] looks: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
) {
    let window: &[IVec4] = window;
    let world = World {
        window,
        ground,
        looks,
        shards,
        overwrites,
        eye,
        pool: way::tree::Read(pool),
    };
    crate::eye::origin(eye, world, seen);
}

/// Each beam ray of its observer's lattice, a thread a corner: the camera
/// grid's (pxl-spec-core `beam::Grid::corners`), or the sun's map's
/// (`sun::BEAM`), as its head names (observer.rs `beam`, `Either`).
#[spirv(compute(threads(8, 8, 1), entry_point_name = "beam_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn beam_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] looks: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] moved: &Moved,
) {
    let window: &[IVec4] = window;
    let o = Either::of(eye, moved);
    let [across, down] = o.corners();
    if id.x >= across || id.y >= down {
        return;
    }
    let world = World {
        window,
        ground,
        looks,
        shards,
        overwrites,
        eye,
        pool: way::tree::Read(pool),
    };
    let base = beam::Grid::line(g.corner_index([id.x, id.y]));
    let met = cast::rooted(
        &mut Writing {
            world,
            seen: &mut *seen,
            tag,
            blocks: true,
        },
        origin,
        &ray,
        detail,
        Until::Beam,
        &mut Leaving(beams, base),
    );
    tallied(seen, tally::EYE, tally::BEAM, met.visits);
    seen_put(beams, base + beam::LINE_T, g.handed(met.stop.t).to_bits());
    seen_put(beams, base + beam::LINE_VISITS, met.visits);
}

/// A beam ray's line as it leaves it (pxl-spec-core `cast::Keep`), into the
/// words `W` hold at `base` (`beam::LINE_WORDS`), as `Beamed` reads it: the
/// eye's beam rays' and the sun's.
pub(crate) struct Leaving<'a, W: ?Sized>(pub(crate) &'a mut W, pub(crate) u32);

impl<W: Words + ?Sized> cast::Keep<Writ> for Leaving<'_, W> {
    fn keep(&mut self, met: &Met<Writ>, stack: &[Writ; cast::DEPTH]) {
        let (n, live) = (&met.stop.node, met.stop.at);
        let base = self.1;
        let w = &mut *self.0;
        node_put(w, base + beam::LINE_NODE, n);
        let mut z = 0;
        while z <= n.zoom {
            let held = if z == n.zoom { live } else { stack[z as usize] };
            let a = held.at.words();
            let k = base + beam::LINE_AT + beam::LINE_STEP * z;
            w.set(k, a[0]);
            w.set(k + 1, a[1]);
            w.set(k + 2, a[2]);
            w.set(k + 3, a[3]);
            w.set(k + 4, a[4]);
            z += 1;
        }
    }
}

/// Where each cell's rays start (pxl-spec-core `beam::CELL_WORDS`), a
/// thread a cell of its observer's lattice (observer.rs `cell`): the
/// screen's (`beam::Grid::span`), past the lines in the beam rays' buffer
/// (`Grid::cell_at`), or the sun's map's (`sun::cell`).
#[spirv(compute(threads(8, 8, 1), entry_point_name = "cell_cs"))]
pub fn cell_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] moved: &Moved,
) {
    let o = Either::of(eye, moved);
    let [across, down] = o.cells();
    if id.x >= across || id.y >= down {
        return;
    }
    observer::cell(&o, beams, [id.x, id.y]);
}

/// Each sample of its observer's lattice, a thread each (observer.rs
/// `sample`): the screen's (pxl-spec-core `beam::Grid::samples`) or the
/// sun's map's (`sun::SAMPLE`). The node and asset it stopped at left for
/// the raster (`beam::SAMPLE_WORDS`; raster.rs), `beam::SAMPLE_NONE` where
/// it went on to its reach, and whether what it stopped at lacks. What lies
/// about the face it entered is asked only of a sample that draws a box,
/// the one whose words a pixel reads (raster.rs `face_cs`).
#[spirv(compute(threads(8, 8, 1), entry_point_name = "sample_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn sample_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] looks: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] moved: &Moved,
) {
    let window: &[IVec4] = window;
    let o = Either::of(eye, moved);
    let [across, down] = o.samples();
    if id.x >= across || id.y >= down {
        return;
    }
    let world = World {
        window,
        ground,
        looks,
        shards,
        overwrites,
        eye,
        pool: way::tree::Read(pool),
    };
    let met = resumed(
        &mut Writing {
            world,
            seen: &mut *seen,
            tag,
            blocks: true,
        },
        origin,
        &ray,
        detail,
        t,
        &line,
        line.node(),
    );
    tallied(seen, tally::EYE, tally::SAMPLE, met.visits);
    let met = Met {
        visits: met.visits + line.visits(),
        ..met
    };
    sample_put(beams, g.sample_at([id.x, id.y]), &met);
}
