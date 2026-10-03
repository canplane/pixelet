// pxl-geo-bake/deck/source.rs

//! What a page's solve reads.

use postgres::Client;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use postgres::types::ToSql;
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::pvf3::ivec::{self, IVec};

use crate::deck::piece::BridgeRow;
use crate::deck::relate::{Water, Way};
use crate::elevation::Raster;
use crate::util::{pack, unpack, write_atomic, Level};

/// The tables the solve's queries name, for the catalog.
pub const READS: [&str; 2] = ["FROM overture_transportation", "FROM overture_base_water"];

const FLAGGED: &str = "COALESCE(t.road_flags, t.rail_flags)::text ~ 'is_bridge|is_tunnel'";

const ROW: &str = "t.gers_id, t.subtype, t.class, COALESCE(t.road_flags, t.rail_flags)::text AS flags, t.level_rules::text AS levels,
    t.width_rules::text AS widths, t.access_restrictions::text AS access,
    t.subclass, t.subclass_rules::text AS subclasses, ST_AsBinary(t.geom) AS geom_wkb";

fn bridge_row(r: &postgres::Row) -> BridgeRow {
    BridgeRow {
        gers_id: r.get("gers_id"),
        subtype: r.get("subtype"),
        class: r.get("class"),
        flags: r.get("flags"),
        levels: r.get("levels"),
        widths: r.get("widths"),
        access: r.get("access"),
        subclass: r.get("subclass"),
        subclasses: r.get("subclasses"),
        geom_wkb: r.get::<_, &[u8]>("geom_wkb").into(),
    }
}

/// Everything a solve reads: the bridge and tunnel rows, every way touching
/// them, and the water under them.
pub struct Input {
    pub rows: Vec<BridgeRow>,
    pub ways: Vec<Way>,
    pub water: Vec<Water>,
    pub window: Option<Window>,
}

/// What a page's solve is bounded to: its pages' box, and their ground.
pub type Window = ([[f64; 2]; 2], HashMap<IVec, Vec<i16>>);

/// One page's rows, as a page's other rows are read: whatever overlaps it.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct PageRows {
    rows: Vec<BridgeRow>,
    ways: Vec<BridgeRow>,
    water: Vec<(String, Option<String>, Vec<u8>)>,
    grid: Option<Vec<i16>>,
}

fn page_rows(client: &mut Client, raster: &Raster, page: IVec) -> Result<PageRows, postgres::Error> {
    let [min, max] = epsg3857::get_page_bbox(page);
    let env: [&(dyn ToSql + Sync); 4] = [&min[0], &min[1], &max[0], &max[1]];
    let read = |client: &mut Client, sql: &str| client.query(sql, &env);
    let rows = read(client, &format!("SELECT {ROW} FROM overture_transportation t WHERE {FLAGGED} AND t.geom && ST_MakeEnvelope($1, $2, $3, $4, 3857)"))?;
    let ways = read(client, &format!("SELECT {ROW} FROM overture_transportation t WHERE t.geom && ST_MakeEnvelope($1, $2, $3, $4, 3857)"))?;
    let water = read(client, "SELECT subtype, class, ST_AsBinary(geom) AS geom_wkb FROM overture_base_water WHERE subtype <> 'ocean' AND geom && ST_MakeEnvelope($1, $2, $3, $4, 3857)")?;
    Ok(PageRows {
        rows: rows.iter().map(bridge_row).collect(),
        ways: ways.iter().map(bridge_row).collect(),
        water: water.iter().map(|r| (r.get("subtype"), r.get("class"), r.get::<_, &[u8]>("geom_wkb").to_vec())).collect(),
        grid: raster.page_grid(client, [min, max])?,
    })
}

/// Where a page's rows are kept for its neighbours.
pub trait Store {
    fn page(&mut self, client: &mut Client, raster: &Raster, page: IVec) -> Result<PageRows, String>;
}

/// On disk, shared by the processes that bake pages on demand.
pub struct Disk<'a>(pub &'a Path);

impl Store for Disk<'_> {
    fn page(&mut self, client: &mut Client, raster: &Raster, page: IVec) -> Result<PageRows, String> {
        memo_rows(client, raster, page, self.0)
    }
}

/// In memory, for a walk from page to neighbouring page: holds a page's
/// eight around it and no more.
#[derive(Default)]
pub struct Walk {
    held: HashMap<IVec, PageRows>,
}

impl Walk {
    /// Drops what is not in `page`'s 3×3.
    pub fn keep_around(&mut self, page: IVec) {
        let near = |p: IVec| (ivec::X(p) - ivec::X(page)).abs() <= 1 && (ivec::Y(p) - ivec::Y(page)).abs() <= 1;
        self.held.retain(|&p, _| near(p));
    }
}

impl Store for Walk {
    fn page(&mut self, client: &mut Client, raster: &Raster, page: IVec) -> Result<PageRows, String> {
        if let Some(rows) = self.held.get(&page) {
            return Ok(rows.clone());
        }
        let rows = page_rows(client, raster, page).map_err(|e| e.to_string())?;
        self.held.insert(page, rows.clone());
        Ok(rows)
    }
}

/// A page's rows from `memo` when read before, else from the DB into it.
fn memo_rows(client: &mut Client, raster: &Raster, page: IVec, memo: &Path) -> Result<PageRows, String> {
    let file = memo.join(page.to_string());
    if let Some(rows) = std::fs::read(&file).ok().and_then(|b| unpack(&b).ok()).and_then(|b| serde_json::from_slice(&b).ok()) {
        return Ok(rows);
    }
    let rows = page_rows(client, raster, page).map_err(|e| e.to_string())?;
    let packed = pack(&serde_json::to_vec(&rows).map_err(|e| e.to_string())?, Level::Live).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(memo).map_err(|e| e.to_string())?;
    write_atomic(&file, &packed).map_err(|e| e.to_string())?;
    Ok(rows)
}

/// What a page and the eight around it hold, each read once into `store`.
pub fn read_ring(client: &mut Client, page: IVec, store: &mut impl Store) -> Result<Input, String> {
    let (px, py) = (ivec::X(page), ivec::Y(page));
    let raster = Raster::open(client).map_err(|e| e.to_string())?;
    let (mut rows, mut ways, mut water, mut grids) = (BTreeMap::new(), BTreeMap::new(), BTreeMap::new(), HashMap::new());
    let ([lo, _], [_, hi]) = (epsg3857::get_page_bbox(ivec::gather(px - 1, py - 1, 0)), epsg3857::get_page_bbox(ivec::gather(px + 1, py + 1, 0)));
    for (dx, dy) in (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy))) {
        let at = ivec::gather(px + dx, py + dy, 0);
        let p = store.page(client, &raster, at)?;
        if let Some(grid) = p.grid {
            grids.insert(at, grid);
        }
        rows.extend(p.rows.into_iter().map(|r| (r.gers_id.clone(), r)));
        ways.extend(p.ways.into_iter().map(|r| (r.gers_id.clone(), r)));
        water.extend(p.water.into_iter().map(|w| (w.2.clone(), w)));
    }
    Ok(Input {
        rows: rows.into_values().collect(),
        ways: ways.into_values().filter_map(Way::of).collect(),
        water: water.into_values().map(|(s, c, wkb)| Water::of(s, c, &wkb)).collect(),
        window: Some(([[lo[0].min(hi[0]), lo[1].min(hi[1])], [lo[0].max(hi[0]), lo[1].max(hi[1])]], grids)),
    })
}
