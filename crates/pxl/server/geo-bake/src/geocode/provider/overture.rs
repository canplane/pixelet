// pxl-geo-bake/geocode/provider/overture.rs

//! Overture Divisions by point-in-polygon at the page's center. Most Korean
//! localities have no polygon, so the nearest `division` point inside the
//! matched polygon is appended as a finer part.

use postgres::{Client, Row, Statement};
use pxl_geo_spec::overture::division::{broad_rank, rank_of, FINE_SUBTYPE_PRIORITY};
use pxl_spec::bake::place_page::{leaf_slot, Division, PlaceGroup, APPROX, GROUP_PAGES, NO_LABEL};
use pxl_spec::place::PlaceRank;
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::pvf3::ivec::{self, IVec};

use crate::geocode::{names_of, PartSource, PlaceDict};

/// Only for the unconstrained fallback where no polygon covers the point;
/// 2 km covers a dong without reaching the next.
const FINE_POINT_RADIUS_M: f64 = 2000.0;

/// A stable order: a resumed bake must number places the same way.
const ALL_DIVISIONS_SQL: &str = concat!(
    "SELECT gers_id, subtype, name, ",
    pxl_geo_spec::division_names!("overture_divisions"),
    " AS names_common
FROM overture_divisions
WHERE name IS NOT NULL AND subtype IN ",
    pxl_geo_spec::division_areas!(),
    "
ORDER BY gers_id"
);

/// Every statement the provider runs, for the catalog check.
pub const READS: [&str; 3] = [CONTAINING_SQL, NEAREST_POINT_SQL, ALL_DIVISIONS_SQL];

const CONTAINING_DIVISIONS_SQL: &str = "SELECT gers_id, subtype, name
FROM overture_divisions
WHERE ST_Contains(geom, ST_SetSRID(ST_MakePoint($1, $2), 3857))";

/// Inside the matched polygon rather than a radius, so it cannot cross a boundary.
const NEAREST_POINT_INSIDE_SQL: &str = "SELECT p.subtype, p.name, p.names_common
FROM overture_division_points p
JOIN overture_divisions d ON d.gers_id = $3
WHERE ST_Contains(d.geom, p.geom)
ORDER BY p.geom <-> ST_SetSRID(ST_MakePoint($1, $2), 3857)
LIMIT 1";

const NEAREST_POINT_IN_RADIUS_SQL: &str = "SELECT subtype, name, names_common
FROM overture_division_points
WHERE ST_DWithin(geom, ST_SetSRID(ST_MakePoint($1, $2), 3857), $3)
ORDER BY geom <-> ST_SetSRID(ST_MakePoint($1, $2), 3857)
LIMIT 1";

struct Area {
    gers_id: String,
    subtype: String,
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

    pub fn dictionary(&self, client: &mut Client) -> Result<Vec<(String, Named)>, postgres::Error> {
        Ok(client
            .query(ALL_DIVISIONS_SQL, &[])?
            .iter()
            .map(|r: &Row| Division::new(r.get("gers_id"), r.get("subtype"), r.get("name"), r.get("names_common")))
            .filter(|d| rank_of(&d.subtype).is_some())
            .map(|d| (d.gers_id.clone(), named(&d)))
            .collect())
    }

    /// An area is a dictionary place; a point names a place near the page, an
    /// approximation, and points are too many for the dictionary.
    fn reverse(&self, client: &mut Client, leaves: &[IVec]) -> Result<Vec<Vec<PartSource>>, postgres::Error> {
        let (xs, ys): (Vec<f64>, Vec<f64>) = leaves
            .iter()
            .map(|&leaf| {
                let bb = epsg3857::get_page_bbox(leaf);
                ((bb[0][0] + bb[1][0]) / 2.0, (bb[0][1] + bb[1][1]) / 2.0)
            })
            .unzip();
        let areas = division::by_point(leaves.len(), client.query(&self.containing, &[&xs, &ys])?.iter().map(division_of));
        let within: Vec<Option<String>> = areas.iter().map(|a| division::within(a)).collect();
        let reach: Vec<f64> = ys.iter().map(|&y| division::reach_at(y)).collect();
        let points = division::by_point(leaves.len(), client.query(&self.nearest, &[&xs, &ys, &within, &reach])?.iter().map(division_of));
        Ok(areas
            .iter()
            .zip(&points)
            .map(|(areas, point)| {
                division::label(areas, point.first())
                    .into_iter()
                    .map(|(d, approx)| {
                        if approx {
                            PartSource::Local { place: named(d), approx }
                        } else {
                            PartSource::Shared(d.gers_id.clone())
                        }
                    })
                    .collect()
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

        // Overture has duplicate same-named polygons (two 옹진군 counties, two
        // 경기도 regions), so dedupe by name.
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
        if let Some(point) = &fine_point {
            if !seen.contains(&point.name.as_str()) {
                // A point names a place near the page, an approximation, and points are too
                // many for the dictionary.
                parts.push(PartSource::Local {
                    place: Named {
                        rank: rank_of(&point.subtype).unwrap_or(PlaceRank::Neighborhood),
                        name: point.name.clone(),
                        names: names_of(&point.common).into_iter().collect(),
                    },
                    approx: true,
                });
            }
        }

        Ok((!parts.is_empty()).then_some(parts))
    }
}

impl GeocodeProvider {
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
            let label = group.labels.iter().position(|l| *l == numbers).unwrap_or_else(|| {
                group.labels.push(numbers);
                group.labels.len() - 1
            });
            group.pages[leaf_slot(ivec::X(leaf), ivec::Y(leaf))] = label as u16;
        }
        Ok((!group.labels.is_empty()).then_some(group))
    }
}
