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

/// Every segment a bridge flag is on.
pub fn bridge_rows(client: &mut Client) -> Result<Vec<BridgeRow>, postgres::Error> {
    let rows = client.query(
        "SELECT gers_id, subtype, class, COALESCE(road_flags, rail_flags)::text AS flags, level_rules::text AS levels,
                width_rules::text AS widths, access_restrictions::text AS access,
                subclass, subclass_rules::text AS subclasses, ST_AsBinary(geom) AS geom_wkb
         FROM overture_transportation
         WHERE COALESCE(road_flags, rail_flags)::text LIKE '%is_bridge%'",
        &[],
    )?;
    Ok(rows
        .iter()
        .map(|r| BridgeRow {
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
        })
        .collect())
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
    let water = read(client, "SELECT subtype, class, ST_AsBinary(geom) AS geom_wkb FROM overture_base_water WHERE subtype <> 'ocean' AND geom && ST_MakeEnvelope($1, $2, $3, $4, 3857)")?;
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
        })
        .collect())
}

/// Where a piece crosses one of a lower level: the upper's piece and how far
/// along it, the lower's and how far along it, in metres.
pub struct DeckCrossing {
    pub upper: usize,
    pub upper_at: f64,
    pub lower: usize,
    pub lower_at: f64,
}

pub fn decks_crossing(client: &mut Client) -> Result<Vec<DeckCrossing>, postgres::Error> {
    let rows = client.query(
        "SELECT u.id AS upper, ST_LineLocatePoint(u.geom, d.geom) * ST_Length(u.geom) AS upper_at,
                l.id AS lower, ST_LineLocatePoint(l.geom, d.geom) * ST_Length(l.geom) AS lower_at
         FROM deck_piece u
         JOIN deck_piece l ON l.geom && u.geom AND l.level < u.level AND ST_Intersects(l.geom, u.geom)
         CROSS JOIN LATERAL ST_Dump(ST_Intersection(u.geom, l.geom)) d
         WHERE GeometryType(d.geom) = 'POINT'",
        &[],
    )?;
    Ok(rows
        .iter()
        .map(|r| DeckCrossing {
            upper: r.get::<_, i32>("upper") as usize,
            upper_at: r.get("upper_at"),
            lower: r.get::<_, i32>("lower") as usize,
            lower_at: r.get("lower_at"),
        })
        .collect())
}

/// A solved deck, as `overture_deck` holds it.
pub struct DeckOut<'a> {
    pub piece: &'a Piece,
    /// Each point, its top in metres as Z.
    pub points: Vec<[f64; 3]>,
    pub piers: Vec<u8>,
}

/// `overture_deck`, made again whole: the bake reads it a page at a time
/// (terrain/repository.rs).
pub fn write(client: &mut Client, decks: &[DeckOut]) -> Result<(), postgres::Error> {
    client.batch_execute(
        "DROP TABLE IF EXISTS overture_deck;
         CREATE TABLE overture_deck (gers_id TEXT NOT NULL, subtype TEXT NOT NULL, class TEXT, width_m REAL NOT NULL, piers BYTEA NOT NULL, geom GEOMETRY(LineStringZ, 3857) NOT NULL);",
    )?;
    for batch in decks.chunks(BATCH) {
        let gers: Vec<&str> = batch.iter().map(|d| d.piece.gers_id.as_str()).collect();
        let subtypes: Vec<&str> = batch.iter().map(|d| d.piece.subtype.as_str()).collect();
        let classes: Vec<Option<&str>> = batch.iter().map(|d| d.piece.class.as_deref()).collect();
        let widths: Vec<f32> = batch.iter().map(|d| d.piece.width_m as f32).collect();
        let piers: Vec<&[u8]> = batch.iter().map(|d| d.piers.as_slice()).collect();
        let lines: Vec<String> = batch.iter().map(|d| wkt(d.points.iter().map(|q| format!("{} {} {:.2}", q[0], q[1], q[2])), true)).collect();
        client.execute(
            "INSERT INTO overture_deck SELECT g, s, c, m, p, ST_GeomFromText(w, 3857) FROM unnest($1::text[], $2::text[], $3::text[], $4::real[], $5::bytea[], $6::text[]) AS t(g, s, c, m, p, w)",
            &[&gers, &subtypes, &classes, &widths, &piers, &lines],
        )?;
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
