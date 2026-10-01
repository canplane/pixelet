// pxl-geo-bake/deck/piece.rs

//! A bridge's pieces: the ranges of a segment's length its bridge flag holds
//! on, less any that is not seen from above — a bridge under construction is
//! no deck yet — each with the level it stands at.

use pxl_spec::space::adapter::epsg3857::G3857;

use crate::terrain::pipeline::overture::between::{flag_ranges, split_ranged, value_at, without};
use crate::terrain::pipeline::overture::policy::{DECK_FLAGS, UNSEEN_FLAGS};
use crate::terrain::pipeline::overture::width::{width_m, WayColumns, WayShape};
use crate::wkb::wkb_to_geometries;

/// A segment as the solve reads it.
pub struct BridgeRow {
    pub gers_id: String,
    pub subtype: String,
    pub class: Option<String>,
    pub flags: Option<String>,
    pub levels: Option<String>,
    pub widths: Option<String>,
    pub access: Option<String>,
    pub subclass: Option<String>,
    pub subclasses: Option<String>,
    pub geom_wkb: Box<[u8]>,
}

impl BridgeRow {
    fn columns(&self) -> WayColumns<'_> {
        WayColumns {
            flags: self.flags.as_deref(),
            widths: self.widths.as_deref(),
            access: self.access.as_deref(),
            subclass: self.subclass.as_deref(),
            subclasses: self.subclasses.as_deref(),
        }
    }
}

/// One piece of a bridge, in EPSG:3857 metres.
pub struct Piece {
    pub gers_id: String,
    pub subtype: String,
    pub class: Option<String>,
    /// OSM's `layer`: an order among what crosses, not a height. A bridge
    /// that does not say is one over the ground, 1.
    pub level: i64,
    /// Its width in metres, as its way is painted on the ground.
    pub width_m: f64,
    pub points: Vec<G3857>,
}

/// The deck pieces of a segment.
pub fn pieces_of(row: &BridgeRow) -> Vec<Piece> {
    let flags = row.flags.as_deref();
    let deck = without(&flag_ranges(flags, DECK_FLAGS), &flag_ranges(flags, UNSEEN_FLAGS));
    if deck.is_empty() {
        return Vec::new();
    }
    wkb_to_geometries(&row.geom_wkb)
        .into_iter()
        .flat_map(|g| split_ranged(g, [&deck]))
        .filter(|([on], _, _)| *on)
        .map(|(_, [a, b], g)| {
            let mid = (a + b) / 2.0;
            let shape = WayShape::at(row.columns(), mid);
            Piece {
                gers_id: row.gers_id.clone(),
                subtype: row.subtype.clone(),
                class: row.class.clone(),
                level: value_at(row.levels.as_deref(), mid).map_or(1, |v| v as i64),
                width_m: width_m(&row.subtype, row.class.as_deref(), shape),
                points: g.points,
            }
        })
        .filter(|p| p.points.len() >= 2)
        .collect()
}
