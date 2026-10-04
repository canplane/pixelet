// pxl-geo-bake/bin/bake_earth.rs

//! The Earth's picture, both its forms from one source: NASA's Blue Marble Next
//! Generation, July 2004 (21600 × 10800, the sea a flat blue), as reflectance,
//! and the Black Marble's lights of 2016 (3 km).
//!  - The Earth layer's pages (pxl-spec bake/earth_page.rs) at its top level, a
//!    z6 pixel a texel, to `./out/earth/<version>/<quadkey>`. A page that is one
//!    color says nothing the global picture does not, and has no object.
//!  - The global picture every page falls back to, 2048 × 1024, to
//!    `asset/sky/earth/07.webp`.
//!
//! Credit: NASA's Earth Observatory (Reto Stöckli; Miguel Román and the Black
//! Marble team).
//!
//!   deno task bake:earth

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use image::imageops::{resize, FilterType};
use image::{GrayImage, ImageFormat, Rgb32FImage, RgbImage};

use pxl_geo_bake::util::{git_sha, pack, write_atomic, write_manifest, Level};
use pxl_spec::bake::earth_page::{self, EARTH_LEVEL, EARTH_TEXELS};
use pxl_spec::bake::published::PUBLISHED_VERSION;
use pxl_spec::bake::quadkey::page_name;
use pxl_spec::space::adapter::{epsg3857, epsg4326};
use pxl_spec::space::pvf3::pvf3;
use pxl_spec::space::pyramid::{page_at, pages_around, GROUND};

const DAY: &str = "https://eoimages.gsfc.nasa.gov/images/imagerecords/74000/74393/world.topo.200407.3x21600x10800.jpg";
const NIGHT: &str = "https://eoimages.gsfc.nasa.gov/images/imagerecords/144000/144897/BlackMarble_2016_3km_gray.jpg";
const GLOBAL: (u32, u32) = (2048, 1024);
/// libwebp's lossy quality: a map seen kilometers wide shows nothing finer.
const QUALITY: f32 = 90.0;

/// Blue Marble's bytes are reflectance through a cubic spline, not sRGB
/// (Stöckli et al. 2005, table 1): 0, 25, 75 and 100% are 0, 179, 240 and 255.
/// Fritsch–Carlson's monotone cubic through them, since the natural spline
/// turns back between the last two, and its inverse, a byte's reflectance.
const KNOTS: [(f64, f64); 4] = [(0.0, 0.0), (0.25, 179.0), (0.75, 240.0), (1.0, 255.0)];

fn byte_of(x: f64) -> f64 {
    let n = KNOTS.len() - 1;
    let h: [f64; 3] = core::array::from_fn(|k| KNOTS[k + 1].0 - KNOTS[k].0);
    let d: [f64; 3] = core::array::from_fn(|k| (KNOTS[k + 1].1 - KNOTS[k].1) / h[k]);
    let end = |h0: f64, h1: f64, d0: f64, d1: f64| {
        let m = ((2.0 * h0 + h1) * d0 - h0 * d1) / (h0 + h1);
        if m * d0 <= 0.0 {
            0.0
        } else if d0 * d1 <= 0.0 && m.abs() > 3.0 * d0.abs() {
            3.0 * d0
        } else {
            m
        }
    };
    let mut m = [0.0; 4];
    m[0] = end(h[0], h[1], d[0], d[1]);
    m[n] = end(h[n - 1], h[n - 2], d[n - 1], d[n - 2]);
    for k in 1..n {
        if d[k - 1] * d[k] > 0.0 {
            let (w1, w2) = (2.0 * h[k] + h[k - 1], h[k] + 2.0 * h[k - 1]);
            m[k] = (w1 + w2) / (w1 / d[k - 1] + w2 / d[k]);
        }
    }
    let k = (0..n).find(|&k| x <= KNOTS[k + 1].0).unwrap_or(n - 1);
    let t = (x - KNOTS[k].0) / h[k];
    let (t2, t3) = (t * t, t * t * t);
    (2.0 * t3 - 3.0 * t2 + 1.0) * KNOTS[k].1 + (t3 - 2.0 * t2 + t) * h[k] * m[k] + (-2.0 * t3 + 3.0 * t2) * KNOTS[k + 1].1 + (t3 - t2) * h[k] * m[k + 1]
}

fn reflectance() -> [f32; 256] {
    core::array::from_fn(|b| {
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..40 {
            let mid = 0.5 * (lo + hi);
            if byte_of(mid) < b as f64 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        (0.5 * (lo + hi)) as f32
    })
}

/// The Black Marble's grays are a picture's, so they are light as shown.
fn shown() -> [f32; 256] {
    core::array::from_fn(|b| {
        let c = b as f64 / 255.0;
        (if c <= 0.04045 { c / 12.92 } else { libm::pow((c + 0.055) / 1.055, 2.4) }) as f32
    })
}

fn to_srgb(v: f32) -> u8 {
    let v = (v as f64).clamp(0.0, 1.0);
    let c = if v <= 0.0031308 { 12.92 * v } else { 1.055 * libm::pow(v, 1.0 / 2.4) - 0.055 };
    (c * 255.0).round_ties_even() as u8
}

/// Alpha is not decoded as sRGB, so it holds the lights' square root.
fn lights_byte(v: f32) -> u8 {
    (libm::sqrt(v.clamp(0.0, 1.0) as f64) * 255.0).round_ties_even() as u8
}

/// Kept beside the bake, so a second run reads it from disk.
fn source(url: &str, dir: &Path) -> Result<Vec<u8>, String> {
    let path = dir.join(url.rsplit('/').next().ok_or("a file name")?);
    if let Ok(bytes) = std::fs::read(&path) {
        return Ok(bytes);
    }
    let mut body = Vec::new();
    ureq::get(url)
        .call()
        .map_err(|e| format!("{url}: {e}"))?
        .into_reader()
        .read_to_end(&mut body)
        .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    write_atomic(&path, &body).map_err(|e| e.to_string())?;
    Ok(body)
}

/// An equirectangular raster read in linear light at a longitude and latitude, in degrees, bilinearly.
struct Plate<'a, const N: usize> {
    width: u32,
    height: u32,
    bytes: &'a [u8],
    decode: [f32; 256],
}

impl<const N: usize> Plate<'_, N> {
    fn at(&self, lng: f64, lat: f64) -> [f32; N] {
        let x = (lng + 180.0) / 360.0 * self.width as f64 - 0.5;
        let y = (90.0 - lat) / 180.0 * self.height as f64 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = ((x - x0) as f32, (y - y0) as f32);
        let texel = |dx: i64, dy: i64| -> [f32; N] {
            let px = (x0 as i64 + dx).rem_euclid(self.width as i64) as usize;
            let py = (y0 as i64 + dy).clamp(0, self.height as i64 - 1) as usize;
            let at = (py * self.width as usize + px) * N;
            core::array::from_fn(|k| self.decode[self.bytes[at + k] as usize])
        };
        let (a, b, c, d) = (texel(0, 0), texel(1, 0), texel(0, 1), texel(1, 1));
        core::array::from_fn(|k| (a[k] * (1.0 - fx) + b[k] * fx) * (1.0 - fy) + (c[k] * (1.0 - fx) + d[k] * fx) * fy)
    }
}

fn page_texels(px: i64, py: i64, day: &Plate<'_, 3>, night: &Plate<'_, 1>) -> Option<Vec<u8>> {
    let page = page_at(EARTH_LEVEL, [px, py]);
    let mut rgba = vec![0u8; earth_page::EARTH_BYTES];
    for ty in 0..EARTH_TEXELS {
        for tx in 0..EARTH_TEXELS {
            let index = GROUND.cell_index(tx, ty);
            let mid = GROUND.cell_at(page, index).mid();
            let [lng, lat, _] = epsg4326::from_mercator(&[epsg3857::to_db(pvf3::to_real(pvf3::X(&mid))), epsg3857::to_db(pvf3::to_real(pvf3::Y(&mid))), 0.0]);
            let [r, g, b] = day.at(lng, lat).map(to_srgb);
            let at = index as usize * 4;
            rgba[at..at + 4].copy_from_slice(&[r, g, b, lights_byte(night.at(lng, lat)[0])]);
        }
    }
    let first: [u8; 4] = rgba[..4].try_into().ok()?;
    (!rgba.as_chunks::<4>().0.iter().all(|t| *t == first)).then_some(rgba)
}

fn global(day: &RgbImage, night: &GrayImage, r: &[f32; 256], s: &[f32; 256], out: &Path) -> Result<(), String> {
    // Averaged in linear light, so a coast or a town does not darken as it is taken down.
    let linear_day = Rgb32FImage::from_fn(day.width(), day.height(), |x, y| image::Rgb(day.get_pixel(x, y).0.map(|c| r[c as usize])));
    let small_day = resize(&linear_day, GLOBAL.0, GLOBAL.1, FilterType::Triangle);
    drop(linear_day);
    let linear_night = Rgb32FImage::from_fn(night.width(), night.height(), |x, y| {
        let v = s[night.get_pixel(x, y).0[0] as usize];
        image::Rgb([v, v, v])
    });
    let small_night = resize(&linear_night, GLOBAL.0, GLOBAL.1, FilterType::Triangle);
    let bytes: Vec<u8> = small_day
        .pixels()
        .zip(small_night.pixels())
        .flat_map(|(d, n)| [to_srgb(d.0[0]), to_srgb(d.0[1]), to_srgb(d.0[2]), lights_byte(n.0[0])])
        .collect();
    let mut config = webp::WebPConfig::new().map_err(|()| "a WebP config")?;
    config.quality = QUALITY;
    config.alpha_compression = 1;
    // libwebp otherwise rewrites the color under a zero alpha: the unlit Earth.
    config.exact = 1;
    let encoded = webp::Encoder::from_rgba(&bytes, GLOBAL.0, GLOBAL.1).encode_advanced(&config).map_err(|e| format!("{e:?}"))?;
    write_atomic(out, &encoded).map_err(|e| e.to_string())?;
    println!("[earth] global -> {} ({} KiB)", out.display(), encoded.len() / 1024);
    Ok(())
}

/// A source is larger than the decoder's default limit allows.
fn decode(jpeg: &[u8]) -> Result<image::DynamicImage, String> {
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(jpeg), ImageFormat::Jpeg);
    reader.no_limits();
    reader.decode().map_err(|e| e.to_string())
}

fn run() -> Result<(), String> {
    let root = PathBuf::from("./out/earth");
    let sources = root.join("source");
    let out = root.join(PUBLISHED_VERSION.earth);
    let day = decode(&source(DAY, &sources)?)?.to_rgb8();
    let night = decode(&source(NIGHT, &sources)?)?.to_luma8();
    let (r, s) = (reflectance(), shown());
    global(&day, &night, &r, &s, Path::new("asset/sky/earth/07.webp"))?;

    let day_plate = Plate::<3> {
        width: day.width(),
        height: day.height(),
        bytes: day.as_raw(),
        decode: r,
    };
    let night_plate = Plate::<1> {
        width: night.width(),
        height: night.height(),
        bytes: night.as_raw(),
        decode: s,
    };
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    // Every page of the level: the Mercator square, one world across and as high.
    let half = pages_around(EARTH_LEVEL) / 2;
    let pages: Vec<[i64; 2]> = (-half..half).flat_map(|y| (-half..half).map(move |x| [x, y])).collect();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let written = std::sync::atomic::AtomicU32::new(0);
    let failed = std::sync::Mutex::new(None);
    std::thread::scope(|scope| {
        for t in 0..threads {
            let (pages, day, night, out, written, failed) = (&pages, &day_plate, &night_plate, &out, &written, &failed);
            scope.spawn(move || {
                for &[px, py] in pages.iter().skip(t).step_by(threads) {
                    let Some(rgba) = page_texels(px, py, day, night) else { continue };
                    let name = page_name(page_at(EARTH_LEVEL, [px, py])).to_string();
                    let stored = earth_page::encode(&rgba)
                        .map_err(|e| format!("{e:?}"))
                        .and_then(|pkt| pack(&pkt, Level::Stored).map_err(|e| e.to_string()));
                    match stored.and_then(|b| write_atomic(&out.join(&name), &b).map_err(|e| e.to_string())) {
                        Ok(()) => drop(written.fetch_add(1, std::sync::atomic::Ordering::Relaxed)),
                        Err(e) => *failed.lock().unwrap_or_else(|e| e.into_inner()) = Some(format!("{name}: {e}")),
                    }
                }
            });
        }
    });
    if let Some(e) = failed.into_inner().unwrap_or_else(|e| e.into_inner()) {
        return Err(e);
    }
    let objects = written.into_inner();
    println!("[earth] {objects} pages -> {}", out.display());
    write_manifest(&out, git_sha().as_deref(), serde_json::json!({ "objects": objects, "level": EARTH_LEVEL }))
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[earth] {e}");
            ExitCode::FAILURE
        }
    }
}
