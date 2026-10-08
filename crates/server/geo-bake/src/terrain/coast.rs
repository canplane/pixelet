// pxl-geo-bake/terrain/coast.rs

//! The ground past the coverage, which the pyramid's coarsest tiles show
//! beside it: the coastline a leaf is painted by, OSM's land polygons, laid
//! on each shard the coverage does not hold under a top tile it does, at
//! that shard's own cells. Its sea is water and its land unknown, since no
//! source here says what the land is. Baked from the DB with the leaves, so
//! the fold reads it with none.

use std::collections::HashMap;

use postgres::Client;
use pxl_spec::bake::format::{Ground, Terrain};
use pxl_spec::cell::cube_types::{ground, sentinel};
use pxl_spec::space::coord::adapter::epsg3857;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::Tile;

use crate::coverage::Coverage;
use crate::pyramid::{Layer, TileSet};
use crate::terrain::absent::surface;
use crate::terrain::bake::coast_of;
use crate::terrain::pipeline::ground::bake_ground;
use crate::terrain::repository::{self, COAST_SQL};
use crate::util::{pack, Level};

/// One shard past the coverage.
pub fn bake(client: &mut Client, p: Tile) -> Result<Terrain, String> {
    let [[minx, miny, _], [maxx, maxy, _]] = epsg3857::get_tile_bbox(p.zoom, p.tile);
    let rows = repository::coast(client, COAST_SQL, &[&minx, &miny, &maxx, &maxy]).map_err(|e| e.to_string())?;
    let mut cells = bake_ground(&[], &coast_of(&rows, [minx, miny, 0.0], units(GROUND.cell_zoom(p.zoom)))).cells.into_owned();
    for c in &mut cells {
        if *c == ground::BASE {
            *c = sentinel::unknown;
        }
    }
    Ok(surface(Ground { cells: cells.into() }))
}

/// Bakes each of `tiles` the set does not hold yet.
pub fn bake_missing(client: &mut Client, set: &TileSet, tiles: &[Tile]) -> Result<usize, String> {
    let missing: Vec<Tile> = tiles.iter().copied().filter(|&p| !set.path(p).exists()).collect();
    for &p in &missing {
        let encoded = bake(client, p)?.encode()?;
        set.write(p, &pack(&encoded, Level::Stored).map_err(|e| e.to_string())?)?;
    }
    Ok(missing.len())
}

/// Every shard past `coverage` the set holds; one it lacks is refused, since
/// the tiles beside it would fold from nothing.
pub fn read(set: &TileSet, coverage: &Coverage) -> Result<HashMap<Tile, Terrain>, String> {
    coverage
        .beyond()
        .into_iter()
        .map(|p| {
            Ok((
                p,
                set.read::<Terrain>(p)?
                    .ok_or_else(|| format!("{}: the coast past the coverage is not baked: run --stage leaf", set.path(p).display()))?,
            ))
        })
        .collect()
}
