// pxl-geo-bake/terrain/pipeline/overture/width.rs

//! A way's width in metres: the one width it is painted at on the ground and
//! carried at on a deck (deck/), so a road is one width on and off a bridge.
//!
//! The source says it rarely. `width_rules` is on about one segment in a
//! hundred, and much of it is categorical — an import's width classes, 5.0
//! and 10.0 for whole bands of roads — but it is the source's own word and is
//! taken where it is. Elsewhere a way is as wide as its lanes, a lane as wide
//! as international practice has one: 3.5 m for a motor lane, the default
//! osm2lanes (A/B Street) takes for a locale it has no figure for, the
//! Netherlands' being 3.35 m and Britain's 3.0 m; 2.5 m a footway's and 2.0 m
//! a cycleway's, the same source's. How many lanes a class carries each way
//! and the margin past them — shoulders, kerbs — is the classes' usual
//! cross-section, which Arnis (OSM to Minecraft) lands on too: eleven metres
//! for a motorway's carriageway, nine for a secondary, seven for a tertiary.
//!
//! A one-way way is one carriageway of a road drawn as two, or a lane of its
//! own, and carries its lanes one way; any other carries them both ways. A
//! link — a slip road — is one lane, and so is a service road's own lane
//! (role.rs), a little narrower than a through lane.

use crate::terrain::pipeline::overture::between::{flag_at, value_at};
use crate::terrain::pipeline::overture::policy::LINK_FLAGS;
use crate::terrain::pipeline::overture::role::Role;

/// A motor lane's width.
const LANE: f64 = 3.5;

/// A class's cross-section: lanes each way, each lane's width, and the
/// margin across the whole carriageway. `each_way` none is a way of one lane
/// whichever way it runs — a service road, a track, a footway.
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
        // A track's bed: the gauge, the sleepers and the ballast's shoulder.
        return section(None, if class == "standard_gauge" { 4.0 } else { 3.5 }, 0.0);
    }
    match class {
        // Two lanes, a 3 m hard shoulder and a 1 m inner one.
        "motorway" => section(Some(2.0), LANE, 4.0),
        "trunk" => section(Some(2.0), LANE, 2.0),
        "primary" => section(Some(2.0), LANE, 1.0),
        "secondary" => section(Some(1.0), LANE, 2.0),
        "tertiary" => section(Some(1.0), LANE, 0.0),
        // A local street's lane is narrower than a through road's.
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

/// What a way says of its width.
#[derive(Clone, Copy, Debug, Default)]
pub struct WayShape {
    /// The source's width, where it gives one.
    pub measured: Option<f64>,
    pub oneway: bool,
    pub role: Role,
}

/// The columns of a way that say what it is along its length.
#[derive(Clone, Copy, Default)]
pub struct WayColumns<'a> {
    pub flags: Option<&'a str>,
    pub widths: Option<&'a str>,
    pub access: Option<&'a str>,
    pub subclass: Option<&'a str>,
    pub subclasses: Option<&'a str>,
}

impl WayShape {
    /// What a way's columns say of it at fraction `at` of its length.
    pub fn at(c: WayColumns, at: f64) -> WayShape {
        let role = match Role::at(c.subclass, c.subclasses, at) {
            Role::Way if flag_at(c.flags, LINK_FLAGS, at) => Role::Link,
            role => role,
        };
        WayShape {
            measured: value_at(c.widths, at),
            oneway: is_oneway(c.access),
            role,
        }
    }
}

/// A way's width in metres.
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

/// Whether a way's `access_restrictions` make it one-way: a rule that
/// denies every user travel one way along it, where a rule for a mode or a
/// vehicle — a bus lane, a lorry ban — does not.
pub fn is_oneway(access: Option<&str>) -> bool {
    let Some(rules) = access.and_then(|a| serde_json::from_str::<serde_json::Value>(a).ok()) else {
        return false;
    };
    rules.as_array().into_iter().flatten().any(|r| {
        let when = &r["when"];
        r["access_type"] == "denied" && when["heading"].is_string() && ["mode", "using", "vehicle", "recognized", "during"].iter().all(|k| when[k].is_null())
    })
}
