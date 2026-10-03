// pxl-geo-bake/terrain/pipeline/normalizer.rs

//! Raw features normalized and projected to page cells, shared by every policy.

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::bake::deck::WIDEST;
use pxl_spec::bake::format::CellConfig;
use pxl_spec::bake::model::feature::{Feature, FeatureComputed, FeatureExtra, FeatureShape};
use pxl_spec::space::adapter::epsg3857::{from_db, u_per_m, G3857};
use pxl_spec::space::v3::V3;

pub trait RawFeature {
    fn points(&self) -> &[G3857];
    fn holes(&self) -> &[Vec<G3857>];
    fn is_polygon_source(&self) -> bool;
    fn center(&self) -> Option<G3857>;
}

pub trait NormalizePolicy<F: RawFeature> {
    fn get_id(&self, raw: &F) -> String;
    fn determine_category(&self, raw: &F) -> String;
    fn compute_metrics(&self, raw: &F, category: &str) -> FeatureComputed;
    fn compute_extra(&self, raw: &F, category: &str) -> FeatureExtra;
    fn determine_cell_cfg(&self, raw: &F, category: &str, extra: &FeatureExtra) -> Option<CellConfig>;
    fn should_force_closed(&self, raw: &F) -> bool;
    fn is_linear(&self, raw: &F) -> bool;
}

fn project_to_local_cell(src: G3857, page_min: G3857) -> V3 {
    [from_db(src[0] - page_min[0]) / U_PER_CELL, from_db(src[1] - page_min[1]) / U_PER_CELL, 0.0]
}

/// `None` for a feature with no points; the caller drops it and keeps the page.
pub fn normalize<F: RawFeature, P: NormalizePolicy<F>>(raw_feat: &F, page_min: G3857, policy: &P) -> Option<Feature> {
    let points = raw_feat.points();
    let holes = raw_feat.holes();
    let id = policy.get_id(raw_feat);

    if points.is_empty() {
        return None;
    }

    let category = policy.determine_category(raw_feat);

    // Closed: first and last point within 0.1 of the DB's coordinates.
    let is_geometrically_closed = points.len() >= 3 && (points[0][0] - points[points.len() - 1][0]).abs() < 0.1 && (points[0][1] - points[points.len() - 1][1]).abs() < 0.1;

    let is_closed = !policy.is_linear(raw_feat) && (policy.should_force_closed(raw_feat) || raw_feat.is_polygon_source() || is_geometrically_closed);

    let metrics_m = policy.compute_metrics(raw_feat, &category);

    let extra = policy.compute_extra(raw_feat, &category);

    let cell_cfg = policy.determine_cell_cfg(raw_feat, &category, &extra);

    let cell_per_m = u_per_m(from_db(page_min[1])) / U_PER_CELL;
    let computed = FeatureComputed {
        height: metrics_m.height * cell_per_m,
        width: (metrics_m.width * cell_per_m).min(WIDEST),
        layer: metrics_m.layer,
    };

    let final_center_3857 = raw_feat.center().unwrap_or(points[0]);

    let center = project_to_local_cell(final_center_3857, page_min);
    let local_points: Vec<V3> = points.iter().map(|p| project_to_local_cell(*p, page_min)).collect();
    let local_holes: Option<Vec<Vec<V3>>> = if holes.is_empty() {
        None
    } else {
        Some(holes.iter().map(|ring| ring.iter().map(|p| project_to_local_cell(*p, page_min)).collect()).collect())
    };

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
