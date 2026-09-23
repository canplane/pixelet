// pxl-kernel/layer/terrain/resident.test.rs
//
// What a record carries. The copy itself has no test here: a page's bytes go
// to an address its own coordinate names, so there is nothing to arrange and
// nothing that can be refused.

use pxl_spec::space::pvf3::ivec;

use super::*;
use crate::global::serial;
use crate::layer::terrain::fixed::slot_of;
use crate::layer::terrain::level::{HELD_AXIS, WINDOW_LEVELS};

fn page(level: u32, x: i64, y: i64) -> LevelPage {
    LevelPage::new(level, ivec::gather(x, y, 0))
}

#[test]
fn a_page_takes_the_record_its_coordinate_names() {
    let _g = serial();
    let p = page(0, 11, 12);
    free_slot(p);
    assert!(!holds_slot(p));
    assert_eq!(take_slot(p), None, "an empty record turns no one out");
    assert!(holds_slot(p));
    free_slot(p);
}

#[test]
fn a_page_an_axis_away_takes_the_record_and_names_who_held_it() {
    let _g = serial();
    let p = page(2, 3, 4);
    let far = page(2, 3 + HELD_AXIS as i64, 4);
    free_slot(p);
    take_slot(p);
    assert_eq!(slot_of(far), slot_of(p), "an axis apart is one record");
    assert_eq!(take_slot(far), Some(p));
    assert!(holds_slot(far) && !holds_slot(p));
    free_slot(far);
}

#[test]
fn levels_a_window_apart_share_a_group_and_levels_inside_it_do_not() {
    let _g = serial();
    assert_eq!(slot_of(page(1, 3, 4)), slot_of(page(1 + WINDOW_LEVELS, 3, 4)));
    for level in 1..WINDOW_LEVELS {
        assert_ne!(slot_of(page(0, 3, 4)), slot_of(page(level, 3, 4)));
    }
}

#[test]
fn a_page_forgets_only_the_record_it_still_holds() {
    let _g = serial();
    let p = page(0, 7, 8);
    let far = page(0, 7, 8 + HELD_AXIS as i64);
    free_slot(p);
    take_slot(p);
    take_slot(far);
    free_slot(p);
    assert!(holds_slot(far), "the page it turned out cannot forget it");
    free_slot(far);
}
