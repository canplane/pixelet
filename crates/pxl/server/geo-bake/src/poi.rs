// pxl-geo-bake/poi.rs

use std::collections::BTreeMap;

use crate::coverage::Coverage;
use postgres::Client;
use pxl_geo_spec::overture::properties::{category_named, CATEGORY_SQL};
use pxl_spec::bake::poi_page::{self, kind_of, Place, PoiPage, POI_LATTICE};
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pyramid::LEAF_Z;

impl crate::pyramid::Layer for PoiPage {
    fn encode(&self) -> Result<Vec<u8>, String> {
        poi_page::encode(self).map_err(|e| e.to_string())
    }

    fn decode(raw: &[u8]) -> Result<Self, String> {
        poi_page::decode(raw).map_err(|e| e.to_string())
    }
}

const PLACES_SQL: &str = "SELECT ST_X(geom), ST_Y(geom), name, {category}, gers_id FROM overture_places";

/// Every statement the POI bake runs, for the catalog check.
pub const READS: [&str; 2] = [PLACES_SQL, CATEGORY_SQL];

/// A place belongs to the POI-lattice cell its projected position floors
/// into, and is published only on a page of the coverage.
pub fn leaves(client: &mut Client, coverage: &Coverage, within: Option<IVec>) -> Result<BTreeMap<IVec, PoiPage>, String> {
    let select = PLACES_SQL.replace("{category}", CATEGORY_SQL);
    let rows = match within {
        Some(p) => {
            let [[x0, y0, _], [x1, y1, _]] = epsg3857::get_page_bbox(p);
            client.query(&format!("{select} WHERE geom && ST_MakeEnvelope($1, $2, $3, $4, 3857)"), &[&x0, &y0, &x1, &y1])
        }
        None => client.query(&select, &[]),
    }
    .map_err(|e| e.to_string())?;
    let mut places: BTreeMap<IVec, Vec<(Pvf3, String, u8, String)>> = BTreeMap::new();
    for row in rows {
        let at = epsg3857::project(row.get(0), row.get(1), 0.0);
        let leaf = POI_LATTICE.cell_of(LEAF_Z, &at).page();
        if within.is_some_and(|w| leaf.page != w) || !coverage.contains(leaf.page) {
            continue;
        }
        let kind = kind_of(category_named(row.get::<_, Option<serde_json::Value>>(3).as_ref()));
        places.entry(leaf.page).or_default().push((at, row.get(2), kind, row.get(4)));
    }
    Ok(places
        .into_iter()
        .map(|(page, p)| {
            (
                page,
                PoiPage::of_places(
                    LEAF_Z,
                    p.iter().map(|(at, name, kind, id)| {
                        (
                            *at,
                            Place {
                                name,
                                kind: *kind,
                                id,
                                ..Place::default()
                            },
                        )
                    }),
                    true,
                ),
            )
        })
        .collect())
}
