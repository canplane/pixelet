// pxl-spec/bake/format.rs

//! Terrain format helpers for baked page payloads.
//!
//! The wire field order is defined here.
//!
//! Layer model:
//! - L3 page = terrain transport / storage unit
//! - terrain = static terrain layer carried by that page
//! - ground  = categorical 2D terrain channels
//! - ele     = elevation raster payload for that terrain layer
//!
//! In other words:
//!   L3 page
//!     -> terrain layer
//!          -> ground
//!          -> ele
//!
//! raw wire   = [ele, buildings, segments, base, coasts]
//!   buildings = [[gers_id, subtype, class, height, num_floors, geom_wkb], ...]
//!   segments  = [[gers_id, subtype, class, road, geom_wkb], ...]
//!   base      = [[gers_id, source, subtype, class, geom_wkb], ...]  (source: land_use|water|land|infrastructure)
//!   coasts    = [[id, geom_wkb, covers_page], ...]
//!
//! baked wire = [ele, ground]
//!   ground = [land, water, structure, transport]
//!
//! each ground channel a sparse tree, encoded as words

use std::hash::Hasher;

use fnv::FnvHasher;
use xpute_core::codec::json::{parse_str_map, stringify_str_map, StrMap};
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{encoder, BranchCursor, NodeView, TreeEncoderOptions, TreeReader, TreeView};

use super::model::feature::Poi;

// ---- DB ----

pub const BUILDING_ROW_KEYS: [&str; 6] = ["gers_id", "subtype", "class", "height", "num_floors", "geom_wkb"];
pub struct BuildingRow {
    pub gers_id: String,
    pub subtype: Option<String>,
    pub class: Option<String>,
    pub height: Option<f64>,
    pub num_floors: Option<f64>,
    pub geom_wkb: Box<[u8]>, // WKB
}

pub const SEGMENT_ROW_KEYS: [&str; 5] = ["gers_id", "subtype", "class", "road", "geom_wkb"];
pub struct SegmentRow {
    pub gers_id: String,
    pub subtype: String,
    pub class: Option<String>,
    pub road: Option<String>, // raw JSON text (::text of the road JSONB column)
    pub geom_wkb: Box<[u8]>,  // WKB
}

pub const BASE_ROW_KEYS: [&str; 5] = ["gers_id", "source", "subtype", "class", "geom_wkb"];
pub struct BaseRow {
    pub gers_id: String,
    pub source: String, // "land_use" | "water" | "land" | "infrastructure"
    pub subtype: Option<String>,
    pub class: Option<String>,
    pub geom_wkb: Box<[u8]>, // WKB
}

pub const COAST_ROW_KEYS: [&str; 3] = ["id", "geom_wkb", "covers_page"];
pub struct CoastRow {
    pub id: String,
    pub geom_wkb: Box<[u8]>, // WKB
    pub covers_page: bool,
}

// ---- Ground ----

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GroundChannelKey {
    LAND,
    WATER,
    STRUCTURE,
    TRANSPORT,
}

impl GroundChannelKey {
    /// The key's string on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            GroundChannelKey::LAND => "land",
            GroundChannelKey::WATER => "water",
            GroundChannelKey::STRUCTURE => "structure",
            GroundChannelKey::TRANSPORT => "transport",
        }
    }
}

pub const GROUND_CH_KEYS: [GroundChannelKey; 4] = [GroundChannelKey::LAND, GroundChannelKey::WATER, GroundChannelKey::STRUCTURE, GroundChannelKey::TRANSPORT];

/// `Record<GroundChannelKey, bytes>`. A decoded page owns its channels; a
/// resident one reads the blocks it was written into.
#[derive(Clone, Debug)]
pub struct Ground {
    pub land: std::borrow::Cow<'static, [u8]>,
    pub water: std::borrow::Cow<'static, [u8]>,
    pub structure: std::borrow::Cow<'static, [u8]>,
    pub transport: std::borrow::Cow<'static, [u8]>,
}

impl Ground {
    /// `ground[key]`.
    pub fn get(&self, key: GroundChannelKey) -> &[u8] {
        match key {
            GroundChannelKey::LAND => &self.land,
            GroundChannelKey::WATER => &self.water,
            GroundChannelKey::STRUCTURE => &self.structure,
            GroundChannelKey::TRANSPORT => &self.transport,
        }
    }
}

/// Target ground channel + cell type resolved by a provider's own policy for one normalized feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellConfig {
    pub cell_type: u8,
    pub ch_key: GroundChannelKey,
}

// ---- Page ----

/// Static terrain payload carried by one L3 page.
#[derive(Clone, Debug)]
pub struct Terrain {
    pub ground: Ground,
    pub ele: std::borrow::Cow<'static, [i16]>,
}

/// Decoded page payload with the baked terrain layer attached.
#[derive(Clone, Debug)]
pub struct Tile {
    pub terrain: Terrain,
}

// ---- DTO ----

// { ele, ground } -> [ele, [land, water, structure, transport]]
pub fn encode_terrain(ele: &[i16], ground: &Ground) -> Result<Vec<u8>, MarshalError> {
    // Final baked terrain format:
    // - baked = [ele, [land, water, structure, transport]]
    // - each ground channel is an st.encode(...) result
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| {
        b.i16_array(Some(ele)).branch(Some(&mut |g| {
            g.u8_array(Some(&ground.land))
                .u8_array(Some(&ground.water))
                .u8_array(Some(&ground.structure))
                .u8_array(Some(&ground.transport));
        }));
    }));
    encoder().encode(&t, TreeEncoderOptions::default())
}

// [ele, [land, water, structure, transport]] -> { ele, ground }
pub fn decode_terrain(pkt: &[u8]) -> Result<Terrain, MarshalError> {
    let root = TreeReader::new(pkt)?.read_branch()?;
    let ele = root.at(0)?.get_array::<i16>()?.to_vec().into();
    let ground = root.at(1)?.as_branch()?;

    // Copied out: a decoded page outlives the packet it came from.
    Ok(Terrain {
        ele,
        ground: Ground {
            land: ground.at(0)?.get_array::<u8>()?.to_vec().into(),
            water: ground.at(1)?.get_array::<u8>()?.to_vec().into(),
            structure: ground.at(2)?.get_array::<u8>()?.to_vec().into(),
            transport: ground.at(3)?.get_array::<u8>()?.to_vec().into(),
        },
    })
}

// ---- POI page ----
//
// One L3 page's places as a record in lanes, read in place:
//
//   [x, y, z, category, rank, id_hash, [categories...], [ids...], [names...], [tags...]]
//
//   x, y, z    f64 per record — page-local centre, real units
//   category   u16 per record — index into `categories`
//   rank       u16 per record — the provider's importance rank
//   id_hash    u64 per record — FNV-1a of the id's UTF-8, what a client keys a place
//              by without holding the string
//   categories str per distinct category, in first-seen order
//   ids, names, tags  str per record; tags as the JSON of the record's tags
//
// The first six are what the client's index and placement read for every
// record every rebuild, and they are typed-array leaves the client views
// without a copy. The rest are read one record at a time — a badge's name
// when it is drawn, a record's tags when it is opened — and never all at
// once. This is the line issue/20260912-linear-memory-what-is-left.md
// draws: the lanes are the canvas's, the strings are read on demand.

pub fn encode_poi_page(pois: &[Poi]) -> Result<Vec<u8>, MarshalError> {
    let n = pois.len();
    let (mut x, mut y, mut z) = (vec![0f64; n], vec![0f64; n], vec![0f64; n]);
    let (mut category, mut rank) = (vec![0u16; n], vec![0u16; n]);
    let mut id_hash = vec![0u64; n];
    let mut categories: Vec<&str> = Vec::new();
    for (i, poi) in pois.iter().enumerate() {
        x[i] = poi.center[0];
        y[i] = poi.center[1];
        z[i] = poi.center[2];
        let c = match categories.iter().position(|have| *have == poi.category) {
            Some(c) => c,
            None => {
                categories.push(&poi.category);
                categories.len() - 1
            }
        };
        category[i] = c as u16;
        rank[i] = poi.rank.min(0xffff) as u16;
        let mut h = FnvHasher::default();
        h.write(poi.id.as_bytes());
        id_hash[i] = h.finish();
    }
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| {
        b.f64_array(Some(&x))
            .f64_array(Some(&y))
            .f64_array(Some(&z))
            .u16_array(Some(&category))
            .u16_array(Some(&rank))
            .u64_array(Some(&id_hash));
        b.branch(Some(&mut |s| {
            for c in &categories {
                s.str(Some(c));
            }
        }));
        b.branch(Some(&mut |s| {
            for poi in pois {
                s.str(Some(&poi.id));
            }
        }));
        b.branch(Some(&mut |s| {
            for poi in pois {
                s.str(Some(&poi.name));
            }
        }));
        b.branch(Some(&mut |s| {
            for poi in pois {
                s.str(Some(&stringify_str_map(&poi.tags)));
            }
        }));
    }));
    encoder().encode(
        &t,
        TreeEncoderOptions {
            init_cap: Some(1 << 16),
            ..Default::default()
        },
    )
}

/// A POI page opened over its packet. The lanes are views into the packet;
/// the strings are cursors read when asked. `pkt` must sit at a word-aligned
/// offset of its buffer, which is what lets the f64 lanes be viewed at all.
#[derive(Clone, Debug)]
pub struct PoiPage<'a> {
    pub pkt: &'a [u8],
    pub len: u32,
    pub x: &'a [f64],
    pub y: &'a [f64],
    pub z: &'a [f64],
    pub category: &'a [u16],
    pub rank: &'a [u16],
    pub id_hash: &'a [u64],
    /// Decoded once: a page names a few dozen categories at most.
    pub categories: Vec<String>,
    ids: BranchCursor<'a>,
    names: BranchCursor<'a>,
    tags: BranchCursor<'a>,
}

impl<'a> PoiPage<'a> {
    pub fn new(pkt: &'a [u8]) -> Result<PoiPage<'a>, MarshalError> {
        let root = TreeReader::new(pkt)?.read_branch()?;
        let x: &[f64] = root.at(0)?.get_array()?;
        let categories = root.at(6)?.as_branch()?;
        Ok(PoiPage {
            pkt,
            len: x.len() as u32,
            x,
            y: root.at(1)?.get_array()?,
            z: root.at(2)?.get_array()?,
            category: root.at(3)?.get_array()?,
            rank: root.at(4)?.get_array()?,
            id_hash: root.at(5)?.get_array()?,
            categories: categories.iter().map(|c| c?.get_str()).collect::<Result<_, _>>()?,
            ids: root.at(7)?.as_branch()?,
            names: root.at(8)?.as_branch()?,
            tags: root.at(9)?.as_branch()?,
        })
    }

    pub fn category_of(&self, i: u32) -> &str {
        &self.categories[self.category[i as usize] as usize]
    }
    pub fn id_of(&self, i: u32) -> Result<String, MarshalError> {
        self.ids.at(i)?.get_str()
    }
    pub fn name_of(&self, i: u32) -> Result<String, MarshalError> {
        self.names.at(i)?.get_str()
    }
    pub fn tags_of(&self, i: u32) -> Result<StrMap, MarshalError> {
        parse_str_map(&self.tags.at(i)?.get_str()?)
    }

    /// The record as the object form, for a consumer that wants one — a
    /// detail view, a test. One allocation per ask, never per page.
    pub fn record(&self, i: u32) -> Result<Poi, MarshalError> {
        let at = i as usize;
        Ok(Poi {
            id: self.id_of(i)?,
            r#type: "poi",
            category: self.category_of(i).to_string(),
            name: self.name_of(i)?,
            center: [self.x[at], self.y[at], self.z[at]],
            tags: self.tags_of(i)?,
            rank: self.rank[at] as u32,
        })
    }
}

/// An empty page: what a client holds for a page the bake found nothing
/// on, so the lanes exist and have length zero.
pub fn encode_empty_poi_page() -> Result<Vec<u8>, MarshalError> {
    encode_poi_page(&[])
}

#[cfg(test)]
#[path = "poi_page.test.rs"]
mod poi_page_test;

#[cfg(test)]
#[path = "format.test.rs"]
mod test;
