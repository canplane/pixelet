// pxl-kernel/io/policy.test.rs
//
// The kernel's order, as the module's comment argues it.

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
fn the_ocean_goes_before_any_page_and_the_ground_before_what_stands_on_it() {
    let page = at(Why::Fault, true, 0.0);
    let ocean = key(Category::ocean, Ask::PLAIN);
    let terrain = key(Category::terrain, page);
    let asset = key(Category::asset, Ask::PLAIN);
    let places = key(Category::poi, page);
    assert!(before(ocean, terrain) && before(terrain, asset) && before(asset, places));
}

/// What the view needs goes out from the eye, what it sees first; every
/// reason it needs a page for is one order; speculation follows.
#[test]
fn what_the_view_needs_is_asked_for_out_from_the_eye_and_speculation_after() {
    let t = |why, seen, near| key(Category::terrain, at(why, seen, near));
    assert!(before(t(Why::Fault, true, 10.0), t(Why::Fault, true, 20.0)), "nearer first");
    assert!(before(t(Why::Fault, true, 900.0), t(Why::Fault, false, 1.0)), "seen before unseen");
    assert_eq!(t(Why::Floor, true, 10.0), t(Why::Fault, true, 10.0), "the fallback's pages at their own place");
    assert!(before(t(Why::Fault, false, 1e9), t(Why::Hint, true, 0.0)), "speculation after everything needed");
}

/// A lease no view reads has no place in the view's order and waits behind
/// all of it; one the view reads is the view's to place.
#[test]
fn a_lease_the_view_does_not_read_is_aside() {
    let aside = key(Category::terrain, at(Why::Lease, false, f64::INFINITY));
    assert!(before(key(Category::poi, at(Why::Hint, false, 1e9)), aside));
    assert_eq!(aside, key(Category::geocode, Ask::PLAIN), "a tie with everything else unargued");
    let read = Ask::best(at(Why::Lease, false, f64::INFINITY), at(Why::Fault, true, 5.0));
    assert_eq!(read.why, Why::Lease, "held as a lease");
    assert_eq!(key(Category::terrain, read), key(Category::terrain, at(Why::Fault, true, 5.0)), "placed as the view reads it");
}

#[test]
fn only_a_whole_class_better_takes_a_running_credit() {
    let p = Policy::standard();
    let r = |why, seen, near| p.rank(Category::terrain, 0, at(why, seen, near), 0.0);
    assert!(!p.preempts(r(Why::Fault, true, 0.0), r(Why::Fault, true, 5.0)));
    assert!(p.preempts(r(Why::Fault, true, 9.0), r(Why::Fault, false, 0.0)));
    assert!(p.preempts(r(Why::Fault, false, 9.0), r(Why::Hint, true, 0.0)));
}

#[test]
fn only_a_failure_waits() {
    let p = Policy::standard();
    assert_eq!(p.retry_after(Outcome::error), Some(param::RETRY_MS));
    assert_eq!(p.retry_after(Outcome::ok), None);
    assert_eq!(p.retry_after(Outcome::aborted), None);
}
