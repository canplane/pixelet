// pxl-shader/beam.rs

//! The camera grid's beam rays (pxl-spec-core `beam`): each corner's walk
//! and line (`beam_cs`), where each cell's pixels start off them
//! (`cells_cs`), and the samples walked from those starts (`sample_cs`).

use pxl_spec_core::cast::{self, Detail, Ends, Met, Query, Until};
use pxl_spec_core::world::At;
use pxl_spec_core::{beam, observed, ways};
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

/// The beam rays' lines (pxl-spec-core `beam::LINE_WORDS` a corner) as a
/// walk resumes from one: the entry of each node of the line at `base`, and
/// what was handed it (`world::At::words`), its own at its zoom.
#[derive(Clone, Copy)]
pub(crate) struct Beamed<'a>(&'a Beams, u32);

impl Beamed<'_> {
    /// The node the corner's beam ray stopped at.
    fn node(self) -> cast::Node {
        node_at(self.0, self.1 + beam::LINE_NODE)
    }

    /// The first word a zoom of the line holds.
    fn step(self, zoom: u32) -> u32 {
        self.1 + beam::LINE_AT + beam::LINE_STEP * zoom
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
            at: At::of_words([seen_at(w, k + 1), seen_at(w, k + 2), seen_at(w, k + 3), seen_at(w, k + 4), seen_at(w, k + 5)]),
        }
    }
}

/// The node at word `k` of the beam rays' buffer, as a line and a sample
/// each hold one (pxl-spec-core `beam::NODE_WORDS`).
pub(crate) fn node_at(w: &Beams, k: u32) -> cast::Node {
    cast::Node {
        corner: [seen_at(w, k + beam::NODE_CORNER), seen_at(w, k + beam::NODE_CORNER + 1), seen_at(w, k + beam::NODE_CORNER + 2)],
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

/// Where the ray at the screen's tangents `tan` starts from the screen's
/// cell `cell` of `grid`: that cell's least, the corner's line it resumes
/// from, and the ray (pxl-spec-core `beam`).
pub(crate) fn started<'a>(eye: &Eye, grid: &beam::Grid, beams: &'a Beams, cell: [u32; 2], tan: [f32; 2]) -> (cast::Ray, f32, Beamed<'a>) {
    let at = grid.cell_at(cell);
    let t = f32::from_bits(seen_at(beams, at + beam::CELL_T));
    (eye_ray(eye, tan), t, Beamed(beams, beam::Grid::line(seen_at(beams, at + beam::CELL_CORNER))))
}

/// `ray`'s walk to the block that fills, resumed at `t` from the deepest
/// node of `line` holding where it is there, or, where that lies in another
/// root, from the root at it (doc/minutes/20261022-a-cell-casts-a-beam.md,
/// "Where a pixel resumes"); through `q`, reading or writing.
pub(crate) fn resumed<'a, Q: Query>(q: &mut Q, origin: [i32; 3], ray: &cast::Ray, detail: Detail, t: f32, line: &Beamed<'a>) -> Met<Q::At>
where
    Beamed<'a>: cast::Over<Q::At>,
{
    let (h, held) = line.node().over(cast::point_at(origin, ray, t));
    let h = cast::holding(origin, ray, h, t);
    let from = if held {
        (h, <Beamed<'a> as cast::Over<Q::At>>::over(line, h.zoom), t)
    } else {
        (cast::Node::ROOT, q.root(cast::Node::ROOT.index(origin)), t)
    };
    cast::walk(q, origin, ray, detail, Until::Fills, from, line, &mut ())
}

/// Each beam ray of the camera grid, a thread a corner (pxl-spec-core
/// `beam::Grid::corners`): its walk from the world's root to the first block that
/// fills or lacks, or the first split too narrow to share
/// (`cast::Until::Beam`), through the observed pyramid writing what it asks
/// (`Writing`), and its line there, for the pixels about it.
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
        pool: ways::tree::Read(pool),
    };
    let base = beam::Grid::line(g.corner_index([id.x, id.y]));
    let met = cast::rooted(&mut Writing { world, seen, tag }, origin, &ray, detail, Until::Beam, &mut Leaving(beams, base));
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
            let w = held.at.words();
            let k = base + beam::LINE_AT + beam::LINE_STEP * z;
            seen_put(beams, k, held.entry);
            seen_put(beams, k + 1, w[0]);
            seen_put(beams, k + 2, w[1]);
            seen_put(beams, k + 3, w[2]);
            seen_put(beams, k + 4, w[3]);
            seen_put(beams, k + 5, w[4]);
            z += 1;
        }
    }
}

/// Where each cell's pixels start (pxl-spec-core `beam::CELL_WORDS`), a
/// thread a cell of the screen (`beam::Grid::span`): the least of the beam
/// rays within its own window of its corners (`Grid::cell_window`), every
/// one cast since the widest window is the grid's `past`, and that corner,
/// whose line they resume from; past the lines in the beam rays' buffer
/// (`Grid::cell_at`).
#[spirv(compute(threads(8, 8, 1), entry_point_name = "cells_cs"))]
pub fn cells_cs(#[spirv(global_invocation_id)] id: UVec3, #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye, #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams) {
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

/// Each sample of the screen (pxl-spec-core `beam::Grid::samples`), a thread each:
/// its ray resumed from its cell's start as a pixel's is, walked to the
/// block that fills through the observed pyramid writing what it asks
/// (`Writing`), and the node and block it stopped at left for the pixels
/// about it (`beam::SAMPLE_WORDS`), `beam::SAMPLE_NONE` where it went on
/// to its reach, whether what it stopped at lacks, and what lies about the
/// face it entered (`World::faced`), asked here once for every pixel that
/// takes the block.
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
    let (ray, t, line) = started(eye, &g, beams, g.sample_cell([id.x, id.y]), g.sample([id.x, id.y]));
    let tag = observed::tag(seen_at(seen, observed::FRAME), 1);
    let mut world = World {
        window,
        ground,
        looks,
        shards,
        overwrites,
        eye,
        pool: ways::tree::Read(pool),
    };
    let met = resumed(&mut Writing { world, seen, tag }, origin, &ray, detail, t, &line);
    let faces = world.faced(origin, &ray, &met);
    let base = g.sample_at([id.x, id.y]);
    let stop = met.stop;
    node_put(beams, base + beam::SAMPLE_NODE, &stop.node);
    if met.ends == Ends::On {
        seen_put(beams, base + beam::SAMPLE_NODE + beam::NODE_ZOOM, beam::SAMPLE_NONE);
    }
    seen_put(beams, base + beam::SAMPLE_LACKS, (met.ends == Ends::Lacks) as u32);
    seen_put(beams, base + beam::SAMPLE_BLOCK, stop.block[0]);
    seen_put(beams, base + beam::SAMPLE_BLOCK + 1, stop.block[1]);
    seen_put(beams, base + beam::SAMPLE_AXIS, stop.axis);
    seen_put(beams, base + beam::SAMPLE_FACES, faces);
}
