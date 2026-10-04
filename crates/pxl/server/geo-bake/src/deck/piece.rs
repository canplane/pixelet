// pxl-geo-bake/deck/piece.rs

//! A segment's bridge or tunnel ranges, less what is not built or gone.

use pxl_spec::space::adapter::epsg3857::{G3857, Z_SCALE};

use crate::terrain::pipeline::overture::between::{flag_ranges, split_ranged, value_at, without};
use crate::terrain::pipeline::overture::policy::{BORE_FLAGS, DECK_FLAGS, UNSEEN_FLAGS};
use crate::terrain::pipeline::overture::width::{width_m, WayColumns, WayShape};
use crate::wkb::wkb_to_geometries;
use pxl_spec::bake::deck::Form;

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
    pub fn columns(&self) -> WayColumns<'_> {
        WayColumns {
            flags: self.flags.as_deref(),
            widths: self.widths.as_deref(),
            access: self.access.as_deref(),
            subclass: self.subclass.as_deref(),
            subclasses: self.subclasses.as_deref(),
        }
    }
}

pub struct Piece {
    pub gers_id: String,
    pub subtype: String,
    pub class: Option<String>,
    /// OSM's `layer`, an order, not a height; an untagged bridge is 1.
    pub level: i64,
    pub width_m: f64,
    pub form: Form,
    /// A surface way the structure is carried down on (approach.rs).
    pub approach: bool,
    pub points: Vec<G3857>,
}

impl Piece {
    /// Mercator metres per physical metre here; lengths compared with measured
    /// ones are divided by it.
    pub fn scale(&self) -> f64 {
        Z_SCALE(self.points[0][1])
    }
}

pub fn pieces_of(row: &BridgeRow, form: Form) -> Vec<Piece> {
    let flags = row.flags.as_deref();
    let (on, less): (&[&str], Vec<&str>) = match form {
        Form::Deck | Form::Ramp => (DECK_FLAGS, UNSEEN_FLAGS.to_vec()),
        Form::Bore => (BORE_FLAGS, UNSEEN_FLAGS.iter().copied().filter(|f| !BORE_FLAGS.contains(f)).collect()),
    };
    let deck = without(&flag_ranges(flags, on), &flag_ranges(flags, &less));
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
                form,
                approach: false,
                points: g.points,
            }
        })
        .filter(|p| p.points.len() >= 2)
        .collect()
}
