// pxl_shared/record/shipped.rs

//! The palette this build ships: every cube type, every decor kind and its
//! models, and every character, as one record.
//!
//! Ids. A placeable value — a block colour, AIR, a decor kind — keeps the
//! number the object store already writes (spec/cell), which is under 256.
//! A terrain channel's types overlap each other by number, since each is
//! its own lane, so here each channel is offset into its own range. A
//! model is a row from MODEL_ID_BASE, one per glb, grouped by the scatter
//! kind that uses it; a kind's variant order is this file's row order. A
//! character is a row from ACTOR_ID_BASE, grouped "person" or "pet".
//!
//! What a model *is* — its bytes, its bounds — comes from ingested.rs, the
//! table apps/api/src/bake/asset/tool/ingest_asset.ts generates from the
//! files under asset/: the bucket key is the sha256 of the glb and the
//! extent is measured from it. This file only says which models there are,
//! in what order, under which kind, and with which colormap.

use std::sync::LazyLock;

use xpute_core::kit::hash::fnv1a64_str;

use crate::spec::cell::r#type::ChannelColors;
use crate::spec::cell::{cube_colors, cube_id_of, cube_types, decor_types};

use super::ingested::{IngestedAsset, INGESTED};
use super::palette::{encode_palette, PaletteEntry, PaletteKind, PaletteTable};
use super::table::RecordSource;

/// Terrain channels' types, offset so a global id is unique.
pub struct ChannelIdBase {
    pub land: u32,
    pub water: u32,
    pub structure: u32,
    pub transport: u32,
}

pub const CHANNEL_ID_BASE: ChannelIdBase = ChannelIdBase {
    land: 0x100,
    water: 0x200,
    structure: 0x300,
    transport: 0x400,
};
pub const MODEL_ID_BASE: u32 = 0x1000;
pub const ACTOR_ID_BASE: u32 = 0x2000;

const KENNEY_LICENSE: &str = "CC0-1.0";
const KENNEY_AUTHOR: &str = "Kenney";

/// A pack's models share one colormap.
#[derive(Clone, Copy)]
struct Pack {
    dir: &'static str,
    skin: &'static str,
}

const FANTASY_TOWN: Pack = Pack {
    dir: "decor/kenney_fantasy_town",
    skin: "colormap.png",
};
const GRAVEYARD: Pack = Pack {
    dir: "decor/kenney_graveyard_kit",
    skin: "colormap.png",
};
const CAR: Pack = Pack {
    dir: "decor/kenney_car_kit",
    skin: "colormap.png",
};
const TRAIN: Pack = Pack {
    dir: "decor/kenney_train_kit",
    skin: "colormap.png",
};
const WATERCRAFT: Pack = Pack {
    dir: "decor/kenney_watercraft_pack",
    skin: "colormap.png",
};
const MINI_CHARACTERS: Pack = Pack {
    dir: "actor/kenney_mini_characters",
    skin: "colormap.png",
};
const CUBE_PETS: Pack = Pack {
    dir: "actor/kenney_cube_pets",
    skin: "colormap.png",
};

/// A model: its pack and its file, in the order that is the kind's variant
/// order.
type Model = (Pack, &'static str);

const MODELS: &[(&str, &[Model])] = &[
    (
        "tree",
        &[
            (FANTASY_TOWN, "tree.glb"),
            (FANTASY_TOWN, "tree-high.glb"),
            (FANTASY_TOWN, "tree-crooked.glb"),
            (FANTASY_TOWN, "tree-high-round.glb"),
            (FANTASY_TOWN, "tree-high-crooked.glb"),
            (GRAVEYARD, "pine.glb"),
            (GRAVEYARD, "pine-crooked.glb"),
            (GRAVEYARD, "pine-fall.glb"),
            (GRAVEYARD, "pine-fall-crooked.glb"),
        ],
    ),
    ("car", &[(CAR, "sedan.glb"), (CAR, "suv.glb"), (CAR, "van.glb"), (CAR, "hatchback-sports.glb"), (CAR, "taxi.glb")]),
    (
        "train",
        &[
            (TRAIN, "train-locomotive-a.glb"),
            (TRAIN, "train-locomotive-b.glb"),
            (TRAIN, "train-diesel-a.glb"),
            (TRAIN, "train-electric-city-a.glb"),
        ],
    ),
    (
        "boat",
        &[
            (WATERCRAFT, "boat-row-small.glb"),
            (WATERCRAFT, "boat-fishing-small.glb"),
            (WATERCRAFT, "boat-sail-a.glb"),
            (WATERCRAFT, "boat-speed-a.glb"),
        ],
    ),
];

const PETS: [&str; 24] = [
    "beaver",
    "bee",
    "bunny",
    "cat",
    "caterpillar",
    "chick",
    "cow",
    "crab",
    "deer",
    "dog",
    "elephant",
    "fish",
    "fox",
    "giraffe",
    "hog",
    "koala",
    "lion",
    "monkey",
    "panda",
    "parrot",
    "penguin",
    "pig",
    "polar",
    "tiger",
];

/// One kind of character: its group, pack, size and (key, file)s.
type Actors = (&'static str, Pack, f64, Vec<(String, String)>);

/// The characters, by the key the settings store persists; a person is
/// one cell tall, a pet a quarter.
#[allow(non_snake_case)]
fn ACTORS() -> Vec<Actors> {
    let person = [
        ("female-1", "female-a.glb"),
        ("female-2", "female-b.glb"),
        ("female-3", "female-c.glb"),
        ("female-4", "female-d.glb"),
        ("female-5", "female-e.glb"),
        ("female-6", "female-f.glb"),
        ("male-1", "male-a.glb"),
        ("male-2", "male-b.glb"),
        ("male-3", "male-c.glb"),
        ("male-4", "male-d.glb"),
        ("male-5", "male-e.glb"),
        ("male-6", "male-f.glb"),
    ];
    vec![
        ("person", MINI_CHARACTERS, 1.0, person.iter().map(|(k, f)| (k.to_string(), f.to_string())).collect()),
        ("pet", CUBE_PETS, 0.25, PETS.iter().map(|name| (name.to_string(), format!("{name}.glb"))).collect()),
    ]
}

fn ingested(path: &str) -> IngestedAsset {
    match INGESTED.get(path) {
        Some(row) => *row,
        None => panic!("palette: {path} is not in ingested.rs — run the asset ingest"),
    }
}

fn model_row(id: u32, kind: PaletteKind, name: String, group: &str, pack: Pack, file: &str, size: f64) -> PaletteEntry {
    let model = ingested(&format!("{}/{file}", pack.dir));
    let skin = ingested(&format!("{}/{}", pack.dir, pack.skin));
    PaletteEntry {
        id,
        kind,
        name,
        model_hash: fnv1a64_str(model.key),
        model_key: model.key.to_string(),
        skin_key: Some(skin.key.to_string()),
        extent: model.extent.unwrap_or([0.0, 0.0, 0.0]),
        size,
        license: KENNEY_LICENSE.to_string(),
        author: KENNEY_AUTHOR.to_string(),
        color: None,
        group: Some(group.to_string()),
    }
}

fn cube_rows() -> Vec<PaletteEntry> {
    let mut rows = Vec::new();
    let mut channel = |name: &str, base: u32, kind: PaletteKind, types: &[(&str, u8)], colors: &ChannelColors| {
        for (type_name, local) in types {
            let local = *local as u32;
            rows.push(PaletteEntry {
                id: base + local,
                kind,
                name: cube_id_of(name, type_name),
                model_hash: 0,
                model_key: String::new(),
                skin_key: None,
                extent: [1.0, 1.0, 1.0],
                size: 1.0,
                license: String::new(),
                author: String::new(),
                color: colors.get(&local).copied().flatten().map(|s| s.hex.to_string()),
                group: None,
            });
        }
    };
    let colors = |name: &str| cube_colors.get(name).unwrap();
    channel("common", 0, PaletteKind::Cube, cube_types::common::ENTRIES, colors("common"));
    channel("block", 0, PaletteKind::Cube, cube_types::block::ENTRIES, colors("block"));
    channel("land", CHANNEL_ID_BASE.land, PaletteKind::Ground, cube_types::land::ENTRIES, colors("land"));
    channel("water", CHANNEL_ID_BASE.water, PaletteKind::Ground, cube_types::water::ENTRIES, colors("water"));
    channel("structure", CHANNEL_ID_BASE.structure, PaletteKind::Ground, cube_types::structure::ENTRIES, colors("structure"));
    channel("transport", CHANNEL_ID_BASE.transport, PaletteKind::Ground, cube_types::transport::ENTRIES, colors("transport"));
    rows
}

fn decor_rows() -> Vec<PaletteEntry> {
    let mut rows = Vec::new();
    // The placeable decor kinds, by the number the object store writes; a
    // kind's extent is the largest over its models.
    for (name, id) in decor_types::ENTRIES {
        let mut extent = [0.0f64; 3];
        let models = MODELS.iter().find(|(group, _)| group == name).map_or(&[][..], |(_, m)| m);
        for (pack, file) in models {
            let model = ingested(&format!("{}/{file}", pack.dir)).extent.unwrap_or([0.0, 0.0, 0.0]);
            for i in 0..3 {
                // Math.max: a NaN extent stays NaN.
                extent[i] = if extent[i].is_nan() || model[i].is_nan() { f64::NAN } else { extent[i].max(model[i]) };
            }
        }
        rows.push(PaletteEntry {
            id: *id as u32,
            kind: PaletteKind::Decor,
            name: name.to_string(),
            model_hash: 0,
            model_key: String::new(),
            skin_key: None,
            extent,
            size: 1.0,
            license: KENNEY_LICENSE.to_string(),
            author: KENNEY_AUTHOR.to_string(),
            color: None,
            group: Some(name.to_string()),
        });
    }
    let mut id = MODEL_ID_BASE;
    for (group, models) in MODELS {
        for (pack, file) in models.iter() {
            rows.push(model_row(id, PaletteKind::Decor, format!("{group}:{}", file.replacen(".glb", "", 1)), group, *pack, file, 1.0));
            id += 1;
        }
    }
    rows
}

fn actor_rows() -> Vec<PaletteEntry> {
    let mut rows = Vec::new();
    let mut id = ACTOR_ID_BASE;
    for (group, pack, size, files) in ACTORS() {
        for (key, file) in files {
            rows.push(model_row(id, PaletteKind::Actor, key, group, pack, &file, size));
            id += 1;
        }
    }
    rows
}

pub static SHIPPED_PALETTE: LazyLock<Vec<PaletteEntry>> = LazyLock::new(|| {
    let mut rows = cube_rows();
    rows.extend(decor_rows());
    rows.extend(actor_rows());
    rows
});

thread_local! {
    static SHIPPED: PaletteTable = PaletteTable::new(RecordSource::Packet(encode_palette(&SHIPPED_PALETTE).unwrap())).unwrap();
}

/// The shipped palette as the record, encoded once. A table holds views, so
/// the one it hands back is a clone over the same packet.
pub fn shipped_palette() -> PaletteTable {
    SHIPPED.with(|p| p.clone())
}

#[cfg(test)]
#[path = "shipped.test.rs"]
mod test;
