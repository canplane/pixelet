// pxl-geo-bake/terrain/repository.rs

//! One page's raw source, out of the geo database: the elevation raster
//! resampled onto the page's grid, and the Overture Buildings, Transportation,
//! Base and coastline rows its envelope touches.

use postgres::types::ToSql;
use postgres::{Client, Statement};
use pxl_spec::bake::constant::{ELE_PAD, ELE_PADDED_SPAN, PAGE_ELE_SPAN, Z_CELL_PER_M};
use pxl_spec::bake::format::{BaseRow, BuildingRow, CoastRow, SegmentRow};
use pxl_spec::space::adapter::epsg3857::BBox3857;

/// The elevation raster, clipped and resampled onto this page's own grid.
///
/// The padded span is what the bake reads: sampling stays valid at the page
/// boundary only if the source envelope is expanded before the union, and the
/// resample lands on the exact target grid rather than near it.
fn elevation_sql() -> String {
    format!(
        "WITH cfg AS (
  SELECT
    {PAGE_ELE_SPAN}::float8 AS ele_span,
    $5::float8 AS z_cell_per_m,
    {ELE_PAD}::int AS pad,
    {ELE_PADDED_SPAN}::int AS padded_span
),
params AS (
  SELECT
    $1::float8 AS minx,
    $2::float8 AS miny,
    $3::float8 AS maxx,
    $4::float8 AS maxy,
    ($3::float8 - $1::float8) AS width_m
),
calculated AS (
  SELECT width_m / ele_span AS stride_m
  FROM params, cfg
),
ref_rast AS (
  SELECT ST_MakeEmptyRaster(
    (SELECT padded_span FROM cfg), (SELECT padded_span FROM cfg),
    (SELECT minx - (pad * stride_m) FROM params, cfg, calculated),
    (SELECT maxy + (pad * stride_m) FROM params, cfg, calculated),
    (SELECT stride_m FROM calculated), -(SELECT stride_m FROM calculated),
    0, 0, 3857
  ) AS rast
),
env AS (
  -- Expanded source envelope to keep bilinear sampling valid near edges
  SELECT ST_Expand(ST_Envelope(rast), (SELECT stride_m * 2 FROM calculated)) AS e
  FROM ref_rast
),
mosaic AS (
  -- Clip source rasters to the expanded envelope before union
  SELECT ST_Union(ST_Clip(r.rast, env.e)) AS rast
  FROM elevation_raster r, env
  WHERE r.rast && env.e AND ST_Intersects(r.rast, env.e)
),
resampled AS (
  -- Resample onto the exact target grid
  SELECT ST_Resample(m.rast, r.rast, 'Bilinear') AS rast
  FROM mosaic m, ref_rast r
),
final_rast AS (
  -- Final clip to the exact output extent
  SELECT ST_Clip(s.rast, ST_Envelope(r.rast)) AS rast
  FROM resampled s, ref_rast r
),
dumped AS (
  -- Extract the final resampled cell matrix as a 2D value array
  SELECT (ST_DumpValues(rast)).valarray AS z_matrix
  FROM final_rast
)
SELECT
  array_agg(
    COALESCE(round(val * (SELECT z_cell_per_m FROM cfg)), 0)::int2
    ORDER BY row_idx DESC, col_idx ASC
  ) AS ele
FROM dumped,
     unnest(z_matrix) WITH ORDINALITY AS t(val, ord),
     LATERAL (
       SELECT
         ((ord - 1) / (SELECT padded_span FROM cfg))::int AS row_idx,
         ((ord - 1) % (SELECT padded_span FROM cfg))::int AS col_idx
     ) AS l;"
    )
}

// Buildings and roads are not clipped to the page envelope, so a polygon
// grazing a page costs it the whole geometry. Clipping them broke ring
// closure; an attempt has to keep rings closed — clip-and-reclose,
// Weiler–Atherton, Sutherland–Hodgman with holes — never a naive bbox clip.
// The water branch of OVERTURE_BASE_SQL and OVERTURE_COAST_SQL do clip
// (ST_Intersection guarded by ST_IsEmpty): those rows run to hundreds of
// thousands of points.
const OVERTURE_BUILDINGS_SQL: &str = "WITH env AS (
  SELECT ST_MakeEnvelope($1, $2, $3, $4, 3857) AS e
)
SELECT gers_id, subtype, class, height, num_floors, ST_AsBinary(geom) AS geom_wkb
FROM overture_buildings, env
WHERE geom && env.e";

const OVERTURE_TRANSPORTATION_SQL: &str = "WITH env AS (
  SELECT ST_MakeEnvelope($1, $2, $3, $4, 3857) AS e
)
SELECT gers_id, subtype, class, COALESCE(road_flags, rail_flags)::text AS flags, ST_AsBinary(geom) AS geom_wkb
FROM overture_transportation, env
WHERE geom && env.e";

/// Base is fragmented across four tables but shares one row shape — a literal
/// `source` column per branch tells the adapter which table's own
/// subtype/class vocabulary a row came from.
const OVERTURE_BASE_SQL: &str = "WITH env AS (
  SELECT ST_MakeEnvelope($1, $2, $3, $4, 3857) AS e
)
SELECT gers_id, 'land_use' AS source, subtype, class, ST_AsBinary(geom) AS geom_wkb FROM overture_base_land_use, env WHERE geom && env.e
UNION ALL
-- Some water rows are country-scale (up to 444,626 points); clip to the page envelope first. ST_IsEmpty guards && 's bbox-only false positives.
SELECT gers_id, 'water' AS source, subtype, class, ST_AsBinary(clipped) AS geom_wkb FROM (
  SELECT gers_id, subtype, class, ST_Intersection(geom, env.e) AS clipped
  FROM overture_base_water, env
  WHERE geom && env.e
) w WHERE NOT ST_IsEmpty(clipped)
UNION ALL
SELECT gers_id, 'land' AS source, subtype, class, ST_AsBinary(geom) AS geom_wkb FROM overture_base_land, env WHERE geom && env.e
UNION ALL
SELECT gers_id, 'infrastructure' AS source, subtype, class, ST_AsBinary(geom) AS geom_wkb FROM overture_base_infrastructure, env WHERE geom && env.e";

/// Overture's Base `water` theme's `ocean` subtype, QA'd from OpenStreetMap
/// coastline data. Its geometry is ocean-shaped, not land-shaped — so this
/// query inverts it: union whatever ocean touches the page, then take the page
/// envelope's own difference against that union, giving a LAND polygon in the
/// shape `from_coast_row` expects. An empty result means the row is dropped
/// and the page is pure ocean; a result covering the whole envelope is dropped
/// as "nothing to carve, already all land"; anything else is the real partial
/// coastline geometry to carve bare out of a sea-filled base.
const OVERTURE_COAST_SQL: &str = "WITH env AS (
  SELECT ST_MakeEnvelope($1, $2, $3, $4, 3857) AS e
),
ocean_union AS (
  -- Clipped first: ocean rows run up to 163,456 points, and ST_Difference below would otherwise chew through all of them.
  SELECT ST_Union(ST_Intersection(geom, env.e)) AS g FROM overture_base_water, env
  WHERE subtype = 'ocean' AND geom && env.e
),
land AS (
  SELECT COALESCE(ST_Difference(env.e, ocean_union.g), env.e) AS g, env.e AS e
  FROM env LEFT JOIN ocean_union ON true
)
SELECT
  'overture_ocean' AS id,
  ST_AsBinary(g) AS geom_wkb,
  ST_Covers(g, e) AS covers_page
FROM land
WHERE NOT ST_IsEmpty(g)";

pub struct PageSource {
    /// Row-major, or none when the page has no elevation coverage.
    pub ele: Option<Vec<i16>>,
    pub building_rows: Vec<BuildingRow>,
    pub segment_rows: Vec<SegmentRow>,
    pub base_rows: Vec<BaseRow>,
    pub coast_rows: Vec<CoastRow>,
}

/// The five statements, prepared once against one connection.
///
/// An ordinary parameterized query re-planned every call spends ~34 ms of a
/// ~44 ms bbox lookup against `overture_buildings` in planning; prepared once
/// per physical connection, the lookup is ~2–3 ms.
pub struct TerrainRepository {
    ele: Statement,
    buildings: Statement,
    segments: Statement,
    base: Statement,
    coast: Statement,
}

impl TerrainRepository {
    pub fn prepare(client: &mut Client) -> Result<TerrainRepository, postgres::Error> {
        Ok(TerrainRepository {
            ele: client.prepare(&elevation_sql())?,
            buildings: client.prepare(OVERTURE_BUILDINGS_SQL)?,
            segments: client.prepare(OVERTURE_TRANSPORTATION_SQL)?,
            base: client.prepare(OVERTURE_BASE_SQL)?,
            coast: client.prepare(OVERTURE_COAST_SQL)?,
        })
    }

    /// All five per-page queries on the one connection, in order. A page only
    /// ever holds one connection, so page-level concurrency and the worker
    /// count mean the same thing.
    pub fn fetch_page_source(&self, client: &mut Client, page_bbox: BBox3857) -> Result<PageSource, postgres::Error> {
        let (minx, miny, maxx, maxy) = (page_bbox[0][0], page_bbox[0][1], page_bbox[1][0], page_bbox[1][1]);
        let bbox: [&(dyn ToSql + Sync); 4] = [&minx, &miny, &maxx, &maxy];
        let z_cell_per_m = Z_CELL_PER_M(miny);

        let ele = client
            .query(&self.ele, &[&minx, &miny, &maxx, &maxy, &z_cell_per_m])?
            .first()
            .and_then(|row| row.get::<_, Option<Vec<i16>>>("ele"));

        let building_rows = client
            .query(&self.buildings, &bbox)?
            .iter()
            .map(|row| BuildingRow {
                gers_id: row.get("gers_id"),
                subtype: row.get("subtype"),
                class: row.get("class"),
                height: row.get("height"),
                // `num_floors` is an `integer` column; the row model carries
                // every number as f64, as the packet does.
                num_floors: row.get::<_, Option<i32>>("num_floors").map(f64::from),
                geom_wkb: wkb(row),
            })
            .collect();

        let segment_rows = client
            .query(&self.segments, &bbox)?
            .iter()
            .map(|row| SegmentRow {
                gers_id: row.get("gers_id"),
                subtype: row.get("subtype"),
                class: row.get("class"),
                flags: row.get("flags"),
                geom_wkb: wkb(row),
            })
            .collect();

        let base_rows = client
            .query(&self.base, &bbox)?
            .iter()
            .map(|row| BaseRow {
                gers_id: row.get("gers_id"),
                source: row.get("source"),
                subtype: row.get("subtype"),
                class: row.get("class"),
                geom_wkb: wkb(row),
            })
            .collect();

        let coast_rows = client
            .query(&self.coast, &bbox)?
            .iter()
            .map(|row| CoastRow {
                id: row.get("id"),
                geom_wkb: wkb(row),
                covers_page: row.get("covers_page"),
            })
            .collect();

        Ok(PageSource {
            ele,
            building_rows,
            segment_rows,
            base_rows,
            coast_rows,
        })
    }
}

fn wkb(row: &postgres::Row) -> Box<[u8]> {
    row.get::<_, &[u8]>("geom_wkb").into()
}
