// pxl-geo-spec/overture/division.rs

//! Reverse geocoding over Overture Divisions, for the live search and the
//! bake alike: the caller runs the two statements with its own client and
//! hands the rows back here to be read into a name.

use std::collections::BTreeMap;

use pxl_spec::place::{PlacePart, PlaceRank};

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

/// Every division containing each of a batch of points: `$1`/`$2` are
/// EPSG:3857 metres, `idx` counts the points from 1.
pub const CONTAINING_SQL: &str = "SELECT p.idx, d.gers_id, d.subtype, d.name, d.names_common
FROM unnest($1::float8[], $2::float8[]) WITH ORDINALITY AS p(x, y, idx)
JOIN overture_divisions d ON ST_Contains(d.geom, ST_SetSRID(ST_MakePoint(p.x, p.y), 3857))";

/// The nearest division point to each point, inside `$3`'s polygon (from
/// [`within`]) so it cannot cross a boundary, or within 2 km where no polygon
/// covers it: 2 km covers a dong without reaching the next. Most Korean
/// localities have a point and no polygon.
pub const NEAREST_POINT_SQL: &str = "SELECT p.idx, q.gers_id, q.subtype, q.name, q.names_common
FROM unnest($1::float8[], $2::float8[], $3::text[]) WITH ORDINALITY AS p(x, y, within, idx)
CROSS JOIN LATERAL (
  (SELECT q.gers_id, q.subtype, q.name, q.names_common
   FROM overture_divisions d JOIN overture_division_points q ON ST_Contains(d.geom, q.geom)
   WHERE d.gers_id = p.within
   ORDER BY q.geom <-> ST_SetSRID(ST_MakePoint(p.x, p.y), 3857) LIMIT 1)
  UNION ALL
  (SELECT q.gers_id, q.subtype, q.name, q.names_common
   FROM overture_division_points q
   WHERE p.within IS NULL AND ST_DWithin(q.geom, ST_SetSRID(ST_MakePoint(p.x, p.y), 3857), 2000)
   ORDER BY q.geom <-> ST_SetSRID(ST_MakePoint(p.x, p.y), 3857) LIMIT 1)
) q";

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
        Division { gers_id, subtype, name, names: names_of(names_common) }
    }

    /// A point of a subtype the ladder lacks still names a neighbourhood.
    pub fn rank(&self) -> PlaceRank {
        rank_of(&self.subtype).unwrap_or(PlaceRank::Neighborhood)
    }

    pub fn part(&self, approx: bool) -> PlacePart {
        PlacePart { rank: self.rank(), name: self.name.clone(), names: self.names.clone(), approx }
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
