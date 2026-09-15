// pxl_core/module/world/terrain/resident.test.rs
//
// resident.ts's tests of the copy itself are the shell's, which copies; what
// stays here is the ownership and the quarantine.

use core::sync::atomic::{AtomicU64, Ordering};

use super::*;
use crate::global::{serial, set_clock};
use pxl_shared::spec::space::pvf3::ivec;

static NOW_MS: AtomicU64 = AtomicU64::new(0);

fn test_clock() -> f64 {
    f64::from_bits(NOW_MS.load(Ordering::Relaxed))
}

fn at(ms: f64) {
    NOW_MS.store(ms.to_bits(), Ordering::Relaxed);
}

#[test]
fn release_terrain_blocks_go_back_only_after_the_quarantine() {
    let _g = serial();
    set_clock(test_clock);
    at(1000.0);
    take_due_releases();
    let page = ivec::gather(11, 12, 0);
    into_core(page, vec![(4096, 1 << 20), (1 << 24, 4096)], true);
    release_terrain(page, 20.0);
    let during = take_due_releases();
    at(1030.0);
    let after = take_due_releases();
    set_clock(|| 0.0);
    assert!(during.is_empty(), "still borrowed during the quarantine");
    assert_eq!(after, vec![(4096, 1 << 20), (1 << 24, 4096)]);
}

#[test]
fn release_terrain_a_shared_page_is_never_released() {
    let _g = serial();
    set_clock(test_clock);
    at(5000.0);
    take_due_releases();
    let page = ivec::gather(13, 14, 0);
    into_core(page, vec![(8192, 1 << 20)], false);
    release_terrain(page, 0.0);
    at(5020.0);
    let due = take_due_releases();
    set_clock(|| 0.0);
    assert!(due.is_empty(), "the ocean page stays borrowed");
}
