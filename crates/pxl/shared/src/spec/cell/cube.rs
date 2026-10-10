// pxl_shared/spec/cell/cube.rs
// [Layer: cube] The one numeric-id/color catalog for every placeable cell
// type — both the baked 2.5D terrain raster's own four channels
// (land/water/structure/transport, filled by the bake pipeline, one
// independent value per channel per cell) and the flat "block" colors a
// user paints directly. These used to live in two near-identical files
// (terrain.ts + objects.ts); there's no real reason for that (2026-09-06) —
// every channel below is just a Record<name, number>, same shape, whether
// the bake pipeline treats it as real map data or a user-placed palette.
// mod.ts is the one place a placed cube's own single sparse-tree byte gets
// derived FROM these channels (banding land/water/structure/transport into
// disjoint ranges, since a real terrain cell can hold a value from every
// channel at once but a placed cube is exactly one of them) — nothing here
// needs to know about that; this file stays pure cell-definition data.
//
// common.ts used to hold VOID/UNKNOWN as their own separate file/export —
// folded in here as cube_types.common (2026-09-06). Unlike an earlier pass
// at this same merge, land/water/structure/transport/block do NOT spread
// common's keys into their own — common is a shared *registry* of sentinel
// values (VOID: nothing painted; AIR: explicit object-layer erase; UNKNOWN:
// missing/invalid data), not something every channel actually needs. Only
// land/water/structure/transport ever really see VOID/UNKNOWN (real raw
// bytes a baked raster can legitimately hold); AIR only ever means anything
// within the object-placement encoding (mod.ts), never as a raw terrain
// value — spreading it into land/water/etc anyway would've been a sentinel
// that channel can never actually produce, present only for symmetry.
// shader_meta.ts's get_color_meta is the one place that still needs to
// treat a channel's real keys and common's shared ones as one set (for
// VOID/UNKNOWN specifically) — it merges the two explicitly there instead
// of relying on the data itself already being merged.

#![allow(non_upper_case_globals, non_snake_case)]

use std::collections::BTreeMap;
use std::sync::LazyLock;

use super::color;
use super::r#type::{build_colors_from_json, ChannelColors, Channels, Colors};

/// The catalog: a module per channel, each type a constant, and `ENTRIES`
/// the channel as the object's keys walk it. `CHANNELS` is `cube_types`
/// itself, walked.
pub mod cube_types {
    pub mod common {
        /// unset — no data painted at this cell for this channel; also the
        /// object-placement sparse tree's own absence marker (a cell with
        /// genuinely no entry at all reads back as this same 0, since a sparse
        /// tree's own "nothing stored here" and a dense raster's own "nothing
        /// painted here" are the same concept either way).
        pub const VOID: u8 = 0x00;
        /// explicit object-layer air: retain this leaf, erase lower layers —
        /// a real, present leaf, unlike VOID above. Only ever a real value
        /// within the object-placement encoding (mod.ts) — no baked terrain
        /// channel below can ever actually hold this byte, it's registered
        /// here purely as a shared constant.
        pub const AIR: u8 = 0x80;
        /// missing / invalid / unresolved data
        pub const UNKNOWN: u8 = 0xff;

        pub const ENTRIES: &[(&str, u8)] = &[("VOID", VOID), ("AIR", AIR), ("UNKNOWN", UNKNOWN)];
    }

    pub mod land {
        /// bare ground
        pub const BARE: u8 = 1;
        /// sea water (MSB-partitioned)
        pub const SEA: u8 = 1 | 0x80;

        // ---- natural ----

        /// deep green / forest / mountain
        pub const GREEN_DEEP: u8 = 10;
        /// light green / park / grass
        pub const GREEN_LIGHT: u8 = 11;
        /// sand / rock
        pub const SAND: u8 = 12;

        // ---- landuse ----

        /// residential area
        pub const RESIDENTIAL: u8 = 20;
        /// commercial area
        pub const COMMERCIAL: u8 = 21;
        /// industrial area
        pub const INDUSTRIAL: u8 = 22;
        /// sports area
        pub const SPORTS: u8 = 23;
        /// education area
        pub const EDUCATION: u8 = 24;
        /// medical area
        pub const MEDICAL: u8 = 25;
        /// public area
        pub const PUBLIC: u8 = 26;

        // ---- aeroway areas ----

        /// apron
        pub const APRON: u8 = 30;
        /// helipad
        pub const HELIPAD: u8 = 31;

        pub const ENTRIES: &[(&str, u8)] = &[
            ("BARE", BARE),
            ("SEA", SEA),
            ("GREEN_DEEP", GREEN_DEEP),
            ("GREEN_LIGHT", GREEN_LIGHT),
            ("SAND", SAND),
            ("RESIDENTIAL", RESIDENTIAL),
            ("COMMERCIAL", COMMERCIAL),
            ("INDUSTRIAL", INDUSTRIAL),
            ("SPORTS", SPORTS),
            ("EDUCATION", EDUCATION),
            ("MEDICAL", MEDICAL),
            ("PUBLIC", PUBLIC),
            ("APRON", APRON),
            ("HELIPAD", HELIPAD),
        ];
    }

    pub mod water {
        /// flowing water
        pub const RIVER: u8 = 1;
        /// still water
        pub const LAKE: u8 = 2;
        /// wetland
        pub const WETLAND: u8 = 3;

        pub const ENTRIES: &[(&str, u8)] = &[("RIVER", RIVER), ("LAKE", LAKE), ("WETLAND", WETLAND)];
    }

    pub mod structure {
        // ---- buildings ----

        /// generic building
        pub const BUILDING: u8 = 1;
        /// station / terminal building
        pub const BUILDING_STATION: u8 = 2;
        /// education building
        pub const BUILDING_EDUCATION: u8 = 3;
        /// medical building
        pub const BUILDING_MEDICAL: u8 = 4;
        /// public building
        pub const BUILDING_PUBLIC: u8 = 5;

        pub const ENTRIES: &[(&str, u8)] = &[
            ("BUILDING", BUILDING),
            ("BUILDING_STATION", BUILDING_STATION),
            ("BUILDING_EDUCATION", BUILDING_EDUCATION),
            ("BUILDING_MEDICAL", BUILDING_MEDICAL),
            ("BUILDING_PUBLIC", BUILDING_PUBLIC),
        ];
    }

    pub mod transport {
        // ---- roads ----

        /// default road
        pub const ROAD_DEFAULT: u8 = 1;
        /// motorway
        pub const ROAD_MOTORWAY: u8 = 2;
        /// primary road
        pub const ROAD_PRIMARY: u8 = 3;
        /// secondary road
        pub const ROAD_SECONDARY: u8 = 4;
        /// pedestrian path
        pub const PEDESTRIAN: u8 = 5;
        /// parking area
        pub const PARKING: u8 = 6;

        // ---- rails ----

        /// main rail
        pub const RAIL_MAIN: u8 = 10;
        /// subway rail
        pub const RAIL_SUBWAY: u8 = 11;

        // ---- aeroway lines ----

        /// runway
        pub const RUNWAY: u8 = 20;
        /// taxiway
        pub const TAXIWAY: u8 = 21;

        pub const ENTRIES: &[(&str, u8)] = &[
            ("ROAD_DEFAULT", ROAD_DEFAULT),
            ("ROAD_MOTORWAY", ROAD_MOTORWAY),
            ("ROAD_PRIMARY", ROAD_PRIMARY),
            ("ROAD_SECONDARY", ROAD_SECONDARY),
            ("PEDESTRIAN", PEDESTRIAN),
            ("PARKING", PARKING),
            ("RAIL_MAIN", RAIL_MAIN),
            ("RAIL_SUBWAY", RAIL_SUBWAY),
            ("RUNWAY", RUNWAY),
            ("TAXIWAY", TAXIWAY),
        ];
    }

    // Flat user-painted colors only — no sentinels of its own anymore.
    // cube_types.common.VOID (sparse-tree absence) and .AIR (explicit erase)
    // already cover every non-color case a placed cube's own byte can mean;
    // every real key here is a real, pickable color.
    pub mod block {
        pub const white: u8 = 130;
        pub const grey: u8 = 131;
        pub const dark_grey: u8 = 132;
        pub const black: u8 = 133;
        pub const red: u8 = 134;
        pub const orange: u8 = 135;
        pub const brown: u8 = 136;
        pub const yellow: u8 = 137;
        pub const green: u8 = 138;
        pub const dark_green: u8 = 139;
        pub const cyan: u8 = 140;
        pub const blue: u8 = 141;
        pub const violet: u8 = 142;
        pub const magenta: u8 = 143;
        pub const pink: u8 = 144;

        pub const ENTRIES: &[(&str, u8)] = &[
            ("white", white),
            ("grey", grey),
            ("dark_grey", dark_grey),
            ("black", black),
            ("red", red),
            ("orange", orange),
            ("brown", brown),
            ("yellow", yellow),
            ("green", green),
            ("dark_green", dark_green),
            ("cyan", cyan),
            ("blue", blue),
            ("violet", violet),
            ("magenta", magenta),
            ("pink", pink),
        ];
    }

    pub const CHANNELS: &super::Channels = &[
        ("common", common::ENTRIES),
        ("land", land::ENTRIES),
        ("water", water::ENTRIES),
        ("structure", structure::ENTRIES),
        ("transport", transport::ENTRIES),
        ("block", block::ENTRIES),
    ];
}

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
    BTreeMap::from([(cube_types::common::VOID as u32, None), (cube_types::common::UNKNOWN as u32, Some("#D9D0CC"))])
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
        (cube_types::common::VOID as u32, None),
        (cube_types::common::AIR as u32, None),
        (cube_types::common::UNKNOWN as u32, Some("#D9D0CC")),
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
