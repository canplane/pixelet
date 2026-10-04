// pxl-spec/space/lattice/spatial_hash.test.rs

use std::collections::{HashMap, HashSet};

use super::super::hash::{hash3, hash4};
use super::*;

#[derive(Clone, Copy)]
struct Point {
    id: u32,
    x: f64,
    y: f64,
    z: f64,
}

/// Cell size 1; 1021 is prime and far above the point counts.
fn index(cell_size: f64, capacity: u32) -> SpatialHash {
    SpatialHash::new(capacity, 1021, cell_size)
}

fn fill(h: &mut SpatialHash, points: &[Point]) {
    h.begin();
    for p in points {
        h.insert(p.id, p.x, p.y, p.z);
    }
    h.build();
}

/// Sorted: order is by bucket and not promised.
fn found(h: &SpatialHash, x: f64, y: f64, z: f64, radius: f64) -> Vec<u32> {
    let mut out = vec![0u32; 4096];
    let n = h.query(x, y, z, radius, &mut out) as usize;
    let mut ids = out[..n].to_vec();
    ids.sort();
    ids
}

fn within(points: &[Point], x: f64, y: f64, z: f64, radius: f64) -> Vec<u32> {
    let mut ids: Vec<u32> = points
        .iter()
        .filter(|p| (p.x - x).abs() <= radius && (p.y - y).abs() <= radius && (p.z - z).abs() <= radius)
        .map(|p| p.id)
        .collect();
    ids.sort();
    ids
}

fn scatter(n: u32, span: f64) -> Vec<Point> {
    let mut out = Vec::new();
    let mut s: i32 = 0x2545f491;
    let mut next = || {
        s ^= s.wrapping_shl(13);
        s ^= ((s as u32) >> 17) as i32;
        s ^= s.wrapping_shl(5);
        ((s as u32) as f64 / 4_294_967_296.0) * span - span / 2.0
    };
    for i in 0..n {
        let (x, y, z) = (next(), next(), next());
        out.push(Point { id: i, x, y, z });
    }
    out
}

/// No false negatives.
#[test]
fn spatial_hash_a_query_never_misses_a_point_inside_its_box() {
    let points = scatter(2000, 200.0);
    let mut h = index(1.0, 4096);
    fill(&mut h, &points);

    for t in 0..200 {
        let probe = points[(t * 97) % points.len()];
        for radius in [0.5, 1.0, 3.5] {
            let expected = within(&points, probe.x, probe.y, probe.z, radius);
            let reported: HashSet<u32> = found(&h, probe.x, probe.y, probe.z, radius).into_iter().collect();
            for id in expected {
                assert!(reported.contains(&id), "missed {id} at r={radius}");
            }
        }
    }
}

/// Nothing outside the cells the box touches comes back, so a hash collision is never a hit.
#[test]
fn spatial_hash_every_reported_point_lies_within_one_cell_of_the_box() {
    let points = scatter(2000, 200.0);
    let by_id: HashMap<u32, Point> = points.iter().map(|p| (p.id, *p)).collect();
    let cell = 2.0;
    let mut h = index(cell, 4096);
    fill(&mut h, &points);

    for t in 0..200 {
        let probe = points[(t * 53) % points.len()];
        let radius = 3.0;
        for id in found(&h, probe.x, probe.y, probe.z, radius) {
            let p = by_id[&id];
            let slack = radius + cell;
            assert!(
                (p.x - probe.x).abs() <= slack && (p.y - probe.y).abs() <= slack && (p.z - probe.z).abs() <= slack,
                "{id} is further than one cell outside the box"
            );
        }
    }
}

/// A point on a cell boundary is in one cell; a query touching it visits both sides.
#[test]
fn spatial_hash_points_on_a_cell_boundary_are_found_from_either_side() {
    let points = [
        Point { id: 1, x: 0.0, y: 0.0, z: 0.0 },
        Point { id: 2, x: -0.000001, y: 0.0, z: 0.0 },
        Point { id: 3, x: 0.999999, y: 0.0, z: 0.0 },
        Point { id: 4, x: 1.0, y: 0.0, z: 0.0 },
    ];
    let mut h = index(1.0, 4096);
    fill(&mut h, &points);

    assert!(found(&h, 0.0, 0.0, 0.0, 0.000002).contains(&2));
    assert!(found(&h, 0.9999995, 0.0, 0.0, 0.0000015).contains(&4));
    for p in &points {
        assert!(found(&h, p.x, p.y, p.z, 0.0).contains(&p.id), "{} missing from its own cell", p.id);
    }
}

/// A stale bucket must not leak into a later generation.
#[test]
fn spatial_hash_a_thousand_rebuilds_leave_no_trace_of_the_ones_before() {
    let mut h = index(1.0, 4096);
    let points = scatter(500, 100.0);

    for step in 0..1000 {
        let moved: Vec<Point> = points
            .iter()
            .map(|p| Point {
                x: p.x + step as f64 * 0.37,
                y: p.y - step as f64 * 0.11,
                ..*p
            })
            .collect();
        fill(&mut h, &moved);
        let last = found(&h, moved[0].x, moved[0].y, moved[0].z, 2.0);
        for id in within(&moved, moved[0].x, moved[0].y, moved[0].z, 2.0) {
            assert!(last.contains(&id), "step {step} missed {id}");
        }
    }
    assert_eq!(h.stats().dropped, 0);
}

/// A rebuild allocates nothing: the owned buffers keep their identity.
#[test]
fn spatial_hash_a_rebuild_allocates_nothing_it_does_not_already_own() {
    let mut h = index(1.0, 4096);
    let points = scatter(1000, 100.0);

    let owned = |h: &SpatialHash| {
        [&h.counts, &h.starts, &h.stamps, &h.cursors, &h.touched, &h.buckets, &h.ids, &h.items]
            .map(|v| v.as_ptr() as usize)
            .into_iter()
            .chain([&h.ins_cx, &h.ins_cy, &h.ins_cz, &h.item_cx, &h.item_cy, &h.item_cz].map(|v| v.as_ptr() as usize))
            .collect::<Vec<usize>>()
    };
    fill(&mut h, &points);
    let before = owned(&h);

    for step in 0..50 {
        let moved: Vec<Point> = points.iter().map(|p| Point { x: p.x + step as f64, ..*p }).collect();
        fill(&mut h, &moved);
    }
    let after = owned(&h);

    assert_eq!(after.len(), before.len());
    for i in 0..before.len() {
        assert!(after[i] == before[i], "array {i} was replaced");
    }
}

/// Past capacity, insertions are dropped and counted.
#[test]
fn spatial_hash_capacity_is_refused_not_grown() {
    let mut h = index(1.0, 10);
    h.begin();
    for i in 0..25 {
        assert_eq!(h.insert(i, i as f64, 0.0, 0.0), i < 10);
    }
    h.build();

    let s = h.stats();
    assert_eq!(s.count, 10);
    assert_eq!(s.dropped, 15);
    for i in 0..10 {
        assert!(found(&h, i as f64, 0.0, 0.0, 0.0).contains(&i));
    }
}

/// Ten thousand points in a couple of cells land in the few buckets their positions imply.
#[test]
fn spatial_hash_ten_thousand_points_in_one_small_volume_still_resolve() {
    let mut h = index(1.0, 16384);
    let mut points = Vec::new();
    let mut s: i32 = 0x9e3779b9u32 as i32;
    let mut next = || {
        s ^= s.wrapping_shl(13);
        s ^= ((s as u32) >> 17) as i32;
        s ^= s.wrapping_shl(5);
        ((s as u32) as f64 / 4_294_967_296.0) * 2.0
    };
    for i in 0..10000 {
        let (x, y, z) = (next(), next(), next());
        points.push(Point { id: i, x, y, z });
    }
    fill(&mut h, &points);

    assert_eq!(h.stats().count, 10000);
    assert!(h.stats().touched <= 27, "expected a handful of buckets, got {}", h.stats().touched);

    let mut out = vec![0u32; 16384];
    let n = h.query(1.0, 1.0, 1.0, 1.0, &mut out) as usize;
    assert_eq!(out[..n].iter().copied().collect::<HashSet<u32>>().len(), 10000);
}

/// With far more cells than buckets, a query still returns only its own cells.
#[test]
fn spatial_hash_a_saturated_table_returns_no_id_twice_and_none_from_a_colliding_cell() {
    let mut h = SpatialHash::new(4096, 61, 1.0);
    let points = scatter(1500, 100.0);
    let by_id: HashMap<u32, Point> = points.iter().map(|p| (p.id, *p)).collect();
    fill(&mut h, &points);

    let mut out = vec![0u32; 4096];
    for t in 0..100 {
        let probe = points[(t * 31) % points.len()];
        let n = h.query(probe.x, probe.y, probe.z, 1.0, &mut out) as usize;
        let ids = out[..n].to_vec();
        assert_eq!(ids.iter().copied().collect::<HashSet<u32>>().len(), ids.len(), "an id was reported twice");
        for &id in &ids {
            let p = by_id[&id];
            assert!(
                (p.x - probe.x).abs() <= 2.0 && (p.y - probe.y).abs() <= 2.0 && (p.z - probe.z).abs() <= 2.0,
                "{id} came from a colliding cell"
            );
        }
        for id in within(&points, probe.x, probe.y, probe.z, 1.0) {
            assert!(ids.contains(&id), "missed {id}");
        }
    }
}

#[test]
fn lattice_hash_a_tier_folded_in_separates_a_parent_s_address_from_its_child_s() {
    assert_eq!(hash3(3, -4, 5), hash4(3, -4, 5, 0));
    assert!(hash4(1, 1, 1, 0) != hash4(1, 1, 1, 1));
}
