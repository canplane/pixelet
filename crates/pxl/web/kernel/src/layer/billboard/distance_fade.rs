// pxl-kernel/layer/billboard/distance_fade.rs

//! The distance fade of a world-anchored billboard (tier 3, see
//! issue/20260907-world-rendering-tiers.md), on the CPU: what the badge shader
//! shrinks and dims a billboard by, for anything that reasons about the screen
//! box a billboard will take. A billboard at full strength is drawn at one
//! screen pixel per logical pixel, so its drawn width is `logical_w * far`.

use crate::control::camera::controller::look_distance;

/// Where a billboard is at full strength and where it is gone, in the
/// camera's distance to what it looks at: a sphere about the eye that grows
/// with the zoom, so what the view is about keeps its marks at every zoom and
/// the ground far past it — toward the horizon of a tilted view — does not.
/// Looking straight down, the screen's farthest corner is 1.4 zooms away, so
/// nothing on it fades. Product numbers, not derivations.
const FADE_START_ZOOMS: f64 = 2.0;
const FADE_END_ZOOMS: f64 = 4.0;

/// Where the fade starts and ends, in units from the eye, this frame.
pub fn fade_range() -> (f64, f64) {
    let zoom = look_distance();
    (FADE_START_ZOOMS * zoom, FADE_END_ZOOMS * zoom)
}

/// What a billboard `dist` from the eye is drawn at: its size and its
/// opacity, 1 inside the fade and smoothstepped to 0 across it.
pub fn distance_fade_far(dist: f64) -> f64 {
    let (start, end) = fade_range();
    if dist <= start {
        return 1.0;
    }
    if dist >= end {
        return 0.0;
    }
    let t = (dist - start) / (end - start);
    1.0 - t * t * (3.0 - 2.0 * t) // smoothstep
}
