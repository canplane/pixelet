// pxl-shader/beam.rs

//! The beam rays (pxl-engine `beam`): each corner's walk and line
//! (`beam_cs`), where each cell's rays start off them (`cell_cs`), and the
//! samples walked from those starts (`sample_cs`), each cast for the eye's
//! camera grid and, `light_` before its name, for the shadow map; and the
//! words a line, a
//! node and a sample are held in, every observer's.

use pxl_engine::cast::{self, Detail, End, Trace, Query, Until};
use pxl_engine::world::Inheritance;
use pxl_engine::{beam, pyramid, support, way};
use spirv_std::glam::{IVec4, UVec3};
use spirv_std::spirv;

use crate::eye::Eye;
use crate::pyramid::{pyramid_add, pyramid_at, pyramid_most, pyramid_put, Writing, Pyramid};
use crate::observer::{self, Level, Observer};
use crate::light::{Head, Map};
use crate::world::{PaletteWords, World};

/// The beam rays' buffer, the cast's own for a frame (pxl-engine
/// `beam`): the lines, a corner each, the cells' starts and the samples'
/// hits, as the frame's `Grid` lays them (`Grid::size`), or as the light's
/// list does (`light::list_words`) where the pass casts for the map; a
/// runtime array as `Pyramid` is.
pub(crate) type Beams = spirv_std::RuntimeArray<u32>;

/// A beam ray's line (pxl-engine `beam::LINE_WORDS`, at `base` of the
/// words `W` hold: the eye's beam rays' buffer or the light's list) as a walk
/// resumes from one: what was handed each node of it (`world::Inheritance::words`),
/// its own at its zoom. A node's entry is its key's (`pyramid::entry_of`).
pub(crate) struct BeamLine<'a, W: ?Sized>(pub(crate) &'a W, pub(crate) u32);

impl<W: ?Sized> Clone for BeamLine<'_, W> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<W: ?Sized> Copy for BeamLine<'_, W> {}

impl<W: Words + ?Sized> BeamLine<'_, W> {
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
    fn at(self, zoom: u32) -> Inheritance {
        let (w, k) = (self.0, self.1 + beam::LINE_INHERITANCE + beam::LINE_STEP * zoom);
        Inheritance::of_words([
            w.word(k),
            w.word(k + 1),
            w.word(k + 2),
            w.word(k + 3),
        ])
    }
}

impl<W: Words + ?Sized> cast::Over<u32> for BeamLine<'_, W> {
    fn over(&self, _: u32) -> u32 {
        pyramid::NONE
    }
}

impl<W: Words + ?Sized> cast::Over<Inheritance> for BeamLine<'_, W> {
    fn over(&self, zoom: u32) -> Inheritance {
        self.at(zoom)
    }
}

/// A beam ray's line as a reader of the assets about a node near it reads
/// it, in place (pxl-engine `support::Ancestors`): the node it stopped at,
/// by its index from the eye's origin, and what was handed each node of it
/// (`BeamLine`), none of it copied. The eye's: its lines' nodes are held from
/// the one origin its rays start at.
pub(crate) struct BeamAncestors<'a, W: ?Sized> {
    line: BeamLine<'a, W>,
    at: [i32; 3],
    zoom: u32,
}

impl<'a, W: Words + ?Sized> BeamAncestors<'a, W> {
    pub(crate) fn of(line: BeamLine<'a, W>, origin: [i32; 3]) -> BeamAncestors<'a, W> {
        let n = line.node();
        BeamAncestors {
            line,
            at: n.idx(origin),
            zoom: n.zoom,
        }
    }
}

impl<W: Words + ?Sized> support::Ancestors<Inheritance> for BeamAncestors<'_, W> {
    fn at(&self) -> [i32; 3] {
        self.at
    }

    fn zoom(&self) -> u32 {
        self.zoom
    }

    fn ancestor(&self, z: u32) -> Inheritance {
        self.line.at(z)
    }
}

/// Where lines, nodes and samples' words lie (pxl-engine
/// `beam::LINE_WORDS`, `beam::NODE_WORDS`, `beam::SAMPLE_WORDS`): the eye's
/// in the beam rays' buffer; the light's in its list's (pxl-engine
/// `light::line`, `light::sample`).
pub(crate) trait Words {
    fn word(&self, k: u32) -> u32;
    fn set(&mut self, k: u32, v: u32);
    /// `v` added to word `k`, whoever else adds to it: what it held.
    fn add(&mut self, k: u32, v: u32) -> u32;
    /// Word `k` raised to `v` where it is less, whoever else raises it.
    fn most(&mut self, k: u32, v: u32);
}

impl Words for Beams {
    fn word(&self, k: u32) -> u32 {
        pyramid_at(self, k)
    }

    fn set(&mut self, k: u32, v: u32) {
        pyramid_put(self, k, v)
    }

    fn add(&mut self, k: u32, v: u32) -> u32 {
        pyramid_add(self, k, v)
    }

    fn most(&mut self, k: u32, v: u32) {
        pyramid_most(self, k, v)
    }
}

impl Words for [u32] {
    fn word(&self, k: u32) -> u32 {
        pxl_engine::at(self, k)
    }

    fn set(&mut self, k: u32, v: u32) {
        pxl_engine::fill::put(self, k, v)
    }

    fn add(&mut self, k: u32, v: u32) -> u32 {
        crate::add(self, k, v)
    }

    fn most(&mut self, k: u32, v: u32) {
        crate::most(self, k, v)
    }
}

/// The node at word `k`, as a line and a sample each hold one
/// (pxl-engine `beam::NODE_WORDS`).
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

/// A sample's hit `met` put at word `base` (pxl-engine
/// `beam::SAMPLE_WORDS`), as the raster reads it (raster.rs): the node and
/// cell it stopped at, `beam::SAMPLE_NONE` where it went on to its reach,
/// whether what it stopped at lacks, and the axis it entered across; what
/// lies about that face is the eye's box's, asked once a box (raster.rs
/// `face_cs`).
pub(crate) fn sample_put<W: Words + ?Sized, A>(w: &mut W, base: u32, met: &Trace<A>) {
    let hit = &met.hit;
    node_put(w, base + beam::SAMPLE_NODE, &hit.node);
    if met.ends == End::Miss {
        w.set(
            base + beam::SAMPLE_NODE + beam::NODE_ZOOM,
            beam::SAMPLE_NONE,
        );
    }
    w.set(base + beam::SAMPLE_LACKS, (met.ends == End::Lacks) as u32);
    w.set(base + beam::SAMPLE_ASSET, hit.asset[0]);
    w.set(base + beam::SAMPLE_ASSET + 1, hit.asset[1]);
    w.set(base + beam::SAMPLE_AXIS, hit.axis);
    w.set(base + beam::SAMPLE_VISITS, met.visits);
}

/// `ray`'s walk to the cell that fills, resumed at `t` from the deepest
/// node of `line` holding where it is there, or, where that lies in another
/// root, from the root at it; where it `resumes` not, from the world's root
/// at 0, through the same walk; through `q`, reading or writing, its line
/// left where it ends to `keep`.
pub(crate) fn resume_walk<'a, Q: Query, W: Words + ?Sized>(
    q: &mut Q,
    origin: [i32; 3],
    ray: &cast::Ray,
    detail: Detail,
    t: f32,
    line: &BeamLine<'a, W>,
    node: cast::Node,
    resumes: bool,
    keep: &mut impl cast::Keep<Q::At>,
) -> Trace<Q::At>
where
    BeamLine<'a, W>: cast::Over<Q::At>,
{
    let root = (cast::Node::ROOT, cast::Node::ROOT.idx(origin));
    let from = if !resumes {
        (root.0, q.root(root.1), 0.0)
    } else {
        let (h, held) = node.over(cast::point_at(origin, ray, t));
        let h = cast::holding(origin, ray, h, t);
        if held {
            (
                h,
                <BeamLine<'a, W> as cast::Over<Q::At>>::over(line, h.zoom),
                t,
            )
        } else {
            (root.0, q.root(root.1), t)
        }
    };
    cast::walk(q, origin, ray, detail, Until::Opaque, from, line, keep)
}

/// Each beam ray of the camera grid, a thread a corner (observer.rs `beam`,
/// pxl-engine `beam::Grid::corners`), its line left in the eye's beam rays'
/// buffer. The shadow map's is `light_beam_cs`: a pass's observer is its
/// entry's, never chosen as it runs, as Adreno's driver refuses what a
/// choice made of each answer of `Observer`
/// (doc/experiment/20261103-shaders-on-two-phones).
#[spirv(compute(threads(8, 8, 1), entry_point_name = "beam_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn beam_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] clipmap: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] palette_indices: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] pyramid: &mut Pyramid,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] _head: &Head,
) {
    let clipmap: &[IVec4] = clipmap;
    let world = World {
        clipmap,
        ground,
        palette_indices,
        shards,
        overwrites,
        eye,
        pool: way::tree::Read(pool),
    };
    beam_by(eye, id, world, pyramid, beams);
}

/// Each beam ray of the shadow map, a thread a corner (`light::BEAM`), its
/// line left in the light's list: `beam_cs` for the light's map.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "light_beam_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn light_beam_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] clipmap: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] palette_indices: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] pyramid: &mut Pyramid,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] head: &Head,
) {
    let clipmap: &[IVec4] = clipmap;
    let world = World {
        clipmap,
        ground,
        palette_indices,
        shards,
        overwrites,
        eye,
        pool: way::tree::Read(pool),
    };
    beam_by(&Map::of(eye, head), id, world, pyramid, beams);
}

/// Corner `id`'s beam ray of `o`'s lattice, where it has one.
fn beam_by<O: Observer, L: PaletteWords>(
    o: &O,
    id: UVec3,
    world: World<'_, L>,
    pyramid: &mut Pyramid,
    beams: &mut Beams,
) {
    let [across, down] = o.corners();
    let c = o.corner_walked(&*beams, [id.x, id.y]);
    if c[0] >= across || c[1] >= down {
        return;
    }
    observer::beam(o, world, pyramid, beams, c);
}

/// Each node the walk over the observed pyramid stopped at (pxl-render
/// beam_hit.wgsl, pxl-engine `beam::ASKS_LIST`), a thread each, asked of the world
/// from what `Inheritance::of` finds and kept for the walk to go on: no ray walks
/// here, so it carries what one node asks and no more.
#[spirv(compute(threads(64), entry_point_name = "ask_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn ask_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] clipmap: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] palette_indices: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] pyramid: &mut Pyramid,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] asks: &[u32],
) {
    if id.x >= asks[3].min(beam::ASKS_ROOM) {
        return;
    }
    let at = (beam::ASKS_LIST + id.x * beam::ASK_NODE_WORDS) as usize;
    let zoom = asks[at];
    let c = [asks[at + 1] as i32, asks[at + 2] as i32, asks[at + 3] as i32];
    let clipmap: &[IVec4] = clipmap;
    let world = World {
        clipmap,
        ground,
        palette_indices,
        shards,
        overwrites,
        eye,
        pool: way::tree::Read(pool),
    };
    let tag = pyramid::tag(pyramid_at(pyramid, pyramid::FRAME), pyramid::BEAM_PASS);
    let mut n = Inheritance::of(&world, (zoom, c));
    let mut q = Writing {
        world,
        pyramid: &mut *pyramid,
        tag,
        assets: true,
    };
    Query::node(&mut q, &mut n, zoom, c, [asks[at + 4], asks[at + 5]], 0.0);
}

/// A beam ray's line as it leaves it (pxl-engine `cast::Keep`), into the
/// words `W` hold at `base` (`beam::LINE_WORDS`), as `BeamLine` reads it: the
/// eye's beam rays' and the light's.
pub(crate) struct Leaving<'a, W: ?Sized>(pub(crate) &'a mut W, pub(crate) u32);

impl<W: Words + ?Sized> cast::Keep<Inheritance> for Leaving<'_, W> {
    fn keep(&mut self, met: &Trace<Inheritance>, stack: &[Inheritance; cast::DEPTH]) {
        let (n, live) = (&met.hit.node, met.hit.at);
        let base = self.1;
        let w = &mut *self.0;
        node_put(w, base + beam::LINE_NODE, n);
        let mut z = 0;
        while z <= n.zoom {
            let held = if z == n.zoom { live } else { stack[z as usize] };
            let a = held.words();
            let k = base + beam::LINE_INHERITANCE + beam::LINE_STEP * z;
            w.set(k, a[0]);
            w.set(k + 1, a[1]);
            w.set(k + 2, a[2]);
            w.set(k + 3, a[3]);
            z += 1;
        }
    }
}

/// Where each cell's rays start (pxl-engine `beam::CELL_WORDS`), a thread a
/// cell of the screen's lattice (observer.rs `cell`, `beam::Grid::span`),
/// past the lines in the beam rays' buffer (`Grid::cell_at`); the shadow
/// map's is `light_cell_cs`.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "cell_cs"))]
pub fn cell_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] _head: &Head,
) {
    cell_by(eye, id, beams);
}

/// `cell_cs` for the shadow map's cells (`light::cell`).
#[spirv(compute(threads(8, 8, 1), entry_point_name = "light_cell_cs"))]
pub fn light_cell_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] head: &Head,
) {
    cell_by(&Map::of(eye, head), id, beams);
}

/// Cell `id` of `o`'s lattice, where it has one.
fn cell_by<O: Observer>(o: &O, id: UVec3, beams: &mut Beams) {
    let [across, down] = o.cells();
    let c = o.take(&*beams, Level::Cell, [id.x, id.y]);
    if c[0] >= across || c[1] >= down {
        return;
    }
    observer::cell(o, beams, c);
}

/// Each sample of the camera grid, a thread each (observer.rs `sample`,
/// pxl-engine `beam::Grid::samples`); the shadow map's is
/// `light_sample_cs`. The node and asset it stopped at left for the raster
/// (`beam::SAMPLE_WORDS`; raster.rs), `beam::SAMPLE_NONE` where it went on
/// to its reach, and whether what it stopped at lacks. What lies about the
/// face it entered is asked only of a sample that draws a box, the one
/// whose words a pixel reads (raster.rs `face_cs`).
#[spirv(compute(threads(8, 8, 1), entry_point_name = "sample_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn sample_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] clipmap: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] palette_indices: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] pyramid: &mut Pyramid,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] _head: &Head,
) {
    let clipmap: &[IVec4] = clipmap;
    let world = World {
        clipmap,
        ground,
        palette_indices,
        shards,
        overwrites,
        eye,
        pool: way::tree::Read(pool),
    };
    sample_by(eye, id, world, pyramid, beams);
}

/// `sample_cs` for the shadow map's samples (`param::light::SAMPLE`).
#[spirv(compute(threads(8, 8, 1), entry_point_name = "light_sample_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn light_sample_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] clipmap: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] palette_indices: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] pyramid: &mut Pyramid,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] head: &Head,
) {
    let clipmap: &[IVec4] = clipmap;
    let world = World {
        clipmap,
        ground,
        palette_indices,
        shards,
        overwrites,
        eye,
        pool: way::tree::Read(pool),
    };
    sample_by(&Map::of(eye, head), id, world, pyramid, beams);
}

/// Sample `id` of `o`'s lattice, where it has one.
fn sample_by<O: Observer, L: PaletteWords>(
    o: &O,
    id: UVec3,
    world: World<'_, L>,
    pyramid: &mut Pyramid,
    beams: &mut Beams,
) {
    let [across, down] = o.samples();
    let s = o.sample_walked(&*beams, [id.x, id.y]);
    if s[0] >= across || s[1] >= down {
        return;
    }
    observer::sample(o, world, pyramid, beams, s);
}
