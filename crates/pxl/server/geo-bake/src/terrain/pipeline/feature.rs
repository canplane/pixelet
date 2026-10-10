// pxl-geo-bake/terrain/pipeline/feature.rs

//! What the ground paints: a shape in the page's cells and the cell it paints.

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::bake::format::GroundChannelKey;
use pxl_spec::space::adapter::epsg3857::{from_db, G3857};

use crate::wkb::WkbGeometry;

pub type P = [f64; 2];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub key: GroundChannelKey,
    pub value: u8,
}

/// The way beside it that it shares one paved strip with; the gap is paved too.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Joins {
    Road,
    Carriageway,
}

#[derive(Clone, Debug)]
pub struct Feature {
    pub points: Vec<P>,
    pub holes: Vec<Vec<P>>,
    pub closed: bool,
    /// A line's width in the page's cells.
    pub width: f64,
    /// Ends a flag or a line cuts: square, not rounded.
    pub cut: [bool; 2],
    pub cell: Cell,
    /// Later paints over earlier.
    pub rank: u32,
    pub joins: Option<Joins>,
}

/// In the page's cells from its corner.
pub fn local(p: G3857, page_min: G3857) -> P {
    [from_db(p[0] - page_min[0]) / U_PER_CELL, from_db(p[1] - page_min[1]) / U_PER_CELL]
}

/// A polygon, or a ring whose ends meet within a tenth of the DB's unit, is an area.
pub fn closes(g: &WkbGeometry) -> bool {
    let p = &g.points;
    g.is_polygon_source || (p.len() >= 3 && (p[0][0] - p[p.len() - 1][0]).abs() < 0.1 && (p[0][1] - p[p.len() - 1][1]).abs() < 0.1)
}

impl Feature {
    /// An area, or a line of no width, from a geometry; None where it has no points.
    pub fn of(g: &WkbGeometry, page_min: G3857, cell: Cell, rank: u32) -> Option<Feature> {
        (!g.points.is_empty()).then(|| Feature {
            points: g.points.iter().map(|&p| local(p, page_min)).collect(),
            holes: g.holes.iter().map(|ring| ring.iter().map(|&p| local(p, page_min)).collect()).collect(),
            closed: closes(g),
            width: 0.0,
            cut: [false; 2],
            cell,
            rank,
            joins: None,
        })
    }
}
