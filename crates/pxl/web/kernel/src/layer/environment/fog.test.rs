// pxl-kernel/layer/environment/fog.test.rs

use super::*;

/// A world unit is under two metres, so these heights are a person, a tower
/// and a view from orbit.
const HEIGHTS: [f64; 3] = [1.0, 512.0, 65536.0];

#[test]
fn the_planet_ends_the_ground_from_the_ground_and_from_orbit_and_the_air_ends_it_between() {
    // At head height the horizon is a few kilometres and the air is clear
    // well past it; a kilometre up the horizon has run past the visual range;
    // from orbit a ray crosses the layer once and the planet is nearer again.
    assert!(horizon_at(1.0) < haze_reach_at(1.0));
    assert!(haze_reach_at(512.0) < horizon_at(512.0));
    assert!(horizon_at(65536.0) < haze_reach_at(65536.0));
    for h in HEIGHTS {
        assert_eq!(reach_at(h), horizon_at(h).min(haze_reach_at(h)).max(NEAREST_REACH));
    }
}

#[test]
fn both_ends_move_out_with_height_and_the_band_and_planes_follow_the_reach() {
    for pair in HEIGHTS.windows(2) {
        let (low, high) = (pair[0], pair[1]);
        assert!(horizon_at(low) < horizon_at(high));
        assert!(haze_reach_at(low) < haze_reach_at(high));
        assert!(reach_at(low) < reach_at(high));
        let (near, far) = band_at(low, f64::INFINITY);
        assert!(near < far, "the band fades before it ends");
        assert!(far_clip_at(low) > far, "the band completes inside the frustum");
        assert!(near_clip_at(low) <= near_clip_at(high), "the near plane only opens out");
    }
}

#[test]
fn the_haze_reach_is_the_visual_range_at_the_ground() {
    // Koschmieder at the surface: the whole layer's density along the ray.
    let range_km = CONTRAST_GONE / HAZE_EXTINCTION;
    let at_ground = haze_reach_at(1e-6) * km_per_u();
    assert!((at_ground - range_km).abs() < range_km * 0.01, "{at_ground} km against {range_km}");
}
