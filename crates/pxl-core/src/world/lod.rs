//! Which tier a chunk shows at a distance: module/world/lod.ts's criterion, in
//! Rust, for the scheduler (sched.rs).
//!
//! A tier's handoff distance is where one of its cells stops being larger
//! on screen than the reference view's cell at the reference distance, so
//! the ladder scales with the viewport's height and the field of view. A
//! chunk already coarser than a threshold keeps its tier until it is a
//! margin inside it, which is what stops a camera at rest on a threshold
//! from flipping a chunk every frame; the first three handoffs blend across
//! a band around the threshold instead of switching at it. Why each number
//! is what it is lives in the TypeScript, which other layers still read and
//! this is held to (sched.test via the module).

use crate::space::constant::U_PER_CELL;

/// The tiers, finest first: cells folded per side.
pub const LOD_STEPS: [u32; 7] = [1, 2, 4, 8, 16, 32, 64];
pub const COARSEST: u32 = 64;

const LOD_MARGIN: f64 = 1.4;
const REFERENCE_VIEWPORT_H: f64 = 1080.0;
const REFERENCE_FOV_DEG: f64 = 50.0;
const REFERENCE_THRESHOLD_0: f64 = 192.0;
const BLEND_BAND_FRACTION: f64 = 0.05;
const BLEND_ELIGIBLE_THRESHOLDS: usize = 3;
fn px_per_unit(viewport_h: f64, fov_deg: f64) -> f64 {
    viewport_h / (2.0 * (fov_deg * core::f64::consts::PI / 360.0).tan())
}

/// What a chunk at a distance shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Guide {
    pub step: u32,
    /// Inside a blend band: the two tiers, and how far across the band the
    /// distance is, 0 at its finer edge and 1 at its coarser one.
    pub blend: Option<(u32, u32, f64)>,
}

/// The ladder for one projection, under a pressure.
#[derive(Clone, Copy)]
pub struct Ladder {
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
    /// every chunk past the finest band one tier coarser — a quarter of
    /// the cells, and about a quarter of the bytes — and one step moves
    /// the outer half of each band. What the memory budget is paid in (sched.rs's pressure): the
    /// set on screen gets coarser until it fits, as Cesium raises a
    /// tileset's screen-space error when its tiles exceed its cache.
    pub fn new(viewport_h: f64, fov_deg: f64, pressure: u32) -> Ladder {
        let target_cell_px = px_per_unit(REFERENCE_VIEWPORT_H, REFERENCE_FOV_DEG) / REFERENCE_THRESHOLD_0;
        let k = px_per_unit(viewport_h, fov_deg) / target_cell_px * 2f64.powf(-(pressure as f64) / 2.0);
        let mut thresholds = [0.0; 6];
        for (i, t) in thresholds.iter_mut().enumerate() {
            *t = LOD_STEPS[i] as f64 * U_PER_CELL * k;
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

    pub fn step_at(&self, dist: f64, current: u32) -> u32 {
        for (i, &raw) in self.thresholds.iter().enumerate() {
            let finer = LOD_STEPS[i];
            let margin = if i < BLEND_ELIGIBLE_THRESHOLDS { 1.0 - BLEND_BAND_FRACTION } else { 1.0 / LOD_MARGIN };
            let t = if current > finer { raw * margin } else { raw };
            if dist < t {
                return finer;
            }
        }
        COARSEST
    }

    pub fn guide(&self, dist: f64, current: u32) -> Guide {
        let step = self.step_at(dist, current);
        for i in 0..BLEND_ELIGIBLE_THRESHOLDS.min(self.thresholds.len()) {
            let t = self.thresholds[i];
            let half = t * BLEND_BAND_FRACTION;
            if dist < t - half || dist > t + half {
                continue;
            }
            let (finer, coarser) = (LOD_STEPS[i], LOD_STEPS[i + 1]);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reference_ladder_hands_off_at_192_and_doubles() {
        let l = Ladder::default();
        assert!((l.thresholds[0] - 192.0).abs() < 1e-9);
        assert_eq!(l.step_at(100.0, 1), 1);
        assert_eq!(l.step_at(300.0, 1), 2);
        assert_eq!(l.step_at(1e6, 1), 64);
        // Already coarser, a margin inside the threshold keeps it.
        assert_eq!(l.step_at(190.0, 2), 2);
        assert_eq!(l.guide(192.0, 1).blend.map(|b| (b.0, b.1)), Some((1, 2)));
        assert_eq!(l.guide(150.0, 1).blend, None);
    }

    #[test]
    fn two_steps_of_pressure_are_one_tier_coarser_everywhere() {
        let l = Ladder::default();
        let mut pressed = l;
        pressed.set_pressure(2);
        for (i, t) in pressed.thresholds.iter().enumerate() {
            assert!((t - l.thresholds[i] / 2.0).abs() < 1e-9);
        }
        // The nearest chunks have no finer tier to lose; past the finest
        // band's pressed edge, one tier.
        assert_eq!(pressed.step_at(50.0, 1), 1);
        for d in [100.0, 300.0, 700.0, 3000.0] {
            let step = l.step_at(d, 1);
            assert_eq!(pressed.step_at(d, 1), (step * 2).min(COARSEST), "at {d}");
        }
        pressed.set_pressure(0);
        assert_eq!(pressed.step_at(300.0, 1), l.step_at(300.0, 1));
    }
}
