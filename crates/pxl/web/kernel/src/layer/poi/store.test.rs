// pxl-kernel/layer/poi/store.test.rs

use super::*;
use pxl_spec::poi::xy;

fn summary(key: u64, count: u64, refinable: bool) -> Summary {
    let b = xy::bounds(key);
    Summary {
        key,
        count,
        bounds: [b[0], b[1], 0.0, b[2] - 1.0, b[3] - 1.0, 100.0],
        refinable,
    }
}
fn root() -> Node {
    let key = xy::parse("132110320000000").unwrap();
    Node {
        revision: 1,
        summary: summary(key, 400, true),
        children: xy::children(key).iter().map(|&c| summary(c, 100, true)).collect(),
    }
}
fn refinement(parent: Summary, revision: u64) -> Node {
    let children = xy::children(parent.key).iter().map(|&c| summary(c, 25, false)).collect();
    Node { revision, summary: parent, children }
}
fn eye() -> [f64; 3] {
    let b = root().summary.bounds;
    [(b[0] + b[3]) * 0.5, (b[1] + b[4]) * 0.5, 50.0]
}
fn count(s: &Store) -> u64 {
    s.cut.iter().map(|s| s.count).sum()
}
fn loaded() -> Store {
    let mut s = Store::default();
    s.select(eye(), 8, RESIDENT_BYTES);
    s.adopt(0, root(), RESIDENT_BYTES).unwrap();
    s
}

#[test]
fn delayed_children_keep_parent_and_refinement_never_changes_membership() {
    let mut s = loaded();
    s.select(eye(), 1, RESIDENT_BYTES);
    assert_eq!(s.cut, vec![root().summary]);
    s.select(eye(), 8, RESIDENT_BYTES);
    assert_eq!(count(&s), 400);
    assert_eq!(s.cut.len(), 4);
    let parent = s.cut[0];
    assert!(s.wanted().contains(&parent.key));
    // A failed/missing response makes no adoption; repeated selection must
    // keep the same population and continue to offer its parent.
    s.select(eye(), 8, RESIDENT_BYTES);
    assert!(s.cut.contains(&parent));
    s.adopt(parent.key, refinement(parent, 1), RESIDENT_BYTES).unwrap();
    s.select(eye(), 8, RESIDENT_BYTES);
    assert_eq!(count(&s), 400);
    assert_eq!(s.cut.len(), 7);
    assert!(!s.cut.contains(&parent));
    assert!(s.bytes() <= RESIDENT_BYTES);
    s.select(eye(), 1, RESIDENT_BYTES);
    assert_eq!(s.cut, vec![root().summary]);
}

#[test]
fn stale_or_inconsistent_children_and_memory_pressure_preserve_parent() {
    let mut s = loaded();
    s.select(eye(), 8, RESIDENT_BYTES);
    let parent = s.cut[0];
    assert!(s.adopt(parent.key, refinement(parent, 2), RESIDENT_BYTES).is_err());
    let mut bad = refinement(parent, 1);
    bad.children[0].count += 1;
    assert!(s.adopt(parent.key, bad, RESIDENT_BYTES).is_err());
    assert!(s.adopt(parent.key, refinement(parent, 1), s.bytes()).is_err());
    s.select(eye(), 8, s.bytes());
    assert!(s.cut.contains(&parent));
    assert_eq!(count(&s), 400);
    assert!(s.wanted().is_empty());
}

#[test]
fn cancellation_cannot_adopt_an_obsolete_response() {
    let mut s = loaded();
    s.select(eye(), 8, RESIDENT_BYTES);
    let parent = s.cut[0];
    s.select([0.0, 0.0, 0.0], 8, RESIDENT_BYTES);
    assert!(s.wanted().is_empty());
    let before = s.bytes();
    s.adopt(parent.key, refinement(parent, 1), RESIDENT_BYTES).unwrap();
    assert_eq!(s.bytes(), before);
    assert_eq!(count(&s), 400);
}
