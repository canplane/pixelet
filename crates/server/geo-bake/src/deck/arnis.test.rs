// pxl-geo-bake/deck/arnis.test.rs

use super::*;

#[test]
fn the_hull_clears_every_requirement_and_never_sags() {
    let reqs = vec![(0.0, 0.0), (3.0, 5.0), (5.0, 2.0), (8.0, 6.0), (10.0, 0.0)];
    let hull = upper_hull(reqs.clone());
    for &(u, y) in &reqs {
        assert!(hull_at(&hull, u) >= y - 1e-9);
    }
    // concave: no point of it lies under the chord of its neighbours
    for w in hull.windows(3) {
        let t = (w[1].0 - w[0].0) / (w[2].0 - w[0].0);
        assert!(w[1].1 >= w[0].1 + (w[2].1 - w[0].1) * t - 1e-9);
    }
    // nothing to clear between two ends is the straight line between them
    let straight = upper_hull(vec![(0.0, 2.0), (4.0, 1.0), (10.0, 12.0)]);
    assert_eq!(straight, vec![(0.0, 2.0), (10.0, 12.0)]);
}

