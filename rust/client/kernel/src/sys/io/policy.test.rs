// pxl-kernel/sys/io/policy.test.rs

use super::*;
use crate::sys::io::demand::Priority;

/// The objects' subjects are files' numbers, the top's 0 no tile: it is
/// ranked without a tile read off it (0529abe78, where that looped some
/// 2^31 times on the main thread).
#[test]
fn object_top_ranked() {
    let ask = Ask {
        priority: Priority::Demand,
        order: Order::FIRST,
    };
    let r = Policy::standard().rank(Stream::object, 0, ask, 0.0);
    assert_eq!((r.class, r.detail), (OBJECT, 0));
}
