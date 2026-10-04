// pxl-tool/bin/download_earth.rs

//! The Earth's face, the floor every tile is drawn over: NASA's Blue Marble Next
//! Generation, a month of 2004 a map (land with its relief shaded, the sea a
//! flat blue), taken down to 2048 × 1024 in linear light and written as WebP
//! to `asset/sky/earth/<month>.webp`. Credit: NASA's Earth Observatory
//! (Reto Stöckli), as NASA's Scientific Visualization Studio serves it.
//!
//!   deno task download:earth

use std::process::ExitCode;

use image::imageops::{resize, FilterType};
use image::{ImageFormat, Rgb32FImage};

use pxl_tool::root;

const SOURCE: &str = "https://svs.gsfc.nasa.gov/vis/a010000/a012500/a012564/frames/5400x2700_2x1_60p/world.topo.2004-";
const WIDTH: u32 = 2048;
const HEIGHT: u32 = 1024;
/// libwebp's lossy quality: a map seen kilometers wide shows nothing finer.
const QUALITY: f32 = 90.0;

fn to_linear(c: u8) -> f32 {
    let c = c as f64 / 255.0;
    (if c <= 0.04045 { c / 12.92 } else { libm::pow((c + 0.055) / 1.055, 2.4) }) as f32
}

fn to_srgb(v: f32) -> u8 {
    let v = (v as f64).clamp(0.0, 1.0);
    let c = if v <= 0.0031308 { 12.92 * v } else { 1.055 * libm::pow(v, 1.0 / 2.4) - 0.055 };
    (c * 255.0).round_ties_even() as u8
}

fn month(m: u32) -> Result<(), String> {
    let url = format!("{SOURCE}{m:02}.png");
    let mut body = Vec::new();
    ureq::get(&url)
        .call()
        .map_err(|e| format!("{url}: {e}"))?
        .into_reader()
        .read_to_end(&mut body)
        .map_err(|e| e.to_string())?;
    let map = image::load_from_memory_with_format(&body, ImageFormat::Png).map_err(|e| e.to_string())?.to_rgb8();
    // Averaged in linear light, so a coast does not darken as it is taken down.
    let linear = Rgb32FImage::from_fn(map.width(), map.height(), |x, y| image::Rgb(map.get_pixel(x, y).0.map(to_linear)));
    let small = resize(&linear, WIDTH, HEIGHT, FilterType::Triangle);
    let bytes: Vec<u8> = small.pixels().flat_map(|p| p.0.map(to_srgb)).collect();
    let encoded = webp::Encoder::from_rgb(&bytes, WIDTH, HEIGHT).encode(QUALITY);
    let out = root().join(format!("asset/sky/earth/{m:02}.webp"));
    std::fs::create_dir_all(out.parent().ok_or("a directory")?).map_err(|e| e.to_string())?;
    std::fs::write(&out, &*encoded).map_err(|e| e.to_string())?;
    println!("[earth] {m:02}: {}x{} -> {} ({} KiB)", map.width(), map.height(), out.display(), encoded.len() / 1024);
    Ok(())
}

fn main() -> ExitCode {
    match (1..=12).try_for_each(month) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[earth] {e}");
            ExitCode::FAILURE
        }
    }
}
