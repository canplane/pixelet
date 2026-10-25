// pxl-geo-bake/wkb.test.rs

use super::*;

struct Wkb(Vec<u8>);

impl Wkb {
    fn new(kind: u32) -> Wkb {
        let mut b = vec![1u8];
        b.extend_from_slice(&kind.to_le_bytes());
        Wkb(b)
    }

    fn u32(mut self, v: u32) -> Wkb {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }

    fn ring(mut self, pts: &[(f64, f64)]) -> Wkb {
        self.0.extend_from_slice(&(pts.len() as u32).to_le_bytes());
        for (x, y) in pts {
            self.0.extend_from_slice(&x.to_le_bytes());
            self.0.extend_from_slice(&y.to_le_bytes());
        }
        self
    }

    fn raw(mut self, other: Wkb) -> Wkb {
        self.0.extend_from_slice(&other.0);
        self
    }
}

impl Wkb {
    fn point(mut self, x: f64, y: f64) -> Wkb {
        self.0.extend_from_slice(&x.to_le_bytes());
        self.0.extend_from_slice(&y.to_le_bytes());
        self
    }
}

#[test]
fn a_point_is_one_geometry_of_one_point_and_not_a_polygon() {
    let g = wkb_to_geometries(&Wkb::new(1).point(1.5, -2.5).0);
    assert_eq!(g.len(), 1);
    assert_eq!(g[0].points, vec![[1.5, -2.5, 0.0]]);
    assert!(!g[0].is_polygon_source);
    assert!(g[0].holes.is_empty());
}

#[test]
fn a_linestring_keeps_its_order_and_stays_open() {
    let bytes = Wkb::new(2).ring(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]).0;
    let g = wkb_to_geometries(&bytes);
    assert_eq!(g.len(), 1);
    assert_eq!(
        g[0].points,
        vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]]
    );
    assert!(!g[0].is_polygon_source);
}

#[test]
fn a_polygons_first_ring_is_the_outer_and_the_rest_are_holes() {
    let bytes = Wkb::new(3)
        .u32(2)
        .ring(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 0.0)])
        .ring(&[(2.0, 2.0), (4.0, 2.0), (4.0, 4.0), (2.0, 2.0)])
        .0;
    let g = wkb_to_geometries(&bytes);
    assert_eq!(g.len(), 1);
    assert!(g[0].is_polygon_source);
    assert_eq!(g[0].points.len(), 4);
    assert_eq!(g[0].holes.len(), 1);
    assert_eq!(g[0].holes[0][0], [2.0, 2.0, 0.0]);
}

/// `is_polygon_source` is what closes a feature, so a ringless row must not
/// read as a line.
#[test]
fn a_polygon_with_no_rings_is_still_a_polygon() {
    let bytes = Wkb::new(3).u32(0).0;
    let g = wkb_to_geometries(&bytes);
    assert_eq!(g.len(), 1);
    assert!(g[0].is_polygon_source);
    assert!(g[0].points.is_empty());
}

#[test]
fn a_multipolygon_is_flattened_to_one_polygon_each() {
    let one = Wkb::new(3)
        .u32(1)
        .ring(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 0.0)]);
    let two = Wkb::new(3)
        .u32(1)
        .ring(&[(5.0, 5.0), (6.0, 5.0), (6.0, 6.0), (5.0, 5.0)]);
    let bytes = Wkb::new(6).u32(2).raw(one).raw(two).0;
    let g = wkb_to_geometries(&bytes);
    assert_eq!(g.len(), 2);
    assert!(g.iter().all(|x| x.is_polygon_source));
    assert_eq!(g[0].points[0], [0.0, 0.0, 0.0]);
    assert_eq!(g[1].points[0], [5.0, 5.0, 0.0]);
}

#[test]
fn ewkb_srid_is_skipped_and_z_kept() {
    let mut bytes = vec![1u8];
    bytes.extend_from_slice(&(1u32 | 0x2000_0000 | 0x8000_0000).to_le_bytes());
    bytes.extend_from_slice(&3857u32.to_le_bytes());
    bytes.extend_from_slice(&1.0f64.to_le_bytes());
    bytes.extend_from_slice(&2.0f64.to_le_bytes());
    bytes.extend_from_slice(&99.0f64.to_le_bytes());
    let g = wkb_to_geometries(&bytes);
    assert_eq!(g[0].points, vec![[1.0, 2.0, 99.0]]);
}

#[test]
fn iso_z_and_m_are_read() {
    let mut bytes = vec![1u8];
    bytes.extend_from_slice(&3002u32.to_le_bytes());
    bytes.extend_from_slice(&2u32.to_le_bytes());
    for v in [1.0f64, 2.0, 3.0, 7.0, 4.0, 5.0, 6.0, 7.0] {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    let g = wkb_to_geometries(&bytes);
    assert_eq!(g[0].points, vec![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]);
}

#[test]
fn an_unsupported_type_and_an_empty_buffer_are_both_no_geometry() {
    assert!(wkb_to_geometries(&[]).is_empty());
    assert!(wkb_to_geometries(&Wkb::new(7).u32(0).0).is_empty());
}

#[test]
fn a_truncated_ring_is_refused_rather_than_read() {
    let mut bytes = Wkb::new(2).ring(&[(0.0, 0.0), (1.0, 1.0)]).0;
    bytes.truncate(bytes.len() - 4);
    assert!(wkb_to_geometries(&bytes).is_empty());

    let mut lying = vec![1u8];
    lying.extend_from_slice(&2u32.to_le_bytes());
    lying.extend_from_slice(&u32::MAX.to_le_bytes());
    assert!(wkb_to_geometries(&lying).is_empty());
}
