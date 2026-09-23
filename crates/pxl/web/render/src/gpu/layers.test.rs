// pxl-render/gpu/layers.test.rs

use super::*;

use pxl_spec::cell::cube_types;

fn table_of(cube: PlaceableCube) -> u32 {
    let (_, table) = plan();
    let mut word = None;
    each_layer(|c, layer, _| {
        if c == Some(cube) {
            word = Some(table[layer as usize]);
        }
    });
    word.expect("the cube stands for a layer")
}

#[test]
fn a_side_the_pack_ships_nothing_for_shares_the_tops_slot() {
    // white ships a top and no side (asset.test.rs): one tile, one slot, and
    // both faces of the layer read it.
    let word = table_of(PlaceableCube::Block(cube_types::Block::white));
    assert_ne!(word & 0xffff, NO_SLOT, "the top is in the array");
    assert_eq!(word & 0xffff, word >> 16, "and the side wears it");
}

#[test]
fn a_side_of_its_own_takes_a_slot_of_its_own() {
    let word = table_of(PlaceableCube::Land(cube_types::Land::bare));
    assert_ne!(word & 0xffff, NO_SLOT);
    assert_ne!(word & 0xffff, word >> 16, "the side is its own tile");
}

#[test]
fn the_array_is_the_packs_tiles_and_not_a_layer_each() {
    // What the array costs is the pack's, not the catalog's: a layer the pack
    // ships nothing for takes no slot, and a side that wears its top shares
    // one. Two arrays of a layer each was half again this.
    let (tiles, table) = plan();
    assert!(tiles.len() < table.len() * 2, "{} slots against {} layers", tiles.len(), table.len());
    assert!(
        table.iter().any(|w| *w == slot_word(NO_SLOT, NO_SLOT)),
        "a layer the pack ships nothing for reads as the box's own colour"
    );
    for w in &table {
        for slot in [w & 0xffff, w >> 16] {
            assert!(slot == NO_SLOT || (slot as usize) < tiles.len(), "a slot inside the array");
        }
    }
}
