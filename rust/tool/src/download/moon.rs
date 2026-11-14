// pxl-tool/download/moon.rs

//! The moon's face for sky.wgsl: NASA SVS's CGI Moon Kit map cut to the near
//! side, as linear-light luminance in 8-bit sRGB, to `asset/content/sky/moon.png`,
//! the map kept in `asset/raw/sky/moon/`.
//! The printed mean reflectance must equal pxl-render frame/shader/word.rs
//! `MOON_MAP_MEAN`.
//! Credit: NASA's Scientific Visualization Studio.
//!
//!   make download-moon

use image::{GrayImage, ImageFormat, Luma};

use crate::color::{linear, srgb};
use crate::{fetch, root};

const SRC: &str = "https://svs.gsfc.nasa.gov/vis/a000000/a004700/a004720/lroc_color_poles_1k.jpg";

pub fn run() -> Result<(), String> {
    let body = fetch::download(SRC, &root().join("asset/raw/sky/moon"))?;
    let map = image::load_from_memory_with_format(&body, ImageFormat::Jpeg)
        .map_err(|e| e.to_string())?
        .to_rgb8();
    let (w, h) = map.dimensions();
    let (x0, side) = (w / 4, w / 2);
    let mut face = GrayImage::new(side, h);
    let (mut sum, mut weight) = (0.0, 0.0);
    for y in 0..h {
        let lat = (0.5 - (y as f64 + 0.5) / h as f64) * core::f64::consts::PI;
        for x in 0..side {
            let [r, g, b] = map.get_pixel(x0 + x, y).0;
            let l = 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b);
            let lon = ((x as f64 + 0.5) / side as f64 - 0.5) * core::f64::consts::PI;
            let area = libm::cos(lat) * libm::cos(lon);
            sum += l * area;
            weight += area;
            face.put_pixel(x, y, Luma([srgb(l)]));
        }
    }
    let out = root().join("asset/content/sky/moon.png");
    std::fs::create_dir_all(out.parent().expect("a directory")).map_err(|e| e.to_string())?;
    face.save_with_format(&out, ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    println!(
        "[moon] {side}x{h} -> {}; the face's mean reflectance, MOON_MAP_MEAN: {:.4}",
        out.display(),
        sum / weight
    );
    Ok(())
}
