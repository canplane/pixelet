// xpute-guest/ipc/file.test.rs

use super::*;

fn sys() -> Sys {
    let range = Box::leak(vec![0u32; 256].into_boxed_slice());
    // SAFETY: leaked, so the range outlives the calls.
    unsafe { Sys::new(crate::mem::Memory::new(range.as_mut_ptr().expose_provenance(), 256 * 4), 0, 256) }
}

#[test]
fn file_a_descriptor_is_found_by_its_owner_once_its_slot_is_taken_again() {
    let (mut sys, mut files) = (sys(), Files::<_, 8>::new());
    let a = files.open(&mut sys, 7, "a", "first");
    files.close(&mut sys, a);
    let b = files.open(&mut sys, 7, "b", "second");
    assert_eq!(files.held_for(|o| *o == "second"), Some(b));
}
