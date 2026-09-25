// pxl-geo-spec/overture/category.test.rs

use super::*;

/// `map` is a binary search, so a table out of order would answer wrongly
/// rather than slowly.
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

/// Every target is a key of the POI badge's theme, and every key of that
/// theme is some entry's target. A target the theme does not know draws the
/// default badge and says nothing about it, and a theme entry nothing maps to
/// is one no place ever wears.
#[test]
fn the_table_and_the_poi_theme_name_the_same_categories() {
    let theme: serde_json::Value = serde_json::from_str(include_str!("../../../../../../spec/pxl/web/kernel/layer/poi/theme.json")).unwrap();
    let keys: std::collections::BTreeSet<&str> = theme["CATEGORY_THEME"].as_object().unwrap().keys().map(String::as_str).filter(|k| !k.starts_with('$')).collect();
    let targets: std::collections::BTreeSet<&str> = OVERTURE_CATEGORY_MAP.iter().map(|&(_, t)| t).collect();
    assert_eq!(targets.difference(&keys).collect::<Vec<_>>(), Vec::<&&str>::new(), "targets the POI theme does not know");
    assert_eq!(keys.difference(&targets).collect::<Vec<_>>(), Vec::<&&str>::new(), "POI theme keys nothing maps to");
}
