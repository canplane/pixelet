// pxl-geo-bake/terrain/repository.rs

//! One page's raw source, out of the geo database: its elevation grid
//! (elevation.rs), and the Overture Buildings, Transportation, Base and
//! coastline rows its envelope touches.

use std::sync::Arc;

use postgres::types::ToSql;
use postgres::{Client, Statement};
use pxl_spec::bake::format::{BaseRow, BuildingRow, CoastRow, DeckRow, SegmentRow};
use pxl_spec::space::adapter::epsg3857::BBox3857;

use crate::elevation::Raster;

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
SELECT gers_id, subtype, class, COALESCE(road_flags, rail_flags)::text AS flags,
  width_rules::text AS widths, access_restrictions::text AS access,
  subclass, subclass_rules::text AS subclasses, ST_AsBinary(geom) AS geom_wkb
FROM overture_transportation, env
WHERE geom && env.e";

/// The decks the offline solve wrote (deck/), whole: a deck is a line and
/// the page keeps the run of it that crosses the page
/// (terrain/pipeline/deck.rs).
const OVERTURE_DECK_SQL: &str = "WITH env AS (
  SELECT ST_MakeEnvelope($1, $2, $3, $4, 3857) AS e
)
SELECT gers_id, subtype, class, width_m, form, ST_AsBinary(geom) AS geom_wkb
FROM overture_deck, env
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
    /// Row-major, or none where the raster does not reach the page.
    pub ele: Option<Vec<i16>>,
    pub building_rows: Vec<BuildingRow>,
    pub segment_rows: Vec<SegmentRow>,
    pub base_rows: Vec<BaseRow>,
    pub coast_rows: Vec<CoastRow>,
}

/// The five statements, prepared once against one connection, and the
/// raster the elevation is read from, which the connections share.
///
/// An ordinary parameterized query re-planned every call spends ~34 ms of a
/// ~44 ms bbox lookup against `overture_buildings` in planning; prepared once
/// per physical connection, the lookup is ~2–3 ms.
pub struct TerrainRepository {
    raster: Arc<Raster>,
    buildings: Statement,
    segments: Statement,
    base: Statement,
    coast: Statement,
    deck: Statement,
}

impl TerrainRepository {
    pub fn prepare(client: &mut Client, raster: Arc<Raster>) -> Result<TerrainRepository, postgres::Error> {
        Ok(TerrainRepository {
            raster,
            buildings: client.prepare(OVERTURE_BUILDINGS_SQL)?,
            segments: client.prepare(OVERTURE_TRANSPORTATION_SQL)?,
            base: client.prepare(OVERTURE_BASE_SQL)?,
            coast: client.prepare(OVERTURE_COAST_SQL)?,
            deck: client.prepare(OVERTURE_DECK_SQL)?,
        })
    }

    /// The page's elevation and its five queries on the one connection, in order. A page only
    /// ever holds one connection, so page-level concurrency and the worker
    /// count mean the same thing.
    pub fn fetch_page_source(&self, client: &mut Client, page_bbox: BBox3857) -> Result<PageSource, postgres::Error> {
        let (minx, miny, maxx, maxy) = (page_bbox[0][0], page_bbox[0][1], page_bbox[1][0], page_bbox[1][1]);
        let bbox: [&(dyn ToSql + Sync); 4] = [&minx, &miny, &maxx, &maxy];

        let ele = self.raster.page_grid(client, page_bbox)?;

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
                widths: row.get("widths"),
                access: row.get("access"),
                subclass: row.get("subclass"),
                subclasses: row.get("subclasses"),
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

    /// The solved lines its envelope touches (deck/): what a page's lines are
    /// made of, apart from its surface.
    pub fn fetch_deck_rows(&self, client: &mut Client, page_bbox: BBox3857) -> Result<Vec<DeckRow>, postgres::Error> {
        let (minx, miny, maxx, maxy) = (page_bbox[0][0], page_bbox[0][1], page_bbox[1][0], page_bbox[1][1]);
        Ok(client
            .query(&self.deck, &[&minx, &miny, &maxx, &maxy])?
            .iter()
            .map(|row| DeckRow {
                gers_id: row.get("gers_id"),
                subtype: row.get("subtype"),
                class: row.get("class"),
                width_m: row.get::<_, f32>("width_m").into(),
                form: row.get::<_, i16>("form") as u8,
                geom_wkb: wkb(row),
            })
            .collect())
    }
}

fn wkb(row: &postgres::Row) -> Box<[u8]> {
    row.get::<_, &[u8]>("geom_wkb").into()
}
