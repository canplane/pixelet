// pxl_core/memory.test.rs

use super::*;

fn regions(m: &Map) -> [u32; 3] {
    [(m.fixed.bytes + MODULE_BYTES) / REGION_BYTES, m.page.bytes / REGION_BYTES, m.mesh.bytes / REGION_BYTES]
}

#[test]
fn eight_regions_are_one_fixed_three_page_and_four_mesh() {
    assert_eq!(regions(&lay_out(8 * REGION_BYTES)), [1, 3, 4]);
}

#[test]
fn four_regions_are_one_fixed_two_page_and_one_mesh() {
    assert_eq!(regions(&lay_out(4 * REGION_BYTES)), [1, 2, 1]);
}

#[test]
fn every_map_is_the_sections_in_address_order_past_the_module_s_to_the_memory_s_end() {
    for n in 3..=16 {
        let m = lay_out(n * REGION_BYTES);
        assert_eq!(m.fixed.offset, MODULE_BYTES);
        assert_eq!(m.page.offset, m.fixed.offset + m.fixed.bytes);
        assert_eq!(m.mesh.offset, m.page.offset + m.page.bytes);
        assert_eq!(m.mesh.offset + m.mesh.bytes, n * REGION_BYTES, "{n} regions");
        for s in [m.fixed, m.page, m.mesh] {
            assert!(s.bytes > 0 && s.bytes.is_multiple_of(s.region_bytes), "{n} regions: {s:?}");
        }
    }
}

#[test]
fn a_memory_s_bytes_past_its_last_whole_region_are_left_out() {
    let m = lay_out(4 * REGION_BYTES + 12345);
    assert_eq!(m.mesh.offset + m.mesh.bytes, 4 * REGION_BYTES);
}
