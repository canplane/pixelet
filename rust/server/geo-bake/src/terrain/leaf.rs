// pxl-geo-bake/terrain/leaf.rs

//! Which leaves exist and their bytes. A leaf exists where the elevation
//! source covers it and land touches it, the coastline it is painted by; the offline bake and the dev server
//! ask the same question, so both produce the same objects.

use pxl_spec::space::layer::tile::LEAF_ZOOM;

use postgres::types::ToSql;
use postgres::Client;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::coord::adapter::epsg3857::{self, merc_from_u};

use crate::separation::{self, source::Store};
use crate::terrain::bake::assemble;
use crate::terrain::pipeline::separation::bake_separation;
use crate::terrain::repository::{self, DRAWN_REACH_U};
use crate::terrain::source::fetch_surface;

/// Whether anything is drawn in a tile: land or a building in it, or a way
/// (not a ferry) within `DRAWN_REACH_U`. `CASE` fixes the order, which
/// `OR` does not, so the costly coastline test runs last.
const HAS_GROUND_SQL: &str = "
  SELECT t.ord, CASE
    WHEN EXISTS (SELECT 1 FROM overture_buildings b WHERE ST_Intersects(b.geom, t.tile)) THEN true
    WHEN EXISTS (SELECT 1 FROM overture_transportation s WHERE ST_DWithin(s.geom, t.tile, $6) AND s.subtype <> 'water') THEN true
    ELSE EXISTS (SELECT 1 FROM osm_land_polygons c WHERE ST_Intersects(c.geom, t.tile))
  END AS has_ground
  FROM (
    SELECT ord, ST_MakeEnvelope(minx, miny, maxx, maxy, 3857) AS tile
    FROM unnest($1::int[], $2::float8[], $3::float8[], $4::float8[], $5::float8[]) AS u(ord, minx, miny, maxx, maxy)
  ) t
";

/// What classifying reads; with a tile's reads, all a terrain bake reads.
pub const GROUND_READS: [&str; 1] = [HAS_GROUND_SQL];
pub const READS: [&[&str]; 3] = [
    &GROUND_READS,
    &repository::READS,
    &separation::source::READS,
];

/// A tile nothing is drawn in is open water; a cheap check, not the carve.
pub fn has_ground(client: &mut Client, tiles: &[[i32; 2]]) -> Result<Vec<bool>, postgres::Error> {
    let bbox: Vec<[f64; 4]> = tiles
        .iter()
        .map(|&p| {
            let bb = epsg3857::get_tile_bbox(LEAF_ZOOM, p);
            [bb[0][0], bb[0][1], bb[1][0], bb[1][1]]
        })
        .collect();
    let ord: Vec<i32> = (0..tiles.len() as i32).collect();
    let col = |k: usize| -> Vec<f64> { bbox.iter().map(|b| b[k]).collect() };
    let (minx, miny, maxx, maxy) = (col(0), col(1), col(2), col(3));
    let reach = merc_from_u(DRAWN_REACH_U);
    let params: [&(dyn ToSql + Sync); 6] = [&ord, &minx, &miny, &maxx, &maxy, &reach];

    let mut out = vec![false; tiles.len()];
    for row in client.query(HAS_GROUND_SQL, &params)? {
        out[row.get::<_, i32>("ord") as usize] = row.get::<_, bool>("has_ground");
    }
    Ok(out)
}

/// A leaf: its separation solved, its surface painted around its lines, the two
/// one tile.
pub fn bake_leaf(
    r: &mut separation::Reader<impl Store>,
    tile: [i32; 2],
) -> Result<TerrainTile, String> {
    let solved = separation::solve_tile(r, tile)?;
    let surface = fetch_surface(r.repo, r.client, tile, &solved.drawn)?;
    let lines = bake_separation(&solved.lines, tile);
    Ok(assemble(tile, surface, lines))
}
