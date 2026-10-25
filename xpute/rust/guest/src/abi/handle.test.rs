// xpute-guest/abi/handle.test.rs

use super::*;

#[test]
fn a_full_table_refuses() {
    let mut t = Table::<(), 3>::new();
    let handles = [t.insert(()).unwrap(), t.insert(()).unwrap()];
    assert_eq!(t.insert(()), None);
    assert_eq!(t.count(), 2);
    t.remove(handles[0]);
    assert!(t.insert(()).is_some());
}

#[test]
fn a_removed_handle_names_nothing_even_once_its_slot_is_taken_again() {
    let mut t = Table::<_, 4>::new();
    let a = t.insert("a").unwrap();
    assert_ne!(a, 0);
    assert_eq!(t.get(a), Some(&"a"));
    assert_eq!(t.remove(a), Some("a"));
    assert_eq!(t.remove(a), None, "a handle is removed once");
    let b = t.insert("b").unwrap();
    assert_eq!(handle_slot(b), handle_slot(a), "the slot is reused");
    assert_eq!((t.get(a), t.get(b)), (None, Some(&"b")));
    assert_eq!(t.iter().collect::<Vec<_>>(), vec![(b, &"b")]);
    assert_eq!(t.get(0), None, "0 is never a handle");
}

#[test]
fn a_rollback_removes_what_its_epoch_took() {
    let mut t = Table::<u8, 8>::new();
    let kept = t.insert(1).unwrap();
    t.begin();
    let made = t.insert(2).unwrap();
    let mut gone = Vec::new();
    t.rollback(|h, v| gone.push((h, v)));
    assert_eq!(gone, vec![(made, 2)]);
    assert!(t.live(kept) && !t.live(made));
}
