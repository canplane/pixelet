// xpute-guest/abi/handle.test.rs

use super::*;

#[test]
fn handle_a_full_table_refuses() {
    let mut t = HandleTable::<3>::new();
    let handles = [t.acquire().unwrap(), t.acquire().unwrap()];
    assert_eq!(t.acquire(), None);
    assert_eq!(t.count(), 2);
    t.release(handles[0]);
    assert!(t.acquire().is_some());
}

#[test]
fn slots_a_removed_handle_names_nothing_even_once_its_slot_is_taken_again() {
    let mut s = Slots::<_, 4>::new();
    let a = s.insert("a");
    assert_ne!(a, 0);
    assert_eq!(s.get(a), Some(&"a"));
    assert_eq!(s.remove(a), Some("a"));
    assert_eq!(s.remove(a), None, "a handle is removed once");
    let b = s.insert("b");
    assert_eq!(handle_slot(b), handle_slot(a), "the slot is reused");
    assert_eq!((s.get(a), s.get(b)), (None, Some(&"b")));
    assert_eq!(s.iter().collect::<Vec<_>>(), vec![(b, &"b")]);
    assert_eq!(s.get(0), None, "0 is never a handle");
}
