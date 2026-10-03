// pxl-geo-bake/terrain/pipeline/overture/adapter.rs

//! Overture Buildings, Transportation and Base rows from WKB, one raw shape
//! per theme because their subtype/class vocabularies differ.

use pxl_spec::bake::format::{BaseRow, BuildingRow, SegmentRow};
use pxl_spec::space::adapter::epsg3857::G3857;

use crate::deck::Carried;
use crate::terrain::pipeline::overture::between::{cell_of, flag_ranges, length_of, merged, split_ranged};
use crate::terrain::pipeline::overture::policy::{BORE_FLAGS, UNSEEN_FLAGS};
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

/// A part of a segment over which its unseen flags and bridge each hold or not.
pub struct OvertureSegmentRawFeature {
    pub gers_id: String,
    pub subtype: String,
    pub class: Option<String>,
    pub is_unseen: bool,
    pub is_deck: bool,
    pub cut: [bool; 2],
    pub shape: WayShape,
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

/// Connectors are skipped. A segment is cut where the deck solve carries it
/// off the ground, which owns whether a bridge or a tunnel is there, so the
/// ground stops where its line starts; and where a flag that hides it from
/// above without carrying it (indoor, unbuilt, gone) starts or stops.
pub fn from_segment_row(row: &SegmentRow, carried: &[Carried]) -> Vec<OvertureRawFeature> {
    let hidden: Vec<&str> = UNSEEN_FLAGS.iter().copied().filter(|f| !BORE_FLAGS.contains(f)).collect();
    let unseen = flag_ranges(row.flags.as_deref(), &hidden, cell_of(length_of(&row.geom_wkb)));
    let deck = merged(carried.iter().filter(|c| c.gers_id == row.gers_id).map(|c| c.range).collect(), 0.0);
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
                cut: [a > 0.0, b < 1.0],
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
