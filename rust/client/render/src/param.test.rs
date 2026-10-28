// pxl-render/param.test.rs

use super::*;
use crate::look::color::linear_rgb;

#[test]
fn every_module_of_settings_is_written() {
    // A module left out of `ALL` would go unwritten into the shaders, and
    // only a shader that reads one of its settings would say so.
    let modules = include_str!("param.rs").matches("params! {").count();
    assert_eq!(ALL.len(), modules, "a module's settings missing from `ALL`");
    let mut paths: Vec<&str> = ALL.iter().map(|m| m[0].0).collect();
    paths.sort_unstable();
    paths.dedup();
    assert_eq!(paths.len(), ALL.len(), "a module's settings in `ALL` twice");
}

#[test]
fn a_colour_in_linear_light_is_its_hex() {
    // Written to six places, as the shaders read them.
    for (hex, linear) in [
        (0xff1493, puck::beacon::DAY),
        (0x00f2ff, puck::beacon::NIGHT),
    ] {
        for (want, got) in linear_rgb(hex).into_iter().zip(linear) {
            assert!(
                (want - got as f64).abs() < 5e-7,
                "#{hex:06x}: {want} against {got}"
            );
        }
    }
}

#[test]
fn the_weather_noise_deviates_as_its_formula_says() {
    // A uniform's 1/√12, and the bilinear smoothstep's weights, ∫(w² + (1 − w)²)
    // = 26/35 an axis, once over the two axes' deviation.
    let sigma = 26.0 / 35.0 / 12f64.sqrt();
    assert!(
        (sigma - cloud::weather::SIGMA as f64).abs() < 5e-5,
        "{sigma}"
    );
}
