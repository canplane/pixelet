// pxl-geo-bake/terrain/leaf.rs

//! Which leaves exist and their bytes. A leaf exists where the elevation
//! source covers it and land touches it; the offline bake and the dev server
//! ask the same question, so both produce the same objects.

use pxl_spec::space::layer::tile::LEAF_ZOOM;
use std::path::Path;

use postgres::types::ToSql;
use postgres::Client;
use pxl_spec::bake::format::Terrain;
use pxl_spec::space::coord::adapter::epsg3857::{self, mer_from_u};
use pxl_spec::space::coord::pvf3::ivec::IVec;

use crate::coverage::Coverage;
use crate::deck::{self, source::Store};
use crate::terrain::bake::assemble;
use crate::terrain::pipeline::deck::bake_decks;
use crate::terrain::repository::{self, DRAWN_REACH_U};
use crate::terrain::source::fetch_surface;
use crate::util::fnv1a;

/// Whether anything is drawn in a tile: land or a building in it, or a way
/// (not a ferry) within `DRAWN_REACH_U`. `CASE` fixes the order, which
/// `OR` does not, so the costly coastline test runs last.
const HAS_GROUND_SQL: &str = "
  SELECT t.ord, CASE
    WHEN EXISTS (SELECT 1 FROM overture_buildings b WHERE ST_Intersects(b.geom, t.tile)) THEN true
    WHEN EXISTS (SELECT 1 FROM overture_transportation s WHERE ST_DWithin(s.geom, t.tile, $6) AND s.subtype <> 'water') THEN true
    ELSE EXISTS (SELECT 1 FROM overture_base_land c WHERE ST_Intersects(c.geom, t.tile))
  END AS has_ground
  FROM (
    SELECT ord, ST_MakeEnvelope(minx, miny, maxx, maxy, 3857) AS tile
    FROM unnest($1::int[], $2::float8[], $3::float8[], $4::float8[], $5::float8[]) AS u(ord, minx, miny, maxx, maxy)
  ) t
";

/// What classifying reads; with a tile's reads, all a terrain bake reads.
pub const GROUND_READS: [&str; 1] = [HAS_GROUND_SQL];
pub const READS: [&[&str]; 3] = [&GROUND_READS, &repository::READS, &deck::source::READS];

/// The rule a land list was classified by: a list from another is stale.
const GROUND: u64 = fnv1a(HAS_GROUND_SQL.as_bytes()) ^ DRAWN_REACH_U.to_bits();

/// A tile nothing is drawn in is open water; a cheap check, not the carve.
pub fn has_ground(client: &mut Client, tiles: &[IVec]) -> Result<Vec<bool>, postgres::Error> {
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
    let reach = mer_from_u(DRAWN_REACH_U);
    let params: [&(dyn ToSql + Sync); 6] = [&ord, &minx, &miny, &maxx, &maxy, &reach];

    let mut out = vec![false; tiles.len()];
    for row in client.query(HAS_GROUND_SQL, &params)? {
        out[row.get::<_, i32>("ord") as usize] = row.get::<_, bool>("has_ground");
    }
    Ok(out)
}

/// A leaf: its decks solved, its surface painted around their lines, the two
/// one tile; and whether it lies off the elevation raster, so flat.
pub fn bake_leaf(r: &mut deck::Reader<impl Store>, tile: IVec) -> Result<(Terrain, bool), String> {
    let solved = deck::solve_tile(r, tile)?;
    let (surface, flat) = fetch_surface(r.repo, r.client, tile, &solved.drawn)?;
    let lines = bake_decks(&solved.lines, epsg3857::get_tile_bbox(LEAF_ZOOM, tile)[0]).ok_or("a tile's decks overflow a record")?;
    Ok((assemble(surface, lines), flat))
}

/// The tiles `has_ground` found, kept for the geocode bake and later stages;
/// refused unless made at this tile size, coverage, rule and release.
pub fn land_tiles(path: &Path, coverage: &Coverage, release: Option<&str>) -> Result<Vec<IVec>, String> {
    let reclassify = "run bake_tiles, which writes it as a side effect of its own classification (`--limit 0` does the classification and bakes nothing)";
    let raw = std::fs::read_to_string(path).map_err(|_| format!("{} not found — {reclassify}.", path.display()))?;
    let v: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let want = land_key(coverage, release.unwrap_or_default());
    for (k, want) in want.as_object().into_iter().flatten() {
        if (k != "release" || release.is_some()) && v.get(k) != Some(want) {
            return Err(format!(
                "{} was classified with {k} {}, and it is now {want} — {reclassify}.",
                path.display(),
                v.get(k).unwrap_or(&serde_json::Value::Null)
            ));
        }
    }
    let list = v.get("tiles").and_then(|p| p.as_array()).ok_or_else(|| format!("{} has no tile list", path.display()))?;
    list.iter().map(|s| s.as_str().ok_or("a tile is not a string")?.parse::<IVec>().map_err(|e| e.to_string())).collect()
}

pub fn write_land_tiles(path: &Path, coverage: &Coverage, release: &str, land: &[IVec]) -> Result<(), String> {
    let mut v = land_key(coverage, release);
    v["tiles"] = land.iter().map(|p| p.to_string()).collect();
    std::fs::write(path, serde_json::to_string(&v).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

fn land_key(coverage: &Coverage, release: &str) -> serde_json::Value {
    serde_json::json!({
        "leaf_z": pxl_spec::space::layer::tile::LEAF_ZOOM,
        "shards": coverage.shards().map(|s| s.name()).collect::<Vec<_>>(),
        "ground": format!("{GROUND:016x}"),
        "release": release,
    })
}
