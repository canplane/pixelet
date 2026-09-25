// pxl-spec/cell/object.test.rs

use std::collections::HashSet;

use super::*;
use crate::cell::cube::{cube_types, ground_pack};

#[test]
fn cube_types_common_void_is_zero_sparse_tree_absence() {
    assert_eq!(cube_types::common::void, 0);
    assert!(!object_value_is_present(cube_types::common::void as u32));
}

#[test]
fn object_value_is_present_anything_but_void_is_present() {
    assert!(object_value_is_present(cube_types::common::air as u32));
    assert!(object_value_is_present(cube_types::block::white as u32));
}

#[test]
fn cube_types_common_air_unknown_the_two_reserved_sentinels() {
    assert_eq!(cube_types::common::air, 0x80);
    assert_eq!(cube_types::common::unknown, 0xff);
    assert!(object_value_is_air(cube_types::common::air as u32));
    assert!(!object_value_is_air(cube_types::block::white as u32));
}

/// A cube spelled as the shell spells it, `"land.bare"`: the words the
/// record kept, from the catalog's name tables.
fn spelled(cube: PlaceableCube) -> String {
    let (channel, t) = placeable_cube_parts(cube);
    let name = channel.map_or("block", |c| ground_pack::CHANNELS[c as usize]);
    let entries = cube_types::CHANNELS.iter().find(|(n, _)| *n == name).unwrap().1;
    format!("{name}.{}", entries.iter().find(|(_, x)| *x == t).unwrap().0)
}

#[test]
fn every_flat_colour_round_trips_through_its_leaf_value_and_has_a_colour() {
    for block in block_cubes() {
        let cube = PlaceableCube::Block(block);
        assert_eq!(placeable_cube_num_id(cube), block as u32);
        assert_eq!(placeable_cube_of(block as u32), Some(cube));
        assert_eq!(terrain_style_cube_source(cube), None);
        assert!(placeable_cube_rgb(cube).is_some(), "{}", spelled(cube));
    }
}

#[test]
fn a_cube_named_by_its_channel_and_number_is_the_cube() {
    for &cube in placeable_cubes.iter() {
        let (channel, t) = placeable_cube_parts(cube);
        assert_eq!(placeable_cube(channel, t as u32), Some(cube), "{}", spelled(cube));
    }
    assert_eq!(placeable_cube(None, cube_types::common::air as u32), None, "air is no flat colour");
    assert_eq!(placeable_cube(Some(GroundChannelKey::WATER), cube_types::common::unknown as u32), None, "unknown is no water cube");
}

#[test]
fn a_terrain_styled_cube_round_trips_through_its_leaf_value() {
    let cube = terrain_style_cube_for(GroundChannelKey::LAND, cube_types::land::green_deep as u32).unwrap();
    assert_eq!(cube, PlaceableCube::Land(cube_types::Land::green_deep));
    assert_eq!(placeable_cube_of(placeable_cube_num_id(cube)), Some(cube));
    assert!(placeable_cube_rgb(cube).is_some());
}

#[test]
fn placeable_cubes_every_flat_color_and_every_terrain_styled_entry_no_duplicates() {
    let mut seen = HashSet::new();
    for &cube in placeable_cubes.iter() {
        let num_id = placeable_cube_num_id(cube);
        assert!(seen.insert(num_id), "duplicate global byte {num_id} ({})", spelled(cube));
    }
    // Every terrain-styled entry adds to the flat colours, never replaces one.
    assert!(placeable_cubes.len() > block_cubes().len());
    for block in block_cubes() {
        assert!(placeable_cubes.contains(&PlaceableCube::Block(block)));
    }
    for cube in [
        PlaceableCube::Land(cube_types::Land::green_deep),
        PlaceableCube::Water(cube_types::Water::river),
        PlaceableCube::Structure(cube_types::Structure::building),
        PlaceableCube::Transport(cube_types::Transport::road_default),
    ] {
        assert!(placeable_cubes.contains(&cube), "{}", spelled(cube));
    }
}

/// What the TypeScript computed, recorded: the generator that wrote it
/// is gone and the vector stands as it is (xpute-core/golden.rs).
const GOLDEN: &str = include_str!("../../golden/spec/cell/object.tsv");

fn list(v: &xpute_core::golden::Golden, base: &str) -> Vec<&'static str> {
    if !v.has(&format!("{base}.0")) {
        return Vec::new();
    }
    (0..v.len(base)).map(|j| v.s(&format!("{base}.{j}"))).collect()
}

#[test]
fn the_catalogs_are_what_the_record_holds() {
    let v = xpute_core::golden::Golden::load(GOLDEN);
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
    // A channel's colours as the record listed them: its sentinels, which a
    // ground channel shares with `common`, then every type with a colour, by
    // number.
    let listed = |name: &str, colour: &dyn Fn(u8) -> Option<crate::cell::r#type::Swatch>, sentinels: &[u8], own: &[u8]| {
        let mut numbers: Vec<u8> = sentinels.iter().copied().chain(own.iter().copied().filter(|&t| colour(t).is_some())).collect();
        numbers.sort();
        numbers.dedup();
        (name.to_string(), numbers.iter().map(|&n| format!("{n}={}", colour(n).map_or("null", |s| s.hex))).collect::<Vec<_>>())
    };
    let (void, air, unknown) = (cube_types::common::void, cube_types::common::air, cube_types::common::unknown);
    let mut colors = vec![listed("common", &color::common, &[void, air, unknown], &[])];
    for channel in GroundChannelKey::ALL {
        colors.push(listed(ground_pack::CHANNELS[channel as usize], &|t| color::ground(channel, t), &[void, unknown], &own_types(channel)));
    }
    let blocks: Vec<u8> = block_cubes().iter().map(|b| *b as u8).collect();
    colors.push(listed("block", &color::block, &[], &blocks));
    let want: Vec<(String, Vec<String>)> = (0..v.len("colors"))
        .map(|j| {
            (
                v.s(&format!("colors.{j}.channel")).to_string(),
                list(&v, &format!("colors.{j}.colors")).iter().map(|s| s.to_string()).collect(),
            )
        })
        .collect();
    assert_eq!(colors, want);
    let ids: Vec<String> = block_cubes()
        .into_iter()
        .map(|b| {
            let cube = PlaceableCube::Block(b);
            format!("{}={}={}", spelled(cube).trim_start_matches("block."), b as u8, placeable_cube_swatch(cube).unwrap().hex)
        })
        .collect();
    assert_eq!(ids, list(&v, "cube_ids"));
    let ids: Vec<String> = decor_ids.iter().map(|id| format!("{id:?}={}", decor_num_id(*id))).collect();
    assert_eq!(ids, list(&v, "decor_ids"));
    let mut n = 0;
    v.each("placeable", |k| {
        let cube = placeable_cubes[n];
        n += 1;
        assert_eq!(spelled(cube), v.s(&format!("{k}.id")), "{k}.id");
        let num = placeable_cube_num_id(cube);
        assert_eq!(num, v.u32(&format!("{k}.num")), "{k}.num");
        assert_eq!(placeable_cube_swatch(cube).map_or("null", |s| s.hex), v.s(&format!("{k}.hex")), "{k}.hex");
        assert_eq!(spelled(placeable_cube_of(num).unwrap()), v.s(&format!("{k}.back")), "{k}.back");
        let source = match terrain_style_cube_source(cube) {
            Some(s) => format!("{}.{}", ground_pack::CHANNELS[s.channel as usize], s.terrain_num_id),
            None => "none".to_string(),
        };
        assert_eq!(source, v.s(&format!("{k}.source")), "{k}.source");
        assert_eq!(object_value_is_decor(num), v.bool(&format!("{k}.is_decor")), "{k}.is_decor");
    });
    assert_eq!(n, placeable_cubes.len());
    for (j, (ch, num)) in [
        (GroundChannelKey::LAND, 12),
        (GroundChannelKey::WATER, 3),
        (GroundChannelKey::TRANSPORT, 21),
        (GroundChannelKey::STRUCTURE, 9),
    ]
    .into_iter()
    .enumerate()
    {
        let got = terrain_style_cube_for(ch, num).map_or("null".to_string(), spelled);
        assert_eq!(got, v.s(&format!("style_for.{j}")), "style_for.{j}");
    }
}
