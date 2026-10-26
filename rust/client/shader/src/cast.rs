// pxl-shader/cast.rs

//! Each drawn pixel's ray (`cast_cs`) off the samples about it or down the
//! observed pyramid, and those that missed it walked again (`miss_cs`); in
//! the oracle's measurement, each stop walked again from the root or from a
//! node over it (`oracle_cs`).

use pxl_spec_core::cast::{self, Ends, Met, Query, Until};
use pxl_spec_core::{beam, eye, ground, observed, proxy, ray, ways};
use spirv_std::glam::{IVec4, UVec3};
use spirv_std::{spirv, Image};

use crate::beam::{node_at, resumed, started, Beams};
use crate::eye::{eye_origin, eye_ray, grid, Eye};
use crate::observed::{seen_at, Reading, Seen, Writing, MISSED};
use crate::world::{face_of, Unlooked, World};

/// The eye's rays, a drawn pixel each, a layer a word of the ray's (`word`;
/// pxl-spec-core `ray`): the cast writes them.
pub(crate) type Rays = Image!(2D, format = r32ui, sampled = false, arrayed = true);

/// A ray's words, a layer each of `Rays`: the value it stopped on and the
/// face it entered through, then how far as f32 bits, then the four looks
/// the ground's block is drawn by, a byte each (`ground::word`), or in the
/// steps view the nodes its descent asked about, then what lies about its
/// face (`World::faced`); where it met nothing, the first two zero.
pub(crate) fn word<A: Copy>(
    ray: &cast::Ray,
    met: Met<A>,
    steps: bool,
    faces: u32,
) -> [u32; ray::WORDS as usize] {
    if met.ends != Ends::Stop {
        return [0, 0, met.visits, 0];
    }
    let s = met.stop;
    let [value, four] = s.block;
    [
        ray::first_word(value, face_of(ray, s.axis), s.node.zoom),
        s.t.to_bits(),
        if steps { met.visits } else { four },
        faces,
    ]
}
const _: () = assert!(
    ray::FACES == 3,
    "what lies about a ray's face is its last word"
);

/// The tangents across and up of the middle of drawn pixel `p` of the
/// `size` the rays are, row 0 at the top, as the screen pass forms its own
/// pixel's: the screen's half-angles' scaled by where it lies.
pub(crate) fn pixel_tan(eye: &Eye, p: UVec3, size: UVec3) -> [f32; 2] {
    let ndc_x = (p.x as f32 + 0.5) / size.x as f32 * 2.0 - 1.0;
    let ndc_y = 1.0 - (p.y as f32 + 0.5) / size.y as f32 * 2.0;
    [ndc_x * eye.tan.x, ndc_y * eye.tan.y]
}

/// A ray's words written into the rays at pixel `p`.
pub(crate) fn written(rays: &Rays, p: UVec3, w: [u32; ray::WORDS as usize]) {
    let mut k = 0;
    while k < ray::WORDS {
        // SAFETY: within the rays, bounded by their size where `p` was.
        unsafe { rays.write(UVec3::new(p.x, p.y, k), w[k as usize]) };
        k += 1;
    }
}

/// Each pixel's ray: the nearest of the blocks the samples about it stopped
/// at that it enters (pxl-spec-core `beam::Grid::about`, `cast::entry`), its
/// words, no step its own; where it enters none, or a node a sample lacks
/// at before it, the ray from its cell's start, reading the
/// observed pyramid alone (`Reading`), its binding read-only so no read of
/// it is atomic: its words where every node it asked was kept, else its
/// first word the mark `miss_cs` walks it again by (pxl-spec-core
/// `ray::MISS`). A block a sample stopped at comes with what lies about
/// its face, the sample's; a pixel that walked itself asks the world's
/// query its own (`World::faced`).
#[spirv(compute(threads(8, 8, 1), entry_point_name = "cast_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn cast_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    // Read-write as the group lays them out, which WebGPU holds a shader's
    // own access to; no atomic touches them here, so their reads stay
    // plain.
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
) {
    let (seen, beams, window): (&Seen, &Beams, &[IVec4]) = (seen, beams, window);
    let size: UVec3 = rays.query_size();
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let (origin, detail) = eye_origin(eye);
    let g = grid(eye);
    let px = pixel_tan(eye, id, size);
    let (ray, t, line) = started(eye, &g, beams, g.cell(px), px);
    let about = g.about(px);
    // The nearest block entered, at the reach where none is.
    let mut hit = cast::Stop {
        t: ray.reach,
        axis: 0,
        node: cast::Node::ROOT,
        at: 0u32,
        block: MISSED,
    };
    // What lies about its face, the sample's.
    let mut faces = 0;
    // The nearest node a sample lacks at that the ray enters.
    let mut lack = ray.reach;
    let mut j = 0;
    while j < 2 * beam::NEAR {
        let mut i = 0;
        while i < 2 * beam::NEAR {
            let k = g.sample_at([about[0] + i, about[1] + j]);
            if seen_at(beams, k + beam::SAMPLE_NODE + beam::NODE_ZOOM) != beam::SAMPLE_NONE {
                let node = node_at(beams, k + beam::SAMPLE_NODE);
                let block = [
                    seen_at(beams, k + beam::SAMPLE_BLOCK),
                    seen_at(beams, k + beam::SAMPLE_BLOCK + 1),
                ];
                let (enter, axis, enters) = cast::entry(origin, &ray, &node, block[0]);
                let lacks = seen_at(beams, k + beam::SAMPLE_LACKS) != 0;
                // A block's face the pixel enters across another axis than its
                // sample did lies under what the sample passed: not the
                // pixel's first, so it is no candidate.
                let exposed = axis == seen_at(beams, k + beam::SAMPLE_AXIS);
                if enters && lacks && enter < lack {
                    lack = enter;
                } else if enters && !lacks && exposed && enter < hit.t {
                    hit = cast::Stop {
                        t: enter,
                        axis,
                        node,
                        at: 0u32,
                        block,
                    };
                    faces = seen_at(beams, k + beam::SAMPLE_FACES);
                }
            }
            i += 1;
        }
        j += 1;
    }
    let steps = shows_steps(eye);
    if hit.t < ray.reach && hit.t < lack {
        written(
            rays,
            id,
            word(
                &ray,
                Met {
                    ends: Ends::Stop,
                    stop: hit,
                    visits: 0,
                },
                steps,
                faces,
            ),
        );
        return;
    }
    let met = resumed(&mut Reading { seen }, origin, &ray, detail, t, &line);
    if met.ends == Ends::Lacks && met.stop.block[0] == MISSED[0] {
        // SAFETY: within the rays, bounded by their size above.
        unsafe { rays.write(UVec3::new(id.x, id.y, 0), ray::MISS) };
    } else {
        let mut world = World {
            window,
            ground,
            looks: Unlooked(0),
            shards,
            overwrites,
            eye,
            pool: ways::tree::Read(pool),
        };
        written(
            rays,
            id,
            word(&ray, met, steps, world.faced(origin, &ray, &met)),
        );
    }
}

/// Each pixel `cast_cs` marked missed, its ray again from its cell's
/// start, writing what it asks into the observed pyramid (`Writing`), and
/// what lies about the face it stopped on (`World::faced`): a thread a
/// drawn pixel, as the cast's, each but the marked returning at once. It
/// runs after `cast_cs` in the cast's one pass and writes every marked
/// pixel's words whole, so the mark never outlives the pass.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "miss_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn miss_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] looks: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
) {
    let (window, beams): (&[IVec4], &Beams) = (window, beams);
    let size: UVec3 = rays.query_size();
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let first: u32 = rays.read(UVec3::new(id.x, id.y, 0));
    if first != ray::MISS {
        return;
    }
    let (origin, detail) = eye_origin(eye);
    let g = grid(eye);
    let px = pixel_tan(eye, id, size);
    let (ray, t, line) = started(eye, &g, beams, g.cell(px), px);
    let tag = observed::tag(seen_at(seen, observed::FRAME), 2);
    let mut world = World {
        window,
        ground,
        looks,
        shards,
        overwrites,
        eye,
        pool: ways::tree::Read(pool),
    };
    let met = resumed(
        &mut Writing { world, seen, tag },
        origin,
        &ray,
        detail,
        t,
        &line,
    );
    let steps = shows_steps(eye);
    let mut w = word(&ray, met, steps, world.faced(origin, &ray, &met));
    // Counted by the proxied cast's check where it runs (pxl-shader
    // proxied.rs `MISSED`).
    if steps && eye.origin.w as u32 & eye::PROXIED != 0 {
        w[2] |= crate::proxied::MISSED;
    }
    written(rays, id, w);
}

/// Whether each ray reports the nodes its descent asked (pxl-spec-core
/// `eye::SHOW_STEPS`).
pub(crate) fn shows_steps(eye: &Eye) -> bool {
    eye.origin.w as u32 & eye::SHOW_STEPS != 0
}

/// How far short of its stop the oracle starts a pixel's walk from the
/// root, a share of the stop's distance: about a drawn pixel's width there,
/// nearer than any proxy's face could start it; and how far past it the
/// point lies that names the stop's node.
pub(crate) const SHORT: f32 = 1.0 / 4096.0;

/// The oracle's counts (pxl-render gpu/ray/cast.rs `Oracle`), by word.
const STOPS: u32 = 0;
const DIFFERED: u32 = 1;
const LACKED: u32 = 2;
const VISITS: u32 = 3;
const LOCATED: u32 = 4;
const LEFT: u32 = 5;
const DIFFERED_WORD: u32 = 6;
const DIFFERED_NEARER: u32 = 7;
const DIFFERED_TRUE: u32 = 8;
const DIFFERED_LEFT: u32 = 9;
const DIFFERED_CLAMPED: u32 = 10;
const DIFFERED_COARSER: u32 = 11;
const COARSER: u32 = 12;
const UNTRUE: u32 = 13;
const TRUTH_LACKED: u32 = 14;
const CLAMPED_LOW: u32 = 15;
const CLAMPED_LOW_TRUE: u32 = 16;
const CLAMPED_HIGH: u32 = 17;
const CLAMPED_HIGH_TRUE: u32 = 18;
const UNLIKE_EYE: u32 = 19;
const HELD: u32 = 20;
/// Where the proxied cast's check counts begin among these (pxl-spec-core
/// `proxied::STOPS` on).
pub(crate) const ORACLE_BASE: u32 = 21;

/// What the steps view shows of a pixel whose walk again stopped otherwise
/// than the cast: the top of its ramp (screen_ray.wgsl).
const DIFFERED_STEPS: u32 = 1024;

/// Whether two stops are one: the same first word, at distances within
/// `SHORT` of the first's.
pub(crate) fn same(w: u32, t: f32, w2: u32, t2: f32) -> bool {
    let d = t2 - t;
    w == w2 && d <= t * SHORT && -d <= t * SHORT
}

/// The observed pyramid's entry for `n`, `observed::NONE` where it keeps
/// none, down its parents from its root as `Reading` steps them: `n.zoom`
/// reads of a children's word. [naga] `n` by value: read through a
/// reference within the loop, it lowers to a local naga refuses.
pub(crate) fn located(seen: &Seen, o: [i32; 3], n: cast::Node) -> u32 {
    let root = cast::Node {
        corner: [0; 3],
        zoom: 0,
        roots: n.roots,
    }
    .index(o);
    let mut at = if observed::keeps(root) {
        observed::octant(root)
    } else {
        observed::NONE
    };
    let mut z = 1;
    while z <= n.zoom && at != observed::NONE {
        let c = seen_at(seen, observed::entry_at(at) + observed::W_CHILDREN);
        at = if c == 0 || c == observed::CLAIMED {
            observed::NONE
        } else {
            observed::child(c - 1, observed::octant(ancestor(&n, z).index(o)))
        };
        z += 1;
    }
    at
}

/// `n`'s ancestor at zoom `z`, no finer than `n`.
pub(crate) fn ancestor(n: &cast::Node, z: u32) -> cast::Node {
    cast::Node { zoom: z, ..*n }.over((n.corner, n.roots)).0
}

/// Over a walk started at a node, what was handed each node over it, read
/// off the observed pyramid only when the walk climbs past its start
/// (`located`).
pub(crate) struct Climbed<'a> {
    pub(crate) seen: &'a Seen,
    pub(crate) origin: [i32; 3],
    pub(crate) start: cast::Node,
}

impl cast::Over<u32> for Climbed<'_> {
    fn over(&self, zoom: u32) -> u32 {
        located(self.seen, self.origin, ancestor(&self.start, zoom))
    }
}

/// Where a ray along `direction` enters the box `lo` to `hi`, planes in
/// units from its origin: the last of its near planes, 0 on an axis it does
/// not move along. Its reader holds it to 0 and up, so that 0 is the
/// unbounded plane's, and no extreme is written that a shader's literal
/// could not name.
pub(crate) fn entered(direction: [f32; 3], lo: [f32; 3], hi: [f32; 3]) -> f32 {
    let near = |d: f32, lo: f32, hi: f32| {
        if d > 0.0 {
            lo / d
        } else if d < 0.0 {
            hi / d
        } else {
            0.0
        }
    };
    let (x, y, z) = (
        near(direction[0], lo[0], hi[0]),
        near(direction[1], lo[1], hi[1]),
        near(direction[2], lo[2], hi[2]),
    );
    let xy = if x > y { x } else { y };
    if xy > z {
        xy
    } else {
        z
    }
}

/// Where the eye's words ask for it (pxl-spec-core `eye::oracle`), after the
/// cast's pass, in a pass timed apart: each pixel the cast stopped walked
/// again down the observed pyramid (`Reading`), from where a perfect proxy
/// would start it: the world's root, `SHORT` of its stop, with no line
/// handed over it (`cast::Rooted`); or the box over its stop that the
/// proxy's cut draws (pxl-spec-core `proxy`), `coarser` as the eye's
/// words say: the coarsest of the stop's ancestors at its cut's zoom or
/// finer (`proxy::asked`), clipped to its slab by its answer the pyramid
/// holds (`proxy::of`). The walk starts `SHORT` short of where the ray
/// enters that box, no later than the stop, in the node over the box's that
/// holds the ray there (`cast::holding`): the box is found off a point the
/// float put past the stop, which where the ray but grazes the stop's block
/// lies in a node beside it, and a walk started in a node the ray is not in
/// passes what it should meet. `ORACLE_LOOSE` starts it at the front, in
/// the box's node, as it was; `ORACLE_FROM_EYE` takes the box over the
/// stop the eye's own walk makes. The start's entry is found down its
/// parents (`located`) and what is over it read only where the walk climbs
/// out (`Climbed`). Where the eye's words say `ORACLE_LOCATE`, the start is
/// found and nothing walked. That walk's words are written in place of the
/// cast's, what lies about the face kept. A pixel that met nothing walks
/// nothing, as no proxy box would lie under it. A walk that lacks a node
/// the pyramid holds no answer for keeps the cast's words.
///
/// With the steps shown, each stop is also walked from the eye, the
/// world's root at 0 (`cast::rooted`), the stop the pixel's own ray truly
/// makes, and counted: each stop, those whose walk again stopped otherwise
/// (`same`), those that lacked, the visits, the reads that found the start
/// and the box's answer, and those whose walk left the start's node; of
/// those that stopped otherwise, those on another word, those nearer,
/// those the walk from the eye agrees with, those that left, those whose
/// front was held to [0, t], below and above, each with those of them the
/// eye's walk agrees with, and how much coarser their box was than the
/// stop, as it is summed over every stop; the stops the cast made that the
/// walk from the eye does not, the walks from the eye that lacked, the
/// walks again that stopped otherwise than the eye's, and the starts held
/// in a node over the box's. A pixel whose walk again stopped otherwise
/// than the cast shows the top of the steps' ramp.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "oracle_cs"))]
pub fn oracle_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &Seen,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 10)] counts: &mut [u32],
) {
    let size: UVec3 = rays.query_size();
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let first: u32 = rays.read(UVec3::new(id.x, id.y, 0));
    if first == 0 {
        return;
    }
    let t = f32::from_bits(rays.read(UVec3::new(id.x, id.y, 1)));
    let faces: u32 = rays.read(UVec3::new(id.x, id.y, ray::FACES));
    let (origin, detail) = eye_origin(eye);
    let ray = eye_ray(eye, pixel_tan(eye, id, size));
    let words = eye.origin.w as u32;
    let from = eye::oracle(words);
    let steps = shows_steps(eye);
    let mut q = Reading { seen };
    // The eye's own walk, where it is counted against or the box is taken
    // over its stop.
    let truth = if steps || words & eye::ORACLE_FROM_EYE != 0 {
        cast::rooted(&mut q, origin, &ray, detail, Until::Fills, &mut ())
    } else {
        Met {
            ends: Ends::Lacks,
            stop: cast::Stop {
                t: 0.0,
                axis: 0,
                node: cast::Node::ROOT,
                at: observed::NONE,
                block: MISSED,
            },
            visits: 0,
        }
    };
    let true_word = word(&ray, truth, false, 0)[0];
    let truth_lacked = truth.ends == Ends::Lacks;
    // The stop the box is taken over: the cast's, or the eye's walk's.
    let (over, over_t) = if words & eye::ORACLE_FROM_EYE != 0 && truth.ends == Ends::Stop {
        (true_word, truth.stop.t)
    } else {
        (first, t)
    };
    let hit = over >> ray::ZOOM_SHIFT & ((1 << ray::ZOOM_BITS) - 1);
    let (mut low, mut high, mut held) = (false, false, false);
    let (met, start, coarser) = if from == eye::ORACLE_ROOT {
        let at = q.root(cast::Node::ROOT.index(origin));
        let met = cast::walk(
            &mut q,
            origin,
            &ray,
            detail,
            Until::Fills,
            (cast::Node::ROOT, at, t - t * SHORT),
            &cast::Rooted(at),
            &mut (),
        );
        (met, cast::Node::ROOT, hit)
    } else {
        let coarser = from - eye::ORACLE_CUT;
        let (p, roots) = cast::point_at(origin, &ray, over_t + over_t * SHORT);
        let leaf = cast::Node {
            corner: p,
            zoom: pxl_spec_core::FINEST_ZOOM,
            roots,
        };
        // The box: the first of its ancestors down from the root at its
        // cut's zoom or finer, the stop's own at the latest.
        let mut z = 0;
        let (mut boxed, mut asked, mut node, mut cut) = (leaf, [0, 0], leaf_proxy(), 0);
        while z <= hit {
            boxed = ancestor(&leaf, z);
            (asked, node, cut) = proxy::asked(origin, detail, coarser, &boxed);
            if z >= cut {
                break;
            }
            z += 1;
        }
        let mut at = located(seen, origin, boxed);
        let i = q.node(&mut at, boxed.zoom, boxed.index(origin), asked, 0.0);
        let b = proxy::of(origin, &boxed, node, cut, i, ground::UNBOUNDED[0]);
        let enter = entered(ray.direction, b.lo, b.hi);
        (low, high) = (enter < 0.0, enter > over_t);
        let front = if low {
            0.0
        } else if high {
            over_t
        } else {
            enter
        };
        let (start, t0) = if words & eye::ORACLE_LOOSE != 0 {
            (boxed, front)
        } else {
            let t0 = front - front * SHORT;
            let start = cast::holding(origin, &ray, boxed, t0);
            if start.zoom != boxed.zoom {
                held = true;
                at = located(seen, origin, start);
            }
            (start, t0)
        };
        if words & eye::ORACLE_LOCATE != 0 {
            // Found and not walked; written only where no box is, so what it
            // found is not optimized away and nothing is written.
            if at ^ t0.to_bits() == u32::MAX && t0 < 0.0 {
                written(rays, id, [at, 0, 0, 0]);
            }
            return;
        }
        let met = cast::walk(
            &mut q,
            origin,
            &ray,
            detail,
            Until::Fills,
            (start, at, t0),
            &Climbed {
                seen,
                origin,
                start,
            },
            &mut (),
        );
        (met, start, hit - boxed.zoom)
    };
    let mut w = word(&ray, met, steps, faces);
    let lacked = met.ends == Ends::Lacks;
    let differed = !lacked && !same(first, t, w[0], met.stop.t);
    if steps && differed {
        w[2] = DIFFERED_STEPS;
    }
    if !lacked {
        written(rays, id, w);
    }
    if steps {
        crate::add(counts, STOPS, 1);
        crate::add(counts, VISITS, met.visits);
        let (n, a) = (met.stop.node, ancestor(&met.stop.node, start.zoom));
        let left = !(n.zoom >= start.zoom
            && a.roots[0] == start.roots[0]
            && a.roots[1] == start.roots[1]
            && a.roots[2] == start.roots[2]
            && a.corner[0] == start.corner[0]
            && a.corner[1] == start.corner[1]
            && a.corner[2] == start.corner[2]);
        crate::add(counts, COARSER, coarser);
        if from != eye::ORACLE_ROOT {
            crate::add(counts, LOCATED, start.zoom + 1);
            if left {
                crate::add(counts, LEFT, 1);
            }
            if held {
                crate::add(counts, HELD, 1);
            }
        }
        let eyes = !truth_lacked && !lacked && same(true_word, truth.stop.t, w[0], met.stop.t);
        if truth_lacked {
            crate::add(counts, TRUTH_LACKED, 1);
        } else {
            if !same(true_word, truth.stop.t, first, t) {
                crate::add(counts, UNTRUE, 1);
            }
            if !lacked && !eyes {
                crate::add(counts, UNLIKE_EYE, 1);
            }
        }
        if lacked {
            crate::add(counts, LACKED, 1);
        } else if differed {
            crate::add(counts, DIFFERED, 1);
            if w[0] != first {
                crate::add(counts, DIFFERED_WORD, 1);
            }
            if met.stop.t < t {
                crate::add(counts, DIFFERED_NEARER, 1);
            }
            if eyes {
                crate::add(counts, DIFFERED_TRUE, 1);
            }
            if left {
                crate::add(counts, DIFFERED_LEFT, 1);
            }
            if low || high {
                crate::add(counts, DIFFERED_CLAMPED, 1);
            }
            if low {
                crate::add(counts, CLAMPED_LOW, 1);
                if eyes {
                    crate::add(counts, CLAMPED_LOW_TRUE, 1);
                }
            }
            if high {
                crate::add(counts, CLAMPED_HIGH, 1);
                if eyes {
                    crate::add(counts, CLAMPED_HIGH_TRUE, 1);
                }
            }
            crate::add(counts, DIFFERED_COARSER, coarser);
        }
    }
}

/// A box of no planes, which the cut's first ask overwrites.
fn leaf_proxy() -> proxy::Proxy {
    proxy::Proxy {
        says: proxy::NONE,
        lo: [0.0; 3],
        hi: [0.0; 3],
    }
}
