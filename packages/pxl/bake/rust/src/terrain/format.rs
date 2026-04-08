// @pxl/bake/terrain/format.rs
//
// Terrain format helpers for baked page payloads.
//
// Wire format:
//   baked = [ele, [land, water, structure, transport]]
//   ele   : I16Array — elevation raster in [cell] units
//   ground: branch of four U8Array sparse-tree channels

use xpute_spec::io::marshal::tree::{
    BranchCursor, SequenceValue, TreeEncoder, TreeReader, TreeView,
};
use xpute_spec::sys::error::MarshalError;

// ---- DB Row Types ----
// Used on the raw-wire side (Deno/TS DB queries). Included here for format completeness.

pub const FEAT_ROW_KEYS: [&str; 4] = ["osm_id", "tags_hstore", "way_wkb", "center_wkb"];

pub struct FeatRow {
    pub osm_id: String,
    pub tags_hstore: Option<String>,
    pub way_wkb: Vec<u8>,
    pub center_wkb: Vec<u8>,
}

pub const COAST_ROW_KEYS: [&str; 3] = ["id", "geom_wkb", "covers_page"];

pub struct CoastRow {
    pub id: String,
    pub geom_wkb: Vec<u8>,
    pub covers_page: bool,
}

// ---- Ground ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroundChannelKey {
    Land,
    Water,
    Structure,
    Transport,
}

pub const GROUND_CH_KEYS: [GroundChannelKey; 4] = [
    GroundChannelKey::Land,
    GroundChannelKey::Water,
    GroundChannelKey::Structure,
    GroundChannelKey::Transport,
];

pub struct Ground {
    pub land: Vec<u8>,
    pub water: Vec<u8>,
    pub structure: Vec<u8>,
    pub transport: Vec<u8>,
}

// ---- Page ----

/// Static terrain payload carried by one L3 page.
pub struct Terrain {
    pub ground: Ground,
    pub ele: Vec<i16>,
}

/// Decoded page payload with the baked terrain layer attached.
pub struct Tile {
    pub terrain: Terrain,
}

// ---- Codec ----

/// Encodes baked terrain into a tree packet.
///
/// Wire layout: [ele, [land, water, structure, transport]]
pub fn encode_terrain(ele: Vec<i16>, ground: Ground) -> Result<Vec<u8>, MarshalError> {
    let mut tree = TreeView::new();
    tree.branch(Some(|b: &mut xpute_spec::io::marshal::tree::BranchView| {
        b.i16_array(Some(ele));
        b.branch(Some(|g: &mut xpute_spec::io::marshal::tree::BranchView| {
            g.u8_array(Some(ground.land));
            g.u8_array(Some(ground.water));
            g.u8_array(Some(ground.structure));
            g.u8_array(Some(ground.transport));
        }));
    }));
    TreeEncoder::default().encode(&tree)
}

/// Decodes a tree packet into a `Terrain`.
///
/// Wire layout: [ele, [land, water, structure, transport]]
pub fn decode_terrain(pkt: Vec<u8>) -> Result<Terrain, MarshalError> {
    let reader = TreeReader::new(pkt)?;
    let root = reader.read_branch()?;

    let ele = get_i16_array(&root, 0)?;
    let ground_branch = root.at_branch(1)?;

    let ground = Ground {
        land:      get_u8_array(&ground_branch, 0)?,
        water:     get_u8_array(&ground_branch, 1)?,
        structure: get_u8_array(&ground_branch, 2)?,
        transport: get_u8_array(&ground_branch, 3)?,
    };

    Ok(Terrain { ele, ground })
}

fn get_i16_array(branch: &BranchCursor, idx: u32) -> Result<Vec<i16>, MarshalError> {
    use xpute_spec::sys::errno;
    match branch.at(idx)?.get()? {
        xpute_spec::io::marshal::tree::CursorValue::Sequence(SequenceValue::I16Array(v)) => Ok(v),
        _ => Err(MarshalError::new(errno::EBADMSG, "expected i16_array")),
    }
}

fn get_u8_array(branch: &BranchCursor, idx: u32) -> Result<Vec<u8>, MarshalError> {
    use xpute_spec::sys::errno;
    match branch.at(idx)?.get()? {
        xpute_spec::io::marshal::tree::CursorValue::Sequence(SequenceValue::U8Array(v)) => Ok(v),
        _ => Err(MarshalError::new(errno::EBADMSG, "expected u8_array")),
    }
}
