// pxl-kernel/world/pyramid/domain.test.rs

use super::*;
use pxl_spec::space::pyramid::chunk_span_of;

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
/// its own level's split distance under the view's criterion and no farther
/// than its parent's plus the parent's diagonal, so a cell drawn covers
/// between two fixed shares of the criterion's pixels wherever the eye is;
/// a level draws no more chunks than LEVEL_CHUNKS; and what the view wants
/// fits what the device holds. The tilt is not an input at all.
#[test]
fn every_drawn_chunk_lies_between_its_split_distances_and_the_view_fits_its_budget() {
    // The domain reads the terrain's and the objects' heights, which are
    // globals.
    let _serial = crate::global::serial();
    for height in [1.0, 50.0, 400.0, 1000.0, 5000.0, 40_000.0, 250_000.0] {
        for (x, y) in [(0.3, 0.2), (511.9, 0.1), (123_456.7, -98_765.4), (-7_000.5, 3_000_000.25)] {
            let eye = Eye {
                at: [x, y, height],
                foot: ivec::gather(f64::floor(x) as i64, f64::floor(y) as i64, 0),
                centre: [x, y, 0.0],
            };
            let d = domain_for(&eye, height, None, 0);
            let s = d.scale;
            assert!(s > 0.0 && s <= 1.0);
            let chunks = drawn_by_rule(&d);
            for &c in &chunks {
                let level = c.level();
                let at = format!("height {height}, eye ({x}, {y}), {c:?}");
                if level > 0 {
                    assert!(d.nearest_to(c) >= split_distance(level as i32) * s, "{at}: nearer than its split distance");
                }
                if level < TOP_LEVEL {
                    let parent = split_distance(level as i32 + 1) * s + chunk_span_of(level + 1) * SQRT_2;
                    assert!(farthest(eye.at, c, 0.0) <= parent, "{at}: farther than its parent could reach");
                }
            }
            for level in 0..=TOP_LEVEL {
                let n = chunks.iter().filter(|c| c.level() == level).count() as u32;
                assert!(n <= LEVEL_CHUNKS, "height {height}: {n} chunks at level {level}");
            }
            assert!(chunks.len() as u32 <= VIEW_CHUNKS, "height {height}: {} chunks", chunks.len());
            assert!(d.pages().count() as u32 <= VIEW_PAGES, "height {height}: {} pages", d.pages().count());
        }
    }
}

/// What a view wants held is every page its drawn chunks lie in, and every
/// ancestor of those up to the pyramid's top, so ground whose page has not
/// come has something coarser held over it.
#[test]
fn every_page_drawn_from_is_wanted_with_every_ancestor_up_to_the_top() {
    let _serial = crate::global::serial();
    for height in [1.0, 400.0, 5000.0, 250_000.0] {
        for (x, y) in [(0.3, 0.2), (511.9, 0.1), (123_456.7, -98_765.4)] {
            let eye = Eye {
                at: [x, y, height],
                foot: ivec::gather(f64::floor(x) as i64, f64::floor(y) as i64, 0),
                centre: [x, y, 0.0],
            };
            let d = domain_for(&eye, height, None, 0);
            for c in drawn_by_rule(&d) {
                let order = d.order_of(c);
                assert!(
                    matches!(d.pages.get(&c.page()), Some(Some(o)) if *o <= order),
                    "height {height}: {c:?} is drawn from a page not wanted, or one ordered after it"
                );
            }
            for (p, _) in d.pages() {
                if p.level < TOP_LEVEL {
                    assert!(d.wants(p.parent()), "height {height}: {p:?} is wanted and the page over it is not");
                }
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
    let eye = Eye {
        at: [x, y, height],
        foot: ivec::gather(f64::floor(x) as i64, f64::floor(y) as i64, 0),
        centre: [x, y, 0.0],
    };
    // One plane, x ≥ the eye's, and five that pass everything.
    let mut planes = [0.0; 24];
    planes[..4].copy_from_slice(&[1.0, 0.0, 0.0, -x]);
    for k in 1..6 {
        planes[k * 4 + 3] = 1.0;
    }
    let east = domain_for(&eye, height, Some(Frustum { planes, planet: 1e12 }), 0);
    let fine: Vec<Chunk> = drawn_by_rule(&east).into_iter().filter(|c| c.level() < TOP_LEVEL).collect();
    assert!(!fine.is_empty(), "the view refines what it sees");
    for c in fine {
        let (x0, _, side) = c.parent().expect("a chunk under the top has a parent").rect();
        assert!(x0 + side / 2.0 + side * SQRT_2 / 2.0 >= x, "{c:?} was refined from a parent the view does not see");
    }
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
        let eye = [3_000_000.0 + v.eye_local[0], 2_000_000.0 + v.eye_local[1], v.eye_local[2]];
        assert!(f.sees(centre, 1.0, eye), "dist {dist}, polar {polar}, azimuth {azimuth}");
        // And not what lies straight behind the eye.
        let behind = [2.0 * eye[0] - centre[0], 2.0 * eye[1] - centre[1], centre[2]];
        assert!(!f.sees(behind, 1.0, eye), "behind, dist {dist}, polar {polar}, azimuth {azimuth}");
    }
}
