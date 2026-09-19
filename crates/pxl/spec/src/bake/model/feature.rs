// pxl-spec/bake/model/feature.rs

use xpute_core::codec::json::StrMap;

use crate::bake::format::CellConfig;
use crate::space::adapter::epsg3857::G3857;
use crate::space::v3::V3;

/// Synthetic tag set attached to coastline rows (coast_adapter.ts's
/// from_coast_row) — coast_policy.ts's own IS_COAST check reads `natural`/
/// `source` off this shape.
pub type CoastTags = StrMap;

/// Raw coastline row form exchanged between coast_adapter.ts and
/// normalizer.ts. Coordinates are expected to be in EPSG:3857 meters. This
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
    /// True when the feature is a tunnel and should not be rasterized.
    pub is_tunnel: bool,
}

/// Final normalized feature form ready for bake/runtime use, shared by every
/// source provider (coastline, Overture Buildings/Transportation/Base).
/// Geometry is already projected into local page space and computed metrics
/// are already converted into engine-local units.
#[derive(Clone, Debug, PartialEq)]
pub struct Feature {
    /// Source-provider identifier (Overture GERS id, or coast_adapter.ts's synthetic row id) carried through normalization.
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

// ---- Derived object models ----

#[derive(Clone, Debug, PartialEq)]
pub struct Poi {
    pub id: String,
    /// Always "poi".
    pub r#type: &'static str,
    pub category: String,
    pub name: String,
    pub center: V3,
    pub tags: StrMap,

    /// Final importance rank assigned by policy.
    pub rank: u32,
}
