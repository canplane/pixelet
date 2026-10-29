// pxl-render/param.test.rs

use std::path::Path;

use super::*;
use crate::look::color::{linear, linear_rgb};

#[test]
fn settings_modules_written() {
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

#[test]
fn the_moon_map_shipped_means_what_the_sky_scales_by() {
    // download_moon's mean, over the shipped map: its gray in linear light,
    // each texel weighed by the disc's area it covers, which the 8-bit map
    // holds to within a few ten-thousandths of the raw one it was cut from.
    let path = Path::new(env!("PXL_REPO")).join("asset/content/sky/moon.png");
    let map = image::open(&path)
        .expect("the Moon's map (make download-moon)")
        .to_luma8();
    let (side, h) = map.dimensions();
    let (mut sum, mut weight) = (0.0, 0.0);
    for (x, y, l) in map.enumerate_pixels() {
        let lat = (0.5 - (y as f64 + 0.5) / h as f64) * core::f64::consts::PI;
        let lon = ((x as f64 + 0.5) / side as f64 - 0.5) * core::f64::consts::PI;
        let area = lat.cos() * lon.cos();
        sum += linear(l.0[0]) * area;
        weight += area;
    }
    let mean = sum / weight;
    assert!(
        (mean - sky::moon::MAP_MEAN as f64).abs() < 5e-4,
        "the map's mean is {mean:.4}"
    );
}
