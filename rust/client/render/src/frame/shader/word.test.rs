// pxl-render/frame/shader/word.test.rs

use std::path::Path;

use super::*;
use crate::look::color::linear;

#[test]
fn moon_map_width() {
    // `make download-moon` writes the map, and the sky binds and reads it at
    // `MOON_TEXELS` a side. A map cut again at another size must change
    // `MOON_TEXELS` with it.
    let png = std::fs::read(Path::new(env!("PXL_REPO")).join("asset/content/sky/moon.png"))
        .expect("the moon's map (make download-moon)");
    let side = |at: usize| u32::from_be_bytes(png[at..at + 4].try_into().unwrap());
    assert_eq!(
        (side(16), side(20)),
        (MOON_TEXELS, MOON_TEXELS),
        "the PNG's header against `MOON_TEXELS`"
    );
}

#[test]
fn moon_map_scale() {
    // The mean `make download-moon` prints, taken here over the shipped map: its
    // gray in linear light, each texel weighed by the area of the disc it
    // covers. The 8-bit map's mean lies within a few ten-thousandths of the
    // mean of the raw map it was cut from.
    let path = Path::new(env!("PXL_REPO")).join("asset/content/sky/moon.png");
    let map = image::open(&path)
        .expect("the moon's map (make download-moon)")
        .to_luma8();
    let (side, h) = map.dimensions();
    let (mut sum, mut weight) = (0.0, 0.0);
    for (x, y, l) in map.enumerate_pixels() {
        let lat = (0.5 - (y as f64 + 0.5) / h as f64) * core::f64::consts::PI;
        let lon = ((x as f64 + 0.5) / side as f64 - 0.5) * core::f64::consts::PI;
        let area = libm::cos(lat) * libm::cos(lon);
        sum += linear(l.0[0]) * area;
        weight += area;
    }
    let mean = sum / weight;
    assert!(
        (mean - MOON_MAP_MEAN as f64).abs() < 5e-4,
        "the map's mean is {mean:.4}"
    );
}
