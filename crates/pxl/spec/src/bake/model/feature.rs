// pxl-spec/bake/model/feature.rs

use crate::bake::format::CellConfig;
use crate::space::adapter::epsg3857::G3857;
use crate::space::v3::V3;

/// Synthetic tag set attached to coastline rows (pipeline/coast/adapter.rs's
/// from_coast_row) — coast/policy.rs's own IS_COAST check reads `natural`/
/// `source` off this shape.
pub type CoastTags = Vec<(String, Option<String>)>;

/// Raw coastline row form exchanged between coast/adapter.rs and
/// pipeline/normalizer.rs. Coordinates are expected to be in EPSG:3857 meters. This
/// type preserves source geometry before local page projection.
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

/// Derived flags computed during normalization for downstream bake/render logic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureExtra {
    /// Rendering priority score. Higher values indicate more important features.
    pub rank: u32,

    /// True when the feature represents ocean/sea fill.
    pub is_ocean: bool,

    /// Parking-area marker.
    pub is_parking: bool,
    /// Institutional-area marker.
    pub is_institutional: bool,
    /// Sports-area marker.
    pub is_sports: bool,
    /// True where nothing of the feature shows from above — it runs under the
    /// ground or inside a building, is not built yet or is no longer there,
    /// or is a route rather than a way — so the ground does not paint it.
    pub is_unseen: bool,
    /// True where the feature is carried above the ground: the page's decks
    /// draw it (bake/deck.rs), and the ground does not.
    pub is_deck: bool,
}

/// Final normalized feature form ready for bake/runtime use, shared by every
/// source provider (coastline, Overture Buildings/Transportation/Base).
/// Geometry is already projected into local page space and computed metrics
/// are already converted into engine-local units.
#[derive(Clone, Debug, PartialEq)]
pub struct Feature {
    /// Source-provider identifier (Overture GERS id, or coast/adapter.rs's synthetic row id) carried through normalization.
    pub id: String,
    /// Feature center used for labels, POI markers, and camera focus.
    pub center: V3,

    /// Final geometry payload ready for rasterization or line rendering.
    pub shape: FeatureShape,

    /// Computed metrics derived from source tags.
    pub computed: FeatureComputed,

    /// Final coarse rendering category.
    pub category: String,

    /// Derived policy flags used by later bake and render stages.
    pub extra: FeatureExtra,

    /// Target ground channel + cell type, resolved by the source provider's own policy. None when the feature should not be rasterized.
    pub cell_cfg: Option<CellConfig>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FeatureShape {
    pub points: Vec<V3>,
    /// Closed-loop marker used to choose fill vs. stroke behavior.
    pub is_closed: bool,
    /// Hole rings assembled from polygon inner members.
    pub holes: Option<Vec<Vec<V3>>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureComputed {
    /// Final building height in local cell scale.
    pub height: f64,
    /// Final width in local cell scale.
    pub width: f64,
    /// Layer used for vertical ordering and offset policy.
    pub layer: i32,
}
