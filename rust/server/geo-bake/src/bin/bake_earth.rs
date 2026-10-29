// pxl-geo-bake/bin/bake_earth.rs

//! The Earth's picture, both its forms from one source: NASA's Blue Marble Next
//! Generation, July 2004 (21600 × 10800, 2 km, the sea a flat blue), as
//! reflectance, and the Black Marble's lights of 2016 (86400 × 43200, 500 m, in
//! eight tiles), as `infra/earth/source.json` names them and
//! `make download-earth` keeps them. Each is averaged in linear light over what a
//! texel covers.
//!  - The earth layer's tiles (pxl-spec bake/tile/earth.rs) at its top zoom, a
//!    ground cell a texel, to its set, `./out/earth/<name>`, as every kind's is
//!    flat under its tiles' names. A tile that is one color says nothing the
//!    global picture does not, and has no object.
//!    Its upload's copy, `./out/earth.publish/`, beside it (script/publish.py).
//!  - The global picture every tile falls back to, 2048 × 1024, the sphere's
//!    and no tile of the pyramid, beside the tiles under its own name
//!    (`TILE_SPHERE`), packed as they are, and published with them.
//!
//! Credit: NASA's Earth Observatory (Reto Stöckli; Miguel Román and the Black
//! Marble team).
//!
//!   make bake-earth

use pxl_spec::space::layer::lattice::Cells;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use image::ImageFormat;

use pxl_geo_bake::pyramid::{TileSet, MANIFEST};
use pxl_geo_bake::util::{git_sha, pack, write_atomic, write_manifest, Level};
use pxl_spec::bake::published::FORMAT;
use pxl_spec::bake::tile::earth::{self, EARTH_ZOOM};
use pxl_spec::route::TILE_SPHERE;
use pxl_spec::space::coord::adapter::{epsg3857, epsg4326};
use pxl_spec::space::coord::q3;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::lattice::TILE_CELLS;
use pxl_spec::space::layer::tile::tile_at;

/// What it reads, and where `make download-earth` keeps it.
const SRC: &str = "infra/earth/source.json";
const RAW: &str = "infra/earth/raw";
const GLOBAL: (u32, u32) = pxl_spec::bake::tile::earth::PICTURE;

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
    (2.0 * t3 - 3.0 * t2 + 1.0) * KNOTS[k].1
        + (t3 - 2.0 * t2 + t) * h[k] * m[k]
        + (-2.0 * t3 + 3.0 * t2) * KNOTS[k + 1].1
        + (t3 - t2) * h[k] * m[k + 1]
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
        (if c <= 0.04045 {
            c / 12.92
        } else {
            libm::pow((c + 0.055) / 1.055, 2.4)
        }) as f32
    })
}

fn to_srgb(v: f32) -> u8 {
    let v = (v as f64).clamp(0.0, 1.0);
    let c = if v <= 0.0031308 {
        12.92 * v
    } else {
        1.055 * libm::pow(v, 1.0 / 2.4) - 0.055
    };
    (c * 255.0).round_ties_even() as u8
}

/// Alpha is not decoded as sRGB, so it holds the lights' square root.
fn lights_byte(v: f32) -> u8 {
    (libm::sqrt(v.clamp(0.0, 1.0) as f64) * 255.0).round_ties_even() as u8
}

/// A file `SOURCE` names, as `download earth` kept it.
fn src(url: &str) -> Result<Vec<u8>, String> {
    let path = Path::new(RAW).join(url.rsplit('/').next().ok_or("a file name")?);
    std::fs::read(&path).map_err(|e| format!("{}: {e} — run `make download-earth`", path.display()))
}

/// An equirectangular raster read in linear light.
struct Plate<'a, const N: usize> {
    width: usize,
    height: usize,
    bytes: &'a [u8],
    decode: [f32; 256],
}

impl<const N: usize> Plate<'_, N> {
    /// The mean over a box of longitude and latitude, in degrees, each pixel
    /// weighed by how much of it the box covers; a box narrower than a pixel is
    /// widened to one, so a coarse source is read as a tent, not as steps.
    fn mean(&self, [lng0, lng1]: [f64; 2], [lat0, lat1]: [f64; 2]) -> [f32; N] {
        let span = |a: f64, b: f64| {
            let (mid, half) = ((a + b) / 2.0, ((b - a).abs() / 2.0).max(0.5));
            (mid - half, mid + half)
        };
        let (x0, x1) = span(
            (lng0 + 180.0) / 360.0 * self.width as f64,
            (lng1 + 180.0) / 360.0 * self.width as f64,
        );
        let (y0, y1) = span(
            (90.0 - lat1) / 180.0 * self.height as f64,
            (90.0 - lat0) / 180.0 * self.height as f64,
        );
        let mut sum = [0f64; N];
        let mut weight = 0.0;
        for py in y0.floor() as i64..y1.ceil() as i64 {
            let wy = (y1.min(py as f64 + 1.0) - y0.max(py as f64)).max(0.0);
            let row = py.clamp(0, self.height as i64 - 1) as usize * self.width;
            for px in x0.floor() as i64..x1.ceil() as i64 {
                let w = wy * (x1.min(px as f64 + 1.0) - x0.max(px as f64)).max(0.0);
                let at = (row + px.rem_euclid(self.width as i64) as usize) * N;
                for (k, s) in sum.iter_mut().enumerate() {
                    *s += w * f64::from(self.decode[self.bytes[at + k] as usize]);
                }
                weight += w;
            }
        }
        sum.map(|s| (s / weight) as f32)
    }
}

/// Longitude and latitude, in degrees, of a point of the plane in units.
fn lng_lat(x: f64, y: f64) -> [f64; 2] {
    let [lng, lat, _] =
        epsg4326::from_mercator(&[epsg3857::mer_from_u(x), epsg3857::mer_from_u(y), 0.0]);
    [lng, lat]
}

fn tile_texels(px: i32, py: i32, day: &Plate<'_, 3>, night: &Plate<'_, 1>) -> Option<Vec<u8>> {
    let tile = tile_at(EARTH_ZOOM, [px, py]);
    let mut rgba = vec![0u8; earth::EARTH_BYTES];
    for ty in 0..TILE_CELLS {
        for tx in 0..TILE_CELLS {
            let idx = GROUND.cell_index(tx, ty);
            let cell = GROUND.cell_at(tile, idx);
            let (base, side) = (cell.base(), cell.side());
            let (x, y) = (
                q3::to_real(i64::from(base[0])),
                q3::to_real(i64::from(base[1])),
            );
            let ([lng0, lat0], [lng1, lat1]) = (lng_lat(x, y), lng_lat(x + side, y + side));
            let [r, g, b] = day.mean([lng0, lng1], [lat0, lat1]).map(to_srgb);
            let at = idx as usize * 4;
            rgba[at..at + 4].copy_from_slice(&[
                r,
                g,
                b,
                lights_byte(night.mean([lng0, lng1], [lat0, lat1])[0]),
            ]);
        }
    }
    let first: [u8; 4] = rgba[..4].try_into().ok()?;
    (!rgba.as_chunks::<4>().0.iter().all(|t| *t == first)).then_some(rgba)
}

fn global(day: &Plate<'_, 3>, night: &Plate<'_, 1>, out: &Path) -> Result<(), String> {
    let (w, h) = (GLOBAL.0 as usize, GLOBAL.1 as usize);
    let mut bytes = vec![0u8; w * h * 4];
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|scope| {
        for (t, rows) in bytes.chunks_mut(w * 4 * h.div_ceil(threads)).enumerate() {
            scope.spawn(move || {
                for (r, row) in rows.chunks_mut(w * 4).enumerate() {
                    let j = t * h.div_ceil(threads) + r;
                    let lat = [
                        90.0 - (j + 1) as f64 * 180.0 / h as f64,
                        90.0 - j as f64 * 180.0 / h as f64,
                    ];
                    for (i, texel) in row.chunks_mut(4).enumerate() {
                        let lng = [
                            i as f64 * 360.0 / w as f64 - 180.0,
                            (i + 1) as f64 * 360.0 / w as f64 - 180.0,
                        ];
                        let [r, g, b] = day.mean(lng, lat).map(to_srgb);
                        texel.copy_from_slice(&[r, g, b, lights_byte(night.mean(lng, lat)[0])]);
                    }
                }
            });
        }
    });
    // Lossless: a tile of one color is left out because the global picture reads
    // the same there, which holds only if the picture keeps every byte; lossy, the
    // open sea came back a step off the tiles' and their edges showed.
    let mut config = webp::WebPConfig::new().map_err(|()| "a WebP config")?;
    config.lossless = 1;
    // libwebp otherwise rewrites the color under a zero alpha: the unlit Earth.
    config.exact = 1;
    let encoded = webp::Encoder::from_rgba(&bytes, GLOBAL.0, GLOBAL.1)
        .encode_advanced(&config)
        .map_err(|e| format!("{e:?}"))?;
    let back = webp::Decoder::new(&encoded)
        .decode()
        .ok_or("the global picture does not decode")?;
    if *back != bytes[..] {
        return Err(
            "the global picture does not keep its bytes, which a left-out tile reads".into(),
        );
    }
    // Packed as every object of the set is, so a version is labeled one way.
    write_atomic(
        out,
        &pack(&encoded, Level::Stored).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!(
        "[earth] global -> {} ({} KiB)",
        out.display(),
        encoded.len() / 1024
    );
    Ok(())
}

/// The Black Marble's eight tiles as one raster.
/// The night's tiles, `rows` of NASA's tiling, put back into one plate.
fn night_mosaic(rows: &[Vec<&str>]) -> Result<(Vec<u8>, usize, usize), String> {
    let mut mosaic = Vec::new();
    let (mut tile, mut width) = (0, 0);
    for (row, names) in rows.iter().enumerate() {
        for (col, name) in names.iter().enumerate() {
            let t = decode(&src(name)?)?.to_luma8();
            if mosaic.is_empty() {
                tile = t.width() as usize;
                width = tile * names.len();
                mosaic = vec![0u8; width * tile * rows.len()];
            }
            if t.width() as usize != tile || t.height() as usize != tile {
                return Err(format!("{name}: {}×{}, not {tile}²", t.width(), t.height()));
            }
            for (y, line) in t.as_raw().chunks(tile).enumerate() {
                let at = (row * tile + y) * width + col * tile;
                mosaic[at..at + tile].copy_from_slice(line);
            }
        }
    }
    Ok((mosaic, width, tile * rows.len()))
}

/// A source is larger than the decoder's default limit allows.
fn decode(jpeg: &[u8]) -> Result<image::DynamicImage, String> {
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(jpeg), ImageFormat::Jpeg);
    reader.no_limits();
    reader.decode().map_err(|e| e.to_string())
}

fn run() -> Result<(), String> {
    pxl_geo_bake::cli::enter()?;
    let out = PathBuf::from("./out/earth");
    let text = std::fs::read_to_string(SRC).map_err(|e| format!("{SRC}: {e}"))?;
    let named: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{SRC}: {e}"))?;
    let day = decode(&src(named["day"]
        .as_str()
        .ok_or(format!("{SRC}: no day"))?)?)?
    .to_rgb8();
    let rows: Vec<Vec<&str>> = named["night"]
        .as_array()
        .ok_or(format!("{SRC}: no night"))?
        .iter()
        .map(|r| {
            r.as_array()
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
                .collect()
        })
        .collect();
    let (night, night_width, night_height) = night_mosaic(&rows)?;
    let day_plate = Plate::<3> {
        width: day.width() as usize,
        height: day.height() as usize,
        bytes: day.as_raw(),
        decode: reflectance(),
    };
    let night_plate = Plate::<1> {
        width: night_width,
        height: night_height,
        bytes: &night,
        decode: shown(),
    };
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    global(&day_plate, &night_plate, &out.join(TILE_SPHERE))?;
    // The set is made whole each time: a tile now one color has no object.
    let set = TileSet::new(&out);
    for p in set.by_zoom()?.into_values().flatten() {
        set.remove(p)?;
    }
    // Every tile of the zoom: the Mercator square, one world across and as high.
    let half = (1 << EARTH_ZOOM) / 2;
    let tiles: Vec<[i32; 2]> = (-half..half)
        .flat_map(|y| (-half..half).map(move |x| [x, y]))
        .collect();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let written = std::sync::atomic::AtomicU32::new(0);
    let failed = std::sync::Mutex::new(None);
    std::thread::scope(|scope| {
        for t in 0..threads {
            let (tiles, day, night, out, written, failed) =
                (&tiles, &day_plate, &night_plate, &out, &written, &failed);
            scope.spawn(move || {
                for &[px, py] in tiles.iter().skip(t).step_by(threads) {
                    let Some(rgba) = tile_texels(px, py, day, night) else {
                        continue;
                    };
                    let name = tile_at(EARTH_ZOOM, [px, py]).name();
                    let stored = earth::encode(&rgba)
                        .map_err(|e| format!("{e:?}"))
                        .and_then(|pkt| pack(&pkt, Level::Stored).map_err(|e| e.to_string()));
                    match stored
                        .and_then(|b| write_atomic(&out.join(&name), &b).map_err(|e| e.to_string()))
                    {
                        Ok(()) => drop(written.fetch_add(1, std::sync::atomic::Ordering::Relaxed)),
                        Err(e) => {
                            *failed.lock().unwrap_or_else(|e| e.into_inner()) =
                                Some(format!("{name}: {e}"))
                        }
                    }
                }
            });
        }
    });
    if let Some(e) = failed.into_inner().unwrap_or_else(|e| e.into_inner()) {
        return Err(e);
    }
    let objects = written.into_inner();
    println!("[earth] {objects} tiles -> {}", out.display());
    write_manifest(
        &out.join(MANIFEST),
        git_sha().as_deref(),
        serde_json::json!({ "objects": objects + 1, "z": EARTH_ZOOM, "format": FORMAT.earth }),
    )?;
    // The copy an upload takes (script/publish.py), its manifest with it, as the other bakes' `publish` stage makes one.
    let copy = PathBuf::from("./out/earth.publish");
    if copy.exists() {
        std::fs::remove_dir_all(&copy).map_err(|e| format!("{}: {e}", copy.display()))?;
    }
    std::fs::create_dir_all(&copy).map_err(|e| e.to_string())?;
    for p in set.by_zoom()?.into_values().flatten() {
        std::fs::hard_link(set.path(p), copy.join(p.name()))
            .map_err(|e| format!("{}: {e}", set.path(p).display()))?;
    }
    for name in [TILE_SPHERE, MANIFEST] {
        std::fs::copy(out.join(name), copy.join(name)).map_err(|e| e.to_string())?;
    }
    Ok(())
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
