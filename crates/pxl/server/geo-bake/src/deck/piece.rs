// pxl-geo-bake/deck/piece.rs

//! A segment's bridge or tunnel ranges, less what is not built or gone.

use pxl_spec::bake::deck::Form;
use pxl_spec::space::adapter::epsg3857::{from_db, scale_at, G3857};

use crate::terrain::pipeline::overture::between::{cell_of, flag_ranges, length_of, split_ranged, value_at, without, Line};
use crate::terrain::pipeline::overture::policy::{BORE_FLAGS, DECK_FLAGS, UNSEEN_FLAGS};
use crate::terrain::pipeline::overture::width::{width_m, WayColumns, WayShape};
use crate::wkb::wkb_to_geometries;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
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
    /// The source's `connectors`: where along it it joins other segments.
    pub connectors: Option<String>,
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

#[derive(Clone)]
pub struct Piece {
    pub gers_id: String,
    pub subtype: String,
    pub class: Option<String>,
    /// OSM's `layer`, an order, not a height; an untagged bridge is 1.
    pub level: i64,
    pub width_m: f64,
    pub form: Form,
    pub points: Vec<G3857>,
    /// What each end is, first and last; ends meet where these are equal.
    pub ends: [End; 2],
    /// How far along its way, the way it runs, it starts, in DB coordinates:
    /// what its samples are laid from, so a clipped piece's fall where the whole's do.
    pub start: f64,
    /// Its way's whole length, in DB coordinates.
    pub way_length: f64,
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

    /// Where `m` ground metres along the piece is on its way, as a fraction.
    pub fn on_way(&self, m: f64) -> f64 {
        (self.start + m * scale_at(from_db(self.points[0][1]))) / self.way_length
    }
}

pub fn pieces_of(row: &BridgeRow, form: Form) -> Vec<Piece> {
    let flags = row.flags.as_deref();
    let (on, less): (&[&str], Vec<&str>) = match form {
        Form::Deck => (DECK_FLAGS, UNSEEN_FLAGS.to_vec()),
        Form::Bore => (BORE_FLAGS, UNSEEN_FLAGS.iter().copied().filter(|f| !BORE_FLAGS.contains(f)).collect()),
        Form::Grade => return Vec::new(), // no flag makes one
    };
    let cell = cell_of(length_of(&row.geom_wkb));
    let deck = without(&flag_ranges(flags, on, cell), &flag_ranges(flags, &less, cell));
    if deck.is_empty() {
        return Vec::new();
    }
    wkb_to_geometries(&row.geom_wkb)
        .into_iter()
        .filter_map(|g| {
            let whole = [*g.points.first()?, *g.points.last()?];
            let length = Line::of(&g.points)?.length();
            Some(split_ranged(g, [&deck]).into_iter().map(move |cut| (cut, whole, length)))
        })
        .flatten()
        .filter(|(([on], _, _), _, _)| *on)
        .map(|((_, [a, b], g), whole, length)| {
            let mid = (a + b) / 2.0;
            let shape = WayShape::at(row.columns(), mid);
            Piece {
                gers_id: row.gers_id.clone(),
                subtype: row.subtype.clone(),
                class: row.class.clone(),
                level: value_at(row.levels.as_deref(), mid).map_or(1, |v| v as i64),
                width_m: width_m(&row.subtype, row.class.as_deref(), shape),
                form,
                ends: [End::of(&row.gers_id, whole, a), End::of(&row.gers_id, whole, b)],
                start: a * length,
                way_length: length,
                points: g.points,
            }
        })
        .filter(|p| p.points.len() >= 2)
        .collect()
}
