// pxl_shared/spec/cell/cube.rs
// [Layer: cube] The one numeric-id/color catalog for every placeable cell
// type — both the baked 2.5D terrain raster's own four channels
// (land/water/structure/transport, filled by the bake pipeline, one
// independent value per channel per cell) and the flat "block" colors a
// user paints directly. Every channel is a name-to-number table of the same
// shape, whether the bake pipeline treats it as real map data or a user's
// palette.
//
// `common` is a shared registry of sentinel values — VOID (nothing painted),
// AIR (an explicit object-layer erase) and UNKNOWN (missing or invalid data)
// — and the other channels do not spread its keys into their own. Only
// land/water/structure/transport can hold VOID or UNKNOWN, those being raw
// bytes a baked raster legitimately produces; AIR means something only
// within the object-placement encoding (mod.rs), never as a terrain value,
// so a channel that can never produce it does not list it.
//
// mod.rs is where a placed cube's own value is derived from these channels,
// banding the terrain channels into disjoint ranges — a real terrain cell
// can hold one value from every channel at once, where a placed cube is
// exactly one of them. Nothing here needs to know that; this file is cell
// definitions and nothing else.

#![allow(non_upper_case_globals, non_snake_case)]

use std::collections::BTreeMap;
use std::sync::LazyLock;

use super::color;
pub use super::cube_type::cube_types;
use super::r#type::{build_colors_from_json, ChannelColors, Channels, Colors, Swatch};

// ============================================================================
// Color Definitions — the palette's colour column (see ./color.rs)
// ============================================================================

/// VOID/UNKNOWN's own shared colors, seeded into every raster channel below
/// (land/water/structure/transport) so a real cell holding either sentinel
/// still resolves to *some* real color — VOID to nothing (None, the same
/// "don't render this" signal build_colors_from_json already uses
/// elsewhere) and UNKNOWN to a visible neutral gray, so an unresolved cell
/// reads as "we don't know" instead of silently showing as whatever
/// happened to render last. AIR is deliberately not included: no raster
/// channel can ever actually hold that byte (see cube_types' own top
/// comment), so seeding a color for it here would be dead data.
fn RASTER_SEED() -> ChannelColors {
    BTreeMap::from([(cube_types::common::void as u32, None), (cube_types::common::unknown as u32, Some(Swatch::new("#D9D0CC", 0xd9d0cc)))])
}

pub static cube_colors: LazyLock<Colors> = LazyLock::new(|| {
    // build_colors_from_json walks every channel of the file it is handed,
    // whatever `types` lists — so the raster call takes the file without
    // its block section, or it would refuse "unknown channel block".
    let file = &color::default;
    let raster = super::r#type::ColorJsonFile {
        _meta: None,
        channels: &file.channels[..file.channels.len() - 1],
    };
    let block = super::r#type::ColorJsonFile {
        _meta: None,
        channels: &file.channels[file.channels.len() - 1..],
    };
    assert_eq!(block.channels[0].0, "block", "color.rs: block is the file's last channel");

    let raster_types: &Channels = &[
        ("land", cube_types::land::ENTRIES),
        ("water", cube_types::water::ENTRIES),
        ("structure", cube_types::structure::ENTRIES),
        ("transport", cube_types::transport::ENTRIES),
    ];
    let common = BTreeMap::from([
        (cube_types::common::void as u32, None),
        (cube_types::common::air as u32, None),
        (cube_types::common::unknown as u32, Some(Swatch::new("#D9D0CC", 0xd9d0cc))),
    ]);
    let mut out = Colors(vec![("common", common)]);
    out.0.extend(build_colors_from_json(raster_types, &raster, &RASTER_SEED()).0);
    // No seed at all for block — every one of its keys is a real color
    // straight from the palette file, nothing to pre-populate.
    out.0.extend(build_colors_from_json(&[("block", cube_types::block::ENTRIES)], &block, &BTreeMap::new()).0);
    out
});

/// A real, user-paintable flat color — every key of cube_types::block, no
/// exclusion needed (block carries no sentinels of its own any more).
pub type CubeColorId = &'static str;
