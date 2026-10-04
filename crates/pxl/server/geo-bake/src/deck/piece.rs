// pxl-geo-bake/deck/piece.rs

//! A bridge's or a tunnel's pieces: the ranges of a segment's length its
//! bridge or tunnel flag holds on, less any that is not built or no longer
//! there — a bridge under construction is no deck yet — each with the level
//! it stands at.

use pxl_spec::bake::deck::Form;
use pxl_spec::space::adapter::epsg3857::{from_db, scale_at, G3857};

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
    /// What each end is, first and last; ends meet where these are equal.
    pub ends: [End; 2],
    /// How far along its way, the way it runs, it starts, in DB coordinates:
    /// what its samples are laid from, so a clipped piece's fall where the whole's do.
    pub start: f64,
}

/// An end by identity, never by a computed point: a segment's own first or
/// last vertex, which every segment at that connector carries bit for bit,
/// or a cut at a fraction of one segment, which only its pieces share.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum End {
    Vertex(u64, u64),
    Cut(String, u64),
}

impl End {
    /// The end at fraction `at` of segment `gers_id`, which runs from `first`
    /// to `last`. `+ 0.0` makes −0 the bits of 0, as `==` has it.
    pub fn of(gers_id: &str, [first, last]: [G3857; 2], at: f64) -> End {
        let vertex = |p: G3857| End::Vertex((p[0] + 0.0).to_bits(), (p[1] + 0.0).to_bits());
        if at == 0.0 {
            vertex(first)
        } else if at == 1.0 {
            vertex(last)
        } else {
            End::Cut(gers_id.to_string(), (at + 0.0).to_bits())
        }
    }
}

impl Piece {
    /// Its way and where on it it starts: a piece in no other place.
    pub fn id(&self) -> (&str, u64) {
        (&self.gers_id, self.start.to_bits())
    }

    pub fn end(&self, last: bool) -> &End {
        &self.ends[usize::from(last)]
    }

    /// Whether any of it lies in `[lo, hi]`.
    pub fn touches(&self, [lo, hi]: [[f64; 2]; 2]) -> bool {
        let (mut a, mut b) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for p in &self.points {
            for k in 0..2 {
                (a[k], b[k]) = (a[k].min(p[k]), b[k].max(p[k]));
            }
        }
        (0..2).all(|k| a[k] <= hi[k] && b[k] >= lo[k])
    }

    /// A length along the piece in DB coordinates, in ground metres.
    pub fn to_m(&self, db: f64) -> f64 {
        db / scale_at(from_db(self.points[0][1]))
    }
}

pub fn pieces_of(row: &Segment, form: Form) -> Vec<Piece> {
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
