// pxl-spec/bake/model/feature.rs

use crate::bake::format::CellConfig;
use crate::space::adapter::epsg3857::G3857;
use crate::space::v3::V3;

/// Synthetic tag set attached to coastline rows.
pub type CoastTags = Vec<(String, Option<String>)>;

/// A raw coastline row in EPSG:3857 metres, before page projection.
#[derive(Clone, Debug, PartialEq)]
pub struct CoastRawFeature {
    pub id: String,
    pub tags: CoastTags,
    pub geometry: CoastGeometry,
    pub center: Option<G3857>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CoastGeometry {
    pub points: Vec<G3857>,
    pub holes: Option<Vec<Vec<G3857>>>,
    pub is_polygon_source: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureExtra {
    /// Higher is more important.
    pub rank: u32,

    pub is_ocean: bool,

    pub is_parking: bool,
    pub is_institutional: bool,
    pub is_sports: bool,
    /// Nothing of it shows from above (underground, inside, unbuilt, or a
    /// route), so the ground does not paint it.
    pub is_unseen: bool,
    /// Carried above the ground: drawn by the decks, not the ground.
    pub is_deck: bool,
    /// First and last end where a flag cuts the way, so it stops square
    /// there instead of rounding past into a bridge or a tunnel.
    pub cut: [bool; 2],
    /// The way beside it that it shares one paved strip with; the gap is paved too.
    pub joins: Option<Joins>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Joins {
    Road,
    Carriageway,
}

/// A normalized feature from any source, geometry in local page space and
/// metrics in engine units.
#[derive(Clone, Debug, PartialEq)]
pub struct Feature {
    /// The source id (an Overture GERS id or a synthetic coast id).
    pub id: String,
    pub center: V3,

    pub shape: FeatureShape,

    pub computed: FeatureComputed,

    pub category: String,

    pub extra: FeatureExtra,

    /// None when the feature should not be rasterized.
    pub cell_cfg: Option<CellConfig>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FeatureShape {
    pub points: Vec<V3>,
    pub is_closed: bool,
    pub holes: Option<Vec<Vec<V3>>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureComputed {
    /// In local cells.
    pub height: f64,
    /// In local cells.
    pub width: f64,
    pub layer: i32,
}
