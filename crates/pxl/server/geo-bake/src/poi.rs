// pxl-geo-bake/poi.rs

use std::collections::BTreeMap;

use postgres::Client;
use pxl_geo_spec::overture::properties::{category_named, CATEGORY_SQL};
use pxl_spec::bake::poi_page::{self, kind_of, Place, PoiPage, POI_LATTICE};
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pvf3::pvf3::Pvf3;

impl crate::pyramid::Layer for PoiPage {
    fn encode(&self) -> Result<Vec<u8>, String> {
        poi_page::encode(self).map_err(|e| e.to_string())
    }

    fn decode(raw: &[u8]) -> Result<Self, String> {
        poi_page::decode(raw).map_err(|e| e.to_string())
    }
}

/// A place belongs to the POI-lattice cell its projected position floors into.
pub fn leaves(client: &mut Client, within: Option<IVec>) -> Result<BTreeMap<IVec, PoiPage>, String> {
    let select = format!("SELECT ST_X(geom), ST_Y(geom), name, {CATEGORY_SQL}, gers_id FROM overture_places");
    let rows = match within {
        Some(p) => {
            let [[x0, y0, _], [x1, y1, _]] = epsg3857::get_page_bbox(p);
            client.query(&format!("{select} WHERE geom && ST_MakeEnvelope($1, $2, $3, $4, 3857)"), &[&x0, &y0, &x1, &y1])
        }
        None => client.query(&select, &[]),
    }
    .map_err(|e| e.to_string())?;
    let mut places: BTreeMap<IVec, Vec<(Pvf3, String, u8, u64)>> = BTreeMap::new();
    for row in rows {
        let mut at = Pvf3::default();
        epsg3857::project(&mut at, row.get(0), row.get(1), 0.0);
        let leaf = POI_LATTICE.cell_of(0, &at).page();
        if within.is_some_and(|w| leaf.page != w) {
            continue;
        }
        let kind = kind_of(category_named(row.get::<_, Option<&str>>(3)));
        places.entry(leaf.page).or_default().push((at, row.get(2), kind, id_of(row.get(4))));
    }
    Ok(places
        .into_iter()
        .map(|(page, p)| (page, PoiPage::of_places(0, p.iter().map(|(at, name, kind, id)| (*at, Place { name, kind: *kind, id: *id })), true)))
        .collect())
}

/// FNV-1a 64 of the source's id, so the page carries no provider id format.
fn id_of(source_id: &str) -> u64 {
    source_id.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}
