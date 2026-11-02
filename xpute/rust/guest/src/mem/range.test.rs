// xpute-guest/mem/range.test.rs

use super::*;

#[test]
fn arr_laid_on_lines() {
    assert_eq!(arr_bytes(0, 4), 0);
    assert_eq!(arr_bytes(1, 4), 64);
    assert_eq!(arr_bytes(16, 4), 64);
    assert_eq!(arr_bytes(17, 4), 128);
}

#[test]
fn owned_range_handed_once() {
    let _g = crate::global::serial();
    static RANGE: OwnedRange = OwnedRange::new();
    let s = Section {
        off: 4096,
        bytes: 64,
    };
    RANGE.set(s);
    assert_eq!(RANGE.get(), s);
}
