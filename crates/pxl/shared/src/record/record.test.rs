// pxl_shared/record/record.test.rs

use std::collections::HashSet;

use xpute_core::kit::array::{F32Array, U32Array, U8Array};
use xpute_core::kit::hash::fnv1a64_str;

use super::palette::{encode_palette, PaletteEntry, PaletteKind, PaletteTable, USER_ID_BASE};
use super::placement::{encode_placements, Placement, PlacementTable};
use super::shipped::{shipped_palette, CHANNEL_ID_BASE};
use super::table::{encode_table, Column, ColumnValue, RecordSource, Table};
use super::texture::{shipped_texture_key, shipped_texture_tile, CubeFace, SHIPPED_SHEET};
use crate::spec::cell::{cube_colors, cube_types, decor_types};

fn column(key: &str, value: ColumnValue, required: bool) -> Column {
    Column {
        key: key.to_string(),
        value,
        required,
        stride: None,
    }
}

fn packet(pkt: U8Array) -> RecordSource {
    RecordSource::Packet(pkt)
}

#[test]
fn table_a_record_names_its_kind_its_columns_and_its_count_and_a_reader_finds_columns_by_key() {
    let pkt = encode_table(
        "thing",
        3,
        2,
        &[
            column("a", ColumnValue::Array(U32Array::from(&[1, 2]).view()), true),
            column("s", ColumnValue::Strings(vec!["x".into(), "y".into()]), false),
        ],
    )
    .unwrap();
    let t = Table::new(packet(pkt), &["a", "s"]).unwrap();
    assert_eq!(t.kind, "thing");
    assert_eq!(t.version, 3);
    assert_eq!(t.count, 2);
    assert_eq!(t.lanes::<u32>("a").unwrap().as_slice(), &[1, 2]);
    assert_eq!(t.strings("s").unwrap().unwrap().at(1).unwrap().get_str().unwrap(), "y");
    assert!(!t.has("nope"));
    assert!(t.lanes_opt::<u8>("nope").unwrap().is_none());
}

#[test]
fn table_a_column_a_reader_does_not_know_is_skipped_when_ancillary_and_refused_when_required() {
    let pkt = encode_table(
        "thing",
        1,
        1,
        &[
            column("a", ColumnValue::Array(U8Array::from(&[7]).view()), true),
            column("later", ColumnValue::Array(F32Array::from(&[0.5]).view()), false),
        ],
    )
    .unwrap();
    let t = Table::new(packet(pkt), &["a"]).unwrap();
    assert_eq!(t.lanes::<u8>("a").unwrap()[0], 7);

    let strict = encode_table(
        "thing",
        2,
        1,
        &[
            column("a", ColumnValue::Array(U8Array::from(&[7]).view()), true),
            column("must", ColumnValue::Array(F32Array::from(&[0.5]).view()), true),
        ],
    )
    .unwrap();
    let err = Table::new(packet(strict), &["a"]).unwrap_err();
    assert!(err.to_string().contains("required column must"), "{err}");
}

#[test]
#[should_panic(expected = "has 1 rows, not 2")]
fn table_every_column_must_have_the_record_s_row_count() {
    let _ = encode_table("thing", 1, 2, &[column("a", ColumnValue::Array(U8Array::from(&[1]).view()), false)]);
}

fn placement(id: u64, x: f64, y: f64, z: f64, size: f64, heading: f64, value: u32) -> Placement {
    Placement {
        id,
        x,
        y,
        z,
        size,
        heading,
        value,
        scale: None,
        rotation: None,
        tags: None,
    }
}

#[test]
fn placement_rows_round_trip_ancillary_columns_only_when_some_row_has_them() {
    let rows = [
        placement(fnv1a64_str("tree@1"), 12.5, 3.25, 40.0, 1.0, core::f64::consts::FRAC_PI_2, 7),
        Placement {
            scale: Some(1.25),
            tags: Some(vec![("name".into(), Some("가".into()))]),
            ..placement(42, 0.0, 1024.0, -2.0, 2.0, 0.0, 3)
        },
    ];
    let t = PlacementTable::new(packet(encode_placements(&rows).unwrap())).unwrap();
    assert_eq!(t.count, 2);
    assert_eq!(t.id[0], fnv1a64_str("tree@1"));
    assert_eq!(t.x[0], 12.5);
    assert_eq!(t.size[1], 2.0);
    assert!((t.heading[0] as f64 - core::f64::consts::FRAC_PI_2).abs() < 1e-6);
    assert_eq!(t.value[1], 3);
    assert_eq!(t.scale_of(0), 1.0);
    assert_eq!(t.scale_of(1), 1.25);
    assert!(t.rotation.is_none());
    assert_eq!(t.tags_of(0).unwrap(), None);
    assert_eq!(t.tags_of(1).unwrap(), Some(vec![("name".into(), Some("가".into()))]));
    assert_eq!(t.row(1).unwrap().tags, Some(vec![("name".into(), Some("가".into()))]));

    let bare = PlacementTable::new(packet(encode_placements(&rows[..1]).unwrap())).unwrap();
    assert!(bare.scale.is_none());
    assert_eq!(bare.tags_of(0).unwrap(), None);
}

#[test]
fn placement_a_full_rotation_is_four_per_row() {
    let t = PlacementTable::new(packet(
        encode_placements(&[Placement {
            // The TypeScript's 0.7071, as written there; clippy would read it as
            // FRAC_1_SQRT_2, which it is not.
            #[allow(clippy::approx_constant)]
            rotation: Some([0.0, 0.0, 0.7071, 0.7071]),
            ..placement(1, 0.0, 0.0, 0.0, 1.0, 0.0, 1)
        }])
        .unwrap(),
    ))
    .unwrap();
    #[allow(clippy::approx_constant)]
    let q = 0.7071;
    assert!((t.rotation.as_ref().unwrap()[2] as f64 - q).abs() < 1e-6);
    assert_eq!(t.row(0).unwrap().rotation.unwrap().len(), 4);
}

#[allow(clippy::too_many_arguments)]
fn entry(id: u32, kind: PaletteKind, name: &str, model_hash: u64, model_key: &str, extent: [f64; 3], license: &str, author: &str) -> PaletteEntry {
    PaletteEntry {
        id,
        kind,
        name: name.into(),
        model_hash,
        model_key: model_key.into(),
        skin_key: None,
        extent,
        size: 1.0,
        license: license.into(),
        author: author.into(),
        color: None,
        group: None,
    }
}

#[test]
fn palette_rows_round_trip_and_are_found_by_id() {
    let rows = [
        entry(1, PaletteKind::Cube, "white", 0, "", [1.0, 1.0, 1.0], "", ""),
        entry(USER_ID_BASE + 5, PaletteKind::Decor, "pine", fnv1a64_str("sha256:abc"), "asset/abc", [1.18, 1.33, 2.3], "CC0", "Kenney"),
    ];
    let p = PaletteTable::new(packet(encode_palette(&rows).unwrap())).unwrap();
    assert_eq!(p.count, 2);
    assert_eq!(p.row_of(USER_ID_BASE + 5), 1);
    assert_eq!(p.row_of(99), -1);
    assert_eq!(p.kind_of(1).unwrap(), PaletteKind::Decor);
    assert_eq!(p.name_of(1).unwrap(), "pine");
    assert_eq!(p.model_key_of(1).unwrap(), "asset/abc");
    assert!((p.extent_max(1) as f64 - 2.3).abs() < 1e-6);
    assert_eq!(p.license_of(1).unwrap(), "CC0");
    assert_eq!(p.entry(0).unwrap().kind, PaletteKind::Cube);
}

#[test]
fn shipped_palette_ids_are_unique_placeable_values_keep_their_numbers_every_model_row_has_an_extent() {
    let p = shipped_palette();
    let mut seen = HashSet::new();
    for i in 0..p.count as usize {
        assert!(seen.insert(p.id[i]), "duplicate palette id {}", p.id[i]);
    }
    let row = |id: u32| p.row_of(id) as u32;
    assert_eq!(p.name_of(row(cube_types::block::red as u32)).unwrap(), "block.red");
    assert_eq!(
        p.color_of(row(cube_types::block::red as u32)).unwrap(),
        cube_colors.get("block").unwrap()[&(cube_types::block::red as u32)].unwrap().hex
    );
    assert_eq!(p.name_of(row(decor_types::tree as u32)).unwrap(), "tree");
    assert_eq!(p.kind_of(row(CHANNEL_ID_BASE.land + cube_types::land::sand as u32)).unwrap(), PaletteKind::Ground);
    let trees = p.rows_of_group("tree").unwrap();
    assert_eq!(trees.len(), 1 + 9);
    for r in &trees {
        assert!(p.extent_max(*r) > 2.0);
    }
    assert_eq!(p.rows_of_group("car").unwrap().len(), 5);
    // Every model row names its bytes and its skin by content, and the key
    // is what the client fetches.
    let is_key = |s: &str| s.len() == 6 + 64 && s.starts_with("asset/") && s[6..].bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    let people = p.rows_of_group("person").unwrap();
    let pets = p.rows_of_group("pet").unwrap();
    for r in trees.iter().chain(people.iter()).chain(pets.iter()) {
        if p.model_key_of(*r).unwrap().is_empty() {
            continue;
        }
        assert!(is_key(&p.model_key_of(*r).unwrap()), "{}", p.model_key_of(*r).unwrap());
        assert!(is_key(&p.skin_key_of(*r).unwrap()), "{}", p.skin_key_of(*r).unwrap());
        assert_ne!(p.model_hash[*r as usize], 0);
    }
    assert_eq!(people.len(), 12);
    assert_eq!(pets.len(), 24);
    assert_eq!(p.kind_of(pets[0]).unwrap(), PaletteKind::Actor);
}

#[test]
fn shipped_textures_a_land_top_and_side_a_block_top_and_no_block_side() {
    assert!(shipped_texture_key("land", "BARE", CubeFace::Top).unwrap().starts_with("asset/"));
    assert!(shipped_texture_key("land", "BARE", CubeFace::Side).unwrap().starts_with("asset/"));
    assert!(shipped_texture_key("block", "white", CubeFace::Top).unwrap().starts_with("asset/"));
    assert_eq!(shipped_texture_key("block", "white", CubeFace::Side), None);
    assert_eq!(shipped_texture_key("water", "RIVER", CubeFace::Side), None);
    // The sheet holds every tile the keys name, each at a cell-aligned origin.
    assert!(SHIPPED_SHEET.key.starts_with("asset/"));
    let tile = shipped_texture_tile("land", "BARE", CubeFace::Top).unwrap();
    assert_eq!(tile.x % SHIPPED_SHEET.cell, 0);
    assert_eq!(tile.y % SHIPPED_SHEET.cell, 0);
    assert_eq!(shipped_texture_tile("block", "white", CubeFace::Side), None);
}
