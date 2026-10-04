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

use pxl_spec::bake::deck::{self, Decks};
use pxl_spec::bake::format::{Ground, Terrain};
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};

/// A page's surface: its padded elevation grid and its packed ground cells.
pub struct Surface {
    pub ele: Vec<i16>,
    pub ground: Ground,
}

impl Surface {
    // { ele, ground } -> [ele, cells]
    pub fn encode(&self) -> Result<Vec<u8>, MarshalError> {
        let mut t: TreeView<'_> = TreeView::new();
        t.branch(Some(&mut |b| {
            b.i16_array(Some(&self.ele)).u16_array(Some(&self.ground.cells));
        }));
        encoder().encode(&t, TreeEncoderOptions::default())
    }

    pub fn decode(raw: &[u8]) -> Result<Surface, MarshalError> {
        let root = TreeReader::new(raw)?.read_branch()?;
        Ok(Surface {
            ele: root.at(0)?.get_array::<i16>()?.to_vec(),
            ground: Ground {
                cells: root.at(1)?.get_array::<u16>()?.to_vec().into(),
            },
        })
    }
}

/// A page's lines as the terrain packet carries them.
pub fn encode_lines(decks: &Decks) -> Result<Vec<u8>, MarshalError> {
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| deck::write_branch(decks, b)));
    encoder().encode(&t, TreeEncoderOptions::default())
}

pub fn decode_lines(raw: &[u8]) -> Result<Decks, MarshalError> {
    let root = TreeReader::new(raw)?.read_branch()?;
    Ok(deck::read_branch(root.at_branch(0)?)?.to_decks())
}

/// The page the client fetches: the surface and the lines, and the index of
/// both (pxl-spec bake/format.rs `Terrain::leaf`).
pub fn assemble(surface: Surface, lines: Decks) -> Terrain {
    Terrain::leaf(surface.ele, surface.ground, lines)
}
