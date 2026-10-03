// pxl-geo-bake/terrain/artifact.rs

//! A terrain page is assembled from two artifacts made from one deck solve:
//! the lines (decks and bores) and the surface (ground and elevation), which
//! paints a way only where the solve leaves it on the ground.

use pxl_spec::bake::deck::{self, Decks};
use pxl_spec::bake::format::{Ground, Terrain};
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};

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

pub fn encode_lines(decks: &Decks) -> Result<Vec<u8>, MarshalError> {
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| deck::write_branch(decks, b)));
    encoder().encode(&t, TreeEncoderOptions::default())
}

pub fn decode_lines(raw: &[u8]) -> Result<Decks, MarshalError> {
    let root = TreeReader::new(raw)?.read_branch()?;
    Ok(deck::read_branch(root.at_branch(0)?)?.to_decks())
}

pub fn assemble(surface: Surface, lines: Decks) -> Terrain {
    Terrain::leaf(surface.ele, surface.ground, lines)
}
