// pxl-geo-bake/deck/source.rs

//! What a page's solve reads.

use postgres::Client;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use postgres::types::ToSql;
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::pvf3::ivec::{self, IVec};

use crate::deck::relate::{Water, Way};
use crate::segment::Segment;
use crate::terrain::bake::Surface;
use crate::terrain::repository::TerrainRepository;
use crate::terrain::source::fetch_surface;
use crate::util::{pack, unpack, write_atomic, Level};

/// The tables the solve's queries name, for the catalog.
pub const READS: [&str; 2] = ["FROM overture_transportation", "FROM overture_base_water"];

const SEGMENTS_SQL: &str = concat!(
    "SELECT ",
    crate::segment_columns!(),
    " FROM overture_transportation t WHERE t.geom && ST_MakeEnvelope($1, $2, $3, $4, 3857)"
);

/// Everything a solve reads of some pages: the bridge and tunnel rows, every
/// way, the water, and the ground, as the client draws it, inside `bounds`.
pub struct Input {
    pub rows: Vec<Segment>,
    pub ways: Vec<Way>,
    pub water: Vec<Water>,
    pub bounds: [[f64; 2]; 2],
    pub ground: HashMap<IVec, Surface>,
}

/// One page's rows, as a page's other rows are read: whatever overlaps it.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct PageRows {
    ways: Vec<Segment>,
    water: Vec<(String, Option<String>, Vec<u8>)>,
    /// Its ground as the client draws it: the elevation and the cells whose shift goes on it.
    ele: Vec<i16>,
    cells: Vec<u16>,
}

impl PageRows {
    fn surface(&self) -> Surface {
        Surface {
            ele: self.ele.clone(),
            ground: pxl_spec::bake::format::Ground { cells: self.cells.clone().into() },
        }
    }
}

fn page_rows(client: &mut Client, repo: &TerrainRepository, page: IVec) -> Result<PageRows, String> {
    let [min, max] = epsg3857::get_page_bbox(page);
    let env: [&(dyn ToSql + Sync); 4] = [&min[0], &min[1], &max[0], &max[1]];
    let read = |client: &mut Client, sql: &str| client.query(sql, &env).map_err(|e| e.to_string());
    let ways = read(client, SEGMENTS_SQL)?;
    let water = read(
        client,
        "SELECT subtype, class, ST_AsBinary(geom) AS geom_wkb FROM overture_base_water WHERE subtype <> 'ocean' AND geom && ST_MakeEnvelope($1, $2, $3, $4, 3857)",
    )?;
    // the paint of its ways is no matter to the ground's height, so none is drawn
    let (surface, _) = fetch_surface(repo, client, page, &[])?;
    Ok(PageRows {
        ways: ways.iter().map(Segment::of).collect(),
        water: water.iter().map(|r| (r.get("subtype"), r.get("class"), r.get::<_, &[u8]>("geom_wkb").to_vec())).collect(),
        ele: surface.ele,
        cells: surface.ground.cells.to_vec(),
    })
}

/// Where a page's rows are kept for its neighbours.
pub trait Store {
    fn page(&mut self, client: &mut Client, repo: &TerrainRepository, page: IVec) -> Result<PageRows, String>;
}

/// On disk, shared by the processes that bake pages on demand.
pub struct Disk<'a>(pub &'a Path);

impl Store for Disk<'_> {
    fn page(&mut self, client: &mut Client, repo: &TerrainRepository, page: IVec) -> Result<PageRows, String> {
        memo_rows(client, repo, page, self.0)
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
    fn page(&mut self, client: &mut Client, repo: &TerrainRepository, page: IVec) -> Result<PageRows, String> {
        if let Some(rows) = self.held.get(&page) {
            return Ok(rows.clone());
        }
        let rows = page_rows(client, repo, page)?;
        self.held.insert(page, rows.clone());
        Ok(rows)
    }
}

/// A page's rows from `memo` when read before, else from the DB into it.
fn memo_rows(client: &mut Client, repo: &TerrainRepository, page: IVec, memo: &Path) -> Result<PageRows, String> {
    let file = memo.join(page.to_string());
    if let Some(rows) = std::fs::read(&file).ok().and_then(|b| unpack(&b).ok()).and_then(|b| serde_json::from_slice(&b).ok()) {
        return Ok(rows);
    }
    let rows = page_rows(client, repo, page)?;
    let packed = pack(&serde_json::to_vec(&rows).map_err(|e| e.to_string())?, Level::Live).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(memo).map_err(|e| e.to_string())?;
    write_atomic(&file, &packed).map_err(|e| e.to_string())?;
    Ok(rows)
}

/// What pages hold, each read once into `store`.
pub fn read_pages(client: &mut Client, repo: &TerrainRepository, pages: &[IVec], store: &mut impl Store) -> Result<Input, String> {
    let (mut ways, mut water, mut ground) = (BTreeMap::new(), BTreeMap::new(), HashMap::new());
    let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for &at in pages {
        for corner in epsg3857::get_page_bbox(at) {
            for k in 0..2 {
                (lo[k], hi[k]) = (lo[k].min(corner[k]), hi[k].max(corner[k]));
            }
        }
        let p = store.page(client, repo, at)?;
        ground.insert(at, p.surface());
        ways.extend(p.ways.into_iter().map(|r| (r.gers_id.clone(), r)));
        water.extend(p.water.into_iter().map(|w| (w.2.clone(), w)));
    }
    let ways: Vec<Segment> = ways.into_values().collect();
    Ok(Input {
        rows: ways.iter().filter(|w| w.flagged()).cloned().collect(),
        ways: ways.into_iter().filter_map(Way::of).collect(),
        water: water.into_values().map(|(s, c, wkb)| Water::of(s, c, &wkb)).collect(),
        bounds: [lo, hi],
        ground,
    })
}

/// A page and the eight around it.
pub fn ring(page: IVec) -> Vec<IVec> {
    let (px, py) = (ivec::X(page), ivec::Y(page));
    (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| ivec::gather(px + dx, py + dy, 0))).collect()
}

/// The bridge and tunnel rows touching a point, wherever it is.
pub fn flagged_at(client: &mut Client, [x, y]: [f64; 2]) -> Result<Vec<Segment>, String> {
    let e = TOUCH;
    let env: [&(dyn ToSql + Sync); 4] = [&(x - e), &(y - e), &(x + e), &(y + e)];
    Ok(client.query(SEGMENTS_SQL, &env).map_err(|e| e.to_string())?.iter().map(Segment::of).filter(Segment::flagged).collect())
}

/// How near a row's line must come to a point to be asked of it, in DB
/// coordinates; whether it joins is the ends' identity, not this.
const TOUCH: f64 = 0.01;
