// pxl-geo-bake/geocode/provider/overture.rs

//! Overture Divisions at each tile's center, by the rules `pxl-geo` answers
//! a search with (`pxl-geo-spec overture/division.rs`).

use postgres::{Client, Row, Statement};
use pxl_geo_spec::overture::division::{self, rank_of, Division, CONTAINING_SQL, NEAREST_POINT_SQL};
use pxl_spec::bake::place_tile::{leaf_slot, Named, PlaceGroup, APPROX, GROUP_TILES, LOCAL, NO_LABEL};
use pxl_spec::space::coord::adapter::epsg3857;
use pxl_spec::space::coord::pvf3::ivec::{self, IVec};
use pxl_spec::space::layer::tile::LEAF_ZOOM;

use crate::geocode::{PartSource, PlaceDict};

/// A stable order: a resumed bake must number places the same way.
const ALL_DIVISIONS_SQL: &str = concat!(
    "SELECT DISTINCT ON (gers_id) gers_id, subtype, name, ",
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

pub struct GeocodeProvider {
    containing: Statement,
    nearest: Statement,
}

fn division_of(row: &Row) -> (i64, Division) {
    (row.get("idx"), Division::new(row.get("gers_id"), row.get("subtype"), row.get("name"), row.get("names_common")))
}

fn named(d: &Division) -> Named {
    Named {
        rank: d.rank(),
        name: d.name.clone(),
        names: d.names.clone().into_iter().collect(),
    }
}

impl GeocodeProvider {
    pub fn prepare(client: &mut Client) -> Result<GeocodeProvider, postgres::Error> {
        Ok(GeocodeProvider {
            containing: client.prepare(CONTAINING_SQL)?,
            nearest: client.prepare(NEAREST_POINT_SQL)?,
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

    /// An area is a dictionary place; a point names a place near the tile, an
    /// approximation, and points are too many for the dictionary.
    fn reverse(&self, client: &mut Client, leaves: &[IVec]) -> Result<Vec<Vec<PartSource>>, postgres::Error> {
        let (xs, ys): (Vec<f64>, Vec<f64>) = leaves
            .iter()
            .map(|&leaf| {
                let bb = epsg3857::get_tile_bbox(LEAF_ZOOM, leaf);
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

    pub fn group(&self, client: &mut Client, dict: &PlaceDict, leaves: &[IVec]) -> Result<Option<PlaceGroup>, String> {
        let mut group = PlaceGroup {
            labels: Vec::new(),
            tiles: vec![NO_LABEL; GROUP_TILES],
            locals: Vec::new(),
        };
        let labels = self.reverse(client, leaves).map_err(|e| e.to_string())?;
        for (&leaf, parts) in leaves.iter().zip(labels) {
            if parts.is_empty() {
                continue;
            }
            let mut numbers: Vec<u32> = Vec::with_capacity(parts.len());
            for part in parts {
                numbers.push(match part {
                    PartSource::Shared(id) => dict.number(&id).ok_or_else(|| format!("a label names a place the dictionary has not: {id}"))?,
                    PartSource::Local { place, approx } => {
                        let i = group.locals.iter().position(|l| *l == place).unwrap_or_else(|| {
                            group.locals.push(place);
                            group.locals.len() - 1
                        });
                        i as u32 | LOCAL | if approx { APPROX } else { 0 }
                    }
                });
            }
            let label = group.labels.iter().position(|l| *l == numbers).unwrap_or_else(|| {
                group.labels.push(numbers);
                group.labels.len() - 1
            });
            group.tiles[leaf_slot(ivec::X(leaf), ivec::Y(leaf))] = label as u16;
        }
        Ok((!group.labels.is_empty()).then_some(group))
    }
}
