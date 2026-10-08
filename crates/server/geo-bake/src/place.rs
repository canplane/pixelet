// pxl-geo-bake/place.rs

use std::collections::BTreeMap;

use crate::coverage::Coverage;
use postgres::Client;
use pxl_geo_spec::overture::properties::CATEGORY_SQL;
use pxl_spec::bake::format::Occupancy;
use pxl_spec::bake::place_tile::{self, fold_place, kind_of, Place, PlaceTile};
use pxl_spec::space::coord::adapter::epsg3857;
use pxl_spec::space::coord::pvf3::ivec::IVec;
use pxl_spec::space::coord::pvf3::pvf3::Pvf3;
use pxl_spec::space::coord::zoom::idx;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};

impl crate::pyramid::Layer for PlaceTile {
    fn encode(&self) -> Result<Vec<u8>, String> {
        place_tile::encode(self).map_err(|e| e.to_string())
    }

    fn decode(raw: &[u8]) -> Result<Self, String> {
        place_tile::decode(raw).map_err(|e| e.to_string())
    }

    fn fold_tile<'a>(p: Tile, held: impl Fn(Tile) -> Option<&'a Self>) -> Option<Self> {
        let [a, b, c, d] = p.children().map(held);
        Some(fold_place(p, [a?, b?, c?, d?]))
    }

    fn occupancy(&self) -> Occupancy {
        self.occupancy
    }
}

const PLACES_SQL: &str = "SELECT ST_X(geom), ST_Y(geom), name, {category}, gers_id FROM overture_places";

/// Every statement the places bake runs, for the catalog check.
pub const READS: [&str; 2] = [PLACES_SQL, CATEGORY_SQL];

/// The leaves under `within`, a leaf or a shard, that hold a place: a place
/// belongs to the leaf its projected position floors into, and is published
/// only on a tile of the coverage.
pub fn leaves(client: &mut Client, coverage: &Coverage, within: Tile) -> Result<BTreeMap<IVec, PlaceTile>, String> {
    let select = PLACES_SQL.replace("{category}", CATEGORY_SQL);
    let [[x0, y0, _], [x1, y1, _]] = epsg3857::get_tile_bbox(within.zoom, within.tile);
    let rows = client
        .query(&format!("{select} WHERE geom && ST_MakeEnvelope($1, $2, $3, $4, 3857)"), &[&x0, &y0, &x1, &y1])
        .map_err(|e| e.to_string())?;
    let mut places: BTreeMap<IVec, Vec<(Pvf3, String, u16, String)>> = BTreeMap::new();
    for row in rows {
        let at = epsg3857::project(row.get(0), row.get(1), 0.0);
        let leaf = Tile::over(LEAF_ZOOM, idx(LEAF_ZOOM, at.i));
        if Tile::over(within.zoom, leaf.tile) != within || !coverage.contains(leaf.tile) {
            continue;
        }
        let kind = kind_of(row.get::<_, Option<&str>>(3).unwrap_or(""));
        places.entry(leaf.tile).or_default().push((at, row.get(2), kind, row.get(4)));
    }
    Ok(places
        .into_iter()
        .map(|(at, p)| {
            (
                at,
                PlaceTile::leaf(Tile::new(LEAF_ZOOM, at), p.iter().map(|(point, name, kind, id)| (*point, Place { name, kind: *kind, id }))),
            )
        })
        .filter(|(_, tile)| !tile.is_empty())
        .collect())
}
