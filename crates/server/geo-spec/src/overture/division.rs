// pxl-geo-spec/overture/division.rs

//! Reverse geocoding over Overture's divisions, for the live search and the
//! bake alike, read as Overture holds them: a division is a row of
//! `overture_divisions`, its parent named by `parent_division_id`, and
//! an area a polygon of one (`overture_division_areas`, its `division_id`). The
//! caller runs the statements with its own client and hands the rows back
//! here to be read into a name.

use fnv::FnvHashMap;
use std::collections::BTreeMap;

use pxl_spec::division::{DivisionPart, Subtype};
use pxl_spec::space::coord::adapter::epsg3857::{mer_from_u, u_from_mer, u_per_m};

/// `names.common` in the UI's languages as a JSON object, NULL where none, of
/// the row the given table or alias names.
#[macro_export]
macro_rules! division_names {
    ($t:literal) => {
        concat!(
            "(SELECT jsonb_object_agg(key, value) FROM jsonb_each(CASE jsonb_typeof(",
            $t,
            ".properties->'names'->'common') WHEN 'object' THEN ",
            $t,
            ".properties->'names'->'common' END) WHERE jsonb_typeof(value) = 'string' AND key IN ('en', 'ko') AND value #>> '{}' <> '')"
        )
    };
}

/// How far a point looks for the nearest division where no area of it
/// covers the point.
pub const NEAREST_REACH_M: f64 = 2048.0;

/// [`NEAREST_REACH_M`] at the DB's `y`, in the DB's coordinates.
pub fn reach_at(y: f64) -> f64 {
    mer_from_u(NEAREST_REACH_M * u_per_m(u_from_mer(y)))
}

/// The division each of a batch of points is in: `$1`/`$2` are EPSG:3857
/// metres and `$3` [`reach_at`] them, `idx` counts the points from 1. It is
/// the deepest division an area of contains the point, or, nearer, a child
/// of it with no area, the nearest within reach; where no area contains the
/// point, the nearest division within reach. Most Korean 동 have a point and
/// no area. `approx` marks a division found by nearness.
pub const FINEST_SQL: &str = "
SELECT p.idx, COALESCE(c.id, a.id, n.id) AS division_id, a.id IS NULL OR c.id IS NOT NULL AS approx
FROM unnest($1::float8[], $2::float8[], $3::float8[]) WITH ORDINALITY AS p(x, y, reach, idx)
CROSS JOIN LATERAL (SELECT ST_SetSRID(ST_MakePoint(p.x, p.y), 3857) AS pt) g
LEFT JOIN LATERAL (
  SELECT d.gers_id AS id FROM overture_division_areas s JOIN overture_divisions d ON d.gers_id = s.properties->>'division_id'
  WHERE ST_Contains(s.geom, g.pt)
  ORDER BY jsonb_array_length(d.properties->'hierarchies'->0) DESC, d.gers_id LIMIT 1
) a ON true
LEFT JOIN LATERAL (
  SELECT q.gers_id AS id FROM overture_divisions q
  WHERE a.id IS NOT NULL AND q.properties->>'parent_division_id' = a.id AND ST_DWithin(q.geom, g.pt, p.reach)
    AND NOT EXISTS (SELECT 1 FROM overture_division_areas qs WHERE qs.geom && q.geom AND qs.properties->>'division_id' = q.gers_id)
  ORDER BY q.geom <-> g.pt LIMIT 1
) c ON true
LEFT JOIN LATERAL (
  SELECT q.gers_id AS id FROM overture_divisions q
  WHERE a.id IS NULL AND ST_DWithin(q.geom, g.pt, p.reach)
  ORDER BY q.geom <-> g.pt LIMIT 1
) n ON true
WHERE COALESCE(c.id, a.id, n.id) IS NOT NULL";

/// The divisions `$1` names and every ancestor of each, as the division's
/// own hierarchy lists them.
pub const LINEAGE_SQL: &str = concat!(
    "SELECT d.gers_id, d.subtype, d.name, ",
    division_names!("d"),
    " AS names_common, d.properties->>'parent_division_id' AS parent
FROM overture_divisions d
WHERE d.gers_id IN (
  SELECT e->>'division_id' FROM overture_divisions f, jsonb_array_elements(f.properties->'hierarchies'->0) e WHERE f.gers_id = ANY($1)
  UNION SELECT unnest($1::text[])
)"
);

/// A division as the statements return it.
#[derive(Clone, Debug)]
pub struct Division {
    pub gers_id: String,
    /// None for a subtype newer than this tree, which no name reads.
    pub subtype: Option<Subtype>,
    pub name: String,
    pub names: BTreeMap<String, String>,
    pub parent: Option<String>,
}

impl Division {
    pub fn new(gers_id: String, subtype: &str, name: String, names_common: Option<serde_json::Value>, parent: Option<String>) -> Division {
        Division {
            gers_id,
            subtype: Subtype::named(subtype),
            name,
            names: names_of(names_common),
            parent,
        }
    }

    fn part(&self, approx: bool) -> Option<DivisionPart> {
        Some(DivisionPart {
            subtype: self.subtype?,
            name: self.name.clone(),
            names: self.names.clone(),
            approx,
        })
    }
}

fn names_of(common: Option<serde_json::Value>) -> BTreeMap<String, String> {
    let Some(serde_json::Value::Object(map)) = common else { return BTreeMap::new() };
    map.into_iter().filter_map(|(lang, name)| name.as_str().map(|n| (lang, n.to_string()))).collect()
}

/// [`FINEST_SQL`]'s rows by point; `idx` counts from 1.
pub fn by_point(points: usize, rows: impl IntoIterator<Item = (i64, String, bool)>) -> Vec<Option<(String, bool)>> {
    let mut out = vec![None; points];
    for (idx, id, approx) in rows {
        out[(idx - 1) as usize] = Some((id, approx));
    }
    out
}

/// A division's name, broad to narrow: it and each parent [`LINEAGE_SQL`]
/// held; only the division itself can be approximate. A walk longer than
/// there are subtypes is a cycle, and stops.
pub fn name(id: &str, approx: bool, lineage: &FnvHashMap<String, Division>) -> Vec<DivisionPart> {
    let mut parts: Vec<DivisionPart> = core::iter::successors(lineage.get(id), |d| d.parent.as_ref().and_then(|p| lineage.get(p)))
        .take(Subtype::ALL.len())
        .enumerate()
        .filter_map(|(k, d)| d.part(approx && k == 0))
        .collect();
    parts.reverse();
    parts
}

#[cfg(test)]
#[path = "division.test.rs"]
mod test;
