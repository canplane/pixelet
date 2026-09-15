// pxl_core/probe.rs

pub static mut COUNTS: [usize; 32] = [0; 32];
pub static mut BYTES: [usize; 32] = [0; 32];

pub fn mark() -> usize {
    unsafe { xpute_core::sched::tick::ALLOC_PROBE.map_or(0, |f| f()) }
}

pub fn add(i: usize, a0: usize) -> usize {
    let m = mark();
    unsafe { COUNTS[i] += m - a0 };
    m
}

pub fn top() -> usize {
    crate::host::heap_used()
}

pub fn add_bytes(i: usize, t0: usize) -> usize {
    let t = top();
    unsafe { BYTES[i] += t - t0 };
    t
}
