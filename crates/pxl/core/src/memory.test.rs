// pxl_core/memory.test.rs

use super::*;

/// The fewest regions a map can be cut from: the module's front and the fixed
/// ranges run into the fifth, and the heap needs one past it.
const MIN_REGIONS: u32 = 6;

/// What a linker might leave behind it. The map is a function of this, so the
/// tests ask it at both ends of what MODULE_CEILING allows.
const DATA_END: u32 = 32 << 20;

#[test]
fn the_fixed_ranges_are_what_they_hold_and_nothing_more() {
    for n in MIN_REGIONS..=16 {
        let m = lay_out(n * REGION_BYTES, DATA_END);
        assert_eq!(m.fixed.bytes, FIXED_BYTES, "{n} regions");
        assert!(m.fixed.bytes - SLAB_BYTES < PAGE_BYTES, "{n} regions: rounding only");
    }
}

#[test]
fn every_map_is_the_sections_in_address_order_past_what_the_linker_placed() {
    for n in MIN_REGIONS..=16 {
        let m = lay_out(n * REGION_BYTES, DATA_END);
        assert_eq!(m.fixed.offset, DATA_END, "a page boundary already");
        assert!(m.heap.offset >= m.fixed.offset + SLAB_BYTES, "{n} regions");
        assert_eq!(m.heap.offset + m.heap.bytes, n * REGION_BYTES, "{n} regions");
    }
}

#[test]
fn the_fixed_ranges_follow_the_linker_wherever_it_stopped() {
    for data_end in [1, PAGE_BYTES - 1, PAGE_BYTES, PAGE_BYTES + 1, 32 << 20, MODULE_CEILING] {
        let m = lay_out(16 * REGION_BYTES, data_end);
        assert_eq!(m.fixed.offset, data_end.div_ceil(PAGE_BYTES) * PAGE_BYTES, "{data_end}");
        assert!(m.fixed.offset >= data_end, "{data_end}: never over what the linker placed");
        assert_eq!(m.heap.offset, m.fixed.offset + m.fixed.bytes, "{data_end}");
    }
}

#[test]
fn the_heap_holds_a_whole_region_however_the_fixed_ranges_before_it_end() {
    for n in MIN_REGIONS..=16 {
        let m = lay_out(n * REGION_BYTES, DATA_END);
        assert!(m.heap.bytes >= REGION_BYTES, "{n} regions");
    }
}

#[test]
fn a_memory_s_bytes_past_its_last_whole_region_are_left_out() {
    let m = lay_out(8 * REGION_BYTES + 12345, DATA_END);
    assert_eq!(m.heap.offset + m.heap.bytes, 8 * REGION_BYTES);
}

#[test]
fn every_section_begins_on_a_page_so_every_block_in_it_does() {
    for n in MIN_REGIONS..=16 {
        let m = lay_out(n * REGION_BYTES, DATA_END);
        for s in [m.fixed, m.heap] {
            assert!(s.offset.is_multiple_of(PAGE_BYTES), "{n} regions: {s:?}");
        }
    }
}

/// The bytes between a section's end and the next section's start: under a
/// page, since that is all the rounding can leave.
#[test]
fn no_section_leaves_more_than_a_page_behind_it() {
    for n in MIN_REGIONS..=16 {
        let m = lay_out(n * REGION_BYTES, DATA_END);
        assert!(m.heap.offset - (m.fixed.offset + SLAB_BYTES) < PAGE_BYTES, "{n} regions");
    }
}
