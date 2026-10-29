// pxl-geo-bake/terrain/repository.rs

use std::sync::Arc;

use postgres::types::ToSql;
use postgres::{Client, Statement};
use pxl_spec::space::coord::adapter::epsg3857::{get_tile_bbox, merc_from_u, BBox3857};
use pxl_spec::space::layer::tile::LEAF_ZOOM;

use crate::elevation::Raster;
use crate::segment::Part;
use crate::terrain::pipeline::overture::policy::{INFRASTRUCTURE, LAND, WIDEST_LINE};
use crate::terrain::pipeline::overture::width::carto_u;

// Buildings and roads are not clipped: a naive bbox clip broke ring closure.
// Water and coast are clipped, since their rows reach hundreds of thousands of points.
const OVERTURE_BUILDINGS_SQL: &str = "WITH env AS (
  SELECT ST_MakeEnvelope($1, $2, $3, $4, 3857) AS e
)
SELECT gers_id, class, subtype, ST_Area(geom) AS way_area, ST_AsBinary(geom) AS geom_wkb
FROM overture_buildings, env
WHERE geom && env.e";

const OVERTURE_TRANSPORTATION_SQL: &str = concat!(
    "WITH env AS (
  SELECT ST_MakeEnvelope($1, $2, $3, $4, 3857) AS e
)
SELECT ",
    crate::segment_columns!(),
    "
FROM overture_transportation t, env
WHERE t.geom && env.e"
);

/// How far past a tile what is drawn on it can come from: half the widest line.
pub const DRAWN_REACH_U: f64 = carto_u(WIDEST_LINE) / 2.0;
const _: () = assert!(
    DRAWN_REACH_U <= crate::coverage::BAKE_REACH,
    "a tile's rows come from past the bakes' reach"
);

/// A tile's bbox grown by `DRAWN_REACH_U`, the area its rows are read from; what
/// is baked is still the tile's own cells.
pub fn drawn_from([min, max]: BBox3857) -> BBox3857 {
    let r = merc_from_u(DRAWN_REACH_U);
    [
        [min[0] - r, min[1] - r, min[2]],
        [max[0] + r, max[1] + r, max[2]],
    ]
}

/// Four tables, one row shape; `source` says whose vocabulary a row uses.
/// Land and infrastructure are read only in the classes the policy paints.
const OVERTURE_BASE_SQL: &str = "WITH env AS (
  SELECT ST_MakeEnvelope($1, $2, $3, $4, 3857) AS e
)
SELECT 'land_use' AS source, gers_id, subtype, class, ST_Area(geom) AS way_area, ST_AsBinary(geom) AS geom_wkb FROM overture_base_land_use, env WHERE geom && env.e
UNION ALL
-- Some water rows are country-scale; clip to the tile envelope first. ST_IsEmpty guards && 's bbox-only false positives.
SELECT 'water' AS source, gers_id, subtype, class, way_area, ST_AsBinary(clipped) AS geom_wkb FROM (
  SELECT gers_id, subtype, class, ST_Area(geom) AS way_area, ST_Intersection(geom, env.e) AS clipped FROM overture_base_water, env WHERE geom && env.e
) w WHERE NOT ST_IsEmpty(clipped)
UNION ALL
SELECT 'land' AS source, gers_id, subtype, class, ST_Area(geom) AS way_area, ST_AsBinary(geom) AS geom_wkb FROM overture_base_land, env WHERE geom && env.e AND class = ANY($6)
UNION ALL
SELECT 'infrastructure' AS source, gers_id, subtype, class, ST_Area(geom) AS way_area, ST_AsBinary(geom) AS geom_wkb FROM overture_base_infrastructure, env WHERE geom && env.e AND class = ANY($5)";

/// The tile's land from OSM's land polygons (`infra/geo/coastline`), taken
/// whole: empty means pure ocean, the whole envelope means all land, both dropped.
const COAST_SQL: &str = "WITH env AS (
  SELECT ST_MakeEnvelope($1, $2, $3, $4, 3857) AS e
),
land_union AS (
  SELECT ST_Union(ST_Intersection(geom, env.e)) AS g FROM osm_land_polygons, env
  WHERE geom && env.e
),
land AS (
  SELECT COALESCE(land_union.g, ST_SetSRID('POLYGON EMPTY'::geometry, 3857)) AS g, env.e AS e
  FROM env LEFT JOIN land_union ON true
)
SELECT
  ST_AsBinary(g) AS geom_wkb,
  ST_Covers(g, e) AS covers_tile
FROM land
WHERE NOT ST_IsEmpty(g)";

/// Every statement a tile's bake runs, in the order `prepare` takes them, so
/// the catalog check (`coverage.rs` `Overture`) sees each table they read.
pub const READS: [&str; 4] = [
    OVERTURE_BUILDINGS_SQL,
    OVERTURE_TRANSPORTATION_SQL,
    OVERTURE_BASE_SQL,
    COAST_SQL,
];

pub struct BuildingRow {
    pub gers_id: String,
    /// The whole source's, before a tile clips it or its parts are split.
    pub way_area: f64,
    pub subtype: Option<String>,
    pub class: Option<String>,
    pub geom_wkb: Box<[u8]>,
}

pub struct BaseRow {
    /// Which table: "land_use", "water", "land" or "infrastructure".
    pub src: String,
    pub gers_id: String,
    /// The whole source's, before a tile clips it or its parts are split.
    pub way_area: f64,
    pub subtype: Option<String>,
    pub class: Option<String>,
    pub geom_wkb: Box<[u8]>,
}

pub struct CoastRow {
    pub geom_wkb: Box<[u8]>,
    pub covers_tile: bool,
}

pub struct TileSource {
    pub ele: Vec<i16>,
    pub building_rows: Vec<BuildingRow>,
    pub parts: Vec<Part>,
    pub base_rows: Vec<BaseRow>,
    pub coast_rows: Vec<CoastRow>,
}

/// Prepared once per connection: re-planning spent ~34 ms of a ~44 ms
/// buildings lookup, prepared it is ~2–3 ms.
pub struct TerrainRepository {
    raster: Arc<Raster>,
    buildings: Statement,
    segments: Statement,
    base: Statement,
    coast: Statement,
    /// The base query's `$5` and `$6`: the classes the policy paints.
    infrastructure: Vec<&'static str>,
    land: Vec<&'static str>,
}

impl TerrainRepository {
    pub fn prepare(
        client: &mut Client,
        raster: Arc<Raster>,
    ) -> Result<TerrainRepository, postgres::Error> {
        let [buildings, segments, base, coast] = READS.map(|sql| client.prepare(sql));
        Ok(TerrainRepository {
            raster,
            buildings: buildings?,
            segments: segments?,
            base: base?,
            coast: coast?,
            infrastructure: INFRASTRUCTURE.iter().map(|(class, _)| *class).collect(),
            land: LAND.iter().map(|(class, _)| *class).collect(),
        })
    }

    /// A leaf tile's elevation grid.
    pub fn ele(&self, client: &mut Client, tile: [i32; 2]) -> Result<Vec<i16>, postgres::Error> {
        self.raster.tile_grid(client, tile)
    }

    pub fn fetch_tile_src(
        &self,
        client: &mut Client,
        tile: [i32; 2],
    ) -> Result<TileSource, postgres::Error> {
        let [[minx, miny, _], [maxx, maxy, _]] = drawn_from(get_tile_bbox(LEAF_ZOOM, tile));
        let bbox: [&(dyn ToSql + Sync); 4] = [&minx, &miny, &maxx, &maxy];
        let wkb = |row: &postgres::Row| -> Box<[u8]> { row.get::<_, &[u8]>("geom_wkb").into() };
        Ok(TileSource {
            ele: self.raster.tile_grid(client, tile)?,
            building_rows: client
                .query(&self.buildings, &bbox)?
                .iter()
                .map(|row| BuildingRow {
                    gers_id: row.get("gers_id"),
                    way_area: row.get("way_area"),
                    subtype: row.get("subtype"),
                    class: row.get("class"),
                    geom_wkb: wkb(row),
                })
                .collect(),
            parts: client
                .query(&self.segments, &bbox)?
                .iter()
                .flat_map(Part::of)
                .collect(),
            base_rows: client
                .query(
                    &self.base,
                    &[
                        bbox[0],
                        bbox[1],
                        bbox[2],
                        bbox[3],
                        &self.infrastructure,
                        &self.land,
                    ],
                )?
                .iter()
                .map(|row| BaseRow {
                    src: row.get("source"),
                    gers_id: row.get("gers_id"),
                    way_area: row.get("way_area"),
                    subtype: row.get("subtype"),
                    class: row.get("class"),
                    geom_wkb: wkb(row),
                })
                .collect(),
            coast_rows: client
                .query(&self.coast, &bbox)?
                .iter()
                .map(|row| CoastRow {
                    geom_wkb: wkb(row),
                    covers_tile: row.get("covers_tile"),
                })
                .collect(),
        })
    }
}
