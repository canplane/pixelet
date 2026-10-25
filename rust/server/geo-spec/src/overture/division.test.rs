// pxl-geo-spec/overture/division.test.rs

use super::*;

fn division(id: &str, subtype: &str, name: &str, parent: Option<&str>) -> (String, Division) {
    (id.to_string(), Division::new(id.into(), subtype, name.into(), None, parent.map(String::from)))
}

fn lineage() -> FnvHashMap<String, Division> {
    FnvHashMap::from_iter([
        division("r", "region", "Île-de-France", Some("k")),
        division("k", "country", "France", None),
        division("c", "county", "Paris", Some("r")),
        division("l", "locality", "Montmartre", Some("c")),
    ])
}

#[test]
fn a_name_is_the_division_and_each_parent_broad_to_narrow() {
    let parts: Vec<(String, Subtype, bool)> = name("l", true, &lineage()).into_iter().map(|p| (p.name, p.subtype, p.approx)).collect();
    let want = [
        ("France", Subtype::Country, false),
        ("Île-de-France", Subtype::Region, false),
        ("Paris", Subtype::County, false),
        ("Montmartre", Subtype::Locality, true),
    ];
    assert_eq!(parts, want.map(|(n, s, a)| (n.to_string(), s, a)), "only the division found by nearness is approximate");
}

#[test]
fn a_parent_the_lineage_lacks_ends_the_name_and_a_cycle_stops() {
    let mut held = lineage();
    held.remove("r");
    assert_eq!(name("l", false, &held).iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Paris", "Montmartre"]);
    let looped = FnvHashMap::from_iter([division("a", "region", "A", Some("b")), division("b", "county", "B", Some("a"))]);
    assert!(name("a", false, &looped).len() <= Subtype::ALL.len());
    assert!(name("nowhere", false, &held).is_empty());
}

#[test]
fn a_subtype_newer_than_this_tree_is_left_out_of_a_name() {
    let held = FnvHashMap::from_iter([division("x", "galaxy", "Galaxy", Some("k")), division("k", "country", "France", None)]);
    assert_eq!(name("x", false, &held).iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["France"]);
}

#[test]
fn anything_that_is_not_a_name_a_reader_can_be_shown_is_nothing() {
    assert!(names_of(None).is_empty());
    assert!(names_of(Some(serde_json::json!("Montmartre"))).is_empty(), "a bare string names no language");
    assert!(names_of(Some(serde_json::json!([1, 2]))).is_empty());
    assert_eq!(names_of(Some(serde_json::json!({"en": "Montmartre", "fr": 3}))).len(), 1, "the readable half is kept");
}
