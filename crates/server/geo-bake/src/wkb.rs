// pxl-geo-bake/wkb.rs

//! Minimal WKB decoder: Point, LineString, Polygon, MultiPolygon; EWKB flags
//! and ISO dimensions. SRID is ignored (3857 assumed); Z is kept as a line's top.

use pxl_spec::space::coord::adapter::epsg3857::G3857;

pub struct WkbGeometry {
    pub points: Vec<G3857>,
    pub holes: Vec<Vec<G3857>>,
    pub is_polygon_source: bool,
}

/// A short buffer ends the decode rather than panicking.
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

pub fn wkb_to_geometries(wkb: &[u8]) -> Vec<WkbGeometry> {
    if wkb.is_empty() {
        return Vec::new();
    }
    let mut r = Reader { b: wkb, at: 0 };
    read_any(&mut r).unwrap_or_default()
}

fn read_any(r: &mut Reader) -> Option<Vec<WkbGeometry>> {
    let le = r.u8()? == 1;
    let t = r.u32(le)?;

    let has_srid = t & 0x2000_0000 != 0;
    // ISO writes the dimensions as thousands of the type: 1 Z, 2 M, 3 ZM.
    let iso = (t & 0xffff) / 1000;
    let dims = Dims {
        z: t & 0x8000_0000 != 0 || iso == 1 || iso == 3,
        m: t & 0x4000_0000 != 0 || iso == 2 || iso == 3,
    };
    let base = (t & 0xffff) % 1000;
    if has_srid {
        r.skip(4)?;
    }

    match base {
        1 => {
            let p = read_point(r, le, dims)?;
            Some(vec![WkbGeometry {
                points: vec![p],
                holes: Vec::new(),
                is_polygon_source: false,
            }])
        }
        2 => {
            let pts = read_ring(r, le, dims)?;
            Some(vec![WkbGeometry {
                points: pts,
                holes: Vec::new(),
                is_polygon_source: false,
            }])
        }
        3 => {
            // Ring 0 is the outer, the rest are holes.
            let nr = r.u32(le)?;
            if nr == 0 {
                return Some(vec![WkbGeometry {
                    points: Vec::new(),
                    holes: Vec::new(),
                    is_polygon_source: true,
                }]);
            }
            let outer = read_ring(r, le, dims)?;
            let mut holes: Vec<Vec<G3857>> = Vec::new();
            for _ in 1..nr {
                holes.push(read_ring(r, le, dims)?);
            }
            Some(vec![WkbGeometry {
                points: outer,
                holes,
                is_polygon_source: true,
            }])
        }
        6 => {
            let np = r.u32(le)?;
            let mut out: Vec<WkbGeometry> = Vec::new();
            for _ in 0..np {
                for mut g in read_any(r)? {
                    g.is_polygon_source = true;
                    out.push(g);
                }
            }
            Some(out)
        }
        _ => Some(Vec::new()),
    }
}

#[derive(Clone, Copy)]
struct Dims {
    z: bool,
    m: bool,
}

fn read_point(r: &mut Reader, le: bool, dims: Dims) -> Option<G3857> {
    let (x, y) = (r.f64(le)?, r.f64(le)?);
    let z = if dims.z { r.f64(le)? } else { 0.0 };
    if dims.m {
        r.skip(8)?;
    }
    Some([x, y, z])
}

fn read_ring(r: &mut Reader, le: bool, dims: Dims) -> Option<Vec<G3857>> {
    let n = r.u32(le)? as usize;
    // An untrusted length must not size an allocation: each point is at least 16 bytes.
    if n.saturating_mul(16) > r.b.len() - r.at.min(r.b.len()) {
        return None;
    }
    let mut pts: Vec<G3857> = Vec::with_capacity(n);
    for _ in 0..n {
        pts.push(read_point(r, le, dims)?);
    }
    Some(pts)
}

#[cfg(test)]
#[path = "wkb.test.rs"]
mod test;
