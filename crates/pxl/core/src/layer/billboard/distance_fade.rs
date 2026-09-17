// pxl_core/layer/billboard/distance_fade.rs

//! The distance fade of a world-anchored billboard (tier 3, see
//! issue/20260907-world-rendering-tiers.md), on the CPU: what the badge shader
//! shrinks and dims a billboard by, for anything that reasons about the screen
//! box a billboard will take. A billboard at full strength is drawn at one
//! screen pixel per logical pixel, so its drawn width is `logical_w * far`.

/// Full opacity/size at or inside fade_start_u(), smoothstepped to gone by
/// fade_end_u() — both a fixed number of the terrain's first tier handoff
/// (crates/pxl/render/src/lod.rs), so they move with the projection as the tiers do.
pub use pxl_render::look::lod::{fade_end_u, fade_start_u};

pub fn distance_fade_far(dist: f64) -> f64 {
    let (start, end) = (fade_start_u(), fade_end_u());
    if dist <= start {
        return 1.0;
    }
    if dist >= end {
        return 0.0;
    }
    let t = (dist - start) / (end - start);
    1.0 - t * t * (3.0 - 2.0 * t) // smoothstep
}
