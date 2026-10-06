// pxl-tool/bin/download_moon.rs

//! The moon's face for sky.wgsl: NASA SVS's CGI Moon Kit map cut to the near
//! side, as linear-light luminance in 8-bit sRGB, to `asset/sky/moon.png`.
//! The printed mean reflectance must equal sky.wgsl's MOON_MAP_MEAN.
//! Credit: NASA's Scientific Visualization Studio.
//!
//!   deno task download:moon

use std::process::ExitCode;

use image::{GrayImage, ImageFormat, Luma};

use pxl_tool::root;

const SOURCE: &str = "https://svs.gsfc.nasa.gov/vis/a000000/a004700/a004720/lroc_color_poles_1k.jpg";

fn to_linear(c: u8) -> f64 {
    let c = c as f64 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        libm::pow((c + 0.055) / 1.055, 2.4)
    }
}

fn to_srgb(v: f64) -> u8 {
    let v = v.clamp(0.0, 1.0);
    let c = if v <= 0.0031308 { 12.92 * v } else { 1.055 * libm::pow(v, 1.0 / 2.4) - 0.055 };
    (c * 255.0).round_ties_even() as u8
}

fn run() -> Result<(), String> {
    let mut body = Vec::new();
    ureq::get(SOURCE).call().map_err(|e| e.to_string())?.into_reader().read_to_end(&mut body).map_err(|e| e.to_string())?;
    let map = image::load_from_memory_with_format(&body, ImageFormat::Jpeg).map_err(|e| e.to_string())?.to_rgb8();
    let (w, h) = map.dimensions();
    let (x0, side) = (w / 4, w / 2);
    let mut face = GrayImage::new(side, h);
    let (mut sum, mut weight) = (0.0, 0.0);
    for y in 0..h {
        let lat = (0.5 - (y as f64 + 0.5) / h as f64) * core::f64::consts::PI;
        for x in 0..side {
            let [r, g, b] = map.get_pixel(x0 + x, y).0;
            let l = 0.2126 * to_linear(r) + 0.7152 * to_linear(g) + 0.0722 * to_linear(b);
            let lon = ((x as f64 + 0.5) / side as f64 - 0.5) * core::f64::consts::PI;
            let area = libm::cos(lat) * libm::cos(lon);
            sum += l * area;
            weight += area;
            face.put_pixel(x, y, Luma([to_srgb(l)]));
        }
    }
    let out = root().join("asset/sky/moon.png");
    std::fs::create_dir_all(out.parent().expect("a directory")).map_err(|e| e.to_string())?;
    face.save_with_format(&out, ImageFormat::Png).map_err(|e| e.to_string())?;
    println!("[moon] {side}x{h} -> {}; the face's mean reflectance, MOON_MAP_MEAN: {:.4}", out.display(), sum / weight);
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[moon] {e}");
            ExitCode::FAILURE
        }
    }
}
