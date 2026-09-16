// pxl_core/module/world/terrain/resident.test.rs
//
// What a record carries. The copy itself has no test here: a page's bytes go
// to an address its own coordinate names, so there is nothing to arrange and
// nothing that can be refused.

use super::super::fixed::TERRAIN_AXIS;
use super::*;
use pxl_shared::spec::space::pvf3::ivec;

use crate::sys::global::serial;

#[test]
fn a_page_takes_the_record_its_coordinate_names() {
    let _g = serial();
    let page = ivec::gather(11, 12, 0);
    free_slot(page);
    assert!(!holds_slot(page));
    assert_eq!(take_slot(page), None, "an empty record turns no one out");
    assert!(holds_slot(page));
    free_slot(page);
}

#[test]
fn a_page_an_axis_away_takes_the_record_and_names_who_held_it() {
    let _g = serial();
    let page = ivec::gather(3, 4, 0);
    let far = ivec::gather(3 + TERRAIN_AXIS as i64, 4, 0);
    free_slot(page);
    take_slot(page);
    assert_eq!(slot_of(far), slot_of(page), "an axis apart is one record");
    assert_eq!(take_slot(far), Some(page));
    assert!(holds_slot(far) && !holds_slot(page));
    free_slot(far);
}

#[test]
fn a_page_forgets_only_the_record_it_still_holds() {
    let _g = serial();
    let page = ivec::gather(7, 8, 0);
    let far = ivec::gather(7, 8 + TERRAIN_AXIS as i64, 0);
    free_slot(page);
    take_slot(page);
    take_slot(far);
    free_slot(page);
    assert!(holds_slot(far), "the page it turned out cannot forget it");
    free_slot(far);
}
