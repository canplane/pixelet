// pxl-geo-bake/terrain/pipeline/ground.test.rs

use super::*;

/// Cells a way three cells wide paints in its column, ending at y = 20 on a cell's edge.
fn painted(cut: [bool; 2]) -> Vec<i64> {
    let mut cells = vec![0 as GroundCell; PAGE_AREA];
    let mut target = Target { cells: &mut cells, key: GroundChannelKey::TRANSPORT };
    render_linear_feat(&[[10.5, 10.0, 0.0], [10.5, 20.0, 0.0]], 1, &mut target, 3.0, cut, Mask::None);
    (0..SPAN).filter(|&y| channel_of(cells[cell_idx(10, y)], GroundChannelKey::TRANSPORT) == 1).collect()
}

#[test]
fn a_way_cut_by_a_flag_stops_at_the_cut_and_a_free_end_rounds_past_it() {
    assert_eq!(painted([false, true]).last(), Some(&19));
    assert!(*painted([false, false]).last().unwrap() > 19);
}
