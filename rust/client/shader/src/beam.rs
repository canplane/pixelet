// pxl-shader/beam.rs

//! The beam rays (pxl-spec-core `beam`): each corner's walk and line
//! (`beam_cs`), where each cell's rays start off them (`cell_cs`), and the
//! samples walked from those starts (`sample_cs`), each its pass's entry,
//! cast for the eye's camera grid, and the cells' for the shadow map too, as
//! the head it is handed names (either.rs `Either`); the shadow map's beam
//! rays and samples take entries of their own over the same passes, which
//! tell what their walks ask of (light.rs `light_beam_cs`,
//! `light_sample_cs`); and the words a line, a node and a sample are held
//! in, every observer's.

use pxl_spec_core::cast::{self, Detail, End, Met, Query, Until};
use pxl_spec_core::world::Handed;
use pxl_spec_core::{beam, observed, way};
use spirv_std::glam::{IVec4, UVec3};
use spirv_std::spirv;

use crate::either::Either;
use crate::eye::Eye;
use crate::observed::{seen_at, seen_least, seen_put, Seen, Writ};
use crate::observer::{self, Observer};
use crate::light::Head;
use crate::world::World;

/// The beam rays' buffer, the cast's own for a frame (pxl-spec-core
/// `beam`): the lines, a corner each, the cells' starts and the samples'
/// stops, as the frame's `Grid` lays them (`Grid::size`), or as the light's
/// list does (`light::list_words`) where the pass casts for the map; a
/// runtime array as `Seen` is. A pixel's own ray words say whether it
/// missed (`ray::MISS`), so nothing here is a pixel's.
pub(crate) type Beams = spirv_std::RuntimeArray<u32>;

/// A beam ray's line (pxl-spec-core `beam::LINE_WORDS`, at `base` of the
/// words `W` hold: the eye's beam rays' buffer or the light's list) as a walk
/// resumes from one: what was handed each node of it (`world::Handed::words`),
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
    fn at(self, zoom: u32) -> Handed {
        let (w, k) = (self.0, self.1 + beam::LINE_HANDED + beam::LINE_STEP * zoom);
        Handed::of_words([
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
/// in the beam rays' buffer; the light's in its list's (pxl-spec-core
/// `light::line`, `light::sample`).
pub(crate) trait Words {
    fn word(&self, k: u32) -> u32;
    fn set(&mut self, k: u32, v: u32);
    /// Word `k` lowered to `v` where it is more, whoever else lowers it
    /// (pxl-spec-core `light::held_at`).
    fn least(&mut self, k: u32, v: u32);
}

impl Words for Beams {
    fn word(&self, k: u32) -> u32 {
        seen_at(self, k)
    }

    fn set(&mut self, k: u32, v: u32) {
        seen_put(self, k, v)
    }

    fn least(&mut self, k: u32, v: u32) {
        seen_least(self, k, v)
    }
}

impl Words for [u32] {
    fn word(&self, k: u32) -> u32 {
        pxl_spec_core::at(self, k)
    }

    fn set(&mut self, k: u32, v: u32) {
        pxl_spec_core::fill::put(self, k, v)
    }

    fn least(&mut self, k: u32, v: u32) {
        crate::least(self, k, v)
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
/// cell it stopped at, `beam::SAMPLE_NONE` where it went on to its reach,
/// whether what it stopped at lacks, and the axis it entered across; what
/// lies about that face is the eye's box's, asked once a box (raster.rs
/// `face_cs`).
pub(crate) fn sample_put<W: Words + ?Sized, A>(w: &mut W, base: u32, met: &Met<A>) {
    let stop = &met.stop;
    node_put(w, base + beam::SAMPLE_NODE, &stop.node);
    if met.ends == End::On {
        w.set(
            base + beam::SAMPLE_NODE + beam::NODE_ZOOM,
            beam::SAMPLE_NONE,
        );
    }
    w.set(base + beam::SAMPLE_LACKS, (met.ends == End::Lacks) as u32);
    w.set(base + beam::SAMPLE_ASSET, stop.asset[0]);
    w.set(base + beam::SAMPLE_ASSET + 1, stop.asset[1]);
    w.set(base + beam::SAMPLE_AXIS, stop.axis);
    w.set(base + beam::SAMPLE_VISITS, met.visits);
}

/// `ray`'s walk to the cell that fills, resumed at `t` from the deepest
/// node of `line` holding where it is there, or, where that lies in another
/// root, from the root at it; through `q`, reading or writing, its line
/// left where it ends to `keep` and each node it asks told to `told`.
pub(crate) fn resumed<'a, Q: Query, W: Words + ?Sized>(
    q: &mut Q,
    origin: [i32; 3],
    ray: &cast::Ray,
    detail: Detail,
    t: f32,
    line: &Beamed<'a, W>,
    node: cast::Node,
    keep: &mut impl cast::Keep<Q::At>,
    told: &mut impl cast::Asked,
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
    cast::walk(q, origin, ray, detail, Until::Opaque, from, line, keep, told)
}

/// Each beam ray of the camera grid, a thread a corner (pxl-spec-core
/// `beam::Grid::corners`; observer.rs `beam`), off the eye's head (either.rs
/// `Either`).
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
    #[spirv(uniform, descriptor_set = 0, binding = 11)] head: &Head,
) {
    let window: &[IVec4] = window;
    let o = Either::of(eye, head);
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
    observer::beam(&o, world, seen, beams, [id.x, id.y], &mut ());
}

/// A beam ray's line as it leaves it (pxl-spec-core `cast::Keep`), into the
/// words `W` hold at `base` (`beam::LINE_WORDS`), as `Beamed` reads it: the
/// eye's beam rays' and the light's.
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
            let k = base + beam::LINE_HANDED + beam::LINE_STEP * z;
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
/// (`Grid::cell_at`), or the shadow map's (`light::cell`).
#[spirv(compute(threads(8, 8, 1), entry_point_name = "cell_cs"))]
pub fn cell_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] head: &Head,
) {
    let o = Either::of(eye, head);
    let [across, down] = o.cells();
    if id.x >= across || id.y >= down {
        return;
    }
    observer::cell(&o, beams, [id.x, id.y]);
}

/// Each sample of the camera grid, a thread each (observer.rs `sample`;
/// pxl-spec-core `beam::Grid::samples`), off the eye's head (either.rs
/// `Either`). The node and asset it stopped at left for
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
    #[spirv(uniform, descriptor_set = 0, binding = 11)] head: &Head,
) {
    let window: &[IVec4] = window;
    let o = Either::of(eye, head);
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
    observer::sample(&o, world, seen, beams, [id.x, id.y], &mut ());
}
