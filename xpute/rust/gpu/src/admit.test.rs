// xpute-gpu/admit.test.rs

use super::*;

#[test]
fn admits_below_latency() {
    assert!(admits(0, 1));
    assert!(!admits(1, 1));
    assert!(admits(1, 2));
    assert!(!admits(2, 2));
    assert!(!admits(3, 2));
}

/// Admission follows the block's unfinished frames once its STATUS
/// is READY, and admits nothing before a block is asked for or once the
/// device is lost. The kept block is global, so no other test may touch it.
#[test]
fn frame_reads_block() {
    assert!(!frame::<2>(), "no block asked");
    ask(Vec::leak(vec![0; crate::spec::DEVICE::WORDS]));
    let at = BLOCK.get(|| None).expect("the block kept");
    // SAFETY: the leaked block, its words only ever touched atomically.
    let word = |k: usize| unsafe { AtomicU32::from_ptr(at.add(k)) };
    let (unfinished, status) = (word(DEVICE::UNFINISHED_AT), word(DEVICE::STATUS_AT));
    assert!(!frame::<2>(), "the runtime has written no status");
    unfinished.store(1, Ordering::Release);
    status.store(STATUS::READY, Ordering::Release);
    assert!(frame::<2>(), "one of two unfinished");
    assert!(!frame::<1>(), "one of one unfinished");
    unfinished.store(2, Ordering::Release);
    assert!(!frame::<2>(), "two of two unfinished");
    unfinished.store(0, Ordering::Release);
    assert!(frame::<1>(), "the frames done");
    status.store(STATUS::LOST, Ordering::Release);
    assert!(!frame::<2>(), "the device lost");
}
