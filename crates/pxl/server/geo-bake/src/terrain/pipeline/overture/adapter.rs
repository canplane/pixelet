// pxl-geo-bake/terrain/pipeline/overture/adapter.rs

//! Overture Buildings, Transportation and Base rows from WKB, one raw shape
//! per theme because their subtype/class vocabularies differ.

use pxl_spec::bake::format::{BaseRow, BuildingRow, SegmentRow};
use pxl_spec::space::adapter::epsg3857::G3857;

use crate::terrain::pipeline::overture::between::{flag_ranges, split};
use crate::terrain::pipeline::overture::policy::{DECK_FLAGS, UNSEEN_FLAGS};
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

/// One piece of a segment: a part of its length over which its unseen
/// flags and its bridge each hold or do not, which is the unit the surface
/// is painted in.
pub struct OvertureSegmentRawFeature {
    pub gers_id: String,
    pub subtype: String,
    pub class: Option<String>,
    pub is_unseen: bool,
    pub is_deck: bool,
    pub geometry: WkbGeometry,
    pub center: G3857,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BaseSource {
    LandUse,
    Water,
    Land,
    Infrastructure,
}

impl BaseSource {
    /// Anything unknown is read as land_use.
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
/// skipped. A segment is cut where an unseen flag or its bridge starts or
/// stops, since Overture puts a flag on a range of a segment's length rather
/// than on the whole of it: one segment of a subway line runs over a bridge,
/// along the surface and into a tunnel.
pub fn from_segment_row(row: &SegmentRow) -> Vec<OvertureRawFeature> {
    let flags = row.flags.as_deref();
    let (unseen, deck) = (flag_ranges(flags, UNSEEN_FLAGS), flag_ranges(flags, DECK_FLAGS));
    wkb_to_geometries(&row.geom_wkb)
        .into_iter()
        .flat_map(|g| split(g, [&unseen, &deck]))
        .map(|([is_unseen, is_deck], g)| {
            OvertureRawFeature::Segment(OvertureSegmentRawFeature {
                gers_id: row.gers_id.clone(),
                subtype: row.subtype.clone(),
                class: row.class.clone(),
                is_unseen,
                is_deck,
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
