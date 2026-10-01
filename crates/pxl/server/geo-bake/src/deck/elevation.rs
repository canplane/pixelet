// pxl-geo-bake/deck/elevation.rs

//! The ground's height at many points, as the pages draw it.
//!
//! A deck comes down to the ground and a tunnel's bore is cut under it, so
//! the height they are solved against is the surface the client draws, not
//! the raster under it: a page's elevation is the raster resampled to a grid
//! of 32 samples a side and interpolated bicubically between them
//! (terrain/repository.rs `elevation_sql`, pxl-spec bake/surface.rs), which
//! stands metres off the raw raster on a slope. Each page's grid is made
//! again here as the bake makes it — bilinear at each sample's center,
//! quantized to cells — and each point read from it with the client's own
//! interpolation, so a deck's end and a bore's floor meet the ground on the
//! screen.
//!
//! `ST_Value` in a join is the obvious query for the raster and the wrong
//! one: each row detoasts the whole 256 × 256 tile it reads. Each tile a
//! page's grid needs is fetched once, as WKB, and kept while pages near it
//! are made.

use std::collections::HashMap;

use postgres::Client;
use pxl_spec::bake::constant::{ELE_PAD, ELE_PADDED_SPAN, PAGE_ELE_SPAN, XY_CELL_PER_M, CELL_PER_M};
use pxl_spec::bake::surface::bicubic;
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::indexer::L3_IDX;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::Pvf3;

/// One tile's band, as its WKB carries it.
struct Tile {
    x0: f64,
    y0: f64,
    sx: f64,
    sy: f64,
    w: usize,
    h: usize,
    nodata: Option<f32>,
    values: Vec<f32>,
}

impl Tile {
    /// Raster WKB (PostGIS raster's own, not OGC's): a header, then the
    /// first band as a pixel type, its nodata value and its pixels. Only
    /// what `elevation_raster` holds — one 32-bit float band, in the file —
    /// is read; anything else is no tile.
    fn of_wkb(b: &[u8]) -> Option<Tile> {
        let le = *b.first()? == 1;
        let f64_at = |at: usize| -> Option<f64> {
            let raw: [u8; 8] = b.get(at..at + 8)?.try_into().ok()?;
            Some(if le { f64::from_le_bytes(raw) } else { f64::from_be_bytes(raw) })
        };
        let u16_at = |at: usize| -> Option<u16> {
            let raw: [u8; 2] = b.get(at..at + 2)?.try_into().ok()?;
            Some(if le { u16::from_le_bytes(raw) } else { u16::from_be_bytes(raw) })
        };
        // endian 1, version 2, bands 2, scale x/y, origin x/y, skew x/y as
        // f64, srid 4, width 2, height 2.
        let (sx, sy, x0, y0) = (f64_at(5)?, f64_at(13)?, f64_at(21)?, f64_at(29)?);
        let (w, h) = (u16_at(57)? as usize, u16_at(59)? as usize);
        let band = *b.get(61)?;
        const PT_32BF: u8 = 10;
        const IS_OFFLINE: u8 = 0x80;
        const HAS_NODATA: u8 = 0x40;
        if band & 0x0f != PT_32BF || band & IS_OFFLINE != 0 {
            return None;
        }
        let f32_at = |at: usize| -> Option<f32> {
            let raw: [u8; 4] = b.get(at..at + 4)?.try_into().ok()?;
            Some(if le { f32::from_le_bytes(raw) } else { f32::from_be_bytes(raw) })
        };
        let nodata = (band & HAS_NODATA != 0).then(|| f32_at(62)).flatten();
        let values = (0..w * h).map(|i| f32_at(66 + 4 * i)).collect::<Option<Vec<f32>>>()?;
        Some(Tile { x0, y0, sx, sy, w, h, nodata, values })
    }

    /// The height at (x, y), bilinear between the four pixel centres around
    /// it, a pixel at the tile's edge standing for what lies past it; none
    /// where any of the four is nodata.
    fn sample(&self, x: f64, y: f64) -> Option<f64> {
        let (fx, fy) = ((x - self.x0) / self.sx - 0.5, (y - self.y0) / self.sy - 0.5);
        let (ix, iy) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - ix, fy - iy);
        let at = |i: f64, j: f64| -> Option<f64> {
            let (i, j) = ((i as i64).clamp(0, self.w as i64 - 1) as usize, (j as i64).clamp(0, self.h as i64 - 1) as usize);
            let v = self.values[j * self.w + i];
            (Some(v) != self.nodata && v.is_finite()).then_some(f64::from(v))
        };
        let (a, b, c, d) = (at(ix, iy)?, at(ix + 1.0, iy)?, at(ix, iy + 1.0)?, at(ix + 1.0, iy + 1.0)?);
        Some((a * (1.0 - tx) + b * tx) * (1.0 - ty) + (c * (1.0 - tx) + d * tx) * ty)
    }
}

/// The drawn surface at each point, in metres.
pub fn surface(client: &mut Client, points: &[[f64; 2]]) -> Result<Vec<f64>, postgres::Error> {
    let mut raster = Raster::open(client)?;
    let mut by_page: HashMap<IVec, Vec<usize>> = HashMap::new();
    for (i, p) in points.iter().enumerate() {
        let mut at = Pvf3::default();
        epsg3857::project(&mut at, p[0], p[1], 0.0);
        by_page.entry(L3_IDX(at.i)).or_default().push(i);
    }
    // Pages in row order, so the tiles a page's grid reads are mostly the
    // last page's.
    let mut pages: Vec<IVec> = by_page.keys().copied().collect();
    pages.sort_by_key(|&p| (ivec::Y(p), ivec::X(p)));
    let mut out = vec![0.0; points.len()];
    for page in pages {
        let [min, max] = epsg3857::get_page_bbox(page);
        let cell_per_m = CELL_PER_M(min[1]);
        let stride = (max[0] - min[0]) / f64::from(PAGE_ELE_SPAN);
        let pad = f64::from(ELE_PAD);
        let span = ELE_PADDED_SPAN as usize;
        let mut grid = vec![0i16; span * span];
        for sy in 0..span {
            for sx in 0..span {
                let x = min[0] + (sx as f64 - pad + 0.5) * stride;
                let y = min[1] + (sy as f64 - pad + 0.5) * stride;
                let m = raster.at(client, x, y)?.unwrap_or(0.0);
                grid[sy * span + sx] = (m * cell_per_m).round() as i16;
            }
        }
        for &i in &by_page[&page] {
            let [x, y] = points[i];
            let (tx, ty) = ((x - min[0]) * XY_CELL_PER_M, (y - min[1]) * XY_CELL_PER_M);
            out[i] = f64::from(bicubic(&grid, tx, ty)) / cell_per_m;
        }
    }
    Ok(out)
}

/// The raster's tiles, read as they are asked for and kept a few hundred at
/// a time.
struct Raster {
    origin: [f64; 2],
    tile: [f64; 2],
    rid_of: HashMap<(i64, i64), i32>,
    held: HashMap<i32, Option<Tile>>,
}

/// Tiles held at once: a few rows of pages' worth.
const HELD: usize = 512;

impl Raster {
    /// The tiles' extents, a header read that detoasts nothing past it. They
    /// are one grid of one pixel size, so a point's tile is found by
    /// dividing.
    fn open(client: &mut Client) -> Result<Raster, postgres::Error> {
        let rows = client.query(
            "SELECT rid, ST_UpperLeftX(rast) AS x0, ST_UpperLeftY(rast) AS y0, ST_ScaleX(rast) AS sx, ST_ScaleY(rast) AS sy, ST_Width(rast) AS w FROM elevation_raster",
            &[],
        )?;
        let (sx, sy) = rows.first().map_or((1.0, -1.0), |r| (r.get::<_, f64>("sx"), r.get::<_, f64>("sy")));
        let side = rows.iter().map(|r| r.get::<_, i32>("w")).max().unwrap_or(1) as f64;
        let origin = [rows.iter().map(|r| r.get::<_, f64>("x0")).fold(f64::INFINITY, f64::min), rows.iter().map(|r| r.get::<_, f64>("y0")).fold(f64::NEG_INFINITY, f64::max)];
        let mut raster = Raster {
            origin,
            tile: [sx * side, sy * side],
            rid_of: HashMap::new(),
            held: HashMap::new(),
        };
        for r in &rows {
            let key = raster.key(r.get::<_, f64>("x0") + sx / 2.0, r.get::<_, f64>("y0") + sy / 2.0);
            raster.rid_of.insert(key, r.get("rid"));
        }
        Ok(raster)
    }

    fn key(&self, x: f64, y: f64) -> (i64, i64) {
        (((x - self.origin[0]) / self.tile[0]).floor() as i64, ((y - self.origin[1]) / self.tile[1]).floor() as i64)
    }

    /// The raster's height at (x, y), bilinear, as `ST_Value`'s `'bilinear'`
    /// and the bake's `ST_Resample` read it; none off the raster.
    fn at(&mut self, client: &mut Client, x: f64, y: f64) -> Result<Option<f64>, postgres::Error> {
        let Some(&rid) = self.rid_of.get(&self.key(x, y)) else { return Ok(None) };
        if !self.held.contains_key(&rid) {
            if self.held.len() >= HELD {
                self.held.clear();
            }
            let row = client.query_one("SELECT ST_AsBinary(rast) AS wkb FROM elevation_raster WHERE rid = $1", &[&rid])?;
            self.held.insert(rid, Tile::of_wkb(row.get("wkb")));
        }
        Ok(self.held[&rid].as_ref().and_then(|t| t.sample(x, y)))
    }
}
