// pxl-geo-spec/overture/division.rs

//! Reverse geocoding over Overture's divisions, for the live search and the
//! bake alike, read as Overture holds them: a division is a row of
//! `overture_divisions`, its parent named by `parent_division_id`, and
//! an area a polygon of one (`overture_division_areas`, its `division_id`).
//! A point is in the deepest division one of whose areas holds it, the lesser
//! id at one depth; under that, in the child with no area whose point is
//! nearest, and so on down (`steps`). Each reader gathers the areas and the
//! children its own way and runs the statements with its own client: the bake
//! a leaf tile's, the children within `BAKE_READ_DIST` of the tile, and the
//! search a point's, the children within `BAKE_READ_DIST` of the point.

use fnv::FnvHashMap;

use pxl_spec::division::{DivisionPart, Subtype};

/// The divisions `$1` names and every parent of each.
pub const LINEAGE_SQL: &str = "WITH RECURSIVE up AS (
  SELECT d.gers_id, d.properties FROM overture_divisions d WHERE d.gers_id = ANY($1)
  UNION SELECT p.gers_id, p.properties FROM overture_divisions p JOIN up ON p.gers_id = up.properties->>'parent_division_id'
)
SELECT up.gers_id, up.properties->>'subtype' AS subtype, up.properties->'names'->>'primary' AS name, up.properties->>'parent_division_id' AS parent,
  (up.properties->'cartography'->>'prominence')::int AS prominence,
  EXISTS (SELECT 1 FROM overture_division_areas s WHERE s.properties->>'division_id' = up.gers_id) AS area
FROM up";

/// Each division's children with no area, each with its point in EPSG:3857
/// meters.
pub type Children = FnvHashMap<String, Vec<([f64; 2], String)>>;

/// The divisions a point at `at` steps down through from `first`, the
/// division whose area holds it: to the child in `children` whose point is
/// nearest, then to that one's nearest child, as far as there is one.
/// Equal distances go to the lesser id, and a division already on the way down
/// is not taken again.
pub fn steps<'a>(first: &'a str, at: [f64; 2], children: &'a Children) -> Vec<&'a str> {
    let mut taken = vec![first];
    while let Some(kids) = children.get(taken[taken.len() - 1]) {
        let nearest = kids
            .iter()
            .filter(|(_, id)| !taken.contains(&id.as_str()))
            .map(|(p, id)| {
                let [dx, dy] = [p[0] - at[0], p[1] - at[1]];
                (dx * dx + dy * dy, id)
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(b.1)));
        let Some((_, id)) = nearest else { break };
        taken.push(id);
    }
    taken.split_off(1)
}

/// A division as the statements return it.
#[derive(Clone, Debug)]
pub struct Division {
    pub gers_id: String,
    /// None for a subtype this tree does not know, which no name reads.
    pub subtype: Option<Subtype>,
    pub name: String,
    pub parent: Option<String>,
}

impl Division {
    pub fn new(
        gers_id: String,
        subtype: Option<&str>,
        name: String,
        parent: Option<String>,
    ) -> Division {
        Division {
            gers_id,
            subtype: subtype.and_then(Subtype::of_word),
            name,
            parent,
        }
    }

    fn part(&self, approx: bool) -> Option<DivisionPart> {
        Some(DivisionPart {
            subtype: self.subtype?,
            name: self.name.clone(),
            approx,
        })
    }
}

/// A division's name, broad to narrow: it and each parent [`LINEAGE_SQL`]
/// held, up to its country, as Overture's hierarchy is a tree whose every
/// division but a country has a parent, its subtypes repeating or out of order
/// at places; only the division itself can be approximate.
pub fn name(id: &str, approx: bool, lineage: &FnvHashMap<String, Division>) -> Vec<DivisionPart> {
    let mut parts: Vec<DivisionPart> = core::iter::successors(lineage.get(id), |d| {
        d.parent.as_ref().and_then(|p| lineage.get(p))
    })
    .enumerate()
    .filter_map(|(k, d)| d.part(approx && k == 0))
    .collect();
    parts.reverse();
    parts
}

#[cfg(test)]
#[path = "division.test.rs"]
mod test;
