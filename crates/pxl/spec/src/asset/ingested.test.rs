// pxl-spec/asset/ingested.test.rs

use super::*;

#[test]
fn ingested_every_asset_s_number_is_its_row_and_names_it_back() {
    for (n, a) in Asset::ALL.into_iter().enumerate() {
        assert_eq!(a as usize, n);
        assert_eq!(Asset::of(n as u16), Some(a));
        assert!(a.ingested().bytes > 0);
    }
    assert_eq!(Asset::of(Asset::ALL.len() as u16), None);
}

#[test]
fn ingested_the_sheet_s_tiles_are_numbered_in_order_one_a_cube_face() {
    for (n, &(cube, face, tile)) in SHEET.tiles.iter().enumerate() {
        assert_eq!(tile as usize, n);
        assert!(SHEET.tiles[..n].iter().all(|&(c, f, _)| (c, f) != (cube, face)));
    }
}
