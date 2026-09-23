// pxl-geo-bake/geocode/provider/overture.rs

//! Overture's Divisions theme resolved by point-in-polygon against the page's
//! own centre point (`overture_divisions`: region, county, locality,
//! neighborhood, microhood and macrohood — `country` is the only subtype
//! dropped).
//!
//! Because most of Korea's finest-level localities have no mapped polygon at
//! all, this always additionally looks for the nearest named `division` point
//! (`overture_division_points`) nested inside whatever polygon it already
//! matched, and appends it as an extra, finer part when its name is new — so
//! 분당구 (a real `locality` polygon) and 백현동 (only ever a point) both end up
//! in the label rather than only whichever one happened to have a polygon.
//!
//! One label per page, not per exact coordinate: reverse geocoding is baked at
//! the same L3 granularity terrain and POI already are, and a division polygon
//! — or the point-search radius — is always far larger than one page anyway.
//!
//! Divisions is OSM-derived (ODbL) inside Overture, unlike Places: this adopts
//! Overture here for its schema, not to escape OSM licensing the way the POI
//! switch did.

use postgres::{Client, Row, Statement};
use pxl_geo_spec::overture::division::{broad_rank, rank_of, FINE_SUBTYPE_PRIORITY};
use pxl_spec::bake::place_page::{leaf_slot, Division, PlaceGroup, APPROX, GROUP_PAGES, NO_LABEL};
use pxl_spec::place::PlaceRank;
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::pvf3::ivec::{self, IVec};

use crate::geocode::{names_of, PartSource, PlaceDict};

/// How far to look for a fallback `division` point when nothing narrower is
/// available any other way — the true edge of Overture's Divisions coverage,
/// where no polygon at all covers the point. A constrained lookup (inside an
/// already-matched polygon) does not need this radius; it only bounds the
/// last-resort unconstrained search. 2 km comfortably covers a typical dong's
/// radius without reaching into a genuinely different one, and a false match
/// beyond that would misname the area rather than just be imprecise about its
/// boundary.
const FINE_POINT_RADIUS_M: f64 = 2000.0;

/// Every division, in the one order that does not depend on when it is read.
/// The dictionary is written once from this and a label names a place by its
/// place in it, so a resumed bake has to number them the same way twice.
const ALL_DIVISIONS_SQL: &str = "SELECT gers_id, subtype, name, names_common
FROM overture_divisions
ORDER BY gers_id";

const CONTAINING_DIVISIONS_SQL: &str = "SELECT gers_id, subtype, name
FROM overture_divisions
WHERE ST_Contains(geom, ST_SetSRID(ST_MakePoint($1, $2), 3857))";

/// The nearest named `division` point actually inside a specific
/// already-matched polygon, used to find a finer label nested inside a coarser
/// one already resolved. Constraining to "inside this exact polygon" rather
/// than to a bare radius avoids picking a point that is merely nearby but
/// across a real administrative boundary.
const NEAREST_POINT_INSIDE_SQL: &str = "SELECT p.subtype, p.name, p.names_common
FROM overture_division_points p
JOIN overture_divisions d ON d.gers_id = $3
WHERE ST_Contains(d.geom, p.geom)
ORDER BY p.geom <-> ST_SetSRID(ST_MakePoint($1, $2), 3857)
LIMIT 1";

/// Last resort, only reached when no polygon at all — not even region or
/// county — covers the point.
const NEAREST_POINT_IN_RADIUS_SQL: &str = "SELECT subtype, name, names_common
FROM overture_division_points
WHERE ST_DWithin(geom, ST_SetSRID(ST_MakePoint($1, $2), 3857), $3)
ORDER BY geom <-> ST_SetSRID(ST_MakePoint($1, $2), 3857)
LIMIT 1";

struct Area {
    gers_id: String,
    subtype: String,
    /// Only to tell two matches apart: what it is called for a reader comes
    /// from the dictionary, which is read once rather than a page.
    name: String,
}

struct Point {
    subtype: String,
    name: String,
    common: Option<serde_json::Value>,
}

pub struct GeocodeProvider {
    containing: Statement,
    inside: Statement,
    in_radius: Statement,
}

impl GeocodeProvider {
    pub fn prepare(client: &mut Client) -> Result<GeocodeProvider, postgres::Error> {
        Ok(GeocodeProvider {
            containing: client.prepare(CONTAINING_DIVISIONS_SQL)?,
            inside: client.prepare(NEAREST_POINT_INSIDE_SQL)?,
            in_radius: client.prepare(NEAREST_POINT_IN_RADIUS_SQL)?,
        })
    }

    /// Every place a polygon names, paired with the id a label refers to it
    /// by. A subtype on no rung of the ladder is not one of them — nothing can
    /// refer to it, because `reverse` will not return it.
    pub fn divisions(&self, client: &mut Client) -> Result<Vec<(String, Division)>, postgres::Error> {
        Ok(client
            .query(ALL_DIVISIONS_SQL, &[])?
            .iter()
            .filter_map(|r: &Row| {
                let rank = rank_of(r.get("subtype"))?;
                Some((
                    r.get("gers_id"),
                    Division {
                        rank,
                        name: r.get("name"),
                        names: names_of(&r.get("names_common")).into_iter().collect(),
                    },
                ))
            })
            .collect())
    }

    pub fn reverse(&self, client: &mut Client, page_idx: IVec) -> Result<Option<Vec<PartSource>>, postgres::Error> {
        let bb = epsg3857::get_page_bbox(page_idx);
        let cx = (bb[0][0] + bb[1][0]) / 2.0;
        let cy = (bb[0][1] + bb[1][1]) / 2.0;

        let areas: Vec<Area> = client
            .query(&self.containing, &[&cx, &cy])?
            .iter()
            .map(|r: &Row| Area {
                gers_id: r.get("gers_id"),
                subtype: r.get("subtype"),
                name: r.get("name"),
            })
            .collect();

        let fine_from_polygon = FINE_SUBTYPE_PRIORITY.iter().find_map(|subtype| areas.iter().find(|a| a.subtype == *subtype));

        // Prefer constraining the point search to the smallest polygon already
        // matched — fine subtype first, then county, then region. Region is a
        // last resort, since "somewhere in this province" barely narrows
        // anything down but still beats an unconstrained search.
        let containing_id = fine_from_polygon
            .or_else(|| areas.iter().find(|a| a.subtype == "county"))
            .or_else(|| areas.iter().find(|a| a.subtype == "region"))
            .map(|a| a.gers_id.clone());

        let near = match &containing_id {
            Some(id) => client.query(&self.inside, &[&cx, &cy, id])?,
            None => client.query(&self.in_radius, &[&cx, &cy, &FINE_POINT_RADIUS_M])?,
        };
        let fine_point = near.first().map(|r| Point {
            subtype: r.get("subtype"),
            name: r.get("name"),
            common: r.get("names_common"),
        });

        // Overture's own Divisions extract has occasional overlapping
        // same-named polygons at a given subtype — two distinct `county` rows
        // both named 옹진군 containing the same point, and separately two
        // `region` rows both named 경기도 distinguished only by a `class` of
        // land versus maritime. That is a data-quality artifact upstream, not a
        // bug in the query. Deduping by name rather than by row is the cheap,
        // always-correct fix: a real label never needs the same name twice
        // regardless of why it showed up twice.
        let mut broad: Vec<&Area> = areas.iter().filter(|a| broad_rank(&a.subtype).is_some()).collect();
        broad.sort_by_key(|a| broad_rank(&a.subtype));

        let mut seen: Vec<&str> = Vec::new();
        let mut parts: Vec<PartSource> = Vec::new();
        for a in broad {
            if seen.contains(&a.name.as_str()) || rank_of(&a.subtype).is_none() {
                continue;
            }
            seen.push(&a.name);
            parts.push(PartSource::Division(a.gers_id.clone()));
        }
        if let Some(fine) = fine_from_polygon {
            if !seen.contains(&fine.name.as_str()) && rank_of(&fine.subtype).is_some() {
                seen.push(&fine.name);
                parts.push(PartSource::Division(fine.gers_id.clone()));
            }
        }
        // Appended, not substituted: a real polygon match and a nested
        // point-only place name two *different* real levels (분당구 against
        // 백현동), both worth showing — which is exactly the case a single
        // fine-subtype slot could not represent.
        if let Some(point) = &fine_point {
            if !seen.contains(&point.name.as_str()) {
                // A point carries a subtype like a polygon does, and it is the
                // rung the part sits on. A subtype the ladder has no rung for
                // is still the finest thing this page has a name from.
                parts.push(PartSource::Near(Division {
                    rank: rank_of(&point.subtype).unwrap_or(PlaceRank::Neighbourhood),
                    name: point.name.clone(),
                    names: names_of(&point.common).into_iter().collect(),
                }));
            }
        }

        Ok((!parts.is_empty()).then_some(parts))
    }
}

impl GeocodeProvider {
    /// One group: the label each of `leaves` takes, against `dict`.
    ///
    /// The caller passes the pages worth asking about — terrain's land list
    /// for the bake, every page of the group for a dev server with no such
    /// list — and a group none of them is in any place is nothing.
    pub fn group(&self, client: &mut Client, dict: &PlaceDict, leaves: &[IVec]) -> Result<Option<PlaceGroup>, String> {
        let mut group = PlaceGroup {
            labels: Vec::new(),
            pages: vec![NO_LABEL; GROUP_PAGES],
            locals: Vec::new(),
        };
        for &leaf in leaves {
            let Some(parts) = self.reverse(client, leaf).map_err(|e| e.to_string())? else { continue };
            let mut numbers: Vec<u32> = Vec::with_capacity(parts.len());
            for part in parts {
                numbers.push(match part {
                    PartSource::Division(id) => dict.number(&id).ok_or_else(|| format!("a label names a division the dictionary has not: {id}"))?,
                    PartSource::Near(place) => {
                        let i = group.locals.iter().position(|l| *l == place).unwrap_or_else(|| {
                            group.locals.push(place);
                            group.locals.len() - 1
                        });
                        i as u32 | APPROX
                    }
                });
            }
            // Every page of a district takes the same numbers, which is the
            // whole reason the group is the object: they are stored once and
            // each of its pages points at them.
            let label = group.labels.iter().position(|l| *l == numbers).unwrap_or_else(|| {
                group.labels.push(numbers);
                group.labels.len() - 1
            });
            group.pages[leaf_slot(ivec::X(leaf), ivec::Y(leaf))] = label as u16;
        }
        Ok((!group.labels.is_empty()).then_some(group))
    }
}
