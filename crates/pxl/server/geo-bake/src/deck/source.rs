// pxl-geo-bake/deck/source.rs

//! What a page's solve reads.

use postgres::Client;
use std::collections::HashMap;
use std::path::Path;

use postgres::types::ToSql;
use pxl_spec::bake::format::GroundCell;
use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::pvf3::ivec::{self, IVec};

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

/// One page's rows, as a page's other rows are read: whatever overlaps it.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct PageRows {
    ways: Vec<Segment>,
    water: Vec<(String, Option<String>, Vec<u8>)>,
    /// Its ground as the client draws it: the elevation and the cells whose shift goes on it.
    ele: Vec<i16>,
    cells: Vec<GroundCell>,
}

impl PageRows {
    /// Every way it overlaps, the bridges and tunnels among them.
    pub fn ways(&self) -> &[Segment] {
        &self.ways
    }

    /// Its water as read: subtype, class and geometry.
    pub fn water(&self) -> &[(String, Option<String>, Vec<u8>)] {
        &self.water
    }

    pub fn surface(&self) -> Surface {
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

/// A page and the eight around it.
pub fn ring(page: IVec) -> Vec<IVec> {
    let (px, py) = (ivec::X(page), ivec::Y(page));
    (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| ivec::gather(px + dx, py + dy, 0))).collect()
}
