// pxl_core/memory.test.rs

use super::*;

/// The fewest regions a map can be cut from: the module and the fixed section
/// take the first, the slab runs into the fifth, and the mesh needs one past
/// it.
const MIN_REGIONS: u32 = 6;

#[test]
fn the_slab_is_the_terrain_it_holds_and_nothing_more() {
    for n in MIN_REGIONS..=16 {
        let m = lay_out(n * REGION_BYTES);
        assert_eq!(m.slab.bytes, SLAB_BYTES, "{n} regions");
    }
}

#[test]
fn every_map_is_the_sections_in_address_order_past_the_module_s() {
    for n in MIN_REGIONS..=16 {
        let m = lay_out(n * REGION_BYTES);
        assert_eq!(m.fixed.offset, MODULE_BYTES);
        assert_eq!(m.fixed.offset + m.fixed.bytes, REGION_BYTES);
        assert_eq!(m.slab.offset, REGION_BYTES);
        assert!(m.mesh.offset >= m.slab.offset + m.slab.bytes, "{n} regions");
        assert_eq!(m.mesh.offset + m.mesh.bytes, n * REGION_BYTES, "{n} regions");
    }
}

#[test]
fn the_mesh_holds_a_region_its_pool_can_cut_however_the_slab_before_it_ends() {
    for n in MIN_REGIONS..=16 {
        let m = lay_out(n * REGION_BYTES);
        assert!(m.mesh.bytes >= REGION_BYTES, "{n} regions");
    }
}

#[test]
fn a_memory_s_bytes_past_its_last_whole_region_are_left_out() {
    let m = lay_out(8 * REGION_BYTES + 12345);
    assert_eq!(m.mesh.offset + m.mesh.bytes, 8 * REGION_BYTES);
}

#[test]
fn every_section_begins_on_a_page_so_every_block_a_pool_hands_out_does() {
    for n in MIN_REGIONS..=16 {
        let m = lay_out(n * REGION_BYTES);
        for s in [m.fixed, m.slab, m.mesh] {
            assert!(s.offset.is_multiple_of(PAGE_BYTES), "{n} regions: {s:?}");
        }
    }
}

/// The bytes between a section's end and the next section's start: under a
/// page, since that is all the rounding can leave.
#[test]
fn no_section_leaves_more_than_a_page_behind_it() {
    for n in MIN_REGIONS..=16 {
        let m = lay_out(n * REGION_BYTES);
        assert!(m.slab.offset - (m.fixed.offset + m.fixed.bytes) < PAGE_BYTES, "{n} regions");
        assert!(m.mesh.offset - (m.slab.offset + m.slab.bytes) < PAGE_BYTES, "{n} regions");
    }
}
