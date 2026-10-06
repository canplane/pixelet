// pxl-geo-bake/segment.rs

//! An Overture transportation segment as the bake reads it: cut where any
//! rule it carries begins or ends, so each part holds one value of each
//! along its whole length, as an OSM way holds its tags, which the
//! renderers and converters the bake follows assume. What is not in use, a
//! way abandoned, disused or under construction, is no part at all.

use pxl_spec::space::coord::adapter::epsg3857::G3857;

use crate::terrain::pipeline::overture::between::{cell_of, Line, Rules};
use crate::terrain::pipeline::overture::role::Role;
use crate::wkb::wkb_to_geometries;

/// The columns of `overture_transportation t` a [`Segment`] is read from.
#[macro_export]
macro_rules! segment_columns {
    () => {
        "t.gers_id, t.subtype, t.class, COALESCE(t.road_flags, t.rail_flags)::text AS flags, t.level_rules::text AS levels,
 t.subclass, t.subclass_rules::text AS subclasses, ST_AsBinary(t.geom) AS geom_wkb"
    };
}

/// The flags of a way that is not in use, which nothing draws.
const GONE: &[&str] = &["is_under_construction", "is_abandoned", "is_disused"];

/// What a part's rules say, the one value of each along it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Tags {
    pub bridge: bool,
    pub tunnel: bool,
    pub indoor: bool,
    /// A bridge's or a tunnel's OSM `layer` (`level_rules`), where given.
    pub level: Option<i64>,
    /// Another way's `layer` of 1 or more: what Arnis lets carry a bridge down.
    pub raised: bool,
    pub role: Role,
}

/// A part by identity: its segment and where on it it begins.
pub type PartId = (String, u64);

/// A part of a segment over which every rule it carries holds one value.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Segment {
    pub gers_id: String,
    /// Where it lies on its segment, as fractions of the segment's length.
    pub span: [f64; 2],
    /// Its segment's whole length, in DB coordinates.
    pub segment_length: f64,
    pub subtype: String,
    pub class: Option<String>,
    pub tags: Tags,
    pub ends: [End; 2],
    pub points: Vec<G3857>,
}

/// An end by identity, never by a computed point: a segment's own first or
/// last vertex, which every segment at that connector carries bit for bit,
/// or a cut at a fraction of one segment, which only its parts share.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum End {
    Vertex(u64, u64),
    Cut(String, u64),
    /// Where the ring of tiles a solve reads cuts a part, at its `k`-th sample.
    Ring(PartId, u64),
}

impl End {
    /// The vertex at `p`. `+ 0.0` makes −0 the bits of 0, as `==` has it.
    pub fn at(p: [f64; 2]) -> End {
        End::Vertex((p[0] + 0.0).to_bits(), (p[1] + 0.0).to_bits())
    }

    /// The end at fraction `at` of segment `gers_id`, which runs from `first`
    /// to `last`.
    pub fn of(gers_id: &str, [first, last]: [G3857; 2], at: f64) -> End {
        if at == 0.0 {
            End::at([first[0], first[1]])
        } else if at == 1.0 {
            End::at([last[0], last[1]])
        } else {
            End::Cut(gers_id.to_string(), (at + 0.0).to_bits())
        }
    }
}

impl Segment {
    /// A row's parts, in order along it; none of a row with no line.
    pub fn of(r: &postgres::Row) -> Vec<Segment> {
        let (gers_id, subtype, class, subclass): (String, String, Option<String>, Option<String>) = (r.get("gers_id"), r.get("subtype"), r.get("class"), r.get("subclass"));
        let [flags, levels, subclasses] = ["flags", "levels", "subclasses"].map(|c| Rules::of(r.get::<_, Option<&str>>(c)));
        let Some(g) = wkb_to_geometries(r.get::<_, &[u8]>("geom_wkb")).into_iter().next() else {
            return Vec::new();
        };
        let Some(line) = Line::of(&g.points) else { return Vec::new() };
        let (whole, length) = ([g.points[0], g.points[g.points.len() - 1]], line.length());
        let flag = |wanted: &[&str]| flags.flagged(wanted, cell_of(length));
        let (bridge, tunnel, indoor, link, gone) = (flag(&["is_bridge"]), flag(&["is_tunnel"]), flag(&["is_indoor"]), flag(&["is_link"]), flag(GONE));
        let holds = |s: &[[f64; 2]], at: f64| s.iter().any(|&[a, b]| a <= at && at <= b);
        let mut cuts = vec![0.0, 1.0];
        cuts.extend([&bridge, &tunnel, &indoor, &link, &gone].into_iter().flatten().flatten().copied());
        cuts.extend(levels.bounds().chain(subclasses.bounds()));
        cuts.sort_by(f64::total_cmp);
        cuts.dedup();
        // each run between cuts with what holds over it; a cut no value changes across joins its runs
        let mut runs: Vec<([f64; 2], Tags)> = Vec::new();
        for w in cuts.windows(2).filter(|w| w[0] < w[1]) {
            let mid = (w[0] + w[1]) / 2.0;
            if holds(&gone, mid) {
                continue;
            }
            let role = match Role::at(subclass.as_deref(), &subclasses, mid) {
                Role::Way if holds(&link, mid) => Role::Link,
                role => role,
            };
            let (bridge, tunnel) = (holds(&bridge, mid), holds(&tunnel, mid));
            let level = levels.at(mid).and_then(|v| v.as_f64()).map(|v| v as i64);
            let here = Tags {
                bridge,
                tunnel,
                indoor: holds(&indoor, mid),
                level: level.filter(|_| bridge || tunnel),
                raised: !(bridge || tunnel) && level.is_some_and(|l| l >= 1),
                role,
            };
            match runs.last_mut() {
                Some((span, prev)) if span[1] == w[0] && *prev == here => span[1] = w[1],
                _ => runs.push(([w[0], w[1]], here)),
            }
        }
        let part = |(span, tags): ([f64; 2], Tags), points: Vec<G3857>| Segment {
            gers_id: gers_id.clone(),
            span,
            segment_length: length,
            subtype: subtype.clone(),
            class: class.clone(),
            tags,
            ends: span.map(|at| End::of(&gers_id, whole, at)),
            points,
        };
        // a part that is its whole segment takes the segment's own points
        if runs.len() == 1 && runs[0].0 == [0.0, 1.0] {
            drop(line);
            return vec![part(runs[0], g.points)];
        }
        runs.into_iter().map(|run| part(run, line.between(run.0[0], run.0[1]))).collect()
    }

    pub fn id(&self) -> PartId {
        (self.gers_id.clone(), self.span[0].to_bits())
    }

    pub fn is_subway(&self) -> bool {
        self.subtype == "rail" && self.class.as_deref() == Some("subway")
    }

    /// Its length, in DB coordinates.
    pub fn length(&self) -> f64 {
        (self.span[1] - self.span[0]) * self.segment_length
    }
}
