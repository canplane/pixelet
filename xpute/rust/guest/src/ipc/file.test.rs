// xpute-guest/ipc/file.test.rs

use super::*;
use crate::abi::handle::handle_slot;

fn sys() -> Sys {
    Sys::new(Box::leak(vec![0u32; 256].into_boxed_slice()))
}

#[test]
fn file_an_answer_to_a_closed_descriptor_reaches_no_one_even_once_its_slot_is_taken_again() {
    let (mut sys, mut files) = (sys(), Files::new());
    let a = files.open(&mut sys, 7, "a", "first");
    assert_eq!(files.owner(a), Some(&"first"));
    assert_eq!(files.close(&mut sys, a), Some("first"));
    assert_eq!(files.owner(a), None, "a late answer finds no owner");
    let b = files.open(&mut sys, 7, "b", "second");
    assert_eq!(handle_slot(b), handle_slot(a), "the slot is taken again");
    assert_eq!((files.owner(a), files.owner(b)), (None, Some(&"second")));
    assert_eq!(files.held_for(|o| *o == "second"), Some(b));
}
