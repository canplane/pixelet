// pxl-geo-spec/overture/division.test.rs

use super::*;

#[test]
fn the_rungs_run_the_way_the_subtypes_nest() {
    assert!(rank_of("region") < rank_of("county"), "a 도 contains a 시");
    assert!(rank_of("county") < rank_of("macrohood"));
    assert!(rank_of("macrohood") < rank_of("locality"));
}
