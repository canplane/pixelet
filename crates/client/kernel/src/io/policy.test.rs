// pxl-kernel/io/policy.test.rs

use pxl_spec::space::layer::lattice::GROUND;
use super::*;
use crate::io::demand::Why;

fn at(why: Why, seen: bool, near: f64) -> Ask {
    Ask { why, order: Order { seen, near } }
}

fn key(category: Category, ask: Ask) -> (u8, f64) {
    let r = Policy::standard().rank(category, 0, ask, 0.0);
    (r.class, r.distance)
}

fn before(a: (u8, f64), b: (u8, f64)) -> bool {
    a.0 < b.0 || (a.0 == b.0 && a.1 < b.1)
}

#[test]
fn the_ground_goes_before_what_stands_on_it() {
    let tile = at(Why::Demand, true, 0.0);
    let terrain = key(Category::terrain, tile);
    let asset = key(Category::asset, Ask::PLAIN);
    let places = key(Category::poi, tile);
    assert!(before(terrain, asset) && before(asset, places));
}

#[test]
fn what_the_view_needs_is_asked_for_out_from_the_eye() {
    let t = |why, seen, near| key(Category::terrain, at(why, seen, near));
    assert!(before(t(Why::Demand, true, 10.0), t(Why::Demand, true, 20.0)), "nearer first");
    assert!(before(t(Why::Demand, true, 900.0), t(Why::Demand, false, 1.0)), "seen before unseen");
    assert_eq!(t(Why::Floor, true, 10.0), t(Why::Demand, true, 10.0), "the fallback's tiles at their own place");
}

#[test]
fn only_a_failure_waits() {
    let p = Policy::standard();
    assert_eq!(p.retry_after(Outcome::error), Some(param::RETRY_MS));
    assert_eq!(p.retry_after(Outcome::ok), None);
    assert_eq!(p.retry_after(Outcome::aborted), None);
}

#[test]
fn of_tiles_as_near_the_finer_is_asked_first() {
    use pxl_spec::space::layer::tile::LEAF_ZOOM;
    let p = Policy::standard();
    let eye = at(Why::Floor, true, 0.0);
    let rank = |zoom| p.rank(Category::terrain, Tile { zoom, tile: 0 }.subject(), eye, 0.0);
    let (leaf, top) = (rank(LEAF_ZOOM), rank(GROUND.top()));
    assert_eq!((leaf.class, leaf.distance), (top.class, top.distance));
    assert!(leaf.detail > top.detail);
}
