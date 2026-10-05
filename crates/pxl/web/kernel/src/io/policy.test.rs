// pxl-kernel/io/policy.test.rs

use super::*;

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
    let page = at(Why::Fault, true, 0.0);
    let terrain = key(Category::terrain, page);
    let asset = key(Category::asset, Ask::PLAIN);
    let places = key(Category::poi, page);
    assert!(before(terrain, asset) && before(asset, places));
}

#[test]
fn what_the_view_needs_is_asked_for_out_from_the_eye_and_speculation_after() {
    let t = |why, seen, near| key(Category::terrain, at(why, seen, near));
    assert!(before(t(Why::Fault, true, 10.0), t(Why::Fault, true, 20.0)), "nearer first");
    assert!(before(t(Why::Fault, true, 900.0), t(Why::Fault, false, 1.0)), "seen before unseen");
    assert_eq!(t(Why::Floor, true, 10.0), t(Why::Fault, true, 10.0), "the fallback's pages at their own place");
    assert!(before(t(Why::Fault, false, 1e9), t(Why::Hint, true, 0.0)), "speculation after everything needed");
}

#[test]
fn only_a_failure_waits() {
    let p = Policy::standard();
    assert_eq!(p.retry_after(Outcome::error), Some(param::RETRY_MS));
    assert_eq!(p.retry_after(Outcome::ok), None);
    assert_eq!(p.retry_after(Outcome::aborted), None);
}
