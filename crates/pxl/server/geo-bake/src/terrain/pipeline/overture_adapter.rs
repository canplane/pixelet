// pxl-geo-bake/terrain/pipeline/overture_adapter.rs

//! Overture ingress adapter for WKB-based terrain source rows — the
//! Buildings/Transportation/Base counterpart to
//! terrain/pipeline/coast_adapter.rs. Reuses wkb.rs's decoder as is. Overture
//! rows carry structured columns straight from Postgres instead of hstore, so
//! there is no tag-parsing step to mirror.
//!
//! Deliberately its own raw-feature shape per theme rather than one shared
//! shape — Overture's subtype/class enums vary per theme, so
//! terrain/pipeline/overture_policy.rs reads these shapes directly instead of
//! through a translation layer.

use pxl_spec::bake::format::{BaseRow, BuildingRow, SegmentRow};
use pxl_spec::space::adapter::epsg3857::G3857;

use crate::wkb::{wkb_to_geometries, WkbGeometry};

pub struct OvertureBuildingRawFeature {
    pub gers_id: String,
    pub subtype: Option<String>,
    pub class: Option<String>,
    pub height: Option<f64>,
    pub num_floors: Option<f64>,
    pub geometry: WkbGeometry,
    pub center: G3857,
}

pub struct OvertureSegmentRawFeature {
    pub gers_id: String,
    pub subtype: String,
    pub class: Option<String>,
    pub geometry: WkbGeometry,
    pub center: G3857,
}

/// land_use / water / land / infrastructure — one row shape, four tables with
/// different subtype/class vocabularies; `source` tells overture_policy.rs
/// which one a given row came from.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BaseSource {
    LandUse,
    Water,
    Land,
    Infrastructure,
}

impl BaseSource {
    /// The wire values are exactly these strings — see the table set
    /// ingest_base.ts writes. Anything else is read as land_use.
    fn from_wire(s: &str) -> BaseSource {
        match s {
            "water" => BaseSource::Water,
            "land" => BaseSource::Land,
            "infrastructure" => BaseSource::Infrastructure,
            _ => BaseSource::LandUse,
        }
    }
}

pub struct OvertureBaseRawFeature {
    pub gers_id: String,
    pub source: BaseSource,
    pub subtype: Option<String>,
    pub class: Option<String>,
    pub geometry: WkbGeometry,
    pub center: G3857,
}

pub enum OvertureRawFeature {
    Building(OvertureBuildingRawFeature),
    Segment(OvertureSegmentRawFeature),
    Base(OvertureBaseRawFeature),
}

/// The centre a row's geometry gives, or the origin when it gives none —
/// `g.points[0] ?? [0, 0, 0]`.
fn center_of(g: &WkbGeometry) -> G3857 {
    g.points.first().copied().unwrap_or([0.0, 0.0, 0.0])
}

pub fn from_building_row(row: &BuildingRow) -> Vec<OvertureRawFeature> {
    wkb_to_geometries(&row.geom_wkb)
        .into_iter()
        .map(|g| {
            OvertureRawFeature::Building(OvertureBuildingRawFeature {
                gers_id: row.gers_id.clone(),
                subtype: row.subtype.clone(),
                class: row.class.clone(),
                height: row.height,
                num_floors: row.num_floors,
                center: center_of(&g),
                geometry: g,
            })
        })
        .collect()
}

/// Transportation's segments only — a connector is routing topology and is
/// skipped. The row's `road` JSONB is not read: the policy's one use for it
/// would be a tunnel flag whose field path is not sample-confirmed, so it is
/// parsed by nobody and carried by nothing here.
pub fn from_segment_row(row: &SegmentRow) -> Vec<OvertureRawFeature> {
    wkb_to_geometries(&row.geom_wkb)
        .into_iter()
        .map(|g| {
            OvertureRawFeature::Segment(OvertureSegmentRawFeature {
                gers_id: row.gers_id.clone(),
                subtype: row.subtype.clone(),
                class: row.class.clone(),
                center: center_of(&g),
                geometry: g,
            })
        })
        .collect()
}

pub fn from_base_row(row: &BaseRow) -> Vec<OvertureRawFeature> {
    let source = BaseSource::from_wire(&row.source);
    wkb_to_geometries(&row.geom_wkb)
        .into_iter()
        .map(|g| {
            OvertureRawFeature::Base(OvertureBaseRawFeature {
                gers_id: row.gers_id.clone(),
                source,
                subtype: row.subtype.clone(),
                class: row.class.clone(),
                center: center_of(&g),
                geometry: g,
            })
        })
        .collect()
}
