// pxl_shared/spec/cell/mod.test.rs

use std::collections::HashSet;

use super::cube::cube_types;
use super::*;

#[test]
fn cube_types_common_void_is_zero_sparse_tree_absence() {
    assert_eq!(cube_types::common::VOID, 0);
    assert!(!object_value_is_present(cube_types::common::VOID as u32));
}

#[test]
fn object_value_is_present_anything_but_void_is_present() {
    assert!(object_value_is_present(cube_types::common::AIR as u32));
    assert!(object_value_is_present(cube_types::block::white as u32));
}

#[test]
fn cube_types_common_air_unknown_the_two_reserved_sentinels() {
    assert_eq!(cube_types::common::AIR, 0x80);
    assert_eq!(cube_types::common::UNKNOWN, 0xff);
    assert!(object_value_is_air(cube_types::common::AIR as u32));
    assert!(!object_value_is_air(cube_types::block::white as u32));
}

#[test]
fn cube_num_id_cube_hex_cube_id_from_num_round_trip_every_flat_color() {
    for id in cube_ids.iter() {
        let num_id = cube_num_id(id);
        assert_eq!(cube_id_from_num(num_id), *id);
        assert!(!cube_hex(id).is_empty());
    }
}

#[test]
#[should_panic]
fn cube_id_from_num_throws_for_an_unregistered_numeric_id() {
    cube_id_from_num(0);
}

#[test]
fn cube_ids_no_two_entries_collide_on_the_same_numeric_id() {
    let mut seen = HashSet::new();
    for id in cube_ids.iter() {
        let num_id = cube_num_id(id);
        assert!(seen.insert(num_id), "duplicate numeric id {num_id} ({id})");
    }
}

#[test]
fn placeable_cube_round_trips_a_terrain_styled_id_derived_from_cube_types_own_land_water_structure_transport_channels() {
    let id = "land.GREEN_DEEP";
    assert!(is_terrain_style_cube(id));
    let num_id = placeable_cube_num_id(id);
    assert_eq!(placeable_cube_id_from_num(num_id), id);
    assert!(placeable_cube_hex(id).is_some());
}

#[test]
fn placeable_cube_a_flat_color_id_still_routes_through_cube_num_id_cube_hex_not_the_terrain_style_path() {
    assert!(!is_terrain_style_cube("white"));
    assert_eq!(placeable_cube_num_id("white"), cube_num_id("white"));
    assert_eq!(placeable_cube_hex("white"), Some(cube_hex("white")));
}

#[test]
fn placeable_cube_ids_every_flat_color_and_every_terrain_styled_entry_no_duplicates() {
    let mut seen = HashSet::new();
    for id in placeable_cube_ids.iter() {
        assert!(seen.insert(id.as_str()), "duplicate placeable id {id}");
    }
    // Every terrain-styled entry adds to cube_ids' own 15 flat colors, never replaces one.
    assert_eq!(seen.len(), placeable_cube_ids.len());
    assert!(placeable_cube_ids.len() > cube_ids.len());
    // Every flat color is present...
    for id in cube_ids.iter() {
        assert!(seen.contains(id));
    }
    // ...and at least one terrain-styled entry per channel is present.
    assert!(seen.contains("land.GREEN_DEEP"));
    assert!(seen.contains("water.RIVER"));
    assert!(seen.contains("structure.BUILDING"));
    assert!(seen.contains("transport.ROAD_DEFAULT"));
}

#[test]
fn placeable_cube_num_id_a_terrain_styled_id_never_collides_with_a_flat_color_s_own_global_byte() {
    let mut seen = HashSet::new();
    for id in placeable_cube_ids.iter() {
        let num_id = placeable_cube_num_id(id);
        assert!(seen.insert(num_id), "duplicate global byte {num_id} ({id})");
    }
}

/// script/vector/pxl/shared/spec/cell/mod.ts's output.
const VECTOR: &str = include_str!("../../../vector/spec/cell/mod.tsv");

fn list(v: &xpute_core::vector::Vector, base: &str) -> Vec<&'static str> {
    if !v.has(&format!("{base}.0")) {
        return Vec::new();
    }
    (0..v.len(base)).map(|j| v.s(&format!("{base}.{j}"))).collect()
}

#[test]
fn the_catalogs_are_what_the_typescript_builds() {
    let v = xpute_core::vector::Vector::load(VECTOR);
    let channels: Vec<(String, Vec<String>)> = cube_types::CHANNELS
        .iter()
        .map(|(ch, types)| (ch.to_string(), types.iter().map(|(n, x)| format!("{n}={x}")).collect()))
        .collect();
    let want: Vec<(String, Vec<String>)> = (0..v.len("channels"))
        .map(|j| {
            (
                v.s(&format!("channels.{j}.channel")).to_string(),
                list(&v, &format!("channels.{j}.types")).iter().map(|s| s.to_string()).collect(),
            )
        })
        .collect();
    assert_eq!(channels, want);
    let colors: Vec<(String, Vec<String>)> = cube_colors
        .0
        .iter()
        .map(|(ch, c)| (ch.to_string(), c.iter().map(|(n, hex)| format!("{n}={}", hex.unwrap_or("null"))).collect()))
        .collect();
    let want: Vec<(String, Vec<String>)> = (0..v.len("colors"))
        .map(|j| {
            (
                v.s(&format!("colors.{j}.channel")).to_string(),
                list(&v, &format!("colors.{j}.colors")).iter().map(|s| s.to_string()).collect(),
            )
        })
        .collect();
    assert_eq!(colors, want);
    let ids: Vec<String> = cube_ids.iter().map(|id| format!("{id}={}={}", cube_num_id(id), cube_hex(id))).collect();
    assert_eq!(ids, list(&v, "cube_ids"));
    let ids: Vec<String> = decor_ids.iter().map(|id| format!("{id}={}", decor_num_id(id))).collect();
    assert_eq!(ids, list(&v, "decor_ids"));
    let mut n = 0;
    v.each("placeable", |k| {
        let id = &placeable_cube_ids[n];
        n += 1;
        assert_eq!(id, v.s(&format!("{k}.id")), "{k}.id");
        let num = placeable_cube_num_id(id);
        assert_eq!(num, v.u32(&format!("{k}.num")), "{k}.num");
        assert_eq!(placeable_cube_hex(id).unwrap_or("null"), v.s(&format!("{k}.hex")), "{k}.hex");
        assert_eq!(placeable_cube_id_from_num(num), v.s(&format!("{k}.back")), "{k}.back");
        let source = if is_terrain_style_cube(id) {
            let s = terrain_style_cube_source(id);
            format!("{}.{}", s.channel.as_str(), s.terrain_num_id)
        } else {
            "none".to_string()
        };
        assert_eq!(source, v.s(&format!("{k}.source")), "{k}.source");
        assert_eq!(object_value_is_decor(num), v.bool(&format!("{k}.is_decor")), "{k}.is_decor");
    });
    assert_eq!(n, placeable_cube_ids.len());
    for (j, (ch, num)) in [
        (GroundChannelKey::LAND, 12),
        (GroundChannelKey::WATER, 3),
        (GroundChannelKey::TRANSPORT, 21),
        (GroundChannelKey::STRUCTURE, 9),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(terrain_style_cube_for(ch, num).unwrap_or("null".to_string()), v.s(&format!("style_for.{j}")), "style_for.{j}");
    }
}
