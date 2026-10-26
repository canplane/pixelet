// pxl-shader/proxy.rs

//! The cut a rasterized proxy draws (pxl-spec-core `proxy`), built each frame
//! down the pyramid a round a dispatch from the seeds over the eye's
//! coarsest window: each node a round holds asked what it is (`proxy_cs`),
//! its box kept or its descendants queued for the next round, whose dispatch
//! the one after lays (`proxy_args_cs`). Nothing outlives the frame.

use pxl_spec_core::answer::Kind;
use pxl_spec_core::cast::{Node, Query};
use pxl_spec_core::ground::UNBOUNDED;
use pxl_spec_core::proxy::{self, Proxy, BOX, UNDER};
use pxl_spec_core::world::At;
use spirv_std::glam::{IVec4, UVec3, Vec4};
use spirv_std::memory::{Scope, Semantics};
use spirv_std::spirv;

use crate::add;
use crate::eye::{eye_origin, Eye};
use crate::world::{Unlooked, World};

/// Word `i` of the cut's counts, read atomically, as every other touch of
/// them is: none past them.
fn count(counts: &mut [u32], i: u32) -> u32 {
    if (i as usize) < counts.len() {
        // SAFETY: just bounded by the slice's own length.
        unsafe {
            spirv_std::arch::atomic_load::<
                u32,
                { Scope::QueueFamily as u32 },
                { Semantics::NONE.bits() },
            >(counts.get_unchecked_mut(i as usize))
        }
    } else {
        0
    }
}

/// Word `i` of the cut's scratch set to `v`; none past it.
fn put(data: &mut [u32], i: u32, v: u32) {
    pxl_spec_core::fill::put(data, i, v);
}

/// Whether the box `p`, in units from the eye, may hold something the eye
/// sees: its nearest point within the eye's reach, and some of it inside
/// each of the four planes the screen's edges and the eye make, each
/// tested at the box's corner farthest inside it.
fn seen(eye: &Eye, p: &Proxy) -> bool {
    let near = |a: usize| {
        let v = if p.lo[a] > 0.0 {
            p.lo[a]
        } else if p.hi[a] < 0.0 {
            p.hi[a]
        } else {
            0.0
        };
        v * v
    };
    let reach = eye.forward_far.w;
    if near(0) + near(1) + near(2) > reach * reach {
        return false;
    }
    let (f, r, u) = (eye.forward_far, eye.right_tau, eye.up);
    let inside = |n: [f32; 3]| {
        let most = |a: usize| {
            let (lo, hi) = (n[a] * p.lo[a], n[a] * p.hi[a]);
            if lo > hi {
                lo
            } else {
                hi
            }
        };
        most(0) + most(1) + most(2) >= 0.0
    };
    let plane = |t: f32, e: Vec4, s: f32| [t * f.x + s * e.x, t * f.y + s * e.y, t * f.z + s * e.z];
    inside(plane(eye.tan.x, r, -1.0))
        && inside(plane(eye.tan.x, r, 1.0))
        && inside(plane(eye.tan.y, u, -1.0))
        && inside(plane(eye.tan.y, u, 1.0))
}

/// Node `k` of queue `q` in the cut's scratch (pxl-spec-core
/// `proxy::NODE_WORDS`).
fn node_at(data: &[u32], q: u32, k: u32) -> Node {
    let w = proxy::queue_at(q, k);
    let packed = pxl_spec_core::at(data, w + 3);
    proxy::node_of([
        pxl_spec_core::at(data, w),
        pxl_spec_core::at(data, w + 1),
        pxl_spec_core::at(data, w + 2),
        packed,
    ])
}

/// Each node of the round the cut is at, a thread a node of the queue it
/// reads (pxl-spec-core `proxy::COUNT_LEVEL`, `proxy::COUNT_QUEUE`): kept
/// where the eye may see it, asked what it is through the world's one query
/// with nothing handed down (`World::node`, `world::At::of`) at the zooms
/// the cast asks it at, with its floor (`proxy::floor`); a box written into
/// the scratch and counted, a node over the cut's descendants queued for
/// the next round, as many zooms finer as no box lies between
/// (`proxy::steps`), each that meets its clipped box along z and that the
/// eye may see; none at all where the eye is not in the open
/// (`proxy::aired`). A box is its node, its planes grown as the raster draws
/// them (`proxy::inflated`), its entry in the observed pyramid none, the
/// cast's to read, and whether it is about the eye (`proxy::eyed`), listed
/// in the counts (`proxy::COUNT_EYE`). What would pass the scratch's room is
/// dropped and counted.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "proxy_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn proxy_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] counts: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] data: &mut [u32],
) {
    let level = count(counts, proxy::COUNT_LEVEL);
    let q = level % 2;
    let n = count(counts, proxy::COUNT_QUEUE + q);
    if id.x >= n || id.x >= proxy::QUEUE_NODES {
        return;
    }
    add(counts, proxy::COUNT_NODES, 1);
    let node = node_at(data, q, id.x);
    let (origin, detail) = eye_origin(eye);
    let coarser = count(counts, proxy::COUNT_COARSER);
    let (zooms, planes, cut) = proxy::asked(origin, detail, coarser, &node);
    if !seen(eye, &planes) {
        return;
    }
    let mut world = World {
        window,
        ground,
        looks: Unlooked(0),
        shards,
        overwrites,
        eye,
        pool: pxl_spec_core::ways::tree::Read(pool),
    };
    // An eye not in the open has no proxy: its roots are kept back.
    if level == 0 && !proxy::aired(&world, origin) {
        add(counts, proxy::COUNT_SHUT, 1);
        return;
    }
    let c = node.index(origin);
    let mut at = At::of(&world, (node.zoom, c));
    let i = world.node(&mut at, node.zoom, c, zooms, 0.0);
    let floor = if i.kind == Kind::Split {
        let lanes = world;
        proxy::floor(&mut world, &lanes, origin, detail, &node)
    } else {
        UNBOUNDED[0]
    };
    let p = proxy::of(origin, &node, planes, cut, i, floor);
    if p.says == BOX {
        let k = add(counts, proxy::COUNT_BOXES, 1);
        if floor == UNBOUNDED[0] {
            add(counts, proxy::COUNT_UNFLOORED, 1);
        }
        if k >= proxy::BOXES {
            add(counts, proxy::COUNT_DROPPED, 1);
            add(counts, proxy::COUNT_DROPPED_BOXES, 1);
            return;
        }
        let w = proxy::box_at(k);
        let words = proxy::node_words(&node);
        let mut j = 0;
        while j < proxy::NODE_WORDS {
            put(data, w + j, words[j as usize]);
            j += 1;
        }
        // Grown as the raster draws it, and about the eye or not.
        let g = proxy::inflated(&p, f32::from_bits(count(counts, proxy::COUNT_PIXEL)));
        let b = [g.lo[0], g.lo[1], g.lo[2], g.hi[0], g.hi[1], g.hi[2]];
        let mut j = 0;
        while j < 6 {
            put(data, w + proxy::BOX_LO + j, b[j as usize].to_bits());
            j += 1;
        }
        put(data, w + proxy::BOX_ENTRY, pxl_spec_core::observed::NONE);
        let eyed = proxy::eyed(&g, [eye.tan.x, eye.tan.y]);
        put(
            data,
            w + proxy::BOX_FLAGS,
            if eyed { proxy::EYED } else { 0 },
        );
        if eyed {
            let e = add(counts, proxy::COUNT_EYES, 1);
            if e < proxy::EYES {
                add(counts, proxy::COUNT_EYE + e, k + 1);
            } else {
                add(counts, proxy::COUNT_DROPPED, 1);
            }
        }
        return;
    }
    if p.says != UNDER {
        return;
    }
    // Its descendants as many zooms finer as no box lies between
    // (`proxy::steps`), each queued that meets its clipped box along z and
    // that the eye may see.
    let k = proxy::steps(origin, detail, coarser, &node);
    let mut d = 0;
    while d < 1 << (3 * k) {
        let under = proxy::descendant(&node, k, d);
        let (_, b, _) = proxy::asked(origin, detail, coarser, &under);
        if b.lo[2] < p.hi[2] && p.lo[2] < b.hi[2] && seen(eye, &b) {
            let s = add(counts, proxy::COUNT_QUEUE + 1 - q, 1);
            if s >= proxy::QUEUE_NODES {
                add(counts, proxy::COUNT_DROPPED, 1);
            } else {
                let words = proxy::node_words(&under);
                let w = proxy::queue_at(1 - q, s);
                let mut j = 0;
                while j < proxy::NODE_WORDS {
                    put(data, w + j, words[j as usize]);
                    j += 1;
                }
            }
        }
        d += 1;
    }
}

/// The next round's dispatch, a thread for the cut's whole: a workgroup a
/// `proxy_cs` workgroup's share of the queue the round just read filled,
/// that queue emptied for the round after, the round stepped; and the boxes'
/// draw args (pxl-spec-core `proxy::ARGS_DRAW`), a box's corners an
/// instance, as a raster pass would read them.
#[spirv(compute(threads(1, 1, 1), entry_point_name = "proxy_args_cs"))]
pub fn proxy_args_cs(
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] counts: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 10)] args: &mut [u32],
) {
    let level = pxl_spec_core::at(counts, proxy::COUNT_LEVEL);
    let q = level % 2;
    let next = pxl_spec_core::at(counts, proxy::COUNT_QUEUE + 1 - q);
    let next = if next > proxy::QUEUE_NODES {
        proxy::QUEUE_NODES
    } else {
        next
    };
    pxl_spec_core::fill::put(counts, proxy::COUNT_QUEUE + q, 0);
    pxl_spec_core::fill::put(counts, proxy::COUNT_LEVEL, level + 1);
    let boxes = pxl_spec_core::at(counts, proxy::COUNT_BOXES);
    let boxes = if boxes > proxy::BOXES {
        proxy::BOXES
    } else {
        boxes
    };
    let a = [
        (next + proxy::WORKGROUP - 1) / proxy::WORKGROUP,
        1,
        1,
        proxy::BOX_VERTICES,
        boxes,
        0,
        0,
    ];
    let mut j = 0;
    while j < a.len() {
        pxl_spec_core::fill::put(args, j as u32, a[j]);
        j += 1;
    }
}
