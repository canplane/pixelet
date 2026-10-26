// pxl-shader/proxied.rs

//! The cast a rasterized proxy starts (pxl-spec-core `proxied`;
//! doc/minutes/20261024-a-proxy-only-starts-the-ray.md): each box's observed
//! entry found once (`proxied_entry_cs`), which of the cast's dispatches the
//! frame runs laid off the cut's counts (`proxied_route_cs`), each drawn
//! pixel's ray walked from where the raster's front-most box starts it
//! (`proxied_cs`), those the observed pyramid could not answer walked again
//! from the eye writing it (`proxied_miss_cs`), and, with the steps shown,
//! each checked against the eye's own walk (`proxied_check_cs`).

use pxl_spec_core::cast::{self, Detail, Ends, Query, Until};
use pxl_spec_core::world::At;
use pxl_spec_core::{at, observed, proxied, proxy, ray, ways};
use spirv_std::glam::{IVec4, UVec3, UVec4};
use spirv_std::{spirv, Image};

use crate::cast::{entered, located, pixel_tan, same, shows_steps, word, written, Climbed, Rays};
use crate::eye::{eye_origin, eye_ray, Eye};
use crate::observed::{seen_at, Reading, Seen, Writ, Writing};
use crate::world::{Unlooked, World};

/// The raster's front-most box at each drawn pixel, its number plus one, 0
/// where none covers it (pxl-render gpu/ray/proxy.rs), read as an array of
/// one layer; its depth there, its second word, is not read: a walk starts
/// at its box's node's own entry (`begun`).
type Index = Image!(2D, type = u32, sampled = true, arrayed = true);

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

fn started(counts: &[u32], data: &[u32], index: &Index, p: UVec3, ray: &cast::Ray) -> Start {
    let d = ray.direction;
    let c = proxy::COUNT_COVER;
    let cover = left(d, floats(counts, c), floats(counts, c + 3), ray.reach);
    let mut eyed = 0.0;
    let eyes = at(counts, proxy::COUNT_EYES);
    let mut k = 0;
    while k < eyes && k < proxy::EYES {
        let b = at(counts, proxy::COUNT_EYE + k);
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

/// The node a pixel's walk past the boxes about the eye starts in, and
/// where: where the ray enters its box as the raster drew it (the record's
/// planes, grown and clipped to its slab), no nearer than the eye's boxes'
/// end. That box is the front-most of the grown boxes at the pixel, and a
/// stop lies a grown margin past its own box's front, far past the raster's
/// rounding, so nothing the ray meets lies before it; the entry into the
/// box's ungrown node may (a neighbour met within the margin). In the box's
/// node where that point lies within its own planes, else in the node over
/// it that holds the ray there (`cast::holding`), and whether it was held
/// so: where the ray enters the box through its margin. The world's root,
/// from where the ray leaves the eye's coarsest window, where no box covers
/// it or that is nearer.
fn begun(
    origin: [i32; 3],
    detail: Detail,
    ray: &cast::Ray,
    data: &[u32],
    s: &Start,
) -> (cast::Node, f32, bool) {
    if s.boxed == 0 {
        return (cast::Node::ROOT, later(s.cover, s.eyed), false);
    }
    let b = node_of(data, s.boxed - 1);
    let d = ray.direction;
    let (lo, hi) = planes(data, s.boxed - 1);
    let enter = entered(d, lo, hi);
    let t = later(if enter > 0.0 { enter } else { 0.0 }, s.eyed);
    let (_, own, _) = proxy::asked(origin, detail, 0, &b);
    let (from, to) = (
        entered(d, own.lo, own.hi),
        left(d, own.lo, own.hi, ray.reach),
    );
    if s.cover < t {
        (cast::Node::ROOT, later(s.cover, s.eyed), false)
    } else if from <= t && t < to {
        (b, t, false)
    } else {
        (cast::holding(origin, ray, b, t), t, true)
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

/// How a pixel `proxied_cs` could not finish is marked in its words for
/// `proxied_miss_cs`: its first word `ray::MISS`, then where it starts
/// again, f32 bits, then its box's number plus one, none from the world's
/// root, then whether its start was held over the box's node (`HELD_START`)
/// or it lacked within the boxes about the eye (`EYED_LACK`).
const HELD_START: u32 = 1;
const EYED_LACK: u32 = 2;

/// What the steps show of a pixel `proxied_miss_cs` walked again: its
/// visits with this bit, the top of the steps' ramp; the check counts and
/// strips it.
const MISSED: u32 = 1 << 31;

/// Each box of the frame's cut, a thread a box: its node's entry in the
/// observed pyramid as this frame's forgetting left it, found down its
/// parents once (`located`), into its record (pxl-spec-core
/// `proxy::BOX_ENTRY`) for every pixel it starts. It runs in the cast's
/// pass after the forgetting, and no entry goes but by it.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "proxied_entry_cs"))]
pub fn proxied_entry_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &Seen,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 11)] counts: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] data: &mut [u32],
) {
    let boxes = at(counts, proxy::COUNT_BOXES);
    if id.x >= boxes || id.x >= proxy::BOXES {
        return;
    }
    let (origin, _) = eye_origin(eye);
    let n = node_of(data, id.x);
    let entry = located(seen, origin, n);
    pxl_spec_core::fill::put(data, proxy::box_at(id.x) + proxy::BOX_ENTRY, entry);
}

/// Which of the cast's dispatches the frame runs (pxl-spec-core `proxied`),
/// a thread for the whole: the proxied cast's where the cut stands, its
/// counts saying nothing was dropped, the eye is in the open and the eye's
/// coarsest window is laid out; the cast as built else; each the workgroups `sizes` gives it, the others none.
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
    #[spirv(storage_buffer, descriptor_set = 0, binding = 11)] counts: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] data: &[u32],
    #[spirv(descriptor_set = 0, binding = 13)] index: &Index,
) {
    let size: UVec3 = rays.query_size();
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let (origin, detail) = eye_origin(eye);
    let ray = eye_ray(eye, pixel_tan(eye, id, size));
    let s = started(counts, data, index, id, &ray);
    if s.sky && s.eyed <= 0.0 {
        written(rays, id, [0, 0, 0, 0]);
        return;
    }
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
            return;
        }
        before = met.visits;
    }
    let (start, t, held) = begun(origin, detail, &ray, data, &s);
    let mut met = if start.zoom == 0 && s.boxed == 0 {
        let root = q.root(start.index(origin));
        cast::walk(
            &mut q,
            origin,
            &ray,
            detail,
            Until::Fills,
            (start, root, t),
            &cast::Rooted(root),
            &mut (),
        )
    } else {
        let entry = if held {
            located(seen, origin, start)
        } else {
            at(data, proxy::box_at(s.boxed - 1) + proxy::BOX_ENTRY)
        };
        cast::walk(
            &mut q,
            origin,
            &ray,
            detail,
            Until::Fills,
            (start, entry, t),
            &Climbed {
                seen,
                origin,
                start,
            },
            &mut (),
        )
    };
    met.visits += before;
    if met.ends == Ends::Lacks {
        let boxed = if start.zoom == 0 && s.boxed == 0 {
            0
        } else {
            s.boxed
        };
        let how = if held { HELD_START } else { 0 };
        written(rays, id, [ray::MISS, t.to_bits(), boxed, how]);
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

/// Over a writing walk started at a node, what was handed each node over
/// it: no entry, so nothing over the start is written again, and what the
/// world's query hands a node with nothing handed down (`world::At::of`),
/// which is what a descent hands it. Read only where the walk climbs out.
struct ClimbedWriting<'a> {
    world: World<'a, &'a [u32]>,
    origin: [i32; 3],
    start: cast::Node,
}

impl cast::Over<Writ> for ClimbedWriting<'_> {
    fn over(&self, zoom: u32) -> Writ {
        let n = crate::cast::ancestor(&self.start, zoom);
        Writ {
            entry: observed::NONE,
            at: At::of(&self.world, (zoom, n.index(self.origin))),
        }
    }
}

/// Each pixel `proxied_cs` marked, its ray again from where that one
/// started it (its marked words), writing what it asks into the observed
/// pyramid (`Writing`): from the eye where it lacked within the boxes about
/// the eye, from the world's root where no box started it, else in the node
/// it started in with what the world's query hands that node
/// (`world::At::of`) and its entry; and what lies about the face it
/// stopped on (`World::faced`). With the steps shown, its visits carry
/// `MISSED`. Each but the marked returns at once.
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
    let met = if how == EYED_LACK || boxed == 0 {
        let from = if how == EYED_LACK { 0.0 } else { t };
        let mut w = Writing { world, seen, tag };
        let root = w.root(cast::Node::ROOT.index(origin));
        cast::walk(
            &mut w,
            origin,
            &ray,
            detail,
            Until::Fills,
            (cast::Node::ROOT, root, from),
            &cast::Rooted(root),
            &mut (),
        )
    } else {
        let b = node_of(data, boxed - 1);
        let (start, entry) = if how == HELD_START {
            let start = cast::holding(origin, &ray, b, t);
            (start, located(seen, origin, start))
        } else {
            (b, at(data, proxy::box_at(boxed - 1) + proxy::BOX_ENTRY))
        };
        let writ = Writ {
            entry,
            at: At::of(&world, (start.zoom, start.index(origin))),
        };
        let mut w = Writing { world, seen, tag };
        cast::walk(
            &mut w,
            origin,
            &ray,
            detail,
            Until::Fills,
            (start, writ, t),
            &ClimbedWriting {
                world,
                origin,
                start,
            },
            &mut (),
        )
    };
    let steps = shows_steps(eye);
    let mut w = word(&ray, met, steps, world.faced(origin, &ray, &met));
    if steps {
        w[2] |= MISSED;
    }
    written(rays, id, w);
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
/// makes otherwise, its walks that lacked, the starts past where it stops,
/// the starts held over their box's node, the visits (`ray::WORDS`' third,
/// the steps'), and the pixels `proxied_miss_cs` walked again (`MISSED`).
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
    #[spirv(storage_buffer, descriptor_set = 0, binding = 11)] counts: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] data: &[u32],
    #[spirv(descriptor_set = 0, binding = 13)] index: &Index,
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
    let s = started(counts, data, index, id, &ray);
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
    if truth.ends == Ends::Lacks {
        checked(checks, proxied::EYE_LACKED, 1);
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
    if truth.ends != Ends::Lacks && !same(word(&ray, truth, false, 0)[0], truth.stop.t, first, t) {
        checked(checks, proxied::UNLIKE_EYE, 1);
    }
    let (_, start, held) = begun(origin, detail, &ray, data, &s);
    if stops && start > truth.stop.t {
        checked(checks, proxied::LATE, 1);
    }
    if held {
        checked(checks, proxied::HELD, 1);
    }
}
