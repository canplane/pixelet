// pxl-geo-bake/deck/elevation.rs

//! The ground's height at many points, read from `elevation_raster` a tile
//! at a time.
//!
//! `ST_Value` in a join is the obvious query and the wrong one: each row
//! detoasts the whole 256 × 256 tile it reads, so two and a half million
//! points along the decks read a tile two and a half million times, some ten
//! minutes. Here each tile a point falls in is fetched once, as WKB, and
//! every point in it is sampled from it — bilinearly, as `ST_Value`'s
//! `'bilinear'` does, on the raster the pages' own elevation is resampled
//! from.

use std::collections::HashMap;

use postgres::Client;

/// Tiles a fetch asks for at once.
const TILES_PER_FETCH: usize = 64;

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

/// The ground at each point, in metres: zero where the raster has none,
/// which is the sea's.
pub fn ground(client: &mut Client, points: &[[f64; 2]]) -> Result<Vec<f64>, postgres::Error> {
    // Which tile each point is in, from the tiles' extents alone: a header
    // read, which detoasts nothing past it.
    let extents = client.query(
        "SELECT rid, ST_UpperLeftX(rast) AS x0, ST_UpperLeftY(rast) AS y0, ST_ScaleX(rast) AS sx, ST_ScaleY(rast) AS sy, ST_Width(rast) AS w, ST_Height(rast) AS h FROM elevation_raster",
        &[],
    )?;
    let Some(first) = extents.first() else { return Ok(vec![0.0; points.len()]) };
    // The tiles are one grid of one pixel size, so a point's tile is found by
    // dividing, keyed on the grid's full tile size.
    let (sx, sy): (f64, f64) = (first.get("sx"), first.get("sy"));
    let side = extents.iter().map(|r| r.get::<_, i32>("w")).max().unwrap_or(1) as f64;
    let origin_x = extents.iter().map(|r| r.get::<_, f64>("x0")).fold(f64::INFINITY, f64::min);
    let origin_y = extents.iter().map(|r| r.get::<_, f64>("y0")).fold(f64::NEG_INFINITY, f64::max);
    let key = |x: f64, y: f64| (((x - origin_x) / (sx * side)).floor() as i64, ((y - origin_y) / (sy * side)).floor() as i64);
    let mut tile_of: HashMap<(i64, i64), i32> = HashMap::new();
    for r in &extents {
        let (x0, y0): (f64, f64) = (r.get("x0"), r.get("y0"));
        // A tile's own key, from a point just inside its upper-left corner.
        tile_of.insert(key(x0 + sx / 2.0, y0 + sy / 2.0), r.get("rid"));
    }
    let mut by_tile: HashMap<i32, Vec<usize>> = HashMap::new();
    for (i, p) in points.iter().enumerate() {
        if let Some(&rid) = tile_of.get(&key(p[0], p[1])) {
            by_tile.entry(rid).or_default().push(i);
        }
    }
    let mut out = vec![0.0; points.len()];
    let rids: Vec<i32> = by_tile.keys().copied().collect();
    for batch in rids.chunks(TILES_PER_FETCH) {
        for r in client.query("SELECT rid, ST_AsBinary(rast) AS wkb FROM elevation_raster WHERE rid = ANY($1)", &[&batch])? {
            let Some(tile) = Tile::of_wkb(r.get("wkb")) else { continue };
            for &i in &by_tile[&r.get::<_, i32>("rid")] {
                out[i] = tile.sample(points[i][0], points[i][1]).unwrap_or(0.0);
            }
        }
    }
    Ok(out)
}
