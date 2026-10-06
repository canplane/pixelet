// pxl-geo-bake/deck/piece.rs

//! A way's bridge or tunnel, as the solve takes it.

use pxl_spec::bake::deck::Form;

use crate::segment::{PartId, Segment};
use crate::terrain::pipeline::overture::width::width_u;

/// A part of a way that is a bridge or a tunnel, whole.
#[derive(Clone)]
pub struct Piece {
    pub way: Segment,
    pub form: Form,
}

impl Piece {
    pub fn id(&self) -> PartId {
        self.way.id()
    }

    pub fn width(&self) -> f64 {
        width_u(&self.way.subtype, self.way.class.as_deref(), self.way.tags.role)
    }

    /// Whether any of it lies in `[lo, hi]`.
    pub fn touches(&self, [lo, hi]: [[f64; 2]; 2]) -> bool {
        let (mut a, mut b) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for p in &self.way.points {
            for k in 0..2 {
                (a[k], b[k]) = (a[k].min(p[k]), b[k].max(p[k]));
            }
        }
        (0..2).all(|k| a[k] <= hi[k] && b[k] >= lo[k])
    }
}

/// A part that is a bridge or a tunnel as Arnis builds one: no bridge indoors
/// (`is_bridge_way`) nor along a tunnel, no road tunnel indoors
/// (`renders_as_highway_tunnel`); a subway runs underground wherever it is no
/// bridge (`wants_rail_tunnel`).
pub fn piece_of(way: &Segment) -> Option<Piece> {
    let t = way.tags;
    let form = if t.bridge && !t.tunnel && !t.indoor {
        Form::Deck
    } else if (t.tunnel && !(way.subtype == "road" && t.indoor)) || (way.is_subway() && !t.bridge) {
        Form::Bore
    } else {
        return None;
    };
    (way.points.len() >= 2).then(|| Piece { way: way.clone(), form })
}
