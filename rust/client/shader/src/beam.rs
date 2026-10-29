// pxl-shader/beam.rs

//! The beam rays (pxl-spec-core `beam`): each corner's walk and line
//! (`beam_cs`), where each cell's rays start off them (`cell_cs`), and the
//! samples walked from those starts (`sample_cs`), each its pass's one
//! entry, cast for the eye's camera grid or the shadow map as the head it is
//! handed names (either.rs `Either`); and the words a line, a node and a
//! sample are held in, every observer's.

use pxl_spec_core::cast::{self, Detail, End, Met, Query, Until};
use pxl_spec_core::world::Handed;
use pxl_spec_core::{beam, observed, way};
use spirv_std::glam::{IVec4, UVec3};
use spirv_std::spirv;

use crate::eye::{eye_origin, eye_ray, grid, Eye};
use crate::observed::{seen_at, seen_put, Seen, Writ, Writing};
use crate::world::World;

/// The beam rays' buffer, the cast's own for a frame (pxl-spec-core
/// `beam`): the lines, a corner each, the cells' starts and the samples'
/// stops, as the frame's `Grid` lays them (`Grid::size`), a runtime array
/// as `Seen` is. A pixel's own ray words say whether it missed
/// (`ray::MISS`), so nothing here is a pixel's.
pub(crate) type Beams = spirv_std::RuntimeArray<u32>;

/// A beam ray's line (pxl-spec-core `beam::LINE_WORDS`, at `base` of the
/// words `W` hold: the eye's beam rays' buffer or the sun's list) as a walk
/// resumes from one: what was handed each node of it (`world::Handed::words`),
/// its own at its zoom. A node's entry is its key's (`observed::entry_of`),
/// so none is read off the line.
pub(crate) struct Beamed<'a, W: ?Sized>(pub(crate) &'a W, pub(crate) u32);

impl Beamed<'_> {
    /// The node the corner's beam ray stopped at.
    fn node(self) -> cast::Node {
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

impl cast::Over<u32> for Beamed<'_> {
    fn over(&self, zoom: u32) -> u32 {
        seen_at(self.0, self.step(zoom))
    }
}

impl cast::Over<Writ> for Beamed<'_> {
    fn over(&self, zoom: u32) -> Writ {
        let (w, k) = (self.0, self.step(zoom));
        Writ {
            entry: seen_at(w, k),
            at: At::of_words([
                seen_at(w, k + 1),
                seen_at(w, k + 2),
                seen_at(w, k + 3),
                seen_at(w, k + 4),
                seen_at(w, k + 5),
            ]),
        }
    }
}

/// The node at word `k` of the beam rays' buffer, as a line and a sample
/// each hold one (pxl-spec-core `beam::NODE_WORDS`).
pub(crate) fn node_at(w: &Beams, k: u32) -> cast::Node {
    cast::Node {
        corner: [
            seen_at(w, k + beam::NODE_CORNER),
            seen_at(w, k + beam::NODE_CORNER + 1),
            seen_at(w, k + beam::NODE_CORNER + 2),
        ],
        zoom: seen_at(w, k + beam::NODE_ZOOM),
        roots: [
            seen_at(w, k + beam::NODE_ROOTS) as i32,
            seen_at(w, k + beam::NODE_ROOTS + 1) as i32,
            seen_at(w, k + beam::NODE_ROOTS + 2) as i32,
        ],
    }
}

/// Node `n` put at word `k` of the beam rays' buffer, as `node_at` reads it.
fn node_put(w: &mut Beams, k: u32, n: &cast::Node) {
    seen_put(w, k + beam::NODE_ZOOM, n.zoom);
    seen_put(w, k + beam::NODE_CORNER, n.corner[0]);
    seen_put(w, k + beam::NODE_CORNER + 1, n.corner[1]);
    seen_put(w, k + beam::NODE_CORNER + 2, n.corner[2]);
    seen_put(w, k + beam::NODE_ROOTS, n.roots[0] as u32);
    seen_put(w, k + beam::NODE_ROOTS + 1, n.roots[1] as u32);
    seen_put(w, k + beam::NODE_ROOTS + 2, n.roots[2] as u32);
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
/// root, from the root at it (doc/minutes/20261022-a-cell-casts-a-beam.md,
/// "Where a pixel resumes"); through `q`, reading or writing.
pub(crate) fn resumed<'a, Q: Query>(
    q: &mut Q,
    origin: [i32; 3],
    ray: &cast::Ray,
    detail: Detail,
    t: f32,
    line: &Beamed<'a>,
) -> Met<Q::At>
where
    Beamed<'a>: cast::Over<Q::At>,
{
    let (h, held) = line.node().over(cast::point_at(origin, ray, t));
    let h = cast::holding(origin, ray, h, t);
    let from = if held {
        (h, <Beamed<'a> as cast::Over<Q::At>>::over(line, h.zoom), t)
    } else {
        (cast::Node::ROOT, q.root(cast::Node::ROOT.idx(origin)), t)
    };
    cast::walk(q, origin, ray, detail, Until::Opaque, from, line, keep)
}

/// Each beam ray of its observer's lattice, a thread a corner: the camera
/// grid's (pxl-spec-core `beam::Grid::corners`), or the shadow map's
/// (`sun::BEAM`), as its head names (observer.rs `beam`; either.rs `Either`).
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
) {
    let window: &[IVec4] = window;
    let (origin, detail) = eye_origin(eye);
    let g = grid(eye);
    if id.x >= g.corners[0] || id.y >= g.corners[1] {
        return;
    }
    let ray = eye_ray(eye, g.corner([id.x, id.y]));
    let tag = observed::tag(seen_at(seen, observed::FRAME), 0);
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
        &mut Writing { world, seen, tag },
        origin,
        &ray,
        detail,
        Until::Beam,
        &mut Leaving(beams, base),
    );
    seen_put(beams, base + beam::LINE_T, g.handed(met.stop.t).to_bits());
}

/// A beam ray's line as it leaves it (pxl-spec-core `cast::Keep`), into the
/// beam rays' words at `base` (`beam::LINE_WORDS` a corner), as `Beamed`
/// reads it.
struct Leaving<'a>(&'a mut Beams, u32);

impl cast::Keep<Writ> for Leaving<'_> {
    fn keep(&mut self, n: &cast::Node, live: Writ, stack: &[Writ; cast::DEPTH]) {
        let base = self.1;
        let beams = &mut *self.0;
        node_put(beams, base + beam::LINE_NODE, n);
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
/// (`Grid::cell_at`), or the shadow map's (`sun::cell`).
#[spirv(compute(threads(8, 8, 1), entry_point_name = "cell_cs"))]
pub fn cell_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
) {
    let g = grid(eye);
    if id.x >= g.span[0] || id.y >= g.span[1] {
        return;
    }
    let k = g.cell_window([id.x, id.y]);
    let [x, y] = g.first([id.x, id.y]);
    let t_of = |c: u32| f32::from_bits(seen_at(beams, beam::Grid::line(c) + beam::LINE_T));
    let first = g.corner_index([x - k, y - k]);
    let (mut least, mut from) = (t_of(first), first);
    let mut j = y - k;
    while j <= y + 1 + k {
        let mut i = x - k;
        while i <= x + 1 + k {
            let c = g.corner_index([i, j]);
            let t = t_of(c);
            if t < least {
                (least, from) = (t, c);
            }
            i += 1;
        }
        j += 1;
    }
    let cell = g.cell_at([id.x, id.y]);
    seen_put(beams, cell + beam::CELL_T, least.to_bits());
    seen_put(beams, cell + beam::CELL_CORNER, from);
}

/// Each sample of its observer's lattice, a thread each (observer.rs
/// `sample`): the screen's (pxl-spec-core `beam::Grid::samples`) or the
/// shadow map's (`param::sun::SAMPLE`). The node and asset it stopped at left for
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
) {
    let window: &[IVec4] = window;
    let (origin, detail) = eye_origin(eye);
    let g = grid(eye);
    if id.x >= g.samples[0] || id.y >= g.samples[1] {
        return;
    }
    let (ray, t, line) = started(
        eye,
        &g,
        beams,
        g.sample_cell([id.x, id.y]),
        g.sample([id.x, id.y]),
    );
    let tag = observed::tag(seen_at(seen, observed::FRAME), 1);
    let mut world = World {
        window,
        ground,
        looks,
        shards,
        overwrites,
        eye,
        pool: way::tree::Read(pool),
    };
    let met = resumed(
        &mut Writing { world, seen, tag },
        origin,
        &ray,
        detail,
        t,
        &line,
    );
    let faces = world.faced(origin, &ray, &met);
    let base = g.sample_at([id.x, id.y]);
    let stop = met.stop;
    node_put(beams, base + beam::SAMPLE_NODE, &stop.node);
    if met.ends == Ends::On {
        seen_put(
            beams,
            base + beam::SAMPLE_NODE + beam::NODE_ZOOM,
            beam::SAMPLE_NONE,
        );
    }
    seen_put(
        beams,
        base + beam::SAMPLE_LACKS,
        (met.ends == Ends::Lacks) as u32,
    );
    seen_put(beams, base + beam::SAMPLE_BLOCK, stop.block[0]);
    seen_put(beams, base + beam::SAMPLE_BLOCK + 1, stop.block[1]);
    seen_put(beams, base + beam::SAMPLE_AXIS, stop.axis);
    seen_put(beams, base + beam::SAMPLE_FACES, faces);
}
