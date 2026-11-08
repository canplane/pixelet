// pxl-render/param.test.rs

use super::*;
use crate::look::color::linear_rgb;

#[test]
fn settings_modules_written() {
    // A module left out of `ALL` would not be written into the shaders, and only a shader
    // that reads one of its settings would notice.
    let modules = include_str!("param.rs").matches("params! {").count();
    assert_eq!(ALL.len(), modules, "a module's settings missing from `ALL`");
    let mut paths: Vec<&str> = ALL.iter().map(|m| m[0].0).collect();
    paths.sort_unstable();
    paths.dedup();
    assert_eq!(paths.len(), ALL.len(), "a module's settings in `ALL` twice");
}

#[test]
fn linear_color_is_hex() {
    // The linear values are written to six decimal places, as the shaders read them.
    for (hex, linear) in [
        (0xff1493, location::beacon::DAY),
        (0x00f2ff, location::beacon::NIGHT),
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
fn weather_noise_deviation() {
    // A uniform value's deviation is 1/√12. The bilinear smoothstep's weights scale the
    // variance by ∫(w² + (1 − w)²) = 26/35 on each axis, so the deviation over both axes is
    // scaled by 26/35 once.
    let sigma = 26.0 / 35.0 / 12f64.sqrt();
    assert!(
        (sigma - cloud::weather::SIGMA as f64).abs() < 5e-5,
        "{sigma}"
    );
}
