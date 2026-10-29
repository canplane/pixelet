// pxl-geo-bake/separation/source.rs

//! What a tile's solve reads.

use fnv::FnvHashMap;
use postgres::Client;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};
use std::path::Path;

use postgres::types::ToSql;
use pxl_spec::space::coord::adapter::epsg3857;

use pxl_spec::bake::separation::Form;

use crate::segment::Part;
use crate::separation::piece::piece_of;
use crate::terrain::repository::TerrainRepository;
use crate::util::{pack, unpack, write_atomic, Level};

/// A road tunnel's end: where a portal may open, and the tunnel's level, which
/// sets how far its approaches can sink (`portals_in`).
#[derive(Clone, Copy)]
pub struct Portal {
    pub at: [f64; 2],
    pub level: i64,
}

/// The tables the solve's queries name, for the catalog.
pub const READS: [&str; 2] = ["FROM overture_transportation", "FROM overture_base_water"];

const SEGMENTS_SQL: &str = concat!(
    "SELECT ",
    crate::segment_columns!(),
    " FROM overture_transportation t WHERE t.geom && ST_MakeEnvelope($1, $2, $3, $4, 3857)"
);

/// The waterways a deck clears, lines alone: a water area, a sea's 444,741
/// points among them, clears nothing.
const WATER_SQL: &str = "SELECT subtype, class, ST_AsBinary(geom) AS geom_wkb FROM overture_base_water WHERE ST_Dimension(geom) = 1 AND geom && ST_MakeEnvelope($1, $2, $3, $4, 3857)";

/// One tile's rows, as a tile's other rows are read: whatever overlaps it.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct TileRows {
    ways: Vec<Part>,
    water: Vec<(String, Option<String>, Vec<u8>)>,
    /// Its elevation, flat where the raster does not reach.
    ele: Vec<i16>,
}

impl TileRows {
    /// Every way it overlaps, the bridges and tunnels among them.
    pub fn ways(&self) -> &[Part] {
        &self.ways
    }

    /// Its water as read: subtype, class and geometry.
    pub fn water(&self) -> &[(String, Option<String>, Vec<u8>)] {
        &self.water
    }

    pub fn ele(&self) -> &[i16] {
        &self.ele
    }

    /// Both ends of every road tunnel among its ways (`piece_of`).
    pub fn portals(&self) -> Vec<Portal> {
        self.ways
            .iter()
            .filter_map(piece_of)
            .filter(|p| p.form == Form::Tunnel && p.way.subtype != "rail")
            .flat_map(|p| {
                let (pts, level) = (&p.way.points, p.way.tags.level.unwrap_or(0));
                [pts[0], pts[pts.len() - 1]].map(|q| Portal {
                    at: [q[0], q[1]],
                    level,
                })
            })
            .collect()
    }
}

fn tile_rows(
    client: &mut Client,
    repo: &TerrainRepository,
    tile: [i32; 2],
) -> Result<TileRows, String> {
    let [min, max] = epsg3857::get_tile_bbox(LEAF_ZOOM, tile);
    let env: [&(dyn ToSql + Sync); 4] = [&min[0], &min[1], &max[0], &max[1]];
    let read = |client: &mut Client, sql: &str| client.query(sql, &env).map_err(|e| e.to_string());
    let ways = read(client, SEGMENTS_SQL)?;
    let water = read(client, WATER_SQL)?;
    let ele = repo.ele(client, tile).map_err(|e| e.to_string())?;
    Ok(TileRows {
        ways: ways.iter().flat_map(Part::of).collect(),
        water: water
            .iter()
            .map(|r| {
                (
                    r.get("subtype"),
                    r.get("class"),
                    r.get::<_, &[u8]>("geom_wkb").to_vec(),
                )
            })
            .collect(),
        ele,
    })
}

/// Where a tile's rows are kept for its neighbors.
pub trait Store {
    fn tile(
        &mut self,
        client: &mut Client,
        repo: &TerrainRepository,
        tile: [i32; 2],
    ) -> Result<TileRows, String>;
}

/// On disk, shared by the processes that bake tiles on demand.
pub struct Disk<'a>(pub &'a Path);

impl Store for Disk<'_> {
    fn tile(
        &mut self,
        client: &mut Client,
        repo: &TerrainRepository,
        tile: [i32; 2],
    ) -> Result<TileRows, String> {
        memo_rows(client, repo, tile, self.0)
    }
}

/// In memory, for a walk from tile to neighboring tile: holds a tile's
/// eight around it and no more.
#[derive(Default)]
pub struct Walk {
    held: FnvHashMap<[i32; 2], TileRows>,
}

impl Walk {
    /// Drops what is not in `tile`'s 3×3.
    pub fn keep_around(&mut self, tile: [i32; 2]) {
        let near = |p: [i32; 2]| (p[0] - tile[0]).abs() <= 1 && (p[1] - tile[1]).abs() <= 1;
        self.held.retain(|&p, _| near(p));
    }
}

impl Store for Walk {
    fn tile(
        &mut self,
        client: &mut Client,
        repo: &TerrainRepository,
        tile: [i32; 2],
    ) -> Result<TileRows, String> {
        if let Some(rows) = self.held.get(&tile) {
            return Ok(rows.clone());
        }
        let rows = tile_rows(client, repo, tile)?;
        self.held.insert(tile, rows.clone());
        Ok(rows)
    }
}

/// The ways a road tunnel may be among, whatever else is there.
const TUNNELS_SQL: &str = concat!(
    "SELECT ",
    crate::segment_columns!(),
    " FROM overture_transportation t WHERE t.geom && ST_MakeEnvelope($1, $2, $3, $4, 3857) AND t.road_flags::text LIKE '%is_tunnel%'"
);

/// The portals of the road tunnels in `[lo, hi]`, DB coordinates: a narrow
/// read of the tunnels alone, so a solve learns whether a neighbor's tunnel
/// opens near its tile before it reads the neighbor.
pub fn portals_in(client: &mut Client, [lo, hi]: [[f64; 2]; 2]) -> Result<Vec<Portal>, String> {
    let env: [&(dyn ToSql + Sync); 4] = [&lo[0], &lo[1], &hi[0], &hi[1]];
    let rows = client.query(TUNNELS_SQL, &env).map_err(|e| e.to_string())?;
    Ok(TileRows {
        ways: rows.iter().flat_map(Part::of).collect(),
        water: Vec::new(),
        ele: Vec::new(),
    }
    .portals())
}

/// A tile's rows from `memo` when read before, else from the DB into it.
fn memo_rows(
    client: &mut Client,
    repo: &TerrainRepository,
    tile: [i32; 2],
    memo: &Path,
) -> Result<TileRows, String> {
    let file = memo.join(Tile::new(LEAF_ZOOM, tile).name());
    if let Some(rows) = std::fs::read(&file)
        .ok()
        .and_then(|b| unpack(&b).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
    {
        return Ok(rows);
    }
    let rows = tile_rows(client, repo, tile)?;
    let packed = pack(
        &serde_json::to_vec(&rows).map_err(|e| e.to_string())?,
        Level::Live,
    )
    .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(memo).map_err(|e| e.to_string())?;
    write_atomic(&file, &packed).map_err(|e| e.to_string())?;
    Ok(rows)
}

/// A tile and the eight around it.
pub fn ring(tile: [i32; 2]) -> Vec<[i32; 2]> {
    let [px, py] = tile;
    (-1..=1)
        .flat_map(|dy| (-1..=1).map(move |dx| [px + dx, py + dy]))
        .collect()
}
