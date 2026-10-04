// pxl-geo-bake/terrain/pipeline/overture/adapter.rs

//! Overture Buildings, Transportation and Base rows from WKB, one raw shape
//! per theme because their subtype/class vocabularies differ.

use pxl_spec::bake::format::{BaseRow, BuildingRow, SegmentRow};

use crate::deck::Drawn;
use pxl_spec::space::adapter::epsg3857::G3857;

use crate::terrain::pipeline::overture::between::{cell_of, flag_ranges, length_of, merged, split_ranged};
use crate::terrain::pipeline::overture::policy::UNSEEN_FLAGS;
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

/// One piece of a segment: the part of its length over which one of
/// `UNSEEN_FLAGS` holds or none does, which is the unit the surface is
/// painted in.
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

/// Connectors are skipped. A segment is cut where an unseen flag starts or
/// stops, and where a line the deck solve draws along it does, so the paint
/// ends where the line begins.
pub fn from_segment_row(row: &SegmentRow, drawn: &[Drawn]) -> Vec<OvertureRawFeature> {
    let unseen = flag_ranges(row.flags.as_deref(), UNSEEN_FLAGS, cell_of(length_of(&row.geom_wkb)));
    let deck = merged(drawn.iter().filter(|c| c.gers_id == row.gers_id).map(|c| c.range).collect(), 0.0);
    wkb_to_geometries(&row.geom_wkb)
        .into_iter()
        .flat_map(|g| pieces(g, &unseen))
        .map(|(is_unseen, g)| {
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

/// The ranges of a segment's length, as fractions of it, over which any of
/// `wanted` holds: sorted and merged. A rule with no `between` holds over the whole
/// segment. Flags that do not parse are no flags, which paints the segment as
/// it was painted before any were read.
fn flag_ranges(flags: Option<&str>, wanted: &[&str]) -> Vec<[f64; 2]> {
    let Some(rules) = flags.and_then(|f| serde_json::from_str::<serde_json::Value>(f).ok()) else {
        return Vec::new();
    };
    let mut ranges: Vec<[f64; 2]> = rules
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| r["values"].as_array().is_some_and(|v| v.iter().any(|v| v.as_str().is_some_and(|v| wanted.contains(&v)))))
        .map(|r| match r["between"].as_array().map(|b| (b.first().and_then(|v| v.as_f64()), b.get(1).and_then(|v| v.as_f64()))) {
            Some((Some(a), Some(b))) => [a.clamp(0.0, 1.0), b.clamp(0.0, 1.0)],
            _ => [0.0, 1.0],
        })
        .filter(|[a, b]| a < b)
        .collect();
    ranges.sort_by(|x, y| x[0].total_cmp(&y[0]));
    let mut merged: Vec<[f64; 2]> = Vec::with_capacity(ranges.len());
    for r in ranges {
        match merged.last_mut() {
            Some(last) if r[0] <= last[1] => last[1] = last[1].max(r[1]),
            _ => merged.push(r),
        }
    }
    merged
}

/// A line cut at the ends of `ranges`, each piece marked by whether it lies
/// inside one. A fraction is of the line's planar length in EPSG:3857, where
/// Overture measures its own along the geodesic; over one segment the two
/// differ by the change in Mercator's scale across it, far under a cell.
fn pieces(g: WkbGeometry, ranges: &[[f64; 2]]) -> Vec<(bool, WkbGeometry)> {
    if ranges.is_empty() || g.is_polygon_source || g.points.len() < 2 {
        return vec![(false, g)];
    }
    let mut cuts: Vec<(f64, f64, bool)> = Vec::with_capacity(ranges.len() * 2 + 1);
    let mut at = 0.0;
    for &[a, b] in ranges {
        if a > at {
            cuts.push((at, a, false));
        }
        cuts.push((a, b, true));
        at = b;
    }
    if at < 1.0 {
        cuts.push((at, 1.0, false));
    }

    let mut cum = Vec::with_capacity(g.points.len());
    let mut len = 0.0;
    for (i, p) in g.points.iter().enumerate() {
        if i > 0 {
            let q = g.points[i - 1];
            len += (p[0] - q[0]).hypot(p[1] - q[1]);
        }
        cum.push(len);
    }
    if len == 0.0 {
        return vec![(false, g)];
    }

    cuts.into_iter()
        .map(|(a, b, inside)| {
            let geometry = WkbGeometry {
                points: sub_line(&g.points, &cum, a * len, b * len),
                holes: Vec::new(),
                is_polygon_source: false,
            };
            (inside, geometry)
        })
        .collect()
}

/// The part of a line between two distances along it, `cum` being the
/// distance at each of its points.
fn sub_line(points: &[G3857], cum: &[f64], from: f64, to: f64) -> Vec<G3857> {
    let at = |d: f64| -> G3857 {
        let i = cum.partition_point(|&c| c < d).clamp(1, points.len() - 1);
        let (p, q) = (points[i - 1], points[i]);
        let span = cum[i] - cum[i - 1];
        let t = if span > 0.0 { (d - cum[i - 1]) / span } else { 0.0 };
        [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t, p[2] + (q[2] - p[2]) * t]
    };
    let mut out = vec![at(from)];
    out.extend(points.iter().zip(cum).filter(|(_, &c)| c > from && c < to).map(|(p, _)| *p));
    out.push(at(to));
    out
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
