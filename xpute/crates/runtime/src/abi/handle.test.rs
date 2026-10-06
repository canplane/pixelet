// xpute-runtime/abi/handle.test.rs

use super::*;

#[test]
fn handle_a_released_handle_no_longer_names_its_slot_and_the_slot_s_next_handle_differs() {
    let mut t = HandleTable::<4>::new();
    let a = t.acquire().unwrap();
    assert!(t.live(a));
    assert!(t.release(a));
    assert!(!t.live(a));
    assert!(!t.release(a), "a handle is released once");
    let b = t.acquire().unwrap();
    assert_eq!(handle_slot(b), handle_slot(a), "the slot is reused");
    assert_ne!(b, a);
    assert!(t.live(b) && !t.live(a));
}

#[test]
fn handle_0_is_never_a_handle_and_a_full_table_refuses() {
    let mut t = HandleTable::<3>::new();
    assert!(!t.live(0));
    let handles = [t.acquire().unwrap(), t.acquire().unwrap()];
    assert!(handles.iter().all(|&h| h != 0 && handle_slot(h) != 0));
    assert_eq!(t.acquire(), None);
    assert_eq!(t.count(), 2);
    t.release(handles[0]);
    assert!(t.acquire().is_some());
}

#[test]
fn slots_a_removed_handle_names_nothing_even_once_its_slot_is_taken_again() {
    let mut s = Slots::new();
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
