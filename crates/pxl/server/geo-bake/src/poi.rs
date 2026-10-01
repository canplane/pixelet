// pxl-geo-bake/poi.rs

//! The POI layer's source: Overture's places, each counted in the leaf cell
//! it stands in and, where it stands there alone, named, given its kind and
//! placed on its z16 ground cell (pxl-spec bake/poi_page.rs). Where the pages are kept and how
//! the levels above are folded are the pyramid's (pyramid.rs).

use std::collections::BTreeMap;

use postgres::Client;
use pxl_geo_spec::overture::properties::category_named;
use pxl_spec::bake::poi_page::{self, kind_of, Place, PoiPage, POI_LATTICE};
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pvf3::pvf3::Pvf3;

/// POI as the pyramid holds it: a count a cell, folded by adding. A page it
/// left as no object holds no places.
impl crate::pyramid::Layer for PoiPage {
    fn encode(&self) -> Result<Vec<u8>, String> {
        poi_page::encode(self).map_err(|e| e.to_string())
    }

    fn decode(raw: &[u8]) -> Result<Self, String> {
        poi_page::decode(raw).map_err(|e| e.to_string())
    }
}

/// The leaf pages Overture's places fill, only the leaf `within` where it is
/// given and everywhere otherwise, each counting the places in its cells and
/// naming the one place a cell holds alone, and its kind by the category the
/// search list gives it (pxl-geo-spec overture/properties.rs), so the two
/// agree (pxl-spec bake/poi_page.rs). A place
/// belongs to the cell of the POI lattice its projected position floors into,
/// which is the rule every lattice's cell is found by (pxl-spec
/// space/pyramid.rs `Lattice::cell_of`).
pub fn leaves(client: &mut Client, within: Option<IVec>) -> Result<BTreeMap<IVec, PoiPage>, String> {
    let rows = match within {
        Some(p) => {
            let [[x0, y0, _], [x1, y1, _]] = epsg3857::get_page_bbox(p);
            client.query(
                "SELECT ST_X(geom), ST_Y(geom), name, properties->'categories'->>'primary' FROM overture_places WHERE geom && ST_MakeEnvelope($1, $2, $3, $4, 3857)",
                &[&x0, &y0, &x1, &y1],
            )
        }
        None => client.query("SELECT ST_X(geom), ST_Y(geom), name, properties->'categories'->>'primary' FROM overture_places", &[]),
    }
    .map_err(|e| e.to_string())?;
    let mut places: BTreeMap<IVec, Vec<(Pvf3, String, u8, u8)>> = BTreeMap::new();
    for row in rows {
        let mut at = Pvf3::default();
        epsg3857::project(&mut at, row.get(0), row.get(1), 0.0);
        let leaf = POI_LATTICE.cell_of(0, &at).page();
        // The envelope is an index's bound, closed on its far edges; the
        // page a place belongs to is the lattice's.
        if within.is_some_and(|w| leaf.page != w) {
            continue;
        }
        let category = category_named(row.get::<_, Option<&str>>(3));
        places.entry(leaf.page).or_default().push((at, row.get(2), kind_of(category), rank_of(category)));
    }
    Ok(places
        .into_iter()
        .map(|(page, p)| (page, PoiPage::of_places(0, p.iter().map(|(at, name, kind, rank)| (*at, Place { name, kind: *kind, rank: *rank })))))
        .collect())
}

/// How prominent a place of `category` is, the more the lower: OpenMapTiles'
/// `poi_class_rank` (layers/poi/class.sql) over five, which keeps every one
/// of its ranks whole in a byte, of the class its `poi` layer files the
/// category under (layers/poi/poi.yaml) — a clinic is a hospital, a subway a
/// railway, a kindergarten a school, a mall or a convenience store a shop, a
/// department store a grocery, a pub beer — and a class it does not rank,
/// 1000 over five, last.
pub fn rank_of(category: &str) -> u8 {
    match category {
        "hospital" | "clinic" => 4,
        "subway" | "station" => 8,
        "bus_station" => 10,
        "attraction" => 14,
        "university" | "college" => 16,
        "school" | "kindergarten" => 17,
        "townhall" => 20,
        "park" => 24,
        "library" => 26,
        "police" => 27,
        "post_office" => 28,
        "mall" | "convenience" => 80,
        "supermarket" | "department_store" => 100,
        "fast_food" => 120,
        "clothes" => 140,
        "bar" => 160,
        _ => 200,
    }
}
