// pxl-kernel/world/pyramid/domain.test.rs

use std::f64::consts::SQRT_2;

use super::*;
use crate::layer::environment::fog::reach_at;
use pxl_spec::space::lod::fault_radius;
use pxl_spec::space::observation::Frustum;

/// An eye `height` over flat ground at (`x`, `y`), looking through `frustum`
/// or all around.
fn domain_at([x, y]: [f64; 2], height: f64, frustum: Option<Frustum>) -> Domain {
    Domain::of(Observation {
        eye: [x, y, height],
        frustum,
        far: reach_at(height).min(fault_radius(TOP_LEVEL)),
        planet: f64::INFINITY,
        ground: 0.0,
    })
}

/// The farthest point of `c`'s ground from `eye`, the ground flat at `z`.
fn farthest(eye: [f64; 3], c: Chunk, z: f64) -> f64 {
    let (x0, y0, side) = c.rect();
    let far = |e: f64, lo: f64| (e - lo).abs().max((lo + side - e).abs());
    far(eye[0], x0).hypot(far(eye[1], y0)).hypot(eye[2] - z)
}

/// The invariant the rule is for, over flat ground and every height the
/// camera can stand at: a chunk drawn under a split parent is no nearer than
/// its own level's split distance and no farther than its parent's plus the
/// parent's diagonal, so a cell drawn covers between two fixed shares of the
/// criterion's pixels wherever the eye is; and a level has no more of them
/// than the rule bounds (`LEVEL_PIECES`). The tilt is not an input at all.
#[test]
fn every_drawn_chunk_lies_between_its_split_distances_and_a_level_within_its_bound() {
    // The domain reads the terrain's and the objects' heights, which are
    // globals.
    let _serial = crate::global::serial();
    for height in [1.0, 50.0, 400.0, 1000.0, 5000.0, 40_000.0, 250_000.0] {
        for (x, y) in [(0.3, 0.2), (511.9, 0.1), (123_456.7, -98_765.4), (-7_000.5, 3_000_000.25)] {
            let d = domain_at([x, y], height, None);
            let eye = [x, y, height];
            let chunks = d.pieces();
            for &c in chunks {
                let level = c.level();
                let at = format!("height {height}, eye ({x}, {y}), {c:?}");
                if level > 0 {
                    assert!(d.nearest_to(c) >= split_distance(level as i32), "{at}: nearer than its split distance");
                }
                if level < TOP_LEVEL {
                    let parent = split_distance(level as i32 + 1) + chunk_span_of(level + 1) * SQRT_2;
                    assert!(farthest(eye, c, 0.0) <= parent, "{at}: farther than its parent could reach");
                }
            }
            for level in 0..=TOP_LEVEL {
                let n = chunks.iter().filter(|c| c.level() == level).count() as u32;
                assert!(n <= LEVEL_PIECES, "height {height}: {n} chunks at level {level}");
            }
        }
    }
}

/// Ground the view does not see is not refined: with a view looking east,
/// every chunk drawn under the top has a parent the view saw — its sphere
/// reaches east of the eye.
#[test]
fn ground_outside_the_frustum_is_not_refined() {
    let _serial = crate::global::serial();
    let (height, x, y) = (400.0, 123_456.7, -98_765.4);
    // One plane, x ≥ the eye's, and five that pass everything.
    let mut planes = [0.0; 24];
    planes[..4].copy_from_slice(&[1.0, 0.0, 0.0, -x]);
    for k in 1..6 {
        planes[k * 4 + 3] = 1.0;
    }
    let east = domain_at([x, y], height, Some(Frustum { planes }));
    let fine: Vec<Chunk> = east.pieces().iter().copied().filter(|c| c.level() < TOP_LEVEL).collect();
    assert!(!fine.is_empty(), "the view refines what it sees");
    for c in fine {
        let (x0, _, side) = c.parent().expect("a chunk under the top has a parent").rect();
        assert!(x0 + side / 2.0 + side * SQRT_2 / 2.0 >= x, "{c:?} was refined from a parent the view does not see");
    }
}

/// A column's objects are held chunk by chunk, where they are: two things
/// stacked far apart in one column make the chunks they stand in and none in
/// the air between, and no chunk the rule draws lies over another.
#[test]
fn things_stacked_apart_hold_their_chunks_and_not_the_air_between() {
    use crate::layer::object::store::place;
    use pxl_spec::cell::cube_types::Block;
    use pxl_spec::cell::object::PlaceableCube;
    let _serial = crate::global::serial();
    // Far from every other test's ground: the store is the kernel's.
    let (x, y, height): (f64, f64, f64) = (-2_000_000.5, 1_500_000.5, 30.0);
    let at = |z: i64| Pvf3 {
        i: ivec::gather(x.floor() as i64, y.floor() as i64, z),
        f: 0,
    };
    let (low, high) = (10, 10 + 64 * 6);
    for z in [low, high] {
        place(&at(z), 1.0, PlaceableCube::Block(Block::red), false);
    }
    let d = domain_at([x, y], height, None);
    let chunks = d.pieces();
    let set: std::collections::HashSet<Chunk> = chunks.iter().copied().collect();
    assert_eq!(set.len(), chunks.len(), "a chunk drawn twice");
    assert!(chunks.iter().all(|c| !c.ancestors().any(|a| set.contains(&a))), "a chunk drawn under another");
    let holding = |z: i64| {
        chunks
            .iter()
            .any(|&c| c.column() == Chunk::of(c.level(), &at(z)).column() && c.node().1[2] == Chunk::of(c.level(), &at(z)).node().1[2])
    };
    assert!(holding(low) && holding(high), "each thing's chunk is drawn");
    assert!(!holding(10 + 64 * 3), "the air between them is no chunk");
}
