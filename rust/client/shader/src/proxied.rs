// pxl-shader/proxied.rs

//! The cast a rasterized proxy starts (pxl-spec-core `proxied`;
//! doc/minutes/20261024-a-proxy-only-starts-the-ray.md): each box's observed
//! entry found once (`proxied_entry_cs`), which of the cast's dispatches the
//! frame runs laid off the cut's counts (`proxied_route_cs`), each drawn
//! pixel's ray walked from where the raster's front-most box starts it
//! (`proxied_cs`), those the observed pyramid could not answer walked again
//! from the eye writing it (`proxied_miss_cs`), and, with the steps shown,
//! each checked against the eye's own walk (`proxied_check_cs`).

use pxl_spec_core::cast::{self, Detail, Ends, Met, Query, Until};
use pxl_spec_core::{at, observed, proxied, proxy, ray, ways};
use spirv_std::glam::{IVec4, UVec3, UVec4};
use spirv_std::{spirv, Image};

use crate::cast::{entered, located, pixel_tan, same, shows_steps, word, written, Climbed, Rays};
use crate::eye::{eye_origin, eye_ray, Eye};
use crate::observed::{seen_at, Reading, Seen, Writing};
use crate::world::{Unlooked, World};

/// The raster's front-most box at each drawn pixel, its number plus one, 0
/// where none covers it, and its depth there, reversed, as f32 bits
/// (pxl-spec-core `proxy::bound`; pxl-render gpu/ray/proxy.rs), read as an
/// array of one layer. [naga] The depth comes in this colour, not its depth
/// target: naga's SPIR-V front takes no depth image.
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
/// header's `COUNT_EYE`), none where it enters none; then the raster's
/// front-most box (`boxed`, its number, none where it drew none) from the
/// nearer of its depth there (`proxy::bound`), where the ray enters that
/// box and where it leaves the eye's coarsest window (`COUNT_COVER`), past
/// which nothing is held; where no box covers it, from where it leaves that
/// window, and nowhere (the sky) where it leaves it past its reach.
struct Start {
    eyed: f32,
    boxed: u32,
    t: f32,
    sky: bool,
}

fn started(
    counts: &[u32],
    data: &[u32],
    index: &Index,
    p: UVec3,
    ray: &cast::Ray,
    cos: f32,
) -> Start {
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
    let bound = proxy::bound(f32::from_bits(kept.y), cos);
    let mut t = cover;
    if boxed != 0 {
        let (lo, hi) = planes(data, boxed - 1);
        let enter = entered(d, lo, hi);
        if enter < t {
            t = enter;
        }
        if bound < t {
            t = bound;
        }
        if t < 0.0 {
            t = 0.0;
        }
    }
    Start {
        eyed,
        boxed,
        t,
        sky: boxed == 0 && cover >= ray.reach,
    }
}

/// A pixel's walk down the observed pyramid (`Reading`) from its start
/// (`started`): from the eye over the boxes about it, where it enters one,
/// then from the start in the node over its box's that holds the ray there
/// (`cast::holding`), its box's entry where that is the box's own and found
/// down its parents else (`located`), what is over it read only where the
/// walk climbs out (`Climbed`); from the world's root there where no box
/// covers it. Whether the start was held over its box's node.
fn walked(
    seen: &Seen,
    data: &[u32],
    origin: [i32; 3],
    ray: &cast::Ray,
    detail: Detail,
    s: &Start,
) -> (Met<u32>, bool) {
    let mut q = Reading { seen };
    let mut before = 0;
    if s.eyed > 0.0 {
        let near = cast::Ray::new(ray.direction, s.eyed);
        let met = cast::rooted(&mut q, origin, &near, detail, Until::Fills, &mut ());
        if met.ends != Ends::On {
            return (met, false);
        }
        before = met.visits;
    }
    let t = if s.t > s.eyed { s.t } else { s.eyed };
    let (mut met, held) = if s.boxed != 0 {
        let b = node_of(data, s.boxed - 1);
        let start = cast::holding(origin, ray, b, t);
        let held = start.zoom != b.zoom;
        let entry = if held {
            located(seen, origin, start)
        } else {
            at(data, proxy::box_at(s.boxed - 1) + proxy::BOX_ENTRY)
        };
        let met = cast::walk(
            &mut q,
            origin,
            ray,
            detail,
            Until::Fills,
            (start, entry, t),
            &Climbed {
                seen,
                origin,
                start,
            },
            &mut (),
        );
        (met, held)
    } else {
        let root = q.root(cast::Node::ROOT.index(origin));
        let met = cast::walk(
            &mut q,
            origin,
            ray,
            detail,
            Until::Fills,
            (cast::Node::ROOT, root, t),
            &cast::Rooted(root),
            &mut (),
        );
        (met, false)
    };
    met.visits += before;
    (met, held)
}

/// The cosine of the ray through drawn pixel `p` with the eye's forward,
/// as `proxy::bound` reads the raster's depth by.
fn cos_of(eye: &Eye, ray: &cast::Ray) -> f32 {
    let f = eye.forward_far;
    ray.direction[0] * f.x + ray.direction[1] * f.y + ray.direction[2] * f.z
}

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
/// counts saying nothing was dropped and the eye is in the open; the cast
/// as built else; each the workgroups `sizes` gives it, the others none.
#[spirv(compute(threads(1, 1, 1), entry_point_name = "proxied_route_cs"))]
pub fn proxied_route_cs(
    #[spirv(storage_buffer, descriptor_set = 0, binding = 11)] counts: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 15)] sizes: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 16)] args: &mut [u32],
) {
    let stands = at(counts, proxy::COUNT_DROPPED) == 0 && at(counts, proxy::COUNT_SHUT) == 0;
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
/// (`started`, `walked`), reading the observed pyramid alone, its binding
/// read-only: its words where it held every node asked, what lies about the
/// face asked of the world's query, as no sample hands it over
/// (`World::faced`); none (the sky) where no box covers it and the window
/// holds it to its reach; else its first word the mark `proxied_miss_cs`
/// walks it again by (pxl-spec-core `ray::MISS`).
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
    let s = started(counts, data, index, id, &ray, cos_of(eye, &ray));
    if s.sky && s.eyed <= 0.0 {
        written(rays, id, [0, 0, 0, 0]);
        return;
    }
    let (met, _) = walked(seen, data, origin, &ray, detail, &s);
    if met.ends == Ends::Lacks {
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
    written(
        rays,
        id,
        word(&ray, met, shows_steps(eye), world.faced(origin, &ray, &met)),
    );
}

/// Each pixel `proxied_cs` marked, its ray again from the eye, writing what
/// it asks into the observed pyramid (`Writing`), and what lies about the
/// face it stopped on (`World::faced`): from the eye, not from its box, as
/// these are few and a start from a box would bind more than a pass may.
/// Each but the marked returns at once.
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
) {
    let size: UVec3 = rays.query_size();
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let first: u32 = rays.read(UVec3::new(id.x, id.y, 0));
    if first != ray::MISS {
        return;
    }
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
    let met = cast::rooted(
        &mut Writing { world, seen, tag },
        origin,
        &ray,
        detail,
        Until::Fills,
        &mut (),
    );
    written(
        rays,
        id,
        word(&ray, met, shows_steps(eye), world.faced(origin, &ray, &met)),
    );
}

/// Count `k` of the check (pxl-spec-core `proxied`) by `v`, past the
/// oracle's own among the counts.
fn checked(checks: &mut [u32], k: u32, v: u32) {
    crate::add(checks, crate::cast::ORACLE_BASE + k, v);
}

/// With the steps shown, after the proxied cast, each drawn pixel checked
/// against the eye's own walk, the world's root at 0 (`cast::rooted`), into
/// the oracle's counts past its own (pxl-spec-core `proxied::STOPS` on):
/// the stops, those left to the sky, those of them the eye's walk stops in,
/// the stops the eye's walk makes otherwise, its walks that lacked, the
/// starts past where it stops, the starts held over their box's node, and
/// the visits (`ray::WORDS`' third, the steps').
#[spirv(compute(threads(8, 8, 1), entry_point_name = "proxied_check_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn proxied_check_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &Seen,
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
    let visits: u32 = rays.read(UVec3::new(id.x, id.y, 2));
    let (origin, detail) = eye_origin(eye);
    let ray = eye_ray(eye, pixel_tan(eye, id, size));
    let s = started(counts, data, index, id, &ray, cos_of(eye, &ray));
    let truth = cast::rooted(
        &mut Reading { seen },
        origin,
        &ray,
        detail,
        Until::Fills,
        &mut (),
    );
    checked(checks, proxied::VISITS, visits);
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
    let start = if s.t > s.eyed { s.t } else { s.eyed };
    if stops && s.boxed != 0 && start > truth.stop.t {
        checked(checks, proxied::LATE, 1);
    }
    if s.boxed != 0 {
        let b = node_of(data, s.boxed - 1);
        if cast::holding(origin, &ray, b, start).zoom != b.zoom {
            checked(checks, proxied::HELD, 1);
        }
    }
}
