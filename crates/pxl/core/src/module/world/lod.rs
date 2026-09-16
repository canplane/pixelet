// pxl_core/module/world/lod.rs

//! Which tier a thing at a distance shows, and everything that keeps step
//! with that ladder.
//!
//! A tier's handoff distance is where one of its cells stops being larger on
//! screen than the reference view's cell at the reference distance, so the
//! ladder scales with the viewport's height and the field of view. A thing
//! already coarser than a threshold keeps its tier until it is a margin
//! inside it, which is what stops a camera at rest on a threshold from
//! flipping it every frame; the first three handoffs blend across a band
//! around the threshold instead of switching at it.
//!
//! `Ladder` is one ladder as a value — the chunk scheduler owns its own, with
//! its own pressure, because the byte budget pulls that one in and nothing
//! else. Everything outside the scheduler reads the process-wide ladder
//! through `lod_step_at` and the derived quantities below — the badge tiers'
//! shift over the cube tiers, the billboards' fade range, the badge
//! occlusion's cell per distance, decor's target cell. The projection and
//! the scheduler's pressure are told to it once a frame (core_tick.rs), so
//! badges climb the rungs the cubes are on.

use pxl_shared::bake::constant::U_PER_CELL;
use xpute_core::kit::fdlibm;

use crate::sys::global::Global;

/// XY sampling stride in L1 blocks inside one 64x64 chunk.
pub const LOD_STEP_BY_LEVEL: [LodStep; 7] = [1, 2, 4, 8, 16, 32, 64];
/// The last rung: what everything past the final handoff shows.
pub const COARSEST: LodStep = LOD_STEP_BY_LEVEL[LOD_STEP_BY_LEVEL.len() - 1];

/// 0..=6.
pub type LodLevel = u32;
/// One of LOD_STEP_BY_LEVEL.
pub type LodStep = u32;

const LOD_MARGIN: f64 = 1.4;

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
    viewport_h / (2.0 * fdlibm::tan((fov_deg * core::f64::consts::PI) / 360.0))
}

/// Six pixels at the reference. For whoever applies the same criterion to
/// something other than a cell — decor, by its extent.
#[allow(non_snake_case)]
pub fn TARGET_CELL_PX() -> f64 {
    px_per_unit(REFERENCE_VIEWPORT_H, REFERENCE_FOV_DEG) / REFERENCE_THRESHOLD_0
}

/// Half-width (fraction of the threshold's own distance) of the band where
/// two LOD steps dither-blend. A wide pulled-back/top-down view puts a huge
/// range of per-chunk distances on screen at once, so too wide a band lets
/// over half of all loaded chunks fall inside some band simultaneously,
/// reading as uniform speckle — arguably worse than the hard pop it was
/// built to hide. 0.05 keeps each band narrow enough that only chunks
/// genuinely near one threshold ever blend.
const BLEND_BAND_FRACTION: f64 = 0.05;

/// Only thresholds inside the current visible range get dither-blended. At the
/// reference projection the third handoff (768u) is the last one comfortably
/// inside render_cfg.RENDER_DIST, so including it removes both its hard pop
/// and the old 29% non-blended hysteresis gap. The handoff that lands at the
/// range's own edge, and everything past it, stay hard transitions: geometry
/// ends there and fog is already opaque, so a second mesh would be retained
/// for nothing.
const BLEND_ELIGIBLE_THRESHOLDS: usize = 3;

/// What a thing at a distance shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Guide {
    pub step: LodStep,
    /// Inside a blend band: the two tiers, and how far across the band the
    /// distance is, 0 at its finer edge and 1 at its coarser one.
    pub blend: Option<(LodStep, LodStep, f64)>,
}

/// The ladder for one projection, under a pressure.
#[derive(Clone, Copy)]
pub struct Ladder {
    /// The handoff distances, finest first. Derived, so they move with the
    /// projection; each is twice the one before, because each tier's cell is.
    thresholds: [f64; 6],
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
        let k = px_per_unit(viewport_h, fov_deg) / TARGET_CELL_PX() * 2f64.powf(-(pressure as f64) / 2.0);
        let mut thresholds = [0.0; 6];
        for (i, t) in thresholds.iter_mut().enumerate() {
            *t = LOD_STEP_BY_LEVEL[i] as f64 * U_PER_CELL * k;
        }
        Ladder {
            thresholds,
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
        self.thresholds[0]
    }

    /// The largest cell drawn at a range, per unit of that range. Just past a
    /// threshold the tier doubles, so a thing at the first threshold shows
    /// two-cell boxes, at twice that distance four-cell ones, and so on: the
    /// cell drawn at any distance is at most this fraction of it, and half
    /// that just before the next handoff. For whoever has to tolerate a
    /// coarse cell's height — the badge occlusion test.
    pub fn cell_per_distance(&self) -> f64 {
        2.0 / self.thresholds[0]
    }

    // Per-threshold hysteresis margin, not one flat factor for every
    // threshold at once (the previous version): LOD_MARGIN's ~29% dead zone
    // is far wider than BLEND_BAND_FRACTION's 5% band, and for a
    // blend-eligible threshold that mismatch is a real bug, not just a wider
    // safety margin — while zooming in, `guide`'s band turns off (the chunk
    // exits it) once dist crosses the band's own inner edge
    // (t*(1-BLEND_BAND_FRACTION)), but the flat-factor version didn't
    // actually commit the step to the finer tier until dist reached the much
    // farther t/LOD_MARGIN. In the gap between those two points, the chunk
    // had already looked like it switched to fine (the blend band's own
    // per-chunk hash comparison shows the finer draw for almost any value
    // near the band's low edge, since the fraction there is near 0) and then
    // visibly snapped back to full-coarse the instant the band turned off — a
    // real "un-does itself" pop during smooth, one-directional zooming, not
    // requiring any reversal in the input. Matching the blend-eligible margin
    // to the band's own inner edge (1-BLEND_BAND_FRACTION) makes both commit
    // to the finer tier at the exact same dist, so there's no gap left to
    // fall into. Non-blend-eligible thresholds (LOD2+) have no band to stay
    // consistent with, so they keep the original, wider LOD_MARGIN-based
    // cushion — a hard pop there is already the least noticeable case (see
    // BLEND_ELIGIBLE_THRESHOLDS's own comment).
    /// The tier a thing at `dist` shows, given the tier it shows now — the one
    /// criterion every lattice consumer climbs, cubes and badges alike, so
    /// they hand off at the same distances with the same hysteresis and move
    /// together on screen.
    pub fn step_at(&self, dist: f64, current: LodStep) -> LodStep {
        for (i, &raw) in self.thresholds.iter().enumerate() {
            let finer = LOD_STEP_BY_LEVEL[i];
            // Hysteresis only ever resists *becoming finer* (shrinking the
            // threshold once already coarser than this tier) — there's no
            // symmetric cushion against becoming coarser, the same asymmetry
            // the original had.
            let margin = if i < BLEND_ELIGIBLE_THRESHOLDS { 1.0 - BLEND_BAND_FRACTION } else { 1.0 / LOD_MARGIN };
            let t = if current > finer { raw * margin } else { raw };
            if dist < t {
                return finer;
            }
        }
        COARSEST
    }

    pub fn guide(&self, dist: f64, current: LodStep) -> Guide {
        let step = self.step_at(dist, current);
        for i in 0..BLEND_ELIGIBLE_THRESHOLDS.min(self.thresholds.len()) {
            let t = self.thresholds[i];
            let half = t * BLEND_BAND_FRACTION;
            if dist < t - half || dist > t + half {
                continue;
            }
            let (finer, coarser) = (LOD_STEP_BY_LEVEL[i], LOD_STEP_BY_LEVEL[i + 1]);
            if step != finer && step != coarser {
                break;
            }
            let f = ((dist - (t - half)) / (2.0 * half)).clamp(0.0, 1.0);
            return Guide {
                step,
                blend: Some((finer, coarser, f)),
            };
        }
        Guide { step, blend: None }
    }
}

// ---- What keeps step with the ladder ----

/// How many terrain cells a place badge's cell spans at any range: the badge
/// ladder is the cube ladder times this (poi/poi_layer.rs). A pixel width of
/// its own comes out at about eight cubes, which gathers places that are
/// plainly apart on screen; four is the setting, not a derivation.
const BADGE_CELLS_PER_CUBE: f64 = 4.0;
/// The same, as a shift on the lattice: a badge node sits this many tiers
/// above the cube tier at its range.
pub const BADGE_SHAMT_OVER_CUBE: u32 = 2;
const _: () = assert!((1 << BADGE_SHAMT_OVER_CUBE) as f64 == BADGE_CELLS_PER_CUBE, "BADGE_CELLS_PER_CUBE must be a power of two");

/// Where a world-anchored billboard starts to fade and where it is gone, in
/// first-handoff units (billboard/distance_fade.rs): full strength inside
/// 5/4 of the first handoff, gone by 10/3 of it — 240u and 640u at the
/// reference. Twice the 120u and 320u the fade was first tuned at, which
/// read as too near once the badges gathered by the cube ladder
/// (2026-09-12); and moving with the projection now rather than staying
/// fixed while the tiers under them moved.
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
/// (core_tick.rs), and a no-op while none of the three moves.
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

/// `Ladder::step_at` on the process-wide ladder.
pub fn lod_step_at(dist: f64, current_lod_step: LodStep) -> LodStep {
    lod().step_at(dist, current_lod_step)
}

/// A step's place on the ladder, which is also its shift: step = 1 << level.
pub fn get_lod_level(lod_step: LodStep) -> LodLevel {
    match lod_step {
        1 => 0,
        2 => 1,
        4 => 2,
        8 => 3,
        16 => 4,
        32 => 5,
        _ => 6,
    }
}

#[cfg(test)]
#[path = "lod.test.rs"]
mod test;
