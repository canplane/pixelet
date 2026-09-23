// pxl-kernel/layer/terrain/resident.test.rs
//
// What a record carries. The copy itself has no test here: the caller is
// handed a record and writes the page's lanes into it, and there is nothing
// in that which can be refused.

use pxl_spec::space::pvf3::ivec;

use super::*;
use crate::global::serial;
use crate::layer::terrain::level::{HELD_AXIS, WINDOW_LEVELS};

fn page(level: u32, x: i64, y: i64) -> LevelPage {
    LevelPage::new(level, ivec::gather(x, y, 0))
}

#[test]
fn a_record_carries_the_page_it_was_given_and_says_who_it_was_taken_from() {
    let _g = serial();
    forget_all();
    let (p, q) = (page(0, 11, 12), page(3, -4, 9));
    let record = free_record().expect("an empty range");
    assert_eq!(put(p, record), None, "a record carrying nothing turns no one out");
    assert_eq!(record_of(p), Some(record));
    assert!(holds_record(p));
    assert_eq!(put(p, record), None, "the page it already carries is not turned out");
    assert_eq!(put(q, record), Some(p));
    assert!(holds_record(q) && !holds_record(p));
}

#[test]
fn any_page_fits_any_record() {
    let _g = serial();
    forget_all();
    // An axis apart at one level, and a window apart in level: what the old
    // addressing made collide, and what a fold and a pan back are made of.
    let near = page(2, 3, 4);
    let far = page(2, 3 + HELD_AXIS as i64, 4);
    let over = page(2 + WINDOW_LEVELS, 3, 4);
    for p in [near, far, over] {
        let record = free_record().expect("an empty range");
        assert_eq!(put(p, record), None);
    }
    assert!(holds_record(near) && holds_record(far) && holds_record(over));
    assert_ne!(record_of(near), record_of(far));
    assert_ne!(record_of(near), record_of(over));
}

#[test]
fn a_full_range_has_no_free_record_and_an_empty_one_is_all_free() {
    let _g = serial();
    forget_all();
    assert_eq!(free_record(), Some(0));
    for k in 0..RECORDS {
        let record = free_record().expect("a record while any is empty");
        put(page(0, k as i64, 0), record);
    }
    assert_eq!(free_record(), None, "every record carries a page");
    assert_eq!(page_in(0), Some(page(0, 0, 0)));
    forget_all();
}
