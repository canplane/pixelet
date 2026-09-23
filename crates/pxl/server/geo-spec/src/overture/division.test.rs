// pxl-geo-spec/overture/division.test.rs

use super::*;

#[test]
fn every_subtype_the_answer_uses_has_a_rung() {
    for subtype in BROAD_SUBTYPE_ORDER.iter().chain(FINE_SUBTYPE_PRIORITY.iter()) {
        assert!(rank_of(subtype).is_some(), "{subtype} is used and has no rung");
    }
}

#[test]
fn the_rungs_run_the_way_the_subtypes_nest() {
    assert!(rank_of("region") < rank_of("county"), "a 도 contains a 시");
    assert!(rank_of("county") < rank_of("macrohood"));
    assert!(rank_of("macrohood") < rank_of("locality"));
}

#[test]
fn the_sub_city_subtypes_that_do_not_nest_share_a_rung() {
    // Overture does not guarantee these three contain each other, so none of
    // them is above another: only which one is preferred differs.
    let fine = rank_of("locality");
    assert_eq!(rank_of("neighborhood"), fine);
    assert_eq!(rank_of("microhood"), fine);
}

#[test]
fn a_subtype_the_answer_has_no_room_for_has_no_rung() {
    assert_eq!(rank_of("country"), None, "Korea's whole polygon is not a part of a label here");
    assert_eq!(rank_of("dependency"), None);
}
