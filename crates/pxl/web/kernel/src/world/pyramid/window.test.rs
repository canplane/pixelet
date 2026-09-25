// pxl-kernel/world/pyramid/window.test.rs

use pxl_spec::space::indexer::FROM_L3_IDX;

use super::*;
use pxl_spec::space::pyramid::{chunk_span_of, neighbourhood};

fn leaf(x: i64, y: i64) -> IVec {
    ivec::gather(x, y, 0)
}

#[test]
fn a_page_over_a_leaf_is_the_leaf_shifted_right_a_floor_on_negatives() {
    assert_eq!(LevelPage::over(2, leaf(9, 4)), LevelPage::new(2, leaf(2, 1)));
    assert_eq!(LevelPage::over(1, leaf(-1, -3)), LevelPage::new(1, leaf(-1, -2)));
    assert_eq!(LevelPage::over(0, leaf(5, 6)).parent(), LevelPage::new(1, leaf(2, 3)));
    assert_eq!(LevelPage::new(3, leaf(-2, 5)).first_leaf(), leaf(-16, 40));
}

#[test]
fn a_subject_carries_the_level_and_gives_it_back() {
    for p in [LevelPage::new(0, leaf(11569, 3671)), LevelPage::new(7, leaf(-3, 90)), LevelPage::new(10, leaf(11, -1))] {
        assert_eq!(LevelPage::of_subject(p.subject()), p);
    }
    // A leaf's subject is its key: z16's level is the lane left at zero.
    assert_eq!(LevelPage::new(0, leaf(11569, 3671)).subject(), leaf(11569, 3671));
}

#[test]
fn a_ring_is_the_axis_squared_centred_on_the_anchors_page_nearest_first() {
    let anchor = FROM_L3_IDX(leaf(40, 20));
    for level in 0..4 {
        let held = ring(level, anchor, HELD_AXIS);
        assert_eq!(held.len(), (HELD_AXIS * HELD_AXIS) as usize);
        assert_eq!(held[0], LevelPage::over(level, leaf(40, 20)));
        assert!(held.windows(2).all(|w| ring_of(anchor, w[0]) <= ring_of(anchor, w[1])));
        assert!(held.iter().all(|&p| ring_of(anchor, p) <= HELD_AXIS / 2));
    }
}

#[test]
fn a_levels_span_doubles_with_the_level() {
    assert_eq!(span_of(0), span_of(0));
    assert_eq!(span_of(4), 16.0 * span_of(0));
}

#[test]
fn the_neighbourhood_is_the_one_the_bake_folds_from() {
    // The bake folds from the same neighbourhood (pxl-geo-bake pyramid.rs):
    // the same pages in the same places, so a page folded here is the page
    // the server sends.
    let around = neighbourhood(LevelPage::new(1, leaf(5, 7)));
    assert_eq!([around[5], around[6], around[9], around[10]].map(|p| p.page), [leaf(10, 14), leaf(11, 14), leaf(10, 15), leaf(11, 15)]);
    assert_eq!(around[0].page, leaf(9, 13));
    assert_eq!(around[15].page, leaf(12, 16));
    assert!(around.iter().all(|p| p.level == 0));
    // X wraps across the seam: west of the westmost is the eastmost.
    let across = 1i64 << pxl_spec::space::pyramid::PAGE_ZOOM;
    let west = neighbourhood(LevelPage::new(1, leaf(-across / 4, 0)));
    assert_eq!(ivec::X(west[5].page), -across / 2);
    assert_eq!(ivec::X(west[4].page), across / 2 - 1);
}

/// The chunks the rule draws of a domain where everything is resident: the
/// roots, each split while it comes nearer than its level's split distance.
fn drawn_by_rule(d: &Domain) -> Vec<Chunk> {
    let mut out = Vec::new();
    let mut open = d.roots().to_vec();
    while let Some(c) = open.pop() {
        if d.splits(c) {
            open.extend(c.children());
        } else {
            out.push(c);
        }
    }
    out
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
/// parent's diagonal, so a cell drawn covers between two fixed shares of a
/// cell's pixels wherever the eye is; and a level draws no more chunks than
/// LEVEL_CHUNKS, all inside the ring its records are held for. The tilt is
/// not an input at all.
#[test]
fn every_drawn_chunk_lies_between_its_split_distances_wherever_the_eye_stands() {
    // The domain reads the terrain's and the objects' heights, which are
    // globals.
    let _serial = crate::global::serial();
    for height in [1.0, 50.0, 400.0, 1000.0, 5000.0, 40_000.0, 250_000.0] {
        for (x, y) in [(0.3, 0.2), (511.9, 0.1), (123_456.7, -98_765.4), (-7_000.5, 3_000_000.25)] {
            let eye = Eye {
                at: [x, y, height],
                foot: ivec::gather(f64::floor(x) as i64, f64::floor(y) as i64, 0),
            };
            let d = domain_for(&eye, height, None);
            let w = d.window();
            assert!(w.end - w.start <= WINDOW_LEVELS);
            let top = w.end - 1;
            let chunks = drawn_by_rule(&d);
            for &c in &chunks {
                let level = c.level();
                let at = format!("height {height}, eye ({x}, {y}), {c:?}");
                if level > w.start {
                    assert!(d.nearest_to(c) >= split_distance(level as i32), "{at}: nearer than its split distance");
                }
                if level < top {
                    let parent = split_distance(level as i32 + 1) + chunk_span_of(level + 1) * SQRT_2;
                    assert!(farthest(eye.at, c, 0.0) <= parent, "{at}: farther than its parent could reach");
                }
                assert!(ring_of(eye.foot, c.page) <= HELD_AXIS / 2, "{at}: outside the ring held for it");
            }
            for level in w.clone() {
                let n = chunks.iter().filter(|c| c.level() == level).count() as u32;
                assert!(n <= LEVEL_CHUNKS, "height {height}: {n} chunks at level {level}");
            }
        }
    }
}

/// The chain over the window's top is every root's page's ancestors, at most
/// 2 × 2 pages a level, so CHAIN_PAGES holds it wherever the eye stands.
#[test]
fn the_chain_over_the_window_holds_every_root_s_ancestors_in_two_by_two_a_level() {
    let _serial = crate::global::serial();
    for height in [1.0, 400.0, 5000.0, 250_000.0] {
        for (x, y) in [(0.3, 0.2), (511.9, 0.1), (123_456.7, -98_765.4)] {
            let eye = Eye {
                at: [x, y, height],
                foot: ivec::gather(f64::floor(x) as i64, f64::floor(y) as i64, 0),
            };
            let d = domain_for(&eye, height, None);
            let chain = d.chain();
            assert!(chain.len() as u32 <= CHAIN_PAGES);
            for level in d.window().end..=TOP_LEVEL {
                let n = chain.iter().filter(|p| p.level == level).count();
                assert!((1..=4).contains(&n), "height {height}: {n} pages at level {level}");
            }
            for &r in d.roots() {
                let mut p = r.page;
                while p.level < TOP_LEVEL {
                    p = p.parent();
                    assert!(chain.contains(&p), "height {height}: {p:?} over a root is not held");
                }
            }
        }
    }
}

/// Ground the view does not see is not refined: with a view looking east,
/// every chunk drawn under the top has a parent the view saw — its sphere
/// reaches east of the eye — and the view refines less than one that sees
/// all around.
#[test]
fn ground_outside_the_frustum_is_not_refined() {
    let _serial = crate::global::serial();
    let (height, x, y) = (400.0, 123_456.7, -98_765.4);
    let eye = Eye {
        at: [x, y, height],
        foot: ivec::gather(f64::floor(x) as i64, f64::floor(y) as i64, 0),
    };
    // One plane, x ≥ the eye's, and five that pass everything.
    let mut planes = [0.0; 24];
    planes[..4].copy_from_slice(&[1.0, 0.0, 0.0, -x]);
    for k in 1..6 {
        planes[k * 4 + 3] = 1.0;
    }
    let east = domain_for(&eye, height, Some(Frustum { planes, planet: 1e12 }));
    let around = domain_for(&eye, height, None);
    let top = east.window().end - 1;
    let fine = |d: &Domain| drawn_by_rule(d).into_iter().filter(|c| c.level() < top).collect::<Vec<_>>();
    for c in fine(&east) {
        let (x0, _, side) = c.parent().expect("a chunk under the top has a parent").rect();
        assert!(x0 + side / 2.0 + side * SQRT_2 / 2.0 >= x, "{c:?} was refined from a parent the view does not see");
    }
    assert!(fine(&east).len() < fine(&around).len());
}

/// The frustum the view draws, carried into world units, sees what the view
/// looks at: the map centre it orbits, from every tilt and turn.
#[test]
fn the_view_s_frustum_sees_the_ground_it_looks_at() {
    let _serial = crate::global::serial();
    let v = crate::world::frame::view::view();
    v.origin = ivec::gather(3_000_000, 2_000_000, 0);
    (v.tx, v.ty, v.tz) = (-500.0, -300.0, -10.0);
    (v.width, v.height) = (1600.0, 900.0);
    for (dist, polar, azimuth) in [(50.0, 0.3, 0.0), (5000.0, 1.3, 2.0), (200.0, 0.8, -1.0)] {
        (v.dist, v.polar, v.azimuth) = (dist, polar, azimuth);
        crate::world::frame::view::view_compose();
        let f = Frustum::of_view();
        let centre = [3_000_000.0 + 500.0, 2_000_000.0 + 300.0, 10.0];
        let v = crate::world::frame::view::view();
        let eye = [3_000_000.0 + v.eye_local.x, 2_000_000.0 + v.eye_local.y, v.eye_local.z];
        assert!(f.sees(centre, 1.0, eye), "dist {dist}, polar {polar}, azimuth {azimuth}");
        // And not what lies straight behind the eye.
        let behind = [2.0 * eye[0] - centre[0], 2.0 * eye[1] - centre[1], centre[2]];
        assert!(!f.sees(behind, 1.0, eye), "behind, dist {dist}, polar {polar}, azimuth {azimuth}");
    }
}
