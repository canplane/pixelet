// pxl-kernel/control/camera/input.test.rs

//
// The eraser's carve, from the tap to what a reload would read back.
//
// The size tier is a live, user-changed selection, and a carve is the one edit
// it must not reach: the leaf a carve writes is AIR, and a zero material is a
// voxel the scan emits nothing for whatever its sub-cell bytes say
// (pxl-render mesh/voxel_scan.rs), so a smaller leaf takes the cell whole anyway
// and records a bite nobody took. The store is saved, so that number would
// outlive the session.

use base64::Engine;
use pxl_spec::bake::cell_spec::OBJECT_SIZES;
use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::cell::cube_types;
use pxl_spec::space::pvf3::ivec;
use xpute_core::wire::xtp::TreeReader;

use super::*;
use crate::global::serial;
use crate::layer::object::store::encode_objects;

/// Every row the save holds, as a size beside its value — read out of the blob
/// the way `load()` reads it, so this is what a reload would see and not what
/// the in-memory store happens to hold.
fn saved_rows() -> Vec<(f64, u32)> {
    let blob = encode_objects().expect("the store encodes");
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(blob).expect("the blob is base64url");
    let root = TreeReader::new(&bytes).unwrap().read_branch().unwrap();
    let mut out = Vec::new();
    for k in 0..root.len {
        let page = root.at_branch(k).unwrap();
        let size = page.at(4).unwrap().get_array::<f32>().unwrap();
        let value = page.at(5).unwrap().get_array::<u32>().unwrap();
        for r in 0..value.len() {
            out.push((size[r] as f64, value[r]));
        }
    }
    out
}

#[test]
fn eraser_carving_bare_terrain_records_a_whole_cell_whichever_size_tier_is_held() {
    let _g = serial();

    let carving_was = settings_store().enable_terrain_carving;
    settings_store().enable_terrain_carving = true;
    camera_store().hovered_object_at = None;

    // The finest tier, which is the one that would record the smallest lie.
    let finest = OBJECT_SIZES[OBJECT_SIZES.len() - 1];
    assert!(finest < U_PER_CELL, "the ladder needs a tier below the cell for this to be worth asserting");
    input().holding = Holding {
        active_tool: ActiveTool::Eraser,
        object_size: finest,
        ..Default::default()
    };

    handle_tap(Pvf3 { i: ivec::gather(200_003, 0, 0), f: 0 });

    let air = cube_types::common::air as u32;
    let carved: Vec<f64> = saved_rows().into_iter().filter(|(_, v)| *v == air).map(|(size, _)| size).collect();
    assert!(!carved.is_empty(), "the tap reached the store");
    for size in carved {
        assert_eq!(size, U_PER_CELL, "an air leaf is a cell, never a tier");
    }

    settings_store().enable_terrain_carving = carving_was;
}

/// The other half of the same gate: with carving off, the tap writes nothing at
/// all, so the ghost the hover draws is never previewing an edit this would
/// commit.
#[test]
fn eraser_on_bare_terrain_writes_nothing_while_carving_is_off() {
    let _g = serial();

    let carving_was = settings_store().enable_terrain_carving;
    settings_store().enable_terrain_carving = false;
    camera_store().hovered_object_at = None;
    input().holding = Holding {
        active_tool: ActiveTool::Eraser,
        object_size: OBJECT_SIZES[0],
        ..Default::default()
    };

    let before = saved_rows().len();
    handle_tap(Pvf3 { i: ivec::gather(200_009, 0, 0), f: 0 });
    assert_eq!(saved_rows().len(), before);

    settings_store().enable_terrain_carving = carving_was;
}
