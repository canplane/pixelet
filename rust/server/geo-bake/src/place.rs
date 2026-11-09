// pxl-geo-bake/place.rs

use std::collections::BTreeMap;

use crate::coverage::Coverage;
use crate::pyramid::Base;
use postgres::Client;
use pxl_geo_spec::overture::properties::CATEGORY_SQL;
use pxl_spec::bake::tile::place::{self, category_of, PlaceTile, SourcePlace};
use pxl_spec::space::coord::adapter::epsg3857;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::zoom::{idx_of, LEAF_ZOOM};
use pxl_spec::space::layer::quadrant::Quadrant;

impl Base for PlaceTile {
    fn encode(&self, at: Quadrant) -> Result<Vec<u8>, String> {
        place::encode(self).map_err(|e| format!("{}: {e}", at.key()))
    }

    fn decode(raw: &[u8]) -> Result<Self, String> {
        place::decode(raw).map_err(|e| e.to_string())
    }

    fn fold_tile<'a>(p: Quadrant, get: impl Fn(Quadrant) -> Option<&'a Self>) -> Option<Self> {
        let [a, b, c, d] = p.children().map(get);
        Some(place::fold(p, [a?, b?, c?, d?]))
    }
}

const PLACES_SQL: &str =
    "SELECT ST_X(geom), ST_Y(geom), name, {category}, gers_id FROM overture_places";

/// Every statement the places bake runs, for the catalog check.
pub const READS: [&str; 2] = [PLACES_SQL, CATEGORY_SQL];

/// The leaves under `within`, a leaf or a shard, that hold a place: a place
/// belongs to the leaf its projected position floors into, and is published
/// only on a tile of the coverage.
pub fn leaves(
    client: &mut Client,
    coverage: &Coverage,
    within: Quadrant,
) -> Result<BTreeMap<Quadrant, PlaceTile>, String> {
    let select = PLACES_SQL.replace("{category}", CATEGORY_SQL);
    let [[x0, y0, _], [x1, y1, _]] = epsg3857::get_tile_bbox(within.zoom, within.at);
    let rows = client
        .query(
            &format!("{select} WHERE geom && ST_MakeEnvelope($1, $2, $3, $4, 3857)"),
            &[&x0, &y0, &x1, &y1],
        )
        .map_err(|e| e.to_string())?;
    let mut places: BTreeMap<Quadrant, Vec<(Q3, String, u16, String)>> = BTreeMap::new();
    for row in rows {
        let at = epsg3857::project(row.get(0), row.get(1), 0.0);
        let [x, y, _] = idx_of(LEAF_ZOOM, &at);
        let leaf = Quadrant::new(LEAF_ZOOM, [x, y]);
        if Quadrant::over(within.zoom, leaf.at) != within || !coverage.contains(leaf.at) {
            continue;
        }
        let category = category_of(row.get::<_, Option<&str>>(3).unwrap_or(""));
        places
            .entry(leaf)
            .or_default()
            .push((at, row.get(2), category, row.get(4)));
    }
    Ok(places
        .into_iter()
        .map(|(leaf, p)| {
            (
                leaf,
                PlaceTile::leaf(
                    leaf,
                    p.iter().map(|(point, name, category, id)| {
                        (
                            *point,
                            SourcePlace {
                                name,
                                kind: *category,
                                id,
                            },
                        )
                    }),
                ),
            )
        })
        .filter(|(_, tile)| !tile.is_empty())
        .collect())
}
