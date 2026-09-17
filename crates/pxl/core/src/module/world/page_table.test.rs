// pxl_core/module/world/page_table.test.rs

use pxl_shared::spec::space::pvf3::ivec;

use super::*;
use crate::module::world::change::{world_changes, TERRAIN_PAGE};
use crate::sys::global::serial;

fn page(x: i64, y: i64, z: i64) -> IVec {
    ivec::gather(x, y, z)
}

#[test]
fn page_table_a_page_is_found_by_its_key_with_z_ignored_and_comes_back_as_set() {
    let mut table = PageTable::<&str>::new(None);
    table.set(page(3, 4, 0), "a");
    assert_eq!(table.get(page(3, 4, 0)), Some(&"a"));
    assert_eq!(table.get(page(3, 4, 9)), Some(&"a"));
    assert!(!table.has(page(4, 3, 0)));
    assert_eq!(table.keys().collect::<Vec<_>>(), vec![page(3, 4, 0)]);
}

#[test]
fn page_table_clearing_moves_the_last_page_into_the_hole_and_keeps_every_other_pair() {
    let mut table = PageTable::<&str>::new(None);
    table.set(page(1, 1, 0), "a");
    table.set(page(2, 2, 0), "b");
    table.set(page(3, 3, 0), "c");
    assert_eq!(table.clear(page(1, 1, 0)), Some("a"));
    assert_eq!(table.size(), 2);
    assert_eq!(table.get(page(2, 2, 0)), Some(&"b"));
    assert_eq!(table.get(page(3, 3, 0)), Some(&"c"));
    assert_eq!(table.get(page(1, 1, 0)), None);
    assert_eq!(table.clear(page(1, 1, 0)), None);
    let mut entries: Vec<(IVec, &str)> = table.entries().map(|(k, v)| (k, *v)).collect();
    entries.sort();
    assert_eq!(entries, vec![(page(2, 2, 0), "b"), (page(3, 3, 0), "c")]);
}

#[test]
fn page_table_a_key_past_2_63_survives_the_round_trip_unsigned() {
    let mut table = PageTable::<u32>::new(None);
    let key = page(1 << 20, 1 << 20, 0);
    table.set(key, 1);
    let big = (1u64 << 63) | key;
    table.set(big, 2);
    assert_eq!(table.get(big), Some(&2));
    assert!(table.keys().any(|k| k == big));
}

#[test]
fn page_table_the_revision_moves_on_every_set_and_clear() {
    let mut table = PageTable::<u32>::new(None);
    table.set(page(1, 0, 0), 1);
    table.set(page(1, 0, 0), 2);
    table.clear(page(1, 0, 0));
    assert_eq!(table.revision(), 3);
}

#[test]
fn page_table_a_table_with_a_world_topic_writes_its_pages_to_the_change_log() {
    let _g = serial();
    let mut table = PageTable::<u32>::new(Some(TERRAIN_PAGE));
    let before = table.revision();
    table.set(page(5, 6, 0), 1);
    table.set(page(5, 6, 3), 2);
    table.set(page(7, 8, 0), 3);
    assert_eq!(table.revision(), before + 3);
    assert_eq!(
        world_changes().revision_of(TERRAIN_PAGE, page(5, 6, 0) as i64) as u64,
        before as u64 + 2,
        "each page once per write, Z ignored"
    );
    table.clear(page(7, 8, 0));
    assert_eq!(world_changes().revision_of(TERRAIN_PAGE, page(7, 8, 0) as i64) as u64, 0, "a page that left is forgotten");
}
