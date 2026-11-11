// pxl-core/perf/record.test.rs

use super::*;

/// A name is written once, NUL-ended after the empty name at 0, and found
/// again where it lies; one past the cap is not kept.
#[test]
fn strtab_keeps_each_name_once() {
    let mut t = [0u8; 12];
    let mut len = 1;
    assert_eq!(name_at(&mut t, &mut len, "a.rs"), Some(1));
    assert_eq!(name_at(&mut t, &mut len, "bc.rs"), Some(6));
    assert_eq!(
        name_at(&mut t, &mut len, "a.rs"),
        Some(1),
        "found, not written again"
    );
    assert_eq!(&t[..len], b"\0a.rs\0bc.rs\0");
    assert_eq!(name_at(&mut t, &mut len, "d"), None, "past the cap");
    assert_eq!(len, 12, "a name with no room writes nothing");
}
