// pxl-geo-bake/terrain/pipeline/overture/adapter.rs

//! Overture ingress adapter for WKB-based terrain source rows — the
//! Buildings/Transportation/Base counterpart to
//! terrain/pipeline/coast/adapter.rs. Reuses wkb.rs's decoder as is. Overture
//! rows carry structured columns straight from Postgres instead of hstore, so
//! there is no tag-parsing step to mirror.
//!
//! Deliberately its own raw-feature shape per theme rather than one shared
//! shape — Overture's subtype/class enums vary per theme, so
//! terrain/pipeline/overture/policy.rs reads these shapes directly instead of
//! through a translation layer.

use pxl_spec::bake::format::{BaseRow, BuildingRow, SegmentRow};
use pxl_spec::space::adapter::epsg3857::G3857;

use crate::terrain::pipeline::overture::between::{flag_ranges, split_ranged};
use crate::terrain::pipeline::overture::policy::{DECK_FLAGS, UNSEEN_FLAGS};
use crate::terrain::pipeline::overture::width::{WayColumns, WayShape};
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
    /// What it says of its width over the piece (overture/width.rs).
    pub shape: WayShape,
    pub geometry: WkbGeometry,
    pub center: G3857,
}

/// land_use / water / land / infrastructure — one row shape, four tables with
/// different subtype/class vocabularies; `source` tells overture/policy.rs
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
    /// ingest_base.rs writes. Anything else is read as land_use.
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

/// The center a row's geometry gives, or the origin when it gives none —
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

/// What a segment row's columns say of the way along its length.
trait Columns {
    fn columns(&self) -> WayColumns<'_>;
}

impl Columns for SegmentRow {
    fn columns(&self) -> WayColumns<'_> {
        WayColumns {
            flags: self.flags.as_deref(),
            widths: self.widths.as_deref(),
            access: self.access.as_deref(),
            subclass: self.subclass.as_deref(),
            subclasses: self.subclasses.as_deref(),
        }
    }
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
        .flat_map(|g| split_ranged(g, [&unseen, &deck]))
        .map(|([is_unseen, is_deck], [a, b], g)| {
            let mid = (a + b) / 2.0;
            OvertureRawFeature::Segment(OvertureSegmentRawFeature {
                gers_id: row.gers_id.clone(),
                subtype: row.subtype.clone(),
                class: row.class.clone(),
                is_unseen,
                is_deck,
                shape: WayShape::at(row.columns(), mid),
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
