// pxl-render/look/atmosphere.test.rs

use super::*;

#[test]
fn a_sun_overhead_loses_most_of_its_blue_and_little_of_its_red() {
    // The zenith optical depth at sea level is the coefficients times their
    // scale heights, which a march to the top has to reproduce: Rayleigh
    // 0.046 / 0.108 / 0.265, Mie 0.0053, and the ozone tent's 15 km of area
    // at its peak, 0.0098 / 0.028 / 0.0013.
    let t = transmittance(0.0, 1.0);
    let expected = [(-0.0615f64).exp(), (-0.142f64).exp(), (-0.2714f64).exp()];
    for k in 0..3 {
        assert!((t[k] - expected[k]).abs() < 0.01, "channel {k}: {} against {}", t[k], expected[k]);
    }
    assert!(t[0] > t[1] && t[1] > t[2], "red passes, blue scatters");
}

#[test]
fn a_lower_sun_crosses_more_air() {
    let high = transmittance(0.0, 0.9);
    let low = transmittance(0.0, 0.1);
    for k in 0..3 {
        assert!(low[k] < high[k]);
    }
}

#[test]
fn the_ground_stands_in_every_direction_below_the_horizon() {
    assert_eq!(transmittance(0.0, -0.01), [0.0; 3]);
    // From higher up the horizon dips, and a slightly downward ray clears it.
    assert!(transmittance(10.0, -0.01)[0] > 0.0);
}

#[test]
fn noon_is_white_and_the_sun_sets_across_its_disc() {
    let w = white();
    for v in sun_light(0.0, 1.0, w) {
        assert!((v - SUN_LIGHT).abs() < 1e-9, "noon at sea level is SUN_LIGHT, white");
    }
    let low = sun_light(0.0, 0.05, w);
    assert!(low[0] > low[2], "a low sun is red");
    assert_eq!(sun_light(0.0, -2.0 * SUN_ANGULAR_RADIUS, w), [0.0; 3]);
    let setting = sun_light(0.0, 0.0, w);
    assert!(setting[0] > 0.0 && setting[0] < sun_light(0.0, 2.0 * SUN_ANGULAR_RADIUS, w)[0]);
}

#[test]
fn a_half_moon_gives_a_tenth_of_a_full_ones_light() {
    assert_eq!(moon_phase_light(0.0), 1.0);
    let half = moon_phase_light(core::f64::consts::FRAC_PI_2);
    assert!((0.07..0.11).contains(&half), "{half}");
    // A crescent a few days from new: 160°, some two thousandths.
    assert!(moon_phase_light(2.8) < 0.005);
}
