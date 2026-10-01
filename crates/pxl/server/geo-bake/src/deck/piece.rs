// pxl-geo-bake/deck/piece.rs

//! A bridge's or a tunnel's pieces: the ranges of a segment's length its
//! bridge or tunnel flag holds on, less any that is not built or no longer
//! there — a bridge under construction is no deck yet — each with the level
//! it stands at.

use pxl_spec::space::adapter::epsg3857::{G3857, Z_SCALE};

use crate::terrain::pipeline::overture::between::{flag_ranges, split_ranged, value_at, without};
use crate::terrain::pipeline::overture::policy::{BORE_FLAGS, DECK_FLAGS, UNSEEN_FLAGS};
use crate::terrain::pipeline::overture::width::{width_m, WayColumns, WayShape};
use crate::wkb::wkb_to_geometries;
use pxl_spec::bake::deck::Form;

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
    /// Over the ground or through it.
    pub form: Form,
    /// A way at the surface the structure is carried down to the ground on,
    /// or comes up out of it on (approach.rs), not the bridge or the tunnel
    /// itself.
    pub approach: bool,
    pub points: Vec<G3857>,
}

impl Piece {
    /// Mercator's metres to a physical metre where the piece lies: every
    /// length the solve sets against a measured one — a clearance's run, a
    /// gap — is divided by it, Mercator being conformal.
    pub fn scale(&self) -> f64 {
        Z_SCALE(self.points[0][1])
    }
}

/// A segment's pieces of `form`: its bridge's, less what is not seen, or its
/// tunnel's, less what is not built or gone — a tunnel is unseen, and is
/// what a bore is cut for.
pub fn pieces_of(row: &BridgeRow, form: Form) -> Vec<Piece> {
    let flags = row.flags.as_deref();
    let (on, less): (&[&str], Vec<&str>) = match form {
        Form::Deck | Form::Ramp => (DECK_FLAGS, UNSEEN_FLAGS.to_vec()),
        Form::Bore | Form::Cut => (BORE_FLAGS, UNSEEN_FLAGS.iter().copied().filter(|f| !BORE_FLAGS.contains(f)).collect()),
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
