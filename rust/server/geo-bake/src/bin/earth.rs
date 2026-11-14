// pxl-geo-bake/bin/earth.rs

//! The Earth's picture, both its forms from one source: NASA's Blue Marble Next
//! Generation, July 2004 (21600 × 10800, 2 km, the sea a flat blue), as
//! reflectance, and the Black Marble's lights of 2016 (86400 × 43200, 500 m, in
//! eight tiles), as `infra/earth/source.json` names them and
//! `make download-earth` keeps them. Each is averaged in linear light over what a
//! texel covers.
//!  - The earth layer's tiles (pxl-spec bake/tile/earth.rs) at its top zoom, one
//!    ground cell to a texel, are written to its set, `./out/earth/<name>`, flat
//!    under the tiles' names as every kind's set is. A tile of one color says
//!    nothing the global picture does not, and has no object. The copy an upload
//!    takes lies beside the set, in `./out/earth.publish/` (script/publish.py).
//!  - The global picture every tile falls back to, 2048 × 1024, belongs to the
//!    sphere and is no tile of the pyramid. It lies beside the tiles under its
//!    own name (`TILE_SPHERE`), packed as they are, and is published with them.
//!
//! Credit: NASA's Earth Observatory (Reto Stöckli; Miguel Román and the Black
//! Marble team).
//!
//!   make bake-earth

use pxl_spec::space::layer::lattice::{Grid, GROUND, TILE_CELLS};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use image::ImageFormat;

use pxl_geo_bake::cli;
use pxl_geo_bake::pyramid::{TileSet, MANIFEST};
use pxl_geo_bake::tile::walk::{self, Outcome};
use pxl_geo_bake::util::{git_sha, pack, write_atomic, write_manifest, Level};
use pxl_spec::bake::publication::{BaseKind, FORMAT, TILE_SPHERE};
use pxl_spec::bake::tile::earth::{self, EARTH_ZOOM};
use pxl_spec::color;
use pxl_spec::space::coord::adapter::{epsg3857, epsg4326};
use pxl_spec::space::coord::q3;
use pxl_spec::space::layer::quadrant::Quadrant;

/// What it reads, and where `make download-earth` keeps it.
const SRC: &str = "infra/earth/source.json";
const RAW: &str = "infra/earth/raw";
const GLOBAL: (u32, u32) = earth::PICTURE;

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
fn gray_light() -> [f32; 256] {
    core::array::from_fn(|b| color::linear(b as u8) as f32)
}

fn to_srgb(v: f32) -> u8 {
    color::srgb(f64::from(v))
}

/// Alpha is not decoded as sRGB, so it holds the lights' square root.
fn lights_byte(v: f32) -> u8 {
    (libm::sqrt(v.clamp(0.0, 1.0) as f64) * 255.0).round_ties_even() as u8
}

/// A file `SRC` names, as `make download-earth` kept it.
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
    fn mean(&self, [lon0, lon1]: [f64; 2], [lat0, lat1]: [f64; 2]) -> [f32; N] {
        let span = |a: f64, b: f64| {
            let (mid, half) = ((a + b) / 2.0, ((b - a).abs() / 2.0).max(0.5));
            (mid - half, mid + half)
        };
        let (x0, x1) = span(
            (lon0 + 180.0) / 360.0 * self.width as f64,
            (lon1 + 180.0) / 360.0 * self.width as f64,
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
fn lon_lat(x: f64, y: f64) -> [f64; 2] {
    let [lon, lat, _] =
        epsg4326::from_mercator(&[epsg3857::merc_from_u(x), epsg3857::merc_from_u(y), 0.0]);
    [lon, lat]
}

fn tile_texels(px: i32, py: i32, day: &Plate<'_, 3>, night: &Plate<'_, 1>) -> Option<Vec<u8>> {
    let tile = Quadrant::new(EARTH_ZOOM, [px, py]);
    let mut rgba = vec![0u8; earth::EARTH_BYTES];
    for ty in 0..TILE_CELLS {
        for tx in 0..TILE_CELLS {
            let idx = GROUND.cell_idx(tx, ty);
            let cell = GROUND.cell_at(tile, idx);
            let (base, side) = (cell.base(), cell.side());
            let (x, y) = (
                q3::to_real(i64::from(base[0])),
                q3::to_real(i64::from(base[1])),
            );
            let ([lon0, lat0], [lon1, lat1]) = (lon_lat(x, y), lon_lat(x + side, y + side));
            let [r, g, b] = day.mean([lon0, lon1], [lat0, lat1]).map(to_srgb);
            let at = idx as usize * 4;
            rgba[at..at + 4].copy_from_slice(&[
                r,
                g,
                b,
                lights_byte(night.mean([lon0, lon1], [lat0, lat1])[0]),
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
                        let lon = [
                            i as f64 * 360.0 / w as f64 - 180.0,
                            (i + 1) as f64 * 360.0 / w as f64 - 180.0,
                        ];
                        let [r, g, b] = day.mean(lon, lat).map(to_srgb);
                        texel.copy_from_slice(&[r, g, b, lights_byte(night.mean(lon, lat)[0])]);
                    }
                }
            });
        }
    });
    // Lossless: a tile of one color is left out because the global picture reads
    // the same there, which holds only if the picture keeps every byte.
    let mut cfg = webp::WebPConfig::new().map_err(|()| "a WebP config")?;
    cfg.lossless = 1;
    // libwebp otherwise rewrites the color under a zero alpha: the unlit Earth.
    cfg.exact = 1;
    let encoded = webp::Encoder::from_rgba(&bytes, GLOBAL.0, GLOBAL.1)
        .encode_advanced(&cfg)
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
    cli::enter()?;
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
        decode: gray_light(),
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
    let bands: Vec<&[[i32; 2]]> = tiles.chunks(1).collect();
    let t = walk::over(
        "earth",
        &bands,
        std::thread::available_parallelism().map_or(4, |n| n.get()),
        64,
        |[x, y]| format!("{x}:{y}"),
        || Ok(()),
        |_, [px, py]| {
            let Some(rgba) = tile_texels(px, py, &day_plate, &night_plate) else {
                return Ok(Outcome::Empty);
            };
            let pkt = earth::encode(&rgba).map_err(|e| format!("{e:?}"))?;
            let packed = pack(&pkt, Level::Stored).map_err(|e| e.to_string())?;
            set.write(Quadrant::new(EARTH_ZOOM, [px, py]), &packed)?;
            Ok(Outcome::Tile {
                bytes: packed.len() as u64,
            })
        },
    )?;
    if t.failed > 0 {
        return Err(format!("{} tiles failed", t.failed));
    }
    write_manifest(
        &out.join(MANIFEST),
        git_sha().as_deref(),
        serde_json::json!({ "objects": t.baked + 1, "z": EARTH_ZOOM, "format": FORMAT[BaseKind::Earth as usize] }),
    )?;
    // The copy an upload takes (script/publish.py), the global picture with the tiles.
    let copy = set.publish(|_| true)?;
    std::fs::copy(out.join(TILE_SPHERE), copy.join(TILE_SPHERE))
        .map_err(|e| format!("{}: {e}", out.join(TILE_SPHERE).display()))?;
    Ok(())
}

fn main() -> ExitCode {
    cli::main("earth", run)
}
