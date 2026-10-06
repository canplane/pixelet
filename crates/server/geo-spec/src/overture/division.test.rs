// pxl-geo-spec/overture/division.test.rs

use super::*;

fn area(id: &str, subtype: &str, name: &str) -> Division {
    Division::new(id.into(), subtype.into(), name.into(), None)
}

#[test]
fn the_rungs_run_the_way_the_subtypes_nest() {
    assert!(rank_of("region") < rank_of("county"), "a 도 contains a 시");
    assert!(rank_of("county") < rank_of("macrohood"));
    assert!(rank_of("macrohood") < rank_of("locality"));
}

#[test]
fn a_providers_name_map_becomes_the_answers_own() {
    let d = Division::new("a".into(), "county".into(), "분당구".into(), Some(serde_json::json!({"en": "Bundang-gu", "ko": "분당구"})));
    assert_eq!(d.names.get("en").map(String::as_str), Some("Bundang-gu"));
    assert_eq!(d.names.get("ko").map(String::as_str), Some("분당구"));
}

#[test]
fn anything_that_is_not_a_name_a_reader_can_be_shown_is_nothing() {
    assert!(names_of(None).is_empty());
    assert!(names_of(Some(serde_json::json!("분당구"))).is_empty(), "a bare string names no language");
    assert!(names_of(Some(serde_json::json!([1, 2]))).is_empty());
    assert_eq!(names_of(Some(serde_json::json!({"en": "Bundang-gu", "ko": 3}))).len(), 1, "the readable half is kept");
}

#[test]
fn a_name_runs_broad_to_fine_and_says_each_name_once() {
    let areas = [
        area("l", "locality", "정자동"),
        area("c", "county", "분당구"),
        area("r1", "region", "경기도"),
        area("r2", "region", "경기도"),
        area("n", "neighborhood", "정자1동"),
    ];
    let point = area("p", "locality", "정자동");
    let names: Vec<(&str, bool)> = label(&areas, Some(&point)).iter().map(|(d, approx)| (d.name.as_str(), *approx)).collect();
    assert_eq!(names, [("경기도", false), ("분당구", false), ("정자동", false)]);
    assert_eq!(within(&areas).as_deref(), Some("l"), "the point is looked for inside the finest area");
}

#[test]
fn a_point_no_fine_polygon_covers_is_named_by_the_nearest_point() {
    let areas = [area("r", "region", "경기도"), area("c", "county", "분당구")];
    assert_eq!(within(&areas).as_deref(), Some("c"));
    let point = area("p", "locality", "정자동");
    let parts = label(&areas, Some(&point));
    assert_eq!(parts.last().map(|(d, approx)| (d.name.as_str(), *approx)), Some(("정자동", true)));
    assert_eq!(within(&[]), None, "with no polygon, the radius bounds the search");
}
