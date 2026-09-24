// pxl-geo-bake/terrain/pipeline/normalizer.rs

//! Raw feature normalization and local coordinate projection —
//! provider-agnostic, parameterized by a policy so coast/policy.rs
//! (coastline) and overture/policy.rs (Buildings/Transportation/Base) share
//! the same geometry projection and closure-detection code instead of
//! duplicating it.
//!
//! A projected vector is a `V3`, `[f64; 3]` by value, and a page's burst of
//! them is one `Vec`.

use pxl_spec::bake::constant::{XY_CELL_PER_M, Z_CELL_PER_M};
use pxl_spec::bake::format::CellConfig;
use pxl_spec::bake::model::feature::{Feature, FeatureComputed, FeatureExtra, FeatureShape};
use pxl_spec::space::adapter::epsg3857::G3857;
use pxl_spec::space::v3::V3;

/// A raw feature shape any policy can normalize — a coastline row and each
/// Overture theme all satisfy it.
pub trait RawFeature {
    fn points(&self) -> &[G3857];
    fn holes(&self) -> &[Vec<G3857>];
    fn is_polygon_source(&self) -> bool;
    fn center(&self) -> Option<G3857>;
}

pub trait NormalizePolicy<F: RawFeature> {
    /// A provider-neutral id — a coastline row has `id`, an Overture row has `gers_id`.
    fn get_id(&self, raw: &F) -> String;
    fn determine_category(&self, raw: &F) -> String;
    fn compute_metrics(&self, raw: &F, category: &str) -> FeatureComputed;
    fn compute_extra(&self, raw: &F, category: &str) -> FeatureExtra;
    fn determine_cell_cfg(&self, raw: &F, category: &str, extra: &FeatureExtra) -> Option<CellConfig>;
    fn should_force_closed(&self, raw: &F) -> bool;
    /// True for features that must stay linear (never fill-closed) regardless
    /// of geometry closure — coastline, Overture segments.
    fn is_linear(&self, raw: &F) -> bool;
}

/// Projects a 3857 point into page-local cell coordinates.
fn project_to_local_cell(src: G3857, page_min: G3857) -> V3 {
    [(src[0] - page_min[0]) * XY_CELL_PER_M, (src[1] - page_min[1]) * XY_CELL_PER_M, 0.0]
}

/// Converts a raw 3857-space feature into the final local normalized feature.
///
/// `None` for a feature with no points at all; the caller drops that one
/// feature and keeps the page.
pub fn normalize<F: RawFeature, P: NormalizePolicy<F>>(raw_feat: &F, page_min: G3857, policy: &P) -> Option<Feature> {
    let points = raw_feat.points();
    let holes = raw_feat.holes();
    let id = policy.get_id(raw_feat);

    // 1. Validation
    if points.is_empty() {
        return None;
    }

    // 2. Category policy
    let category = policy.determine_category(raw_feat);

    // 3. Closed-loop detection — geometric closure is first and last point
    //    within 0.1 m.
    let is_geometrically_closed = points.len() >= 3 && (points[0][0] - points[points.len() - 1][0]).abs() < 0.1 && (points[0][1] - points[points.len() - 1][1]).abs() < 0.1;

    let is_closed = !policy.is_linear(raw_feat) && (policy.should_force_closed(raw_feat) || raw_feat.is_polygon_source() || is_geometrically_closed);

    // 4. Metric computation
    let metrics_m = policy.compute_metrics(raw_feat, &category);

    // 5. Extra flags
    let extra = policy.compute_extra(raw_feat, &category);

    // 6. Cell config (target ground channel + cell type, or none to skip rasterization)
    let cell_cfg = policy.determine_cell_cfg(raw_feat, &category, &extra);

    // 7. Meter -> cell scaling
    let computed = FeatureComputed {
        height: metrics_m.height * Z_CELL_PER_M(page_min[1]),
        width: metrics_m.width * XY_CELL_PER_M,
        layer: metrics_m.layer,
    };

    // 8. Center resolution
    let final_center_3857 = raw_feat.center().unwrap_or(points[0]);

    // 9. Geometry projection
    let center = project_to_local_cell(final_center_3857, page_min);
    let local_points: Vec<V3> = points.iter().map(|p| project_to_local_cell(*p, page_min)).collect();
    let local_holes: Option<Vec<Vec<V3>>> = if holes.is_empty() {
        None
    } else {
        Some(holes.iter().map(|ring| ring.iter().map(|p| project_to_local_cell(*p, page_min)).collect()).collect())
    };

    // 10. Final construction
    Some(Feature {
        id,
        center,
        shape: FeatureShape {
            points: local_points,
            is_closed,
            holes: local_holes,
        },
        computed,
        category,
        extra,
        cell_cfg,
    })
}
