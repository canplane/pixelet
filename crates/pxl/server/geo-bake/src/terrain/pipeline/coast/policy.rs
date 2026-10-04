// pxl-geo-bake/terrain/pipeline/coast/policy.rs

//! Normalize policy for coastline rows.

use pxl_spec::bake::format::{CellConfig, GroundChannelKey};
use pxl_spec::bake::model::feature::{CoastRawFeature, FeatureComputed, FeatureExtra};
use pxl_spec::cell::cube::cube_types;

use crate::terrain::pipeline::normalizer::NormalizePolicy;

fn tag<'a>(raw: &'a CoastRawFeature, key: &str) -> Option<&'a str> {
    raw.tags.iter().find(|(k, _)| k == key).and_then(|(_, v)| v.as_deref())
}

fn is_coast(raw: &CoastRawFeature) -> bool {
    tag(raw, "natural") == Some("ocean") || tag(raw, "source") == Some("overture_ocean")
}

pub struct CoastPolicy;

impl NormalizePolicy<CoastRawFeature> for CoastPolicy {
    fn get_id(&self, raw: &CoastRawFeature) -> String {
        raw.id.clone()
    }

    fn determine_category(&self, raw: &CoastRawFeature) -> String {
        if is_coast(raw) {
            "sea".to_string()
        } else {
            "landuse".to_string()
        }
    }

    fn compute_metrics(&self, _raw: &CoastRawFeature, category: &str) -> FeatureComputed {
        FeatureComputed {
            height: 0.0,
            width: 0.0,
            layer: if category == "sea" { -5 } else { 0 },
        }
    }

    fn should_force_closed(&self, raw: &CoastRawFeature) -> bool {
        is_coast(raw)
    }

    fn is_linear(&self, _raw: &CoastRawFeature) -> bool {
        false
    }

    fn compute_extra(&self, raw: &CoastRawFeature, category: &str) -> FeatureExtra {
        let is_ocean = is_coast(raw) || category == "sea";
        FeatureExtra {
            rank: if is_ocean { 110 } else { 10 },
            is_ocean,
            is_parking: false,
            is_institutional: false,
            is_sports: false,
            is_unseen: false,
            is_deck: false,
        }
    }

    fn determine_cell_cfg(&self, _raw: &CoastRawFeature, category: &str, extra: &FeatureExtra) -> Option<CellConfig> {
        if extra.is_ocean || category == "sea" {
            return Some(CellConfig {
                cell_type: cube_types::land::sea,
                ch_key: GroundChannelKey::LAND,
            });
        }
        None
    }
}
