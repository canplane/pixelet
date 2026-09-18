// pxl-geo-bake/wkb.rs

//! Minimal WKB decoder shared by every source adapter that reads
//! `ST_AsBinary(geom)` rows (terrain/pipeline/coast_adapter.rs,
//! overture/model/overture_adapter.rs). Supports Point, LineString, Polygon
//! and MultiPolygon. Accepts EWKB flags (Z/SRID) and ignores SRID and Z
//! because the pipeline assumes 3857.

use pxl_spec::space::adapter::epsg3857::G3857;

pub struct WKBGeometry {
    pub points: Vec<G3857>,
    pub holes: Vec<Vec<G3857>>,
    pub is_polygon_source: bool,
}

/// A cursor over the bytes. Every read is bounds-checked and a short buffer
/// ends the decode rather than panicking: the TypeScript's `DataView` throws
/// on a read past the end and the caller treats a malformed row as no
/// geometry, which is what `None` here becomes.
struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn u8(&mut self) -> Option<u8> {
        let v = *self.b.get(self.at)?;
        self.at += 1;
        Some(v)
    }

    fn u32(&mut self, le: bool) -> Option<u32> {
        let raw: [u8; 4] = self.b.get(self.at..self.at + 4)?.try_into().ok()?;
        self.at += 4;
        Some(if le { u32::from_le_bytes(raw) } else { u32::from_be_bytes(raw) })
    }

    fn f64(&mut self, le: bool) -> Option<f64> {
        let raw: [u8; 8] = self.b.get(self.at..self.at + 8)?.try_into().ok()?;
        self.at += 8;
        Some(if le { f64::from_le_bytes(raw) } else { f64::from_be_bytes(raw) })
    }

    fn skip(&mut self, n: usize) -> Option<()> {
        self.at = self.at.checked_add(n).filter(|at| *at <= self.b.len())?;
        Some(())
    }
}

pub fn wkb_to_geometries(wkb: &[u8]) -> Vec<WKBGeometry> {
    if wkb.is_empty() {
        return Vec::new();
    }
    let mut r = Reader { b: wkb, at: 0 };
    read_any(&mut r).unwrap_or_default()
}

fn read_any(r: &mut Reader) -> Option<Vec<WKBGeometry>> {
    let le = r.u8()? == 1;
    let t = r.u32(le)?;

    let has_srid = t & 0x2000_0000 != 0;
    let has_z = t & 0x8000_0000 != 0;
    let base = t & 0xff;
    if has_srid {
        r.skip(4)?;
    }

    match base {
        1 => {
            // Point
            let x = r.f64(le)?;
            let y = r.f64(le)?;
            if has_z {
                r.skip(8)?;
            }
            Some(vec![WKBGeometry {
                points: vec![[x, y, 0.0]],
                holes: Vec::new(),
                is_polygon_source: false,
            }])
        }
        2 => {
            // LineString
            let pts = read_ring(r, le, has_z)?;
            Some(vec![WKBGeometry {
                points: pts,
                holes: Vec::new(),
                is_polygon_source: false,
            }])
        }
        3 => {
            // Polygon: ring 0 is the outer, the rest are holes.
            let nr = r.u32(le)?;
            if nr == 0 {
                return Some(vec![WKBGeometry {
                    points: Vec::new(),
                    holes: Vec::new(),
                    is_polygon_source: true,
                }]);
            }
            let outer = read_ring(r, le, has_z)?;
            let mut holes: Vec<Vec<G3857>> = Vec::new();
            for _ in 1..nr {
                holes.push(read_ring(r, le, has_z)?);
            }
            Some(vec![WKBGeometry {
                points: outer,
                holes,
                is_polygon_source: true,
            }])
        }
        6 => {
            // MultiPolygon
            let np = r.u32(le)?;
            let mut out: Vec<WKBGeometry> = Vec::new();
            for _ in 0..np {
                for mut g in read_any(r)? {
                    // Each element is a Polygon; force polygon semantics.
                    g.is_polygon_source = true;
                    out.push(g);
                }
            }
            Some(out)
        }
        // unsupported => empty
        _ => Some(Vec::new()),
    }
}

fn read_ring(r: &mut Reader, le: bool, has_z: bool) -> Option<Vec<G3857>> {
    let n = r.u32(le)? as usize;
    // A length word off the wire sizes an allocation, so it is not trusted:
    // each point is at least 16 bytes, and a ring claiming more than the
    // buffer could hold is a malformed row rather than a reservation.
    if n.saturating_mul(16) > r.b.len() - r.at.min(r.b.len()) {
        return None;
    }
    let mut pts: Vec<G3857> = Vec::with_capacity(n);
    for _ in 0..n {
        let x = r.f64(le)?;
        let y = r.f64(le)?;
        if has_z {
            r.skip(8)?;
        }
        pts.push([x, y, 0.0]);
    }
    Some(pts)
}

#[cfg(test)]
#[path = "wkb.test.rs"]
mod test;
