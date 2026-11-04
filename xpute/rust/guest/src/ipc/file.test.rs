// xpute-guest/ipc/file.test.rs

use super::*;

fn door() -> (Door, Box<[u8]>) {
    use crate::ipc::ring::{ring_bytes, slots_bytes, Ring};
    use crate::mem::Memory;
    const CAP: u32 = 64;
    const SLOT: u32 = 256;
    let cq_desc = ring_bytes(CAP);
    let sq_slot = cq_desc + ring_bytes(CAP);
    let cq_slot = sq_slot + slots_bytes(CAP, SLOT);
    let mut buf = vec![0u8; (cq_slot + slots_bytes(CAP, SLOT)) as usize].into_boxed_slice();
    let mem = unsafe { Memory::new(buf.as_mut_ptr().expose_provenance(), buf.len()) };
    // SAFETY: the buffer outlives the door: both are returned together.
    let (sq, cq) = unsafe {
        (
            Ring::init(mem, 0, CAP, sq_slot, SLOT),
            Ring::init(mem, cq_desc as usize, CAP, cq_slot, SLOT),
        )
    };
    (Door::new(sq, cq, 0), buf)
}

#[test]
fn descriptor_owner() {
    let ((door, _buf), mut files) = (door(), Files::<_, 8>::new());
    let a = files.open(&door, 7, "a", "first");
    files.close(&door, a);
    let b = files.open(&door, 7, "b", "second");
    assert_eq!(files.held_for(|o| *o == "second"), Some(b));
}

#[test]
fn failed_open_closed() {
    let ((door, _buf), mut files) = (door(), Files::<_, 8>::new());
    let failed = files.open(&door, 7, "a", "first");
    let answer = FsOpened {
        fd: failed,
        res: -2,
        detail: 0,
    };
    assert_eq!(files.complete_open(&door, &answer), Some("first"));
    assert_eq!(files.owner(failed), None, "a failed open is closed");
    let held = files.open(&door, 7, "b", "second");
    let answer = FsOpened {
        fd: held,
        res: 5,
        detail: 0,
    };
    assert_eq!(files.complete_open(&door, &answer), Some("second"));
    assert_eq!(
        files.owner(held),
        Some(&"second"),
        "one that yielded bytes stays open"
    );
}

#[test]
fn answer_read_once() {
    let answer = |res| FsOpened {
        fd: 1,
        res,
        detail: 404,
    };
    assert_eq!(Opened::of(&answer(5)), Opened::Bytes(5));
    assert_eq!(Opened::of(&answer(-2)), Opened::Absent);
    assert_eq!(
        Opened::of(&answer(-5)),
        Opened::Failed {
            errnum: Errno::EIO,
            detail: 404
        }
    );
    assert_eq!(
        Opened::of(&answer(-4095)),
        Opened::Failed {
            errnum: Errno::EPROTO,
            detail: 404
        },
        "a code no errno names breaks the protocol"
    );
}

#[test]
fn read_whole_closes() {
    let ((door, _buf), mut files) = (door(), Files::<_, 8>::new());
    let fd = files.open(&door, 7, "a", "first");
    // SAFETY: nothing runs the records, so nothing fills the buffer.
    let bytes = unsafe { files.read_whole(&door, fd, 5) };
    assert_eq!(bytes.map(|b| b.len()), Ok(5));
    assert_eq!(files.owner(fd), None, "read whole, it is closed");
}

#[test]
fn stale_answer_ownerless() {
    let ((door, _buf), mut files) = (door(), Files::<_, 8>::new());
    let stale = files.open(&door, 7, "a", "first");
    files.close(&door, stale);
    let answer = FsOpened {
        fd: stale,
        res: -2,
        detail: 0,
    };
    assert_eq!(files.complete_open(&door, &answer), None);
}
