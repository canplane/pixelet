// pxl-shader/proxied.rs

//! The cast a rasterized proxy starts (pxl-spec-core `proxied`;
//! doc/minutes/20261024-a-proxy-only-starts-the-ray.md): each box's node and
//! the line down to it written into the observed pyramid once
//! (`proxied_entry_cs`), which of the cast's dispatches the frame runs laid
//! off the cut's counts (`proxied_route_cs`), each drawn pixel's ray walked
//! from where the raster's front-most box starts it, reading the pyramid
//! (`proxied_cs`) or writing it (`proxied_write_cs`), those a reading walk
//! could not answer walked again from their start writing it
//! (`proxied_miss_cs`), and, with the steps shown, each checked against the
//! eye's own walk (`proxied_check_cs`); or each pixel walked as the cast
//! walks one no sample answers, from its cell's beam line, but no sooner
//! than its box starts it (`proxied_beam_cs`).

use pxl_spec_core::cast::{self, Detail, Ends, Met, Query, Until};
use pxl_spec_core::world::At;
use pxl_spec_core::{at, eye, observed, proxied, proxy, ray, ways};
use spirv_std::glam::{IVec4, UVec3, UVec4};
use spirv_std::{spirv, Image};

use crate::beam::{resumed, started as cell_started, Beams};
use crate::cast::{
    ancestor, entered, located, pixel_tan, same, shows_steps, word, written, Climbed, Rays,
};
use crate::eye::{eye_origin, eye_ray, grid, Eye};
use crate::observed::{seen_at, Reading, Seen, Writ, Writing, MISSED as MISSED_BLOCK};
use crate::world::{Looks, Unlooked, World};

/// The raster's front-most box at each drawn pixel, its number plus one, 0
/// where none covers it (pxl-render gpu/ray/proxy.rs), read as an array of
/// one layer; its depth there, its second word, is not read: a walk starts
/// where the ray enters the box (`begun`).
type Index = Image!(2D, type = u32, sampled = true, arrayed = true);

/// The cut's counts (pxl-spec-core `proxy::COUNTS`), copied into a uniform
/// ahead of the cast's pass: read where a pass binds the most storage a
/// device must give.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Header {
    words: [UVec4; proxy::COUNTS as usize / 4],
}
const _: () = assert!(proxy::COUNTS % 4 == 0);

/// Word `i` of the cut's counts.
fn told(h: &Header, i: u32) -> u32 {
    let v = h.words[(i >> 2) as usize];
    match i & 3 {
        0 => v.x,
        1 => v.y,
        2 => v.z,
        _ => v.w,
    }
}

/// The three f32s from word `i` of the cut's counts.
fn told_floats(h: &Header, i: u32) -> [f32; 3] {
    [
        f32::from_bits(told(h, i)),
        f32::from_bits(told(h, i + 1)),
        f32::from_bits(told(h, i + 2)),
    ]
}

/// A box's planes in units from the eye, grown as the raster drew them
/// (pxl-spec-core `proxy::inflated`), off the cut's record at `k`.
fn planes(data: &[u32], k: u32) -> ([f32; 3], [f32; 3]) {
    let w = proxy::box_at(k);
    (
        floats(data, w + proxy::BOX_LO),
        floats(data, w + proxy::BOX_HI),
    )
}

/// The three f32s from word `i` of `s`. [naga] Not a closure over `s`: a
/// closure holding a slice is a value naga takes no pointer in.
fn floats(s: &[u32], i: u32) -> [f32; 3] {
    [
        f32::from_bits(at(s, i)),
        f32::from_bits(at(s, i + 1)),
        f32::from_bits(at(s, i + 2)),
    ]
}

/// The node of the cut's record at `k`.
fn node_of(data: &[u32], k: u32) -> cast::Node {
    let w = proxy::box_at(k);
    proxy::node_of([
        at(data, w),
        at(data, w + 1),
        at(data, w + 2),
        at(data, w + 3),
    ])
}

/// Where a ray along `direction` leaves the box `lo` to `hi`: the first of
/// its far planes, `reach` on an axis it does not move along.
fn left(direction: [f32; 3], lo: [f32; 3], hi: [f32; 3], reach: f32) -> f32 {
    let far = |d: f32, lo: f32, hi: f32| {
        if d > 0.0 {
            hi / d
        } else if d < 0.0 {
            lo / d
        } else {
            reach
        }
    };
    let (x, y, z) = (
        far(direction[0], lo[0], hi[0]),
        far(direction[1], lo[1], hi[1]),
        far(direction[2], lo[2], hi[2]),
    );
    let xy = if x < y { x } else { y };
    if xy < z {
        xy
    } else {
        z
    }
}

/// The later of `t` and `u`. [naga] Not a closure over the start: one
/// holding a reference is a value naga takes no pointer in.
fn later(t: f32, u: f32) -> f32 {
    if t > u {
        t
    } else {
        u
    }
}

fn sooner(t: f32, u: f32) -> f32 {
    if t < u {
        t
    } else {
        u
    }
}

/// Where a pixel's walk starts, as the proxy has it: from the eye to where
/// the ray leaves the last box about the eye it enters (`eyed`, the
/// header's `COUNT_EYE`), 0 where it enters none; then the raster's
/// front-most box (`boxed`, its number, none where it drew none); and
/// where the ray leaves the eye's coarsest window (`cover`, `COUNT_COVER`),
/// past which nothing is held; the sky where no box covers it and it leaves
/// that window past its reach.
struct Start {
    eyed: f32,
    boxed: u32,
    cover: f32,
    sky: bool,
}

fn started(h: &Header, data: &[u32], index: &Index, p: UVec3, ray: &cast::Ray) -> Start {
    let d = ray.direction;
    let c = proxy::COUNT_COVER;
    let cover = left(d, told_floats(h, c), told_floats(h, c + 3), ray.reach);
    let mut eyed = 0.0;
    let eyes = told(h, proxy::COUNT_EYES);
    let mut k = 0;
    while k < eyes && k < proxy::EYES {
        let b = told(h, proxy::COUNT_EYE + k);
        if b != 0 {
            let (lo, hi) = planes(data, b - 1);
            let (enter, leave) = (entered(d, lo, hi), left(d, lo, hi, ray.reach));
            let enter = if enter < 0.0 { 0.0 } else { enter };
            if leave > enter && leave > eyed {
                eyed = leave;
            }
        }
        k += 1;
    }
    // [naga] Fetched at u32s: its SPIR-V front finds a coordinate's vector
    // only among the module's types, and the eye's zooms make a u32 pair.
    let kept: UVec4 = index.fetch(UVec3::new(p.x, p.y, 0));
    let boxed = kept.x;
    Start {
        eyed,
        boxed,
        cover,
        sky: boxed == 0 && cover >= ray.reach,
    }
}

/// Where a pixel's walk past the boxes about the eye starts: in which node,
/// from where, whether that node is over its box's (`held`), and whether it
/// is the world's root (`rooted`), where no box covers the ray or the ray
/// leaves the eye's coarsest window nearer, from there.
struct Begun {
    node: cast::Node,
    t: f32,
    held: bool,
    rooted: bool,
}

/// The start past the boxes about the eye, no nearer than their end. In the
/// box the raster kept, the front-most of the grown boxes at the pixel: a
/// stop lies at least the box's grown margin `m` past where the ray enters
/// the grown box it lies in, far past the raster's rounding, so no stop
/// lies within `m` past where it enters this one (`t_b`). So it starts where
/// it enters the box's own node, where that is no later than `t_b + m`, in
/// that node; else at `t_b + m`, in the node over it that holds the ray
/// there (`cast::holding`), where it but grazes the box's margin.
fn begun(origin: [i32; 3], detail: Detail, ray: &cast::Ray, data: &[u32], s: &Start) -> Begun {
    let root = Begun {
        node: cast::Node::ROOT,
        t: later(s.cover, s.eyed),
        held: false,
        rooted: true,
    };
    if s.boxed == 0 {
        return root;
    }
    let b = node_of(data, s.boxed - 1);
    let d = ray.direction;
    let (lo, hi) = planes(data, s.boxed - 1);
    let (_, own, _) = proxy::asked(origin, detail, 0, &b);
    let m = own.lo[0] - lo[0];
    let grown = entered(d, lo, hi);
    let grown = if grown > 0.0 { grown } else { 0.0 };
    let (from, to) = (
        entered(d, own.lo, own.hi),
        left(d, own.lo, own.hi, ray.reach),
    );
    let t = later(sooner(later(from, grown), grown + m), s.eyed);
    if s.cover < t {
        root
    } else if from <= t && t < to {
        Begun {
            node: b,
            t,
            held: false,
            rooted: false,
        }
    } else {
        Begun {
            node: cast::holding(origin, ray, b, t),
            t,
            held: true,
            rooted: false,
        }
    }
}

/// How a pixel `proxied_cs` could not finish is marked in its words for
/// `proxied_miss_cs`: its first word `ray::MISS`, then where it starts
/// again, f32 bits, then its box's number plus one, none from the world's
/// root, then whether its start was held over the box's node (`HELD_START`)
/// or it lacked within the boxes about the eye (`EYED_LACK`).
const HELD_START: u32 = 1;
const EYED_LACK: u32 = 2;

/// What the steps show of a pixel `proxied_miss_cs`, or in the beam's walk
/// `miss_cs`, walked again: its visits with this bit, the top of the steps'
/// ramp; the check counts and strips it.
pub(crate) const MISSED: u32 = 1 << 31;

/// Each box of the frame's cut, a thread a box: the line from its root down
/// to its node held in the observed pyramid (`line`), so every walk it
/// starts keeps what it writes under it; its entry into its record (pxl-spec-core
/// `proxy::BOX_ENTRY`). It runs in the cast's pass after the forgetting,
/// the beam rays' pass's tag its own, as no beam ray runs.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "proxied_entry_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn proxied_entry_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] looks: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] data: &mut [u32],
    #[spirv(uniform, descriptor_set = 0, binding = 17)] header: &Header,
) {
    let boxes = told(header, proxy::COUNT_BOXES);
    if id.x >= boxes || id.x >= proxy::BOXES {
        return;
    }
    let (origin, _) = eye_origin(eye);
    let n = node_of(data, id.x);
    let world = World {
        window,
        ground,
        looks,
        shards,
        overwrites,
        eye,
        pool: ways::tree::Read(pool),
    };
    let tag = observed::tag(seen_at(seen, observed::FRAME), 0);
    let mut w = Writing { world, seen, tag };
    let writ = line(&mut w, origin, n);
    pxl_spec_core::fill::put(data, proxy::box_at(id.x) + proxy::BOX_ENTRY, writ.entry);
}

/// The line from `n`'s root down to `n` held in the observed pyramid
/// through `w`, each node's children's tile taken as a walk takes them:
/// what `w` hands `n`. Its answers are not asked: the first walk to read
/// one misses it and walks again writing it, under an entry now held.
/// [naga] `n` by value: read through a reference within the loop, it
/// lowers to a local naga refuses; and no node asked outside a walk.
fn line<L: Looks>(w: &mut Writing<'_, L>, o: [i32; 3], n: cast::Node) -> Writ {
    let root = cast::Node {
        corner: [0; 3],
        zoom: 0,
        roots: n.roots,
    };
    let mut writ = w.root(root.index(o));
    let mut z = 1;
    while z <= n.zoom {
        writ = w.child(&writ, z, ancestor(&n, z).index(o));
        z += 1;
    }
    writ
}

/// Which of the cast's dispatches the frame runs (pxl-spec-core `proxied`),
/// a thread for the whole: the proxied cast's where the cut stands, its
/// counts saying nothing was dropped, the eye is in the open and the eye's
/// coarsest window is laid out; the cast as built else; each the
/// workgroups `sizes` gives it, the others none.
#[spirv(compute(threads(1, 1, 1), entry_point_name = "proxied_route_cs"))]
pub fn proxied_route_cs(
    #[spirv(storage_buffer, descriptor_set = 0, binding = 11)] counts: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 15)] sizes: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 16)] args: &mut [u32],
) {
    // A window not laid out yet has no planes, and the cut holds nothing.
    let c = proxy::COUNT_COVER;
    let laid = at(counts, c) != at(counts, c + 3);
    let stands =
        at(counts, proxy::COUNT_DROPPED) == 0 && at(counts, proxy::COUNT_SHUT) == 0 && laid;
    let mut r = 0;
    while r < proxied::ROUTES {
        let on = proxied::proxied(r) == stands;
        let mut j = 0;
        while j < 3 {
            let w = 3 * r + j;
            let v = if on {
                at(sizes, w)
            } else if j == 0 {
                0
            } else {
                1
            };
            pxl_spec_core::fill::put(args, w, v);
            j += 1;
        }
        r += 1;
    }
}

/// Each drawn pixel's ray walked from where the proxy starts it
/// (`started`, `begun`), reading the observed pyramid alone (`Reading`),
/// its binding read-only: first from the eye over the boxes about it,
/// where it enters one, then from its start, its box's entry where the
/// start is the box's node, found down its parents where it was held over
/// it (`located`), what is over it read only where the walk climbs out
/// (`Climbed`). Its words where it held every node asked, what lies about
/// the face asked of the world's query, as no sample hands it over
/// (`World::faced`); none (the sky) where no box covers it and the window
/// holds it to its reach; else the mark `proxied_miss_cs` walks it again
/// by, with where it starts.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "proxied_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn proxied_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &Seen,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] data: &[u32],
    #[spirv(descriptor_set = 0, binding = 13)] index: &Index,
    #[spirv(uniform, descriptor_set = 0, binding = 17)] header: &Header,
) {
    let size: UVec3 = rays.query_size();
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let (origin, detail) = eye_origin(eye);
    let ray = eye_ray(eye, pixel_tan(eye, id, size));
    let s = started(header, data, index, id, &ray);
    if s.sky && s.eyed <= 0.0 {
        written(rays, id, [0, 0, 0, 0]);
        return;
    }
    let mut world = World {
        window,
        ground,
        looks: Unlooked(0),
        shards,
        overwrites,
        eye,
        pool: ways::tree::Read(pool),
    };
    let mut q = Reading { seen };
    let mut before = 0;
    if s.eyed > 0.0 {
        let near = cast::Ray::new(ray.direction, s.eyed);
        let met = cast::rooted(&mut q, origin, &near, detail, Until::Fills, &mut ());
        if met.ends == Ends::Lacks {
            written(rays, id, [ray::MISS, 0, 0, EYED_LACK]);
            return;
        }
        if met.ends == Ends::Stop {
            let faces = world.faced(origin, &ray, &met);
            written(rays, id, word(&ray, met, shows_steps(eye), faces));
            return;
        }
        before = met.visits;
    }
    let b = begun(origin, detail, &ray, data, &s);
    let mut met = if b.rooted {
        let root = q.root(b.node.index(origin));
        cast::walk(
            &mut q,
            origin,
            &ray,
            detail,
            Until::Fills,
            (b.node, root, b.t),
            &cast::Rooted(root),
            &mut (),
        )
    } else {
        let entry = if b.held {
            located(seen, origin, b.node)
        } else {
            at(data, proxy::box_at(s.boxed - 1) + proxy::BOX_ENTRY)
        };
        cast::walk(
            &mut q,
            origin,
            &ray,
            detail,
            Until::Fills,
            (b.node, entry, b.t),
            &Climbed {
                seen,
                origin,
                start: b.node,
            },
            &mut (),
        )
    };
    met.visits += before;
    if met.ends == Ends::Lacks {
        let boxed = if b.rooted { 0 } else { s.boxed };
        let how = if b.held { HELD_START } else { 0 };
        written(rays, id, [ray::MISS, b.t.to_bits(), boxed, how]);
        return;
    }
    let faces = world.faced(origin, &ray, &met);
    written(rays, id, word(&ray, met, shows_steps(eye), faces));
}

/// Over a writing walk started at a node, what was handed each node over
/// it: no entry, so nothing over the start is written again, and what the
/// world's query hands a node with nothing handed down (`world::At::of`),
/// which is what a descent hands it. Read only where the walk climbs out.
struct ClimbedWriting<'a, L> {
    world: World<'a, L>,
    origin: [i32; 3],
    start: cast::Node,
}

impl<L: Looks + Copy> cast::Over<Writ> for ClimbedWriting<'_, L> {
    fn over(&self, zoom: u32) -> Writ {
        let n = ancestor(&self.start, zoom);
        Writ {
            entry: observed::NONE,
            at: At::of(&self.world, (zoom, n.index(self.origin))),
        }
    }
}

/// `ray`'s walk from `start` at `t`, writing the observed pyramid
/// (`Writing`, its pass's `tag`): from the world's root where `rooted`,
/// else from the node with `entry` and what the world's query hands it
/// (`world::At::of`), what is over it as `ClimbedWriting` hands it.
#[allow(clippy::too_many_arguments)]
fn written_walk<L: Looks + Copy>(
    world: World<'_, L>,
    seen: &mut Seen,
    tag: u32,
    origin: [i32; 3],
    detail: Detail,
    ray: &cast::Ray,
    (start, entry, t, rooted): (cast::Node, u32, f32, bool),
) -> Met<Writ> {
    let at = At::of(&world, (start.zoom, start.index(origin)));
    let mut w = Writing { world, seen, tag };
    if rooted {
        let root = w.root(start.index(origin));
        cast::walk(
            &mut w,
            origin,
            ray,
            detail,
            Until::Fills,
            (start, root, t),
            &cast::Rooted(root),
            &mut (),
        )
    } else {
        cast::walk(
            &mut w,
            origin,
            ray,
            detail,
            Until::Fills,
            (start, Writ { entry, at }, t),
            &ClimbedWriting {
                world,
                origin,
                start,
            },
            &mut (),
        )
    }
}

/// Each pixel `proxied_cs` marked, its ray again from where that one
/// started it (its marked words), writing what it asks into the observed
/// pyramid (`written_walk`): from the eye where it lacked within the boxes
/// about the eye, from the world's root where no box started it, else in the
/// node it started in; and what lies about the face it stopped on
/// (`World::faced`). With the steps shown, its visits carry `MISSED`. Each
/// but the marked returns at once.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "proxied_miss_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn proxied_miss_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] looks: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] data: &[u32],
) {
    let size: UVec3 = rays.query_size();
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let first: u32 = rays.read(UVec3::new(id.x, id.y, 0));
    if first != ray::MISS {
        return;
    }
    let t = f32::from_bits(rays.read(UVec3::new(id.x, id.y, 1)));
    let boxed: u32 = rays.read(UVec3::new(id.x, id.y, 2));
    let how: u32 = rays.read(UVec3::new(id.x, id.y, ray::FACES));
    let (origin, detail) = eye_origin(eye);
    let ray = eye_ray(eye, pixel_tan(eye, id, size));
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
    let from = if how == EYED_LACK || boxed == 0 {
        let t = if how == EYED_LACK { 0.0 } else { t };
        (cast::Node::ROOT, observed::NONE, t, true)
    } else {
        let b = node_of(data, boxed - 1);
        if how == HELD_START {
            let start = cast::holding(origin, &ray, b, t);
            (start, located(seen, origin, start), t, false)
        } else {
            let entry = at(data, proxy::box_at(boxed - 1) + proxy::BOX_ENTRY);
            (b, entry, t, false)
        }
    };
    let met = written_walk(world, seen, tag, origin, detail, &ray, from);
    let steps = shows_steps(eye);
    let mut w = word(&ray, met, steps, world.faced(origin, &ray, &met));
    if steps {
        w[2] |= MISSED;
    }
    written(rays, id, w);
}

/// Each drawn pixel's ray walked from where the proxy starts it as
/// `proxied_cs` walks it, but writing the observed pyramid as it goes
/// (`written_walk`), so none is walked again; and what lies about the face
/// it stopped on (`World::faced`). The one walk a pixel where the eye's
/// words ask for it (pxl-spec-core `eye::PROXIED_WRITE`).
#[spirv(compute(threads(8, 8, 1), entry_point_name = "proxied_write_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn proxied_write_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] looks: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] data: &[u32],
    #[spirv(descriptor_set = 0, binding = 13)] index: &Index,
    #[spirv(uniform, descriptor_set = 0, binding = 17)] header: &Header,
) {
    let size: UVec3 = rays.query_size();
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let (origin, detail) = eye_origin(eye);
    let ray = eye_ray(eye, pixel_tan(eye, id, size));
    let s = started(header, data, index, id, &ray);
    if s.sky && s.eyed <= 0.0 {
        written(rays, id, [0, 0, 0, 0]);
        return;
    }
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
    let steps = shows_steps(eye);
    let mut before = 0;
    if s.eyed > 0.0 {
        let near = cast::Ray::new(ray.direction, s.eyed);
        let from = (cast::Node::ROOT, observed::NONE, 0.0, true);
        let met = written_walk(world, seen, tag, origin, detail, &near, from);
        if met.ends != Ends::On {
            let faces = world.faced(origin, &ray, &met);
            written(rays, id, word(&ray, met, steps, faces));
            return;
        }
        before = met.visits;
    }
    let b = begun(origin, detail, &ray, data, &s);
    let entry = if b.rooted {
        observed::NONE
    } else if b.held {
        located(seen, origin, b.node)
    } else {
        at(data, proxy::box_at(s.boxed - 1) + proxy::BOX_ENTRY)
    };
    let from = (b.node, entry, b.t, b.rooted);
    let mut met = written_walk(world, seen, tag, origin, detail, &ray, from);
    met.visits += before;
    let faces = world.faced(origin, &ray, &met);
    written(rays, id, word(&ray, met, steps, faces));
}

/// Count `k` of the check (pxl-spec-core `proxied`) by `v`, past the
/// oracle's own among the counts.
fn checked(checks: &mut [u32], k: u32, v: u32) {
    crate::add(checks, crate::cast::ORACLE_BASE + k, v);
}

/// With the steps shown, after the proxied cast, each drawn pixel checked
/// against the eye's own walk through the world's one query (`World`),
/// which lacks nothing the windows hold, into the oracle's counts past its
/// own (pxl-spec-core `proxied::STOPS` on): the stops, those left to the
/// sky, those of them the eye's walk stops in, the stops the eye's walk
/// makes otherwise, its walks that lacked among the stops, the starts past
/// where it stops, the starts held over their box's node, the visits
/// (`ray::WORDS`' third, the steps'), and the pixels `proxied_miss_cs`
/// walked again (`MISSED`).
#[spirv(compute(threads(8, 8, 1), entry_point_name = "proxied_check_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn proxied_check_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 10)] checks: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] data: &[u32],
    #[spirv(descriptor_set = 0, binding = 13)] index: &Index,
    #[spirv(uniform, descriptor_set = 0, binding = 17)] header: &Header,
) {
    let size: UVec3 = rays.query_size();
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let first: u32 = rays.read(UVec3::new(id.x, id.y, 0));
    let t = f32::from_bits(rays.read(UVec3::new(id.x, id.y, 1)));
    let steps: u32 = rays.read(UVec3::new(id.x, id.y, 2));
    let (origin, detail) = eye_origin(eye);
    let ray = eye_ray(eye, pixel_tan(eye, id, size));
    let s = started(header, data, index, id, &ray);
    let mut world = World {
        window,
        ground,
        looks: Unlooked(0),
        shards,
        overwrites,
        eye,
        pool: ways::tree::Read(pool),
    };
    let truth = cast::rooted(&mut world, origin, &ray, detail, Until::Fills, &mut ());
    checked(checks, proxied::VISITS, steps & !MISSED);
    if steps & MISSED != 0 {
        checked(checks, proxied::MISSED, 1);
    }
    let stops = truth.ends == Ends::Stop;
    if first == 0 {
        checked(checks, proxied::SKY, 1);
        if stops {
            checked(checks, proxied::HOLES, 1);
        }
        return;
    }
    checked(checks, proxied::STOPS, 1);
    if truth.ends == Ends::Lacks {
        checked(checks, proxied::EYE_LACKED, 1);
    } else if !same(word(&ray, truth, false, 0)[0], truth.stop.t, first, t) {
        checked(checks, proxied::UNLIKE_EYE, 1);
    }
    // The beam's walk starts no later than the box's front, the others at
    // their own start.
    let (start, held) = if eye.origin.w as u32 & eye::PROXIED_BEAM != 0 {
        (fronted(data, index, id, &ray), false)
    } else {
        let b = begun(origin, detail, &ray, data, &s);
        (b.t, b.held)
    };
    if stops && start > truth.stop.t {
        checked(checks, proxied::LATE, 1);
    }
    if held {
        checked(checks, proxied::HELD, 1);
    }
}

/// Each drawn pixel's ray as `cast_cs` walks it where no sample answers
/// it: from its cell's beam line (pxl-shader beam.rs `started`, `resumed`),
/// what is over its start read off the beam ray's kept line, reading the
/// observed pyramid; but from where the ray enters the box the raster kept
/// at the pixel, where that is later than its cell's start: no stop lies
/// before it (`begun`). No sample is walked and none answers a pixel. Its
/// words, what lies about its face asked of the world's query
/// (`World::faced`), or the mark `miss_cs` walks it again by, from its
/// cell's start. Where the eye's words say `eye::PROXIED_BEAM`.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "proxied_beam_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn proxied_beam_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &Seen,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] data: &[u32],
    #[spirv(descriptor_set = 0, binding = 13)] index: &Index,
) {
    let size: UVec3 = rays.query_size();
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let (origin, detail) = eye_origin(eye);
    let g = grid(eye);
    let px = pixel_tan(eye, id, size);
    let (ray, t, line) = cell_started(eye, &g, beams, g.cell(px), px);
    let t = later(t, fronted(data, index, id, &ray));
    let met = resumed(&mut Reading { seen }, origin, &ray, detail, t, &line);
    if met.ends == Ends::Lacks && met.stop.block[0] == MISSED_BLOCK[0] {
        // SAFETY: within the rays, bounded by their size above.
        unsafe { rays.write(UVec3::new(id.x, id.y, 0), ray::MISS) };
        return;
    }
    let mut world = World {
        window,
        ground,
        looks: Unlooked(0),
        shards,
        overwrites,
        eye,
        pool: ways::tree::Read(pool),
    };
    let faces = world.faced(origin, &ray, &met);
    written(rays, id, word(&ray, met, shows_steps(eye), faces));
}

/// Where the ray through drawn pixel `p` enters the box the raster kept
/// there, as the raster drew it (the record's planes, grown and clipped to
/// its slab), at least 0; 0 where it kept none.
fn fronted(data: &[u32], index: &Index, p: UVec3, ray: &cast::Ray) -> f32 {
    // [naga] Fetched at u32s, as `started` fetches.
    let kept: UVec4 = index.fetch(UVec3::new(p.x, p.y, 0));
    if kept.x == 0 {
        return 0.0;
    }
    let (lo, hi) = planes(data, kept.x - 1);
    let front = entered(ray.direction, lo, hi);
    if front > 0.0 {
        front
    } else {
        0.0
    }
}
