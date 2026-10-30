// xpute-guest/ipc/file.test.rs

use super::*;

fn sys() -> Sys {
    let range = Box::leak(vec![0u32; 256].into_boxed_slice());
    // SAFETY: leaked, so the range outlives the calls.
    unsafe {
        Sys::new(
            crate::mem::Memory::new(range.as_mut_ptr().expose_provenance(), 256 * 4),
            0,
            256,
        )
    }
}

#[test]
fn descriptor_owner() {
    let (mut sys, mut files) = (sys(), Files::<_, 8>::new());
    let a = files.open(&mut sys, 7, "a", "first");
    files.close(&mut sys, a);
    let b = files.open(&mut sys, 7, "b", "second");
    assert_eq!(files.held_for(|o| *o == "second"), Some(b));
}

#[test]
fn failed_open_closed() {
    let (mut sys, mut files) = (sys(), Files::<_, 8>::new());
    let failed = files.open(&mut sys, 7, "a", "first");
    let answer = SysOpened {
        fd: failed,
        res: -2,
        detail: 0,
    };
    assert_eq!(files.opened(&mut sys, &answer), Some("first"));
    assert_eq!(files.owner(failed), None, "a failed open is closed");
    let held = files.open(&mut sys, 7, "b", "second");
    let answer = SysOpened {
        fd: held,
        res: 5,
        detail: 0,
    };
    assert_eq!(files.opened(&mut sys, &answer), Some("second"));
    assert_eq!(
        files.owner(held),
        Some(&"second"),
        "one that yielded bytes stays open"
    );
}

#[test]
fn stale_answer_ownerless() {
    let (mut sys, mut files) = (sys(), Files::<_, 8>::new());
    let stale = files.open(&mut sys, 7, "a", "first");
    files.close(&mut sys, stale);
    let answer = SysOpened {
        fd: stale,
        res: -2,
        detail: 0,
    };
    assert_eq!(files.opened(&mut sys, &answer), None);
}
