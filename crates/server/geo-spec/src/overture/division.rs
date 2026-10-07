// pxl-geo-spec/overture/division.rs

//! Reverse geocoding over Overture Divisions, for the live search and the
//! bake alike: the caller runs the two statements with its own client and
//! hands the rows back here to be read into a name.

use std::collections::BTreeMap;

use pxl_spec::place::{PlacePart, PlaceRank};
use pxl_spec::space::coord::adapter::epsg3857::{mer_from_u, u_from_mer, u_per_m};
use pxl_spec::space::coord::zoom::{units, SHARD_ZOOM};
use pxl_spec::space::hierarchy::morton;

/// The three sub-city subtypes share a rung because Overture does not
/// guarantee they nest.
pub fn rank_of(subtype: &str) -> Option<PlaceRank> {
    Some(match subtype {
        "region" => PlaceRank::Region,
        "county" => PlaceRank::County,
        "macrohood" => PlaceRank::District,
        "locality" | "neighborhood" | "microhood" => PlaceRank::Neighborhood,
        _ => return None,
    })
}

const BROAD_SUBTYPE_ORDER: [&str; 2] = ["region", "county"];

/// A formal 동 (`locality`) wins over an informal area at the same point.
const FINE_SUBTYPE_PRIORITY: [&str; 4] = ["locality", "neighborhood", "microhood", "macrohood"];

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

/// The area subtypes a name is built from; `country` is above the label.
#[macro_export]
macro_rules! division_areas {
    () => {
        "('region', 'county', 'locality', 'neighborhood', 'microhood', 'macrohood')"
    };
}

/// The point subtypes: a province or a city always has its polygon.
#[macro_export]
macro_rules! division_points {
    () => {
        "('locality', 'neighborhood', 'microhood', 'macrohood')"
    };
}

/// How far [`NEAREST_POINT_SQL`] looks where no polygon covers a point.
pub const NEAREST_REACH_M: f64 = 2048.0;

/// [`NEAREST_REACH_M`] at the DB's `y`, in the DB's coordinates.
pub fn reach_at(y: f64) -> f64 {
    mer_from_u(NEAREST_REACH_M * u_per_m(u_from_mer(y)))
}

/// The shard a point lies in, by name. A division is a row in every shard
/// its apron meets, so one containing the point is a row in the point's
/// shard: that copy alone is read, not each of a region's dozens.
/// A point outside every ingested shard names nothing: an apron holds only
/// what reaches into it, so what it would answer there is partial.
pub fn shard_of(x: f64, y: f64) -> String {
    let at = [x, y].map(|m| (u_from_mer(m) / units(SHARD_ZOOM)).floor() as i64);
    morton::name::<2>(morton::key_of(SHARD_ZOOM, at))
}

/// Every division containing each of a batch of points: `$1`/`$2` are
/// EPSG:3857 metres, `$3` each point's shard ([`shard_of`]), `idx` counts the
/// points from 1.
pub const CONTAINING_SQL: &str = concat!(
    "SELECT p.idx, d.gers_id, d.subtype, d.name, ",
    division_names!("d"),
    " AS names_common
FROM unnest($1::float8[], $2::float8[], $3::text[]) WITH ORDINALITY AS p(x, y, node, idx)
JOIN overture_divisions d ON d.node = p.node AND ST_Contains(d.geom, ST_SetSRID(ST_MakePoint(p.x, p.y), 3857))
WHERE d.name IS NOT NULL AND d.subtype IN ",
    division_areas!()
);

/// The nearest division point to each point, inside `$3`'s polygon (from
/// [`within`], its copy in `$5`, the point's shard) so it cannot cross a
/// boundary, or within `$4` ([`reach_at`]) where no polygon covers it. Most Korean localities have a point and no
/// polygon.
pub const NEAREST_POINT_SQL: &str = concat!(
    "SELECT p.idx, q.gers_id, q.subtype, q.name, q.names_common
FROM unnest($1::float8[], $2::float8[], $3::text[], $4::float8[], $5::text[]) WITH ORDINALITY AS p(x, y, within, reach, node, idx)
CROSS JOIN LATERAL (
  (SELECT q.gers_id, q.subtype, q.name, ",
    division_names!("q"),
    " AS names_common
   FROM overture_divisions d JOIN overture_division_points q ON ST_Contains(d.geom, q.geom)
   WHERE d.gers_id = p.within AND d.node = p.node AND q.name IS NOT NULL AND q.subtype IN ",
    division_points!(),
    "
   ORDER BY q.geom <-> ST_SetSRID(ST_MakePoint(p.x, p.y), 3857) LIMIT 1)
  UNION ALL
  (SELECT q.gers_id, q.subtype, q.name, ",
    division_names!("q"),
    " AS names_common
   FROM overture_division_points q
   WHERE p.within IS NULL AND ST_DWithin(q.geom, ST_SetSRID(ST_MakePoint(p.x, p.y), 3857), p.reach) AND q.name IS NOT NULL AND q.subtype IN ",
    division_points!(),
    "
   ORDER BY q.geom <-> ST_SetSRID(ST_MakePoint(p.x, p.y), 3857) LIMIT 1)
) q"
);

/// A division polygon or point, as both statements return it.
#[derive(Clone, Debug)]
pub struct Division {
    pub gers_id: String,
    pub subtype: String,
    pub name: String,
    pub names: BTreeMap<String, String>,
}

impl Division {
    pub fn new(gers_id: String, subtype: String, name: String, names_common: Option<serde_json::Value>) -> Division {
        Division {
            gers_id,
            subtype,
            name,
            names: names_of(names_common),
        }
    }

    /// A point of a subtype the ladder lacks still names a neighbourhood.
    pub fn rank(&self) -> PlaceRank {
        rank_of(&self.subtype).unwrap_or(PlaceRank::Neighborhood)
    }

    pub fn part(&self, approx: bool) -> PlacePart {
        PlacePart {
            rank: self.rank(),
            name: self.name.clone(),
            names: self.names.clone(),
            approx,
        }
    }
}

fn names_of(common: Option<serde_json::Value>) -> BTreeMap<String, String> {
    let Some(serde_json::Value::Object(map)) = common else { return BTreeMap::new() };
    map.into_iter().filter_map(|(lang, name)| name.as_str().map(|n| (lang, n.to_string()))).collect()
}

/// Rows of either statement, by point; `idx` counts from 1.
pub fn by_point(points: usize, rows: impl IntoIterator<Item = (i64, Division)>) -> Vec<Vec<Division>> {
    let mut out: Vec<Vec<Division>> = (0..points).map(|_| Vec::new()).collect();
    for (idx, d) in rows {
        out[(idx - 1) as usize].push(d);
    }
    out
}

fn fine(areas: &[Division]) -> Option<&Division> {
    FINE_SUBTYPE_PRIORITY.iter().find_map(|subtype| areas.iter().find(|a| a.subtype == *subtype))
}

/// The polygon a point's nearest division point is looked for in, as
/// [`NEAREST_POINT_SQL`]'s `$3`: the finest containing area.
pub fn within(areas: &[Division]) -> Option<String> {
    fine(areas)
        .or_else(|| BROAD_SUBTYPE_ORDER.iter().rev().find_map(|subtype| areas.iter().find(|a| a.subtype == *subtype)))
        .map(|a| a.gers_id.clone())
}

/// The broad areas, broad to narrow, each name once: Overture has duplicate
/// same-named polygons (two 옹진군 counties, two 경기도 regions).
pub fn broad(areas: &[Division]) -> Vec<&Division> {
    let mut seen: Vec<&Division> = Vec::new();
    for subtype in BROAD_SUBTYPE_ORDER {
        for a in areas.iter().filter(|a| a.subtype == subtype) {
            if !seen.iter().any(|s| s.name == a.name) {
                seen.push(a);
            }
        }
    }
    seen
}

/// A point's name, broad to narrow: the broad areas, the finest area, then the
/// nearest point, which is an approximation (`true`).
pub fn label<'a>(areas: &'a [Division], point: Option<&'a Division>) -> Vec<(&'a Division, bool)> {
    let mut parts: Vec<(&Division, bool)> = broad(areas).into_iter().map(|a| (a, false)).collect();
    for (d, approx) in fine(areas).map(|a| (a, false)).into_iter().chain(point.map(|p| (p, true))) {
        if !parts.iter().any(|(s, _)| s.name == d.name) {
            parts.push((d, approx));
        }
    }
    parts
}

#[cfg(test)]
#[path = "division.test.rs"]
mod test;
