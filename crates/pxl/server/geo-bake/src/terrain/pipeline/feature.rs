// pxl-geo-bake/terrain/pipeline/feature.rs

//! What the ground paints: a shape in the tile's cells and the cell it paints.

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::bake::format::GroundCell;
use pxl_spec::space::coord::adapter::epsg3857::{from_db, G3857};
use xpute_core::math::scalar::round;

use crate::wkb::WkbGeometry;

pub type P = [f64; 2];

/// OpenStreetMap Carto's layers in the order `project.mml` stacks them
/// (gravitystorm/openstreetmap-carto 1cc4b89), but buildings over roads: a
/// way is painted at its width, which would bury a small building Carto draws
/// beside a hairline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    Landcover,
    /// `landcover-line`: a cutline.
    LandcoverLine,
    /// `water-lines`: a river, canal, stream, drain or ditch drawn as a line.
    WaterLine,
    Water,
    /// `water-barriers-poly` and `-line`: a dam, a weir.
    WaterBarrier,
    /// `piers-poly` and `-line`: a pier, a breakwater, a groyne.
    Pier,
    /// `highway-area-fill`: pedestrian areas, platforms, an aeroway's areas.
    RoadArea,
    Road,
    Building,
    Aeroway,
}

/// Later paints over earlier, compared in field order: Carto's sort within
/// the layer, then the source's id, so no row order decides; only one
/// source's parts share a rank.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Rank {
    pub layer: Layer,
    pub order: i64,
    /// Carto's `feature` (`landuse_forest`), its tie within landcover.
    pub feature: &'static str,
    pub id: Box<str>,
}

impl Rank {
    pub fn new(layer: Layer, order: i64, feature: &'static str, id: &str) -> Rank {
        Rank { layer, order, feature, id: id.into() }
    }

    /// Carto sorts an area layer by `way_area`, larger first: the whole
    /// source's, so every tile and every part of it ranks alike.
    pub fn by_area(layer: Layer, way_area: f64, feature: &'static str, id: &str) -> Rank {
        Rank::new(layer, -(round(way_area) as i64), feature, id)
    }
}

#[derive(Clone, Debug)]
pub struct Feature {
    pub points: Vec<P>,
    pub holes: Vec<Vec<P>>,
    pub closed: bool,
    /// A line's width in u.
    pub width: f64,
    /// Ends a flag or a line cuts: square, not rounded.
    pub cut: [bool; 2],
    pub cell: GroundCell,
    pub rank: Rank,
}

/// In the tile's cells from its corner.
pub fn local(p: G3857, tile_min: G3857) -> P {
    [from_db(p[0] - tile_min[0]) / U_PER_CELL, from_db(p[1] - tile_min[1]) / U_PER_CELL]
}

/// A polygon, or a ring whose ends meet within a tenth of the DB's unit, is an area.
pub fn closes(g: &WkbGeometry) -> bool {
    let p = &g.points;
    g.is_polygon_source || (p.len() >= 3 && (p[0][0] - p[p.len() - 1][0]).abs() < 0.1 && (p[0][1] - p[p.len() - 1][1]).abs() < 0.1)
}

impl Feature {
    /// A line of no width yet, from its points; None where it has none.
    pub fn line(points: &[G3857], tile_min: G3857, cell: GroundCell, rank: Rank) -> Option<Feature> {
        (!points.is_empty()).then(|| Feature {
            points: points.iter().map(|&p| local(p, tile_min)).collect(),
            holes: Vec::new(),
            closed: false,
            width: 0.0,
            cut: [false; 2],
            cell,
            rank,
        })
    }

    /// An area, or a line of no width, from a geometry; None where it has no points.
    pub fn of(g: &WkbGeometry, tile_min: G3857, cell: GroundCell, rank: Rank) -> Option<Feature> {
        (!g.points.is_empty()).then(|| Feature {
            points: g.points.iter().map(|&p| local(p, tile_min)).collect(),
            holes: g.holes.iter().map(|ring| ring.iter().map(|&p| local(p, tile_min)).collect()).collect(),
            closed: closes(g),
            width: 0.0,
            cut: [false; 2],
            cell,
            rank,
        })
    }
}
