// pxl-geo-spec/overture/category.test.rs

use super::*;

/// `map` is a binary search, so a table out of order would answer wrongly
/// rather than slowly. The generator sorts it; this is what says so.
#[test]
fn the_table_is_sorted_and_has_each_overture_category_once() {
    for pair in OVERTURE_CATEGORY_MAP.windows(2) {
        assert!(pair[0].0 < pair[1].0, "{} is not before {}", pair[0].0, pair[1].0);
    }
}

#[test]
fn a_mapped_category_resolves_and_an_unmapped_one_does_not() {
    assert_eq!(map("medical_center"), Some("hospital"));
    assert_eq!(map("coffee_shop"), Some("cafe"));
    // The first and last of the table, which a binary search gets wrong most
    // easily.
    assert_eq!(map(OVERTURE_CATEGORY_MAP[0].0), Some(OVERTURE_CATEGORY_MAP[0].1));
    let last = OVERTURE_CATEGORY_MAP[OVERTURE_CATEGORY_MAP.len() - 1];
    assert_eq!(map(last.0), Some(last.1));

    assert_eq!(map("nail_salon"), None);
    assert_eq!(map(""), None);
}
