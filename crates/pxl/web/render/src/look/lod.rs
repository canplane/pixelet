// pxl-render/look/lod.rs

//! The screen-space criterion the world's tiers are measured by, and
//! everything that keeps step with it.
//!
//! A tier's handoff distance is where one of its cells stops being larger on
//! screen than the reference view's cell at the reference distance, so the
//! handoffs scale with the viewport's height and the field of view. Which
//! level and cube a chunk is drawn in is the kernel's rule
//! (kernel world/pyramid/window.rs `level_for`, world/chunk/table.rs
//! `wanted_cube`); what stays here is the projection the rest keeps step
//! with — the billboards' fade range, the badge occlusion's cell per
//! distance, decor's target cell — told once a frame with the scheduler's
//! pressure (crates/pxl/web/kernel/src/world/chunk/stage.rs).

use pxl_spec::bake::constant::U_PER_CELL;
use std::sync::LazyLock;

use xpute_runtime::global::Global;

/// The coarsest step a chunk's mesh is drawn at: its cells 64 a side of one
/// box, what everything past the rule's last handoff shows.
pub const COARSEST: LodStep = 64;

/// A mesh's step: how many of a chunk's cells one box spans a side, a power
/// of two.
pub type LodStep = u32;

/// The criterion: a tier hands off to the next when one of its cells would
/// project to about this many pixels on screen.
///
/// Screen-space error, the ordinary rule for chunked terrain LOD, rather than
/// a table of world distances. A table is this same rule frozen at one
/// viewport and one field of view, and it cannot say what should happen when
/// either moves — which is why the value below is not a fresh one: it is the
/// ladder that was already here (192u for the finest tier at 1080px and 50
/// degrees), restated. A change of reasoning, not a retune.
///
/// Every screen-space criterion in the world — a cell's tier, a decor kind by
/// its extent, a badge's rung — is this reference scaled by the projection,
/// so they move together when the viewport or the field of view does.
pub const REFERENCE_VIEWPORT_H: f64 = 1080.0;
pub const REFERENCE_FOV_DEG: f64 = 50.0;
pub const REFERENCE_THRESHOLD_0: f64 = 192.0;

/// Pixels a world unit covers at one unit of distance — the same projection
/// factor the badges and billboard/distance_fade.rs size their own screen
/// measurements with.
pub fn px_per_unit(viewport_h: f64, fov_deg: f64) -> f64 {
    viewport_h / (2.0 * libm::tan((fov_deg * core::f64::consts::PI) / 360.0))
}

/// Six pixels at the reference. For whoever applies the same criterion to
/// something other than a cell — decor, by its extent.
pub static TARGET_CELL_PX: LazyLock<f64> = LazyLock::new(|| px_per_unit(REFERENCE_VIEWPORT_H, REFERENCE_FOV_DEG) / REFERENCE_THRESHOLD_0);

/// The criterion for one projection, under a pressure.
#[derive(Clone, Copy)]
pub struct Ladder {
    /// The first handoff distance. Derived, so it moves with the projection;
    /// each handoff past it is twice the one before, because each tier's cell
    /// is.
    threshold_0: f64,
    viewport_h: f64,
    fov_deg: f64,
    pressure: u32,
}

impl Default for Ladder {
    fn default() -> Self {
        Ladder::new(REFERENCE_VIEWPORT_H, REFERENCE_FOV_DEG, 0)
    }
}

impl Ladder {
    /// The ladder scaled by the projection, then pulled in by `pressure`:
    /// every threshold at 2^(-pressure/2), so two steps of pressure move
    /// every chunk past the finest band one tier coarser — a quarter of the
    /// cells, and about a quarter of the bytes — and one step moves the outer
    /// half of each band. What the memory budget is paid in (the scheduler's
    /// pressure): the set on screen gets coarser until it fits, as Cesium
    /// raises a tileset's screen-space error when its tiles exceed its cache.
    pub fn new(viewport_h: f64, fov_deg: f64, pressure: u32) -> Ladder {
        let k = px_per_unit(viewport_h, fov_deg) / *TARGET_CELL_PX * 2f64.powf(-(pressure as f64) / 2.0);
        Ladder {
            threshold_0: U_PER_CELL * k,
            viewport_h,
            fov_deg,
            pressure,
        }
    }

    /// The same ladder unless the projection changed.
    pub fn set_projection(&mut self, viewport_h: f64, fov_deg: f64) {
        if viewport_h != self.viewport_h || fov_deg != self.fov_deg {
            *self = Ladder::new(viewport_h, fov_deg, self.pressure);
        }
    }

    pub fn pressure(&self) -> u32 {
        self.pressure
    }

    pub fn set_pressure(&mut self, pressure: u32) {
        if pressure != self.pressure {
            *self = Ladder::new(self.viewport_h, self.fov_deg, pressure);
        }
    }

    /// The ladder's own scale, live with the projection: the first handoff
    /// distance in world units — 192u at the reference viewport and field of
    /// view, more on a taller screen, less on a shorter one. Everything that
    /// should keep step with the terrain's tiers is a multiple of it.
    pub fn threshold_0(&self) -> f64 {
        self.threshold_0
    }

    /// The largest cell drawn at a range, per unit of that range. Just past a
    /// threshold the tier doubles, so a thing at the first threshold shows
    /// two-cell boxes, at twice that distance four-cell ones, and so on: the
    /// cell drawn at any distance is at most this fraction of it, and half
    /// that just before the next handoff. For whoever has to tolerate a
    /// coarse cell's height — the badge occlusion test.
    pub fn cell_per_distance(&self) -> f64 {
        2.0 / self.threshold_0
    }
}

// ---- What keeps step with the ladder ----

/// Where a world-anchored billboard starts to fade and where it is gone, in
/// first-handoff units (billboard/distance_fade.rs): full strength inside
/// 5/4 of the first handoff, gone by 10/3 of it — 240u and 640u at the
/// reference, moving with the projection as the tiers under them do.
const FADE_START_TIERS: f64 = 240.0 / REFERENCE_THRESHOLD_0;
const FADE_END_TIERS: f64 = 640.0 / REFERENCE_THRESHOLD_0;

static LOD: Global<Ladder> = Global::new();

fn lod() -> &'static mut Ladder {
    LOD.get(Ladder::default)
}

/// The largest cell drawn at a range, per unit of that range, on the
/// process-wide ladder.
#[allow(non_snake_case)]
pub fn LOD_CELL_PER_DISTANCE() -> f64 {
    lod().cell_per_distance()
}

pub fn fade_start_u() -> f64 {
    FADE_START_TIERS * lod().threshold_0()
}

pub fn fade_end_u() -> f64 {
    FADE_END_TIERS * lod().threshold_0()
}

/// Tells the process-wide ladder what it is projecting onto, and under which
/// of the scheduler's pressures. Called once a frame by the tick
/// (crates/pxl/web/kernel/src/world/chunk/stage.rs), and a no-op while none of the three moves.
///
/// Viewport height in CSS pixels, not device pixels: at dpr 2 the same cell
/// covers twice the device pixels, and following that would double the
/// geometry a retina screen builds. Supersampling is what dpr is spent on
/// here (issue/20260911-taa-tried-and-closed.md), while which tier to show is
/// a question about how large a cell looks.
pub fn set_lod_projection(viewport_h: f64, fov_deg: f64, pressure: u32) {
    let l = lod();
    l.set_projection(viewport_h, fov_deg);
    l.set_pressure(pressure);
}

#[cfg(test)]
#[path = "lod.test.rs"]
mod test;
