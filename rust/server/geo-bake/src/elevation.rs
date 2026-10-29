// pxl-geo-bake/elevation.rs

//! A tile's elevation grid, the one definition of ground height for the bake,
//! the client and the separation solve. It reproduces PostGIS's
//! `ST_Resample(…, 'Bilinear')` (GDAL's warper) value for value; the steps
//! below name the GDAL/PostGIS function each copies.

use fnv::FnvHashMap;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use crate::coverage::Coverage;
use postgres::Client;
use pxl_spec::bake::constant::{ele_index, ELE_PAD, ELE_PADDED_SPAN, TILE_ELE_SPAN};
use pxl_spec::procedural::Frame;
use pxl_spec::space::coord::adapter::epsg3857::get_tile_bbox;
use pxl_spec::space::coord::adapter::epsg3857::BBox3857;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};
use xpute_kit::math::scalar::round;

const HELD: usize = 512;

/// A side of the raster's blocks, in samples: the ingest cuts its warps into
/// them (`raster2pgsql -t`), each a tile of the pyramid at `BLOCK_ZOOM`, kept
/// under that tile's name.
pub const BLOCK: u32 = 256;

/// The zoom a block is a tile at: `BLOCK` samples of a leaf's `TILE_ELE_SPAN`.
pub const BLOCK_ZOOM: Zoom = LEAF_ZOOM - (BLOCK / TILE_ELE_SPAN).trailing_zeros() as Zoom;

const _: () = assert!(
    BLOCK.is_multiple_of(TILE_ELE_SPAN) && (BLOCK / TILE_ELE_SPAN).is_power_of_two(),
    "a block is a whole tile of the pyramid"
);

// The padded samples and the resample's two strides past them stay in a leaf.
const _: () = assert!(
    ELE_PAD + 2 <= TILE_ELE_SPAN,
    "a tile's elevation reads past its shard's apron"
);

/// A tile's padded grid, `ST_MakeEmptyRaster` at the stride: its top-left
/// corner, `ELE_PAD` samples out from the tile's, and the stride.
fn grid_of([min, max]: BBox3857) -> ([f64; 2], f64) {
    let stride = (max[0] - min[0]) / f64::from(TILE_ELE_SPAN);
    let pad = f64::from(ELE_PAD) * stride;
    ([min[0] - pad, max[1] + pad], stride)
}

/// What a tile's grid reads of the raster, `[minx, miny, maxx, maxy]` in
/// EPSG:3857: its padded samples and the resample's reach past them.
pub fn tile_reads(tile_bbox: BBox3857) -> [f64; 4] {
    let ([ref_x, ref_y], stride) = grid_of(tile_bbox);
    let (side, reach) = (f64::from(ELE_PADDED_SPAN) * stride, stride * 2.0);
    [
        ref_x - reach,
        ref_y - side - reach,
        ref_x + side + reach,
        ref_y + reach,
    ]
}

pub struct Raster {
    origin: [f64; 2],
    sx: f64,
    sy: f64,
    raster_tiles: Vec<RasterTileAt>,
    by_key: FnvHashMap<(i64, i64), usize>,
    side: i64,
    held: Mutex<Held>,
}

#[derive(Clone)]
struct RasterTileAt {
    tile: String,
    x0: f64,
    y0: f64,
    gx: i64,
    gy: i64,
    w: i64,
    h: i64,
}

struct RasterTile {
    vals: Vec<f32>,
}

/// Taken before its tile is read, so two workers asking for one tile read it once.
type Slot = Mutex<Option<Arc<RasterTile>>>;

#[derive(Default)]
struct Held {
    slots: FnvHashMap<usize, (u64, Arc<Slot>)>,
    tick: u64,
}

impl Raster {
    /// Refused unless the raster holds exactly the coverage's shards, each whole.
    pub fn open(client: &mut Client) -> Result<Raster, String> {
        let want: BTreeSet<String> = Coverage::of_src()?.shards().map(|s| s.name()).collect();
        let have: BTreeSet<String> = client
            .query("SELECT shard FROM elevation_catalog", &[])
            .map_err(|e| e.to_string())?
            .iter()
            .map(|r| r.get(0))
            .collect();
        if have != want {
            let (missing, extra) = (
                want.difference(&have).count(),
                have.difference(&want).count(),
            );
            return Err(format!(
                "elevation lacks {missing} of the coverage's shards and holds {extra} past them: run ./infra/geo/elevation/ingest.sh"
            ));
        }
        let rows = client
            .query(
                "SELECT tile, ST_UpperLeftX(rast) AS x0, ST_UpperLeftY(rast) AS y0, ST_ScaleX(rast) AS sx, ST_ScaleY(rast) AS sy,
                    ST_Width(rast) AS w, ST_Height(rast) AS h FROM elevation_raster",
                &[],
            )
            .map_err(|e| e.to_string())?;
        let (sx, sy) = rows.first().map_or((1.0, -1.0), |r| {
            (r.get::<_, f64>("sx"), r.get::<_, f64>("sy"))
        });
        let origin = rows.first().map_or([0.0, 0.0], |r| {
            [r.get::<_, f64>("x0"), r.get::<_, f64>("y0")]
        });
        let side = rows
            .iter()
            .map(|r| i64::from(r.get::<_, i32>("w")).max(i64::from(r.get::<_, i32>("h"))))
            .max()
            .unwrap_or(1);
        let raster_tiles: Vec<RasterTileAt> = rows
            .iter()
            .map(|r| RasterTileAt {
                tile: r.get("tile"),
                x0: r.get("x0"),
                y0: r.get("y0"),
                gx: round((r.get::<_, f64>("x0") - origin[0]) / sx) as i64,
                gy: round((r.get::<_, f64>("y0") - origin[1]) / sy) as i64,
                w: i64::from(r.get::<_, i32>("w")),
                h: i64::from(r.get::<_, i32>("h")),
            })
            .collect();
        let by_key = raster_tiles
            .iter()
            .enumerate()
            .map(|(i, t)| ((t.gx.div_euclid(side), t.gy.div_euclid(side)), i))
            .collect();
        Ok(Raster {
            origin,
            sx,
            sy,
            raster_tiles,
            by_key,
            side,
            held: Mutex::new(Held::default()),
        })
    }

    fn raster_tile(
        &self,
        client: &mut Client,
        i: usize,
    ) -> Result<Arc<RasterTile>, postgres::Error> {
        let slot = {
            let mut held = self.held.lock().expect("raster");
            held.tick += 1;
            let tick = held.tick;
            if !held.slots.contains_key(&i) && held.slots.len() >= HELD {
                let oldest = held
                    .slots
                    .iter()
                    .min_by_key(|(_, (t, _))| *t)
                    .map(|(&k, _)| k);
                if let Some(k) = oldest {
                    held.slots.remove(&k);
                }
            }
            let entry = held
                .slots
                .entry(i)
                .or_insert_with(|| (tick, Arc::new(Mutex::new(None))));
            entry.0 = tick;
            entry.1.clone()
        };
        let mut slot = slot.lock().expect("tile");
        if let Some(raster_tile) = &*slot {
            return Ok(raster_tile.clone());
        }
        let at = &self.raster_tiles[i];
        let row = client.query_one(
            "SELECT ST_AsBinary(rast) AS wkb FROM elevation_raster WHERE tile = $1",
            &[&at.tile],
        )?;
        let raster_tile = Arc::new(RasterTile::of_wkb(row.get("wkb"), (at.w * at.h) as usize));
        *slot = Some(raster_tile.clone());
        Ok(raster_tile)
    }

    /// Leaf `tile`'s padded grid of heights, in leaf cells at its scale
    /// (pxl-spec-core `cells_of_m`, pxl-spec `Frame::per_m`).
    pub fn tile_grid(
        &self,
        client: &mut Client,
        tile: [i32; 2],
    ) -> Result<Vec<i16>, postgres::Error> {
        let tile_bbox = get_tile_bbox(LEAF_ZOOM, tile);
        let per_m = Frame::of(Tile::new(LEAF_ZOOM, tile)).per_m();
        let span = ELE_PADDED_SPAN as usize;
        let ([ref_x, ref_y], stride) = grid_of(tile_bbox);
        let env = tile_reads(tile_bbox);

        // `ST_Union(ST_Clip(tile, env))`.
        let gx = |x: f64| (x - self.origin[0]) / self.sx;
        let gy = |y: f64| (y - self.origin[1]) / self.sy;
        let (cx0, cx1) = (gx(env[0]).floor() as i64, gx(env[2]).ceil() as i64);
        let (cy0, cy1) = (gy(env[3]).floor() as i64, gy(env[1]).ceil() as i64);
        let mut hull: Option<[i64; 4]> = None;
        let mut reached = Vec::new();
        for ky in cy0.div_euclid(self.side)..=(cy1 - 1).div_euclid(self.side) {
            for kx in cx0.div_euclid(self.side)..=(cx1 - 1).div_euclid(self.side) {
                let Some(&i) = self.by_key.get(&(kx, ky)) else {
                    continue;
                };
                let t = &self.raster_tiles[i];
                let Some(c) = self.clip(t, env) else { continue };
                hull = Some(hull.map_or(c, |h| {
                    [
                        h[0].min(c[0]),
                        h[1].min(c[1]),
                        h[2].max(c[2]),
                        h[3].max(c[3]),
                    ]
                }));
                reached.push(i);
            }
        }
        // raster2pgsql stores no block that is all no data.
        let Some([mx0, my0, mx1, my1]) = hull else {
            return Ok(vec![0; span * span]);
        };
        let (mw, mh) = ((mx1 - mx0) as usize, (my1 - my0) as usize);
        let mut mosaic = vec![None; mw * mh];
        for i in reached {
            let t = &self.raster_tiles[i];
            let raster_tile = self.raster_tile(client, i)?;
            for y in t.gy.max(my0)..(t.gy + t.h).min(my1) {
                for x in t.gx.max(mx0)..(t.gx + t.w).min(mx1) {
                    mosaic[(y - my0) as usize * mw + (x - mx0) as usize] =
                        raster_tile.at(((y - t.gy) * t.w + (x - t.gx)) as usize);
                }
            }
        }
        let m_x = self.origin[0] + mx0 as f64 * self.sx;
        let m_y = self.origin[1] + my0 as f64 * self.sy;

        // `ST_Resample`'s output grid (rt_warp.c `rt_raster_gdal_warp`).
        let snap = |r: f64| {
            if (round(r) - r).abs() <= f64::from(f32::EPSILON) {
                round(r)
            } else {
                r.floor()
            }
        };
        let along = |w: f64| ((w + stride / 2.0) / stride).max(1.0) as i64;
        let kx = snap((m_x - ref_x) / stride);
        let ky = snap((m_y - ref_y) / -stride);
        let out_x = ref_x + kx * stride;
        let out_y = ref_y + ky * -stride;
        let mut out_w = along(mw as f64 * self.sx);
        let mut out_h = along(mh as f64 * -self.sy);
        if (out_x - m_x).abs() > f64::from(f32::EPSILON) {
            out_w += 1;
        }
        if (out_y - m_y).abs() > f64::from(f32::EPSILON) {
            out_h += 1;
        }
        let (out_w, out_h) = (out_w as usize, out_h as usize);

        // Output pixel to mosaic pixel (`GDALGenImgProjTransform`).
        let inv = [-m_x / self.sx, 1.0 / self.sx, -m_y / self.sy, 1.0 / self.sy];
        let to_src = |x: f64, y: f64| -> (f64, f64) {
            let (gx, gy) = (out_x + x * stride, out_y + y * -stride);
            (inv[0] + gx * inv[1], inv[2] + gy * inv[3])
        };
        let x_axis = Axis::of(to_src(0.0, 0.0).0, to_src(out_w as f64, 0.0).0, out_w, mw);
        let y_axis = Axis::of(to_src(0.0, 0.0).1, to_src(0.0, out_h as f64).1, out_h, mh);
        let src = Source {
            mosaic: &mosaic,
            w: mw,
            x: x_axis,
            y: y_axis,
        };

        let mut grid = vec![0i16; span * span];
        // The tile's grid from the warp's: an offset either way, since the
        // raster can start inside the padded tile at the coverage's edge.
        let (col0, row0) = (-kx, -ky);
        let last = out_w - 1;
        for r in 0..span {
            let y = row0 + r as f64 + 0.5;
            let (s0, s1) = (to_src(0.5, y), to_src(last as f64 + 0.5, y));
            let reach = last as f64;
            let (dx, dy) = ((s1.0 - s0.0) / reach, (s1.1 - s0.1) / reach);
            for c in 0..span {
                let dist = col0 + c as f64;
                let (px, py) = (s0.0 + dx * dist, s0.1 + dy * dist);
                // PostgreSQL rounds half to even. No data is 0, the source's
                // sea: a shard's apron covers every pad, so none lacks a shard.
                if let Some(v) = src.sample(px, py) {
                    grid[ele_index(c as u32, (span - 1 - r) as u32)] =
                        pxl_spec_core::cells_of_m(v, per_m) as i16;
                }
            }
        }
        Ok(grid)
    }
}

impl Raster {
    /// `ST_Clip`'s pixels of tile `t` (rt_raster.c `rt_raster_gdal_rasterize`).
    fn clip(&self, t: &RasterTileAt, env: [f64; 4]) -> Option<[i64; 4]> {
        let (sx, sy) = (self.sx, self.sy);
        let left = env[0].max(t.x0);
        let right = env[2].min(t.x0 + t.w as f64 * sx);
        let top = env[3].min(t.y0);
        let bottom = env[1].max(t.y0 + t.h as f64 * sy);
        if left >= right || bottom >= top {
            return None;
        }
        let along = |w: f64, scale: f64| ((w + scale.abs() / 2.0) / scale.abs()).max(1.0) as i64;
        let snap = |r: f64| {
            if (round(r) - r).abs() <= f64::from(f32::EPSILON) {
                round(r)
            } else {
                r.floor()
            }
        };
        let (cx, cy) = (snap((left - t.x0) / sx), snap((top - t.y0) / sy));
        let mut w = along(right - left, sx);
        let mut h = along(top - bottom, sy);
        if (t.x0 + cx * sx - left).abs() > f64::from(f32::EPSILON) {
            w += 1;
        }
        if (t.y0 + cy * sy - top).abs() > f64::from(f32::EPSILON) {
            h += 1;
        }
        let (x0, y0) = (t.gx + cx as i64, t.gy + cy as i64);
        let c = [
            x0.max(t.gx),
            y0.max(t.gy),
            (x0 + w).min(t.gx + t.w),
            (y0 + h).min(t.gy + t.h),
        ];
        (c[0] < c[2] && c[1] < c[3]).then_some(c)
    }
}

impl RasterTile {
    fn of_wkb(b: &[u8], px: usize) -> RasterTile {
        RasterTile::read(b, px).unwrap_or(RasterTile {
            vals: vec![f32::NAN; px],
        })
    }

    fn read(b: &[u8], px: usize) -> Option<RasterTile> {
        let le = *b.first()? == 1;
        // endian 1, version 2, bands 2, scale, origin and skew as f64, srid 4,
        // width 2, height 2, then the band's pixel type and nodata.
        let band = *b.get(61)?;
        const PT_32BF: u8 = 10;
        const IS_OFFLINE: u8 = 0x80;
        const HAS_NODATA: u8 = 0x40;
        if band & 0x0f != PT_32BF || band & IS_OFFLINE != 0 {
            return None;
        }
        let f32_at = |at: usize| -> Option<f32> {
            let raw: [u8; 4] = b.get(at..at + 4)?.try_into().ok()?;
            Some(if le {
                f32::from_le_bytes(raw)
            } else {
                f32::from_be_bytes(raw)
            })
        };
        let nodata = (band & HAS_NODATA != 0).then(|| f32_at(62)).flatten();
        let values = (0..px)
            .map(|i| f32_at(66 + 4 * i).map(|v| if Some(v) == nodata { f32::NAN } else { v }))
            .collect::<Option<Vec<f32>>>()?;
        Some(RasterTile { vals: values })
    }

    fn at(&self, i: usize) -> Option<f32> {
        self.vals.get(i).copied().filter(|v| !v.is_nan())
    }
}

/// One axis as the warper sizes it (`ComputeSourceWindow`, `PerformWarp`).
#[derive(Clone, Copy)]
struct Axis {
    off: i64,
    sz: i64,
    scale: f64,
    radius: i64,
}

impl Axis {
    fn of(from: f64, to: f64, out: usize, raster: usize) -> Axis {
        let (lo, hi) = (from.min(to), from.max(to));
        let (out, raster) = (out as f64, raster as i64);
        let window_scale = (out / (hi - lo)).max(1e-3);
        let window_radius = if window_scale < 0.95 {
            (1.0 / window_scale).ceil() as i64
        } else {
            1
        };
        let lo_clamped = lo.max(0.0) as i64;
        let hi_clamped = hi.ceil().min(raster as f64) as i64;
        let raw = ((raster - lo_clamped) as f64).min(hi - lo).max(0.0);
        let (off, sz) = if (hi_clamped - lo_clamped) as f64 > 0.9 * raster as f64 {
            (0, raster)
        } else {
            let off = (lo_clamped - window_radius).min(raster).max(0);
            (
                off,
                (raster - off).min(hi_clamped - off + window_radius).max(0),
            )
        };
        let mut scale = out / raw;
        if sz as f64 >= out && sz as f64 <= out + (sz as f64 - raw) {
            scale = 1.0;
        }
        if scale < 1.0 {
            let reciprocal = 1.0 / scale;
            let whole = round(reciprocal) as i64;
            if (reciprocal - whole as f64).abs() < 0.05 {
                scale = 1.0 / whole as f64;
            }
        }
        let radius = if scale < 1.0 {
            (1.0 / scale).ceil() as i64
        } else {
            1
        };
        Axis {
            off,
            sz,
            scale,
            radius,
        }
    }
}

/// The mosaic as the warper reads it.
struct Source<'a> {
    mosaic: &'a [Option<f32>],
    w: usize,
    x: Axis,
    y: Axis,
}

impl Source<'_> {
    fn pixel(&self, x: i64, y: i64) -> Option<f64> {
        self.mosaic[y as usize * self.w + x as usize].map(f64::from)
    }

    /// As `GWKRealCaseThread`/`GWKResample` write it into a 32-bit band.
    fn sample(&self, px: f64, py: f64) -> Option<f32> {
        let (x, y) = (self.x, self.y);
        if px < x.off as f64
            || py < y.off as f64
            || px + 1e-10 > (x.sz + x.off) as f64
            || py + 1e-10 > (y.sz + y.off) as f64
        {
            return None;
        }
        let cx = ((px + 1e-10) as i64 - x.off).min(x.sz - 1);
        let cy = ((py + 1e-10) as i64 - y.off).min(y.sz - 1);
        self.pixel(cx + x.off, cy + y.off)?;

        let (sx, sy) = (px - x.off as f64, py - y.off as f64);
        let ix = (sx - 0.5).floor() as i64;
        let iy = (sy - 0.5).floor() as i64;
        let (fx, fy) = (sx - 0.5 - ix as f64, sy - 0.5 - iy as f64);
        let weight = |d: f64, a: Axis| {
            let t = if a.scale < 1.0 {
                (d * a.scale).abs()
            } else {
                d.abs()
            };
            if t <= 1.0 {
                1.0 - t
            } else {
                0.0
            }
        };
        let (j0, j1) = ((-y.radius).max(-iy), y.radius.min(y.sz - iy - 1));
        let (i0, i1) = ((-x.radius).max(-ix), x.radius.min(x.sz - ix - 1));
        // Weights are renormalized over pixels with data, except when they already
        // sum to one within the warper's tolerance.
        let (mut val, mut total) = (0.0, 0.0);
        for j in j0..=j1 {
            let wy = weight(j as f64 - fy, y);
            let (mut row_val, mut row_total) = (0.0, 0.0);
            for i in i0..=i1 {
                let Some(v) = self.pixel(ix + i + x.off, iy + j + y.off) else {
                    continue;
                };
                let wx = weight(i as f64 - fx, x);
                row_val += v * wx;
                row_total += wx;
            }
            val += row_val * wy;
            total += row_total * wy;
        }
        if total < 0.000001 {
            return None;
        }
        Some(if !(0.99999..=1.00001).contains(&total) {
            val / total
        } else {
            val
        } as f32)
    }
}
