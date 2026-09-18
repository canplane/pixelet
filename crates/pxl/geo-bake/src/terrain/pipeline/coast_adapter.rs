// pxl-geo-bake/terrain/pipeline/coast_adapter.rs

//! Coastline ingress adapter for the repository's OVERTURE_COAST_SQL rows —
//! computed from `overture_base_water`'s `ocean` subtype (see that query's own
//! comment for why and how), not any OSM table.

use pxl_spec::bake::format::CoastRow;
use pxl_spec::bake::model::feature::{CoastGeometry, CoastRawFeature};

use crate::terrain::pipeline::normalizer::RawFeature;
use crate::wkb::wkb_to_geometries;

pub fn from_coast_row(row: &CoastRow) -> Vec<CoastRawFeature> {
    // A clipped full-page coast polygon is a bogus sea-fill candidate.
    if row.covers_page {
        return Vec::new();
    }

    wkb_to_geometries(row.geom_wkb.as_slice())
        .into_iter()
        .map(|g| {
            let center = g.points.first().copied().unwrap_or([0.0, 0.0, 0.0]);
            CoastRawFeature {
                id: row.id.clone(),
                tags: vec![("natural".to_string(), Some("ocean".to_string())), ("source".to_string(), Some("overture_ocean".to_string()))],
                geometry: CoastGeometry {
                    points: g.points,
                    holes: Some(g.holes),
                    is_polygon_source: Some(g.is_polygon_source),
                },
                center: Some(center),
            }
        })
        .collect()
}

impl RawFeature for CoastRawFeature {
    fn points(&self) -> &[pxl_spec::space::adapter::epsg3857::G3857] {
        &self.geometry.points
    }

    fn holes(&self) -> &[Vec<pxl_spec::space::adapter::epsg3857::G3857>] {
        self.geometry.holes.as_deref().unwrap_or(&[])
    }

    fn is_polygon_source(&self) -> bool {
        self.geometry.is_polygon_source.unwrap_or(false)
    }

    fn center(&self) -> Option<pxl_spec::space::adapter::epsg3857::G3857> {
        self.center
    }
}
