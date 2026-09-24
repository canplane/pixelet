// pxl-geo-bake/poi.rs

//! The POI layer's source: Overture's places, each counted in the leaf cell
//! it stands in (pxl-spec bake/poi_page.rs). Where the pages are kept and how
//! the levels above are folded are the pyramid's (pyramid.rs).

use std::collections::BTreeMap;

use postgres::Client;
use pxl_spec::bake::poi_page::{self, PoiPage};
use pxl_spec::space::adapter::epsg3857::{self, M_PER_U};
use pxl_spec::space::indexer::L3_SHAMT;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pyramid::{cell_of, LevelPage};

/// POI as the pyramid holds it: a count a cell, folded by adding. A page it
/// left as no object holds no places.
impl crate::pyramid::Layer for PoiPage {
    fn encode(&self) -> Result<Vec<u8>, String> {
        poi_page::encode(self).map_err(str::to_owned)
    }

    fn decode(raw: &[u8]) -> Result<Self, String> {
        poi_page::decode(raw).map_err(str::to_owned)
    }
}

/// The leaf pages Overture's places fill, under `within` where it is given
/// and everywhere otherwise, each counting the places in its cells. A place
/// belongs to the cell its projected position floors into, which is the rule
/// every layer's cell is found by (pxl-spec space/pyramid.rs `cell_of`).
pub fn leaves(client: &mut Client, within: Option<LevelPage>) -> Result<BTreeMap<IVec, PoiPage>, String> {
    let rows = match within {
        Some(p) => {
            let [x0, y0, x1, y1] = extent_m(p);
            client.query(
                "SELECT ST_X(geom), ST_Y(geom) FROM overture_places WHERE geom && ST_MakeEnvelope($1, $2, $3, $4, 3857)",
                &[&x0, &y0, &x1, &y1],
            )
        }
        None => client.query("SELECT ST_X(geom), ST_Y(geom) FROM overture_places", &[]),
    }
    .map_err(|e| e.to_string())?;
    let mut cells: BTreeMap<IVec, Vec<u32>> = BTreeMap::new();
    for row in rows {
        let mut at = Pvf3::default();
        epsg3857::project(&mut at, row.get(0), row.get(1), 0.0);
        let (leaf, cell) = cell_of(0, at.i);
        // The envelope is an index's bound, closed on its far edges; the
        // page a place belongs to is the lattice's.
        if within.is_some_and(|w| LevelPage::over(w.level, leaf.page) != w) {
            continue;
        }
        cells.entry(leaf.page).or_default().push(cell);
    }
    Ok(cells.into_iter().map(|(page, c)| (page, PoiPage::of_places(c))).collect())
}

/// A page's extent in EPSG:3857 metres.
fn extent_m(p: LevelPage) -> [f64; 4] {
    let span = (1i64 << (L3_SHAMT + p.level)) as f64;
    let (x, y) = (ivec::X(p.page) as f64 * span, ivec::Y(p.page) as f64 * span);
    [x * M_PER_U, y * M_PER_U, (x + span) * M_PER_U, (y + span) * M_PER_U]
}
