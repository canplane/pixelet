// pxl-geo-bake/terrain/artifact.rs

//! A terrain page is built from two things that change for different
//! reasons, and is assembled from them with nothing else.
//!
//! - **The surface**: the ground channels and the elevation, from the
//!   elevation raster and Overture's rows. It changes when the source data
//!   does, and it is the expensive half, a page's worth of database reads.
//! - **The lines**: the decks, ramps and bores, from what the deck solve
//!   wrote (deck/). They change when the solve's rules do, and they are
//!   cheap, one query a page.
//!
//! The offline bake keeps each as its own artifact, built and invalidated on
//! its own (bin/bake_tiles.rs), so a change to how a deck is solved never
//! bakes a surface again. What is published is neither: it is the page
//! `assemble` makes of the two — the index over both, the packet the client
//! fetches once a page — with no database and no solve in the loop. The dev
//! tile server makes both and assembles them in one go.
//!
//! An artifact outlives the code that wrote it, so each says which layout it
//! was written in (`stamp`), and one written in another is refused rather
//! than read: a surface's cells are dense indices into the catalog's tables
//! as they stood, and read against tables that have since changed they
//! decode without complaint into the wrong ground. A refused surface is
//! baked again; a resumed bake keeps every surface still current.

use pxl_spec::bake::deck::{self, Decks, MAX_DECKS, MAX_POINTS, POINT_SCALE};
use pxl_spec::bake::format::{Ground, Terrain, ELE_LANE, GROUND_LANE};
use pxl_spec::cell::cube::ground_pack;
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};

/// The layout an artifact of `kind` is written in: FNV-1a over what its
/// bytes depend on beyond itself — the lanes' sizes, the catalog's dense
/// tables a packed cell indexes, a line's quantization and bounds.
fn stamp(kind: &str) -> [u8; 8] {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for &b in bytes {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    };
    eat(kind.as_bytes());
    match kind {
        "surface" => {
            eat(&(ELE_LANE as u64).to_le_bytes());
            eat(&(GROUND_LANE as u64).to_le_bytes());
            for table in ground_pack::NUMBER {
                eat(&(table.len() as u64).to_le_bytes());
                eat(table);
            }
        }
        _ => {
            eat(&(MAX_DECKS as u64).to_le_bytes());
            eat(&(MAX_POINTS as u64).to_le_bytes());
            eat(&POINT_SCALE.to_le_bytes());
        }
    }
    h.to_le_bytes()
}

/// An artifact's root, refused unless it is stamped with the layout this
/// code writes.
fn current<'a>(raw: &'a [u8], kind: &str) -> Result<xpute_core::wire::xtp::BranchCursor<'a>, String> {
    let root = TreeReader::new(raw).and_then(|r| r.read_branch()).map_err(|e| format!("{e:?}"))?;
    let stamped = root.at(0).and_then(|n| n.get_array::<u8>()).map_err(|e| format!("{e:?}"))?;
    if stamped != stamp(kind) {
        return Err(format!("a {kind} written in another layout: run --stage {kind} again"));
    }
    Ok(root)
}

/// A page's surface: its padded elevation grid and its packed ground cells.
pub struct Surface {
    pub ele: Vec<i16>,
    pub ground: Ground,
}

impl Surface {
    // { ele, ground } -> [stamp, ele, cells]
    pub fn encode(&self) -> Result<Vec<u8>, MarshalError> {
        let stamp = stamp("surface");
        let mut t: TreeView<'_> = TreeView::new();
        t.branch(Some(&mut |b| {
            b.u8_array(Some(&stamp)).i16_array(Some(&self.ele)).u16_array(Some(&self.ground.cells));
        }));
        encoder().encode(&t, TreeEncoderOptions::default())
    }

    pub fn decode(raw: &[u8]) -> Result<Surface, String> {
        let root = current(raw, "surface")?;
        let lane = |i: u32| root.at(i).map_err(|e| format!("{e:?}"));
        Ok(Surface {
            ele: lane(1)?.get_array::<i16>().map_err(|e| format!("{e:?}"))?.to_vec(),
            ground: Ground {
                cells: lane(2)?.get_array::<u16>().map_err(|e| format!("{e:?}"))?.to_vec().into(),
            },
        })
    }

    /// Whether a stored surface is one this code would write, and can stand.
    pub fn is_current(raw: &[u8]) -> bool {
        current(raw, "surface").is_ok()
    }
}

/// A page's lines as the terrain packet carries them: [stamp, decks].
pub fn encode_lines(decks: &Decks) -> Result<Vec<u8>, MarshalError> {
    let stamp = stamp("lines");
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| {
        b.u8_array(Some(&stamp));
        deck::write_branch(decks, b);
    }));
    encoder().encode(&t, TreeEncoderOptions::default())
}

pub fn decode_lines(raw: &[u8]) -> Result<Decks, String> {
    let root = current(raw, "lines")?;
    let branch = root.at_branch(1).map_err(|e| format!("{e:?}"))?;
    Ok(deck::read_branch(branch).map_err(|e| format!("{e:?}"))?.to_decks())
}

/// The page the client fetches: the surface and the lines, and the index of
/// both (pxl-spec bake/format.rs `Terrain::leaf`).
pub fn assemble(surface: Surface, lines: Decks) -> Terrain {
    Terrain::leaf(surface.ele, surface.ground, lines)
}
