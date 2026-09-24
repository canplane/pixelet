// pxl-kernel/world/pyramid/window.rs

//! Which part of the pyramid a view holds: which levels, and which pages of
//! each (issue/20260921-level-step-and-zoom-are-one-number.md). One window,
//! whichever layer's pages fill it.
//!
//! What a screen shows at once spans a few octaves of distance, and each
//! octave costs the same, so the client holds a fixed number of pages per
//! level rather than a radius: a ring of HELD_AXIS² around the view at every
//! level of a window — the levels the ground in view wants, at most
//! WINDOW_LEVELS — of which the inner RENDER_AXIS² is what is drawn from
//! (window.json says why each is what it is).

use pxl_render::look::lod::REFERENCE_THRESHOLD_0;
use pxl_spec::space::indexer::L3_IDX;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pyramid::{chunk_span_of, CHUNK_CELLS, PAGE_CHUNKS};
use pxl_spec::space::pyramid::{span_of, LevelPage, TOP_LEVEL};
use pxl_spec::space::xy::TO_XY;

use crate::control::camera::config::camera_cfg;
use crate::control::camera::store::camera_store;
use crate::layer::environment::fog::{pivot_height, reach_at};

use super::chunk::Chunk;
use super::window_spec::{HELD_AXIS, RENDER_AXIS, WINDOW_LEVELS};

const _: () = assert!(HELD_AXIS % 2 == 1 && RENDER_AXIS % 2 == 1 && RENDER_AXIS < HELD_AXIS, "a ring is centred on the view's page");
const _: () = assert!(WINDOW_LEVELS <= TOP_LEVEL + 1, "a window no wider than the pyramid");

/// How far a level's drawn ring reaches past the page the anchor stands in,
/// wherever in it the anchor stands: the pages each side of it, whole.
pub fn drawn_reach(level: u32) -> f64 {
    (RENDER_AXIS / 2) as f64 * span_of(level)
}

/// The level whose chunks draw the step the ladder wants at `dist` from the
/// eye. Step 2^j is wanted out to REFERENCE_THRESHOLD_0·2^j, and a level is drawn one
/// step coarser than its own cells, each 2×2 of them averaged — the leaves as
/// they are too, for step 1 — so step 2^j is drawn from level j − 1.
///
/// A page is the fold's vote, and a vote quantizes: a thin thing goes at one
/// level and swells at another. Averaged one level down, what reaches the
/// screen is what a pixel of that ground would show, and the vote sits under
/// a mean. Every step also has one way to be drawn: shown at its own cells, a
/// level would draw step 2^k as the vote where the level below draws it as
/// the mean of the same four cells, and the two would part in colour, in
/// objects and in textures at every level's edge.
fn level_at(dist: f64) -> u32 {
    ((dist / REFERENCE_THRESHOLD_0).log2().ceil().max(0.0) as u32).saturating_sub(1)
}

/// The levels held now, finest first: the ones the ground the view shows
/// wants, and no others.
///
/// The camera sits `d·sinθ` above its pivot, θ the tilt above the horizon,
/// and the pivot some height above the ground under it,
/// and the field of view spans θ ± fov/2 below it. The nearest ground it shows
/// is along the lower edge, straight down once that edge passes the vertical;
/// the farthest along the upper edge, or where fog has hidden it once that
/// edge reaches the horizon. Each wants the level whose step the ladder asks
/// for at that distance, and the window runs from the one to the other: a
/// view looking down at a street holds the leaves alone, and a view out to
/// the horizon the few octaves it spans. WINDOW_LEVELS is the most it can
/// span, which is what `.fixed` is sized for.
pub fn window() -> std::ops::Range<u32> {
    let view = camera_store().view_state;
    let v = crate::world::frame::view::view();
    window_for(view.dist, view.polar, pivot_height(), v.width / v.height)
}

/// `window` for a camera `dist` from its pivot at `polar` from straight down,
/// the pivot `pivot_height` above the ground, through a viewport `aspect`
/// wide for its height.
pub fn window_for(dist: f64, polar: f64, pivot_height: f64, aspect: f64) -> std::ops::Range<u32> {
    let tilt = std::f64::consts::FRAC_PI_2 - polar;
    let half_fov = camera_cfg::projection::FOV.to_radians() / 2.0;
    let height = pivot_height + dist * tilt.sin();
    let near = height / (tilt + half_fov).min(std::f64::consts::FRAC_PI_2).sin();
    let upper = tilt - half_fov;
    // How far the ground is worth drawing from here: the haze near the
    // ground, the planet's edge from high up (environment/fog.rs).
    let reach = reach_at(height);
    let far = if upper > 0.0 { (height / upper.sin()).min(reach) } else { reach };
    // Decoration is scattered from the leaves and reaches past step 1, so
    // the leaves are held while any ground it can reach is in view.
    let base = if near < DECOR_REACH { 0 } else { level_at(near).min(TOP_LEVEL) };
    // The top is what the farthest ground wants, and at least the level
    // whose ring draws all the ground in view: the ring is measured from the
    // view centre rather than the eye, and a wide screen's upper corners see
    // further than its middle, so the ladder's level alone can fall short.
    let seen = ground_reach(dist, polar, pivot_height, aspect, reach) + ANCHOR_SLACK;
    let covering = (seen / drawn_reach(0)).log2().ceil().max(0.0) as u32;
    let top = level_at(far.max(near)).max(covering).clamp(base, (base + WINDOW_LEVELS - 1).min(TOP_LEVEL));
    base..top + 1
}

/// How far the range anchor can stand from the true view centre: a chunk's
/// diagonal, since it is chunk-aligned (world/frame/range_anchor.rs).
const ANCHOR_SLACK: f64 = chunk_span_of(0) * std::f64::consts::SQRT_2;

/// The farthest ground a view shows, from the view centre: the farthest of
/// the frustum's corners on the ground, since a frustum meets a plane in a
/// convex footprint and the farthest point of one is a corner. Where an
/// upper corner meets no ground inside the reach, the footprint ends on the
/// reach's circle around the eye's foot instead, as far round it as that corner's
/// bearing: the view centre stands ahead of the eye, so the arc's far end is
/// the circle's farthest visible point from it, and the part of the circle
/// behind the eye is not in view at all.
fn ground_reach(dist: f64, polar: f64, pivot_height: f64, aspect: f64, reach: f64) -> f64 {
    let half_v = camera_cfg::projection::FOV.to_radians() / 2.0;
    let (tan_v, tan_h) = (half_v.tan(), half_v.tan() * aspect);
    let horiz = dist * polar.sin();
    let height = pivot_height + dist * polar.cos();
    let len = horiz.hypot(dist * polar.cos()).max(f64::MIN_POSITIVE);
    // Forward, right and up, the eye `horiz` behind the pivot along -y.
    let f = [0.0, horiz / len, -dist * polar.cos() / len];
    let u = [0.0, -f[2], f[1]];
    let ground_reach = (reach.powi(2) - height.powi(2)).max(0.0).sqrt();
    let mut worst: f64 = 0.0;
    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let d = [sx * tan_h, f[1] + sy * tan_v * u[1], f[2] + sy * tan_v * u[2]];
        let dl = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let t = if d[2] < 0.0 { height * dl / -d[2] } else { f64::INFINITY };
        let far = if t > reach {
            let cos = d[1] / d[0].hypot(d[1]).max(f64::MIN_POSITIVE);
            (ground_reach * ground_reach + horiz * horiz - 2.0 * ground_reach * horiz * cos).max(0.0).sqrt()
        } else {
            (t * d[0] / dl).hypot(-horiz + t * d[1] / dl)
        };
        worst = worst.max(far);
    }
    worst
}

/// How far from the eye the drawn ground runs, for a camera `dist` from its
/// pivot at `polar`, its window topped at `top`: the top ring's reach, which
/// is measured from the view centre, less how far behind that centre the eye
/// stands and the slack the anchor's own quantization takes.
pub fn drawn_from_eye(dist: f64, polar: f64, top: u32) -> f64 {
    (drawn_reach(top) - dist * polar.sin() - ANCHOR_SLACK).max(0.0)
}

/// How far decoration can be drawn: what the leaves' ring draws to. A kind
/// is drawn while its extent is two of the step's cells, which the ladder
/// wants out to REFERENCE_THRESHOLD_0 · extent / 2 — `decor_reach` — and every kind is
/// held inside this, so decoration never needs a coarser record than the
/// leaves it is scattered from
/// (issue/20260921-level-step-and-zoom-are-one-number.md).
pub const DECOR_REACH: f64 = (RENDER_AXIS / 2) as f64 * span_of(0);

/// How far a decor kind of `extent` is drawn.
pub fn decor_reach(extent: f64) -> f64 {
    REFERENCE_THRESHOLD_0 * extent / 2.0
}

/// The page the anchor stands in at `level`: what every ring is centred on.
pub fn centre_page(level: u32, anchor: IVec) -> LevelPage {
    LevelPage::over(level, TO_XY(L3_IDX(anchor)))
}

/// A level's ring around the page the anchor stands in at that level: every
/// page within `axis / 2` of it in both axes, nearest first by Chebyshev
/// ring, then by row.
pub fn ring(level: u32, anchor: IVec, axis: u32) -> Vec<LevelPage> {
    let center = centre_page(level, anchor);
    let r = (axis / 2) as i64;
    let mut out: Vec<(i64, LevelPage)> = Vec::with_capacity((axis * axis) as usize);
    for dy in -r..=r {
        for dx in -r..=r {
            let page = ivec::add(center.page, ivec::gather(dx, dy, 0));
            out.push((dx.abs().max(dy.abs()), LevelPage::new(level, page)));
        }
    }
    out.sort_by_key(|(ring, _)| *ring);
    out.into_iter().map(|(_, p)| p).collect()
}

/// How far `page` lies from the anchor's page at its level, in pages along
/// the further axis: 0 for the anchor's own, 1 for the ring drawn from.
pub fn ring_of(anchor: IVec, page: LevelPage) -> u32 {
    let center = centre_page(page.level, anchor);
    let d = ivec::sub(page.page, center.page);
    ivec::X(d).unsigned_abs().max(ivec::Y(d).unsigned_abs()) as u32
}

/// A level's held pages: its ring whole, the part the level below also
/// holds included. Those are what a chunk draws where the finer page under
/// it has not come, so the ground coarsens there rather than opening; a
/// clipmap keeps every level whole for the same reason, and `.fixed` has a
/// record for each (mem/fixed.rs).
pub fn held(level: u32, anchor: IVec) -> Vec<LevelPage> {
    ring(level, anchor, HELD_AXIS)
}

/// The ground a level's drawn ring covers around the anchor's page, as a
/// corner and a side.
pub fn ring_rect(level: u32, anchor: IVec) -> (f64, f64, f64) {
    let centre = centre_page(level, anchor);
    let span = span_of(level);
    let r = (RENDER_AXIS / 2) as f64 * span;
    (ivec::X(centre.page) as f64 * span - r, ivec::Y(centre.page) as f64 * span - r, span * RENDER_AXIS as f64)
}

/// What a view wants drawn, before anything is known of what is resident:
/// every piece of the ground in view once, each at the one level the view
/// wants it at. The window's levels ask for the same ground over and over —
/// every ring holds the one under it — and this is them made one partition:
/// the top ring's chunks, each wanted finer wherever it lies whole inside
/// the ring the level below draws, down to the window's base. The rings
/// nest, as a clipmap's do, so what is wanted at a level is its ring less
/// the ring under it.
///
/// Every layer on the pyramid is asked for this same ground at these same
/// levels; what each can draw of it is its own (`Pyramid::cover`).
pub struct Domain {
    window: std::ops::Range<u32>,
    rings: Vec<(f64, f64, f64)>,
    anchor: IVec,
}

pub fn domain(anchor: IVec, window: std::ops::Range<u32>) -> Domain {
    let rings = window.clone().map(|level| ring_rect(level, anchor)).collect();
    Domain { window, rings, anchor }
}

impl Domain {
    pub fn window(&self) -> std::ops::Range<u32> {
        self.window.clone()
    }

    /// Where the partition starts: every chunk of the top level's drawn ring.
    pub fn roots(&self) -> Vec<Chunk> {
        let mut out = Vec::new();
        for page in ring(self.window.end - 1, self.anchor, RENDER_AXIS) {
            for cy in 0..PAGE_CHUNKS {
                for cx in 0..PAGE_CHUNKS {
                    out.push(Chunk {
                        page,
                        bx: cx * CHUNK_CELLS,
                        by: cy * CHUNK_CELLS,
                    });
                }
            }
        }
        out
    }

    /// Whether the view wants `c`'s ground finer than `c`: it lies whole
    /// inside the ring the level below draws.
    pub fn splits(&self, c: Chunk) -> bool {
        let level = c.level();
        if level <= self.window.start {
            return false;
        }
        let (x0, y0, side) = c.rect();
        let (rx, ry, rside) = self.rings[(level - 1 - self.window.start) as usize];
        x0 >= rx && y0 >= ry && x0 + side <= rx + rside && y0 + side <= ry + rside
    }

    /// Whether `c`'s ground is any of what the view wants.
    pub fn contains(&self, c: Chunk) -> bool {
        let (x0, y0, side) = c.rect();
        let (rx, ry, rside) = self.rings[self.rings.len() - 1];
        x0 < rx + rside && rx < x0 + side && y0 < ry + rside && ry < y0 + side
    }
}

/// How far the anchor stands from `p`'s centre, in `p`'s own spans: the
/// measure every scheduler here orders by (sched/priority.rs), rooted, since
/// what reads it packs it into a fraction under one.
pub fn centre_dist(anchor: IVec, p: LevelPage) -> f64 {
    let span = span_of(p.level);
    let at = |a: i64, page: i64| a as f64 - (page as f64 + 0.5) * span;
    let (dx, dy) = (at(ivec::X(anchor), ivec::X(p.page)), at(ivec::Y(anchor), ivec::Y(p.page)));
    crate::sched::priority::in_own_sides(dx * dx + dy * dy, span).sqrt()
}

#[cfg(test)]
#[path = "window.test.rs"]
mod test;
