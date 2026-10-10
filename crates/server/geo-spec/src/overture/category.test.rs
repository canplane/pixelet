// pxl-geo-spec/overture/category.test.rs

use super::*;

/// `map` is a binary search, so an unsorted table answers wrongly.
#[test]
fn the_table_is_sorted_and_has_each_overture_category_once() {
    for pair in OVERTURE_CATEGORY_MAP.windows(2) {
        assert!(pair[0].0 < pair[1].0, "{} is not before {}", pair[0].0, pair[1].0);
    }
}

#[test]
fn the_table_and_the_poi_badges_name_the_same_categories() {
    let badges: serde_json::Value = serde_json::from_str(include_str!("../../../../../spec/spec/bake/poi_badge.json")).unwrap();
    let keys: std::collections::BTreeSet<&str> = badges["CATEGORY_BADGE"].as_object().unwrap().keys().map(String::as_str).filter(|k| !k.starts_with('$')).collect();
    let targets: std::collections::BTreeSet<&str> = OVERTURE_CATEGORY_MAP.iter().map(|&(_, t)| t).collect();
    assert_eq!(targets.difference(&keys).collect::<Vec<_>>(), Vec::<&&str>::new(), "targets the POI badge table does not know");
    assert_eq!(keys.difference(&targets).collect::<Vec<_>>(), Vec::<&&str>::new(), "POI badge keys nothing maps to");
}
