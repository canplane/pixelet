// pxl-geo-bake/terrain/repository.rs

use std::sync::Arc;

use postgres::types::ToSql;
use postgres::{Client, Statement};
use pxl_spec::bake::format::{BaseRow, BuildingRow, CoastRow, DeckRow, SegmentRow};
use pxl_spec::space::adapter::epsg3857::BBox3857;

use crate::elevation::Raster;

// Buildings and roads are not clipped: a naive bbox clip broke ring closure.
// Water and coast are clipped, since their rows reach hundreds of thousands of points.
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

const OVERTURE_DECK_SQL: &str = "WITH env AS (
  SELECT ST_MakeEnvelope($1, $2, $3, $4, 3857) AS e
)
SELECT gers_id, subtype, class, width_m, form, portal, ST_AsBinary(geom) AS geom_wkb
FROM overture_deck, env
WHERE geom && env.e";

/// Four tables, one row shape; `source` says whose vocabulary a row uses.
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

/// Overture's ocean polygons inverted into a land polygon for the page: empty
/// means pure ocean, the whole envelope means all land, both dropped.
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
    pub ele: Option<Vec<i16>>,
    pub building_rows: Vec<BuildingRow>,
    pub segment_rows: Vec<SegmentRow>,
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
                portal_m: row.get::<_, &[u8]>("portal").into(),
                geom_wkb: wkb(row),
            })
            .collect())
    }
}

fn wkb(row: &postgres::Row) -> Box<[u8]> {
    row.get::<_, &[u8]>("geom_wkb").into()
}
