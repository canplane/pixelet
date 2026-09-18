// pxl-geo-bake/poi/provider/overture_provider.rs

//! Overture's Places theme (docs.overturemaps.org/guides/places): licensed
//! CDLA-Permissive-2.0, sourced from Meta/Microsoft/Foursquare/PinMeTo, with
//! none of the display or caching restrictions Google/Naver/Kakao carry. Reads
//! `overture_places` — a Korea-bbox extract pulled from Overture's public S3
//! release and refreshed on whatever cadence `ingest_places.ts` is re-run at,
//! not a live per-request call.
//!
//! Deliberately its own table, kept structurally independent of the terrain
//! pipeline: Overture's per-place shape is a rich nested JSON object with no
//! flat-tag equivalent terrain's own tables use.
//!
//! `rank` has no policy-assigned equivalent here — Overture's own
//! `properties.confidence` is a source-conflation quality score rather than a
//! notability one, and is the closest available signal, scaled to a plain
//! integer. The client ranks by distance to the map centre regardless, so this
//! only matters if a future consumer wants an actual notability cut.

use postgres::Client;
use pxl_geo_spec::overture::properties::{category_of, confidence, name_of, overture_category};
use pxl_spec::bake::constant::XY_CELL_PER_M;
use pxl_spec::bake::model::feature::POI;
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::pvf3::ivec::IVec;

/// Every place whose point falls in the page envelope.
const OVERTURE_PLACES_SQL: &str = "WITH env AS (
  SELECT ST_MakeEnvelope($1, $2, $3, $4, 3857) AS e
)
SELECT
  gers_id,
  properties,
  ST_X(geom) AS x, ST_Y(geom) AS y
FROM overture_places, env
WHERE geom && env.e";

pub struct PoiProvider {
    places: postgres::Statement,
}

impl PoiProvider {
    pub fn prepare(client: &mut Client) -> Result<PoiProvider, postgres::Error> {
        Ok(PoiProvider {
            places: client.prepare(OVERTURE_PLACES_SQL)?,
        })
    }

    pub fn fetch_pois(&self, client: &mut Client, page_idx: IVec) -> Result<Vec<POI>, postgres::Error> {
        let page_bbox = epsg3857::get_page_bbox(page_idx);
        let page_min = page_bbox[0];
        let rows = client.query(&self.places, &[&page_bbox[0][0], &page_bbox[0][1], &page_bbox[1][0], &page_bbox[1][1]])?;

        let mut pois = Vec::with_capacity(rows.len());
        for row in &rows {
            let properties: serde_json::Value = row.get("properties");
            let name = name_of(&properties);
            if name.is_empty() {
                // An unnamed place is not worth a badge.
                continue;
            }
            let (x, y): (f64, f64) = (row.get("x"), row.get("y"));
            pois.push(POI {
                id: row.get("gers_id"),
                r#type: "poi",
                category: category_of(&properties),
                name: name.to_string(),
                center: [(x - page_min[0]) * XY_CELL_PER_M, (y - page_min[1]) * XY_CELL_PER_M, 0.0],
                tags: vec![("category".to_string(), overture_category(&properties).map(str::to_string))],
                rank: (confidence(&properties) * 1000.0).round() as u32,
            });
        }
        Ok(pois)
    }
}
