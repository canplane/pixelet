// pxl-geo-bake/terrain/pipeline/overture/width.rs

//! A way's width in metres, the same on the ground and on a deck.
//! `width_rules` (on ~1% of segments, often an import's classes) wins where
//! given; otherwise lanes: 3.5 m a motor lane (osm2lanes' default), 2.5 m a
//! footway, 2.0 m a cycleway, with class cross-sections close to Arnis's.
//! A link or a service lane is one lane.

use crate::terrain::pipeline::overture::between::{flag_at, value_at};
use crate::terrain::pipeline::overture::policy::LINK_FLAGS;
use crate::terrain::pipeline::overture::role::Role;

const LANE: f64 = 3.5;

/// `each_way` None is a single-lane way whichever way it runs.
struct Section {
    each_way: Option<f64>,
    lane: f64,
    margin: f64,
}

const fn section(each_way: Option<f64>, lane: f64, margin: f64) -> Section {
    Section { each_way, lane, margin }
}

fn section_of(subtype: &str, class: &str) -> Section {
    if subtype == "rail" {
        // The gauge, the sleepers and the ballast's shoulder.
        return section(None, if class == "standard_gauge" { 4.0 } else { 3.5 }, 0.0);
    }
    match class {
        // Two lanes, a 3 m hard shoulder and a 1 m inner one.
        "motorway" => section(Some(2.0), LANE, 4.0),
        "trunk" => section(Some(2.0), LANE, 2.0),
        "primary" => section(Some(2.0), LANE, 1.0),
        "secondary" => section(Some(1.0), LANE, 2.0),
        "tertiary" => section(Some(1.0), LANE, 0.0),
        "residential" | "unclassified" => section(Some(1.0), 2.75, 0.0),
        "living_street" => section(Some(1.0), 2.5, 0.0),
        "service" => section(None, LANE, 0.0),
        "pedestrian" => section(None, 5.0, 0.0),
        "track" => section(None, 3.0, 0.0),
        "footway" | "steps" => section(None, 2.5, 0.0),
        "cycleway" => section(None, 2.0, 0.0),
        "path" | "bridleway" => section(None, 1.5, 0.0),
        _ => section(Some(1.0), 2.75, 0.0),
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct WayShape {
    pub measured: Option<f64>,
    pub oneway: bool,
    pub role: Role,
}

#[derive(Clone, Copy, Default)]
pub struct WayColumns<'a> {
    pub flags: Option<&'a str>,
    pub widths: Option<&'a str>,
    pub access: Option<&'a str>,
    pub subclass: Option<&'a str>,
    pub subclasses: Option<&'a str>,
}

impl WayShape {
    pub fn at(c: WayColumns, at: f64) -> WayShape {
        let role = match Role::at(c.subclass, c.subclasses, at) {
            Role::Way if flag_at(c.flags, LINK_FLAGS, 0.0, at) => Role::Link,
            role => role,
        };
        WayShape {
            measured: value_at(c.widths, at),
            oneway: is_oneway(c.access),
            role,
        }
    }
}

pub fn width_m(subtype: &str, class: Option<&str>, shape: WayShape) -> f64 {
    if let Some(w) = shape.measured.filter(|w| w.is_finite() && *w > 0.0) {
        return w;
    }
    let s = match shape.role {
        Role::Lane => section(None, 3.0, 0.0),
        Role::Sidewalk | Role::Crossing => section_of(subtype, "footway"),
        _ => section_of(subtype, class.unwrap_or("")),
    };
    let lanes = match s.each_way {
        _ if shape.role == Role::Link => 1.0,
        None => 1.0,
        Some(n) if shape.oneway => n,
        Some(n) => 2.0 * n,
    };
    lanes * s.lane + s.margin
}

/// One-way only where a rule denies every user one direction; a mode's rule
/// (a bus lane) does not count.
pub fn is_oneway(access: Option<&str>) -> bool {
    let Some(rules) = access.and_then(|a| serde_json::from_str::<serde_json::Value>(a).ok()) else {
        return false;
    };
    rules.as_array().into_iter().flatten().any(|r| {
        let when = &r["when"];
        r["access_type"] == "denied" && when["heading"].is_string() && ["mode", "using", "vehicle", "recognized", "during"].iter().all(|k| when[k].is_null())
    })
}
