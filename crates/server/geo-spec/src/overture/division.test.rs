// pxl-geo-spec/overture/division.test.rs

use super::*;

fn division(id: &str, subtype: &str, name: &str, parent: Option<&str>) -> (String, Division) {
    (id.to_string(), Division::new(id.into(), subtype, name.into(), None, parent.map(String::from)))
}

fn lineage() -> HashMap<String, Division> {
    HashMap::from([
        division("r", "region", "경기도", Some("k")),
        division("k", "country", "대한민국", None),
        division("c", "county", "성남시", Some("r")),
        division("l", "locality", "정자동", Some("c")),
    ])
}

#[test]
fn a_name_is_the_division_and_each_parent_broad_to_narrow() {
    let parts: Vec<(String, Subtype, bool)> = name("l", true, &lineage()).into_iter().map(|p| (p.name, p.subtype, p.approx)).collect();
    let want = [("대한민국", Subtype::Country, false), ("경기도", Subtype::Region, false), ("성남시", Subtype::County, false), ("정자동", Subtype::Locality, true)];
    assert_eq!(parts, want.map(|(n, s, a)| (n.to_string(), s, a)), "only the division found by nearness is approximate");
}

#[test]
fn a_parent_the_lineage_lacks_ends_the_name_and_a_cycle_stops() {
    let mut held = lineage();
    held.remove("r");
    assert_eq!(name("l", false, &held).iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["성남시", "정자동"]);
    let looped = HashMap::from([division("a", "region", "가", Some("b")), division("b", "county", "나", Some("a"))]);
    assert!(name("a", false, &looped).len() <= Subtype::ALL.len());
    assert!(name("nowhere", false, &held).is_empty());
}

#[test]
fn a_subtype_newer_than_this_tree_is_left_out_of_a_name() {
    let held = HashMap::from([division("x", "galaxy", "은하", Some("k")), division("k", "country", "대한민국", None)]);
    assert_eq!(name("x", false, &held).iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["대한민국"]);
}

#[test]
fn anything_that_is_not_a_name_a_reader_can_be_shown_is_nothing() {
    assert!(names_of(None).is_empty());
    assert!(names_of(Some(serde_json::json!("분당구"))).is_empty(), "a bare string names no language");
    assert!(names_of(Some(serde_json::json!([1, 2]))).is_empty());
    assert_eq!(names_of(Some(serde_json::json!({"en": "Bundang-gu", "ko": 3}))).len(), 1, "the readable half is kept");
}

#[test]
fn the_subtypes_run_broad_to_fine_as_overture_nests_them() {
    let order: Vec<Subtype> = ["country", "region", "county", "locality", "neighborhood", "microhood"].iter().map(|s| Subtype::named(s).expect("Overture's")).collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]));
}
