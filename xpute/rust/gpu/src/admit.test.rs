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

/// The page's admission follows the block's unfinished frames once its STATUS
/// is READY, and admits nothing before a block is asked for or once the
/// device is lost. The kept block is global, so no other test may touch it.
#[test]
fn page_reads_block() {
    assert!(!page::<2>(), "no block asked");
    ask(Vec::leak(vec![0; crate::device::WORDS]));
    let at = BLOCK.get(|| None).expect("the block kept");
    // SAFETY: the leaked block, its words only ever touched atomically.
    let word = |k: usize| unsafe { AtomicU32::from_ptr(at.add(k)) };
    let (unfinished, status) = (word(UNFINISHED_AT), word(STATUS_AT));
    assert!(!page::<2>(), "the runtime has written no status");
    unfinished.store(1, Ordering::Release);
    status.store(STATUS::READY, Ordering::Release);
    assert!(page::<2>(), "one of two unfinished");
    assert!(!page::<1>(), "one of one unfinished");
    unfinished.store(2, Ordering::Release);
    assert!(!page::<2>(), "two of two unfinished");
    unfinished.store(0, Ordering::Release);
    assert!(page::<1>(), "the frames done");
    status.store(STATUS::LOST, Ordering::Release);
    assert!(!page::<2>(), "the device lost");
}
