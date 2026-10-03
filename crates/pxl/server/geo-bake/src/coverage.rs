// pxl-geo-bake/coverage.rs

//! What the bakes publish (from `infra/geo/elevation/metadata.json`
//! `bbox_4326`), what an ingest must take for it, and the one door to the DB.

use std::sync::LazyLock;

use postgres::{Client, NoTls};
use pxl_geo_spec::overture::division::NEAREST_REACH_M;
use pxl_geo_spec::table::CATALOG;
use pxl_spec::space::adapter::epsg3857::{from_db, u_per_m};
use pxl_spec::space::adapter::{epsg3857, epsg4326};
use pxl_spec::space::hierarchy::ancestor_of;
use pxl_spec::space::indexer::{L3_IDX, L3_RANGE};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::Pvf3;

use crate::elevation::page_reads;
use crate::terrain::repository::DRAWN_REACH_U;

const METADATA: &str = include_str!("../../../../../infra/geo/elevation/metadata.json");

/// The raster's `[west, south]`, `[east, north]`.
pub fn source_bbox() -> Result<[[f64; 2]; 2], String> {
    let bad = |why: String| format!("infra/geo/elevation/metadata.json: {why}");
    let entries: serde_json::Value = serde_json::from_str(METADATA).map_err(|e| bad(e.to_string()))?;
    let Some([entry]) = entries.as_array().map(Vec::as_slice) else {
        return Err(bad("want an array of exactly one entry".into()));
    };
    let edge = |k: &str| entry["bbox_4326"][k].as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad(format!("bbox_4326.{k} is not a number")));
    let (west, south, east, north) = (edge("west")?, edge("south")?, edge("east")?, edge("north")?);
    if !(west < east && south < north) {
        return Err(bad("bbox_4326 is empty".into()));
    }
    Ok([[west, south], [east, north]])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Coverage {
    pub x: (i64, i64),
    pub y: (i64, i64),
}

static SOURCE: LazyLock<Result<Coverage, String>> = LazyLock::new(Coverage::derive);

fn project_page(lng: f64, lat: f64) -> IVec {
    let mut p = Pvf3 { i: 0, f: 0 };
    epsg4326::project(&mut p, &[lng, lat, 0.0]);
    L3_IDX(p.i)
}

impl Coverage {
    pub fn of_source() -> Result<Coverage, String> {
        SOURCE.clone()
    }

    /// The z16 pages whose padded elevation grid lies in the raster, each axis on its own.
    fn derive() -> Result<Coverage, String> {
        let [sw, ne] = source_bbox()?;
        let (p0, p1) = (project_page(sw[0], sw[1]), project_page(ne[0], ne[1]));
        let padded = |px: i64, py: i64| {
            let [x0, y0, x1, y1] = page_reads(epsg3857::get_page_bbox(ivec::gather(px, py, 0)));
            let (mut a, mut b) = ([0.0; 3], [0.0; 3]);
            epsg4326::from_mercator(&mut a, &[x0, y0, 0.0]);
            epsg4326::from_mercator(&mut b, &[x1, y1, 0.0]);
            [a[0], a[1], b[0], b[1]]
        };
        let whole = |a: i64, b: i64, inside: &dyn Fn(i64) -> bool| {
            let pass: Vec<i64> = (a.min(b)..=a.max(b)).filter(|&i| inside(i)).collect();
            Some((*pass.first()?, *pass.last()?))
        };
        let x = whole(ivec::X(p0), ivec::X(p1), &|px| {
            let b = padded(px, ivec::Y(p0));
            sw[0] <= b[0] && b[2] <= ne[0]
        });
        let y = whole(ivec::Y(p0), ivec::Y(p1), &|py| {
            let b = padded(ivec::X(p0), py);
            sw[1] <= b[1] && b[3] <= ne[1]
        });
        match (x, y) {
            (Some(x), Some(y)) => Ok(Coverage { x, y }),
            _ => Err("infra/geo/elevation/metadata.json: bbox_4326 covers no whole page".into()),
        }
    }

    pub fn contains(&self, page: IVec) -> bool {
        (self.x.0..=self.x.1).contains(&ivec::X(page)) && (self.y.0..=self.y.1).contains(&ivec::Y(page))
    }

    pub fn meets(&self, level: u32, page: IVec) -> bool {
        let ([x0, y0], [x1, y1]) = (ancestor_of([self.x.0, self.y.0], level), ancestor_of([self.x.1, self.y.1], level));
        (x0..=x1).contains(&ivec::X(page)) && (y0..=y1).contains(&ivec::Y(page))
    }

    pub fn pages(&self) -> Vec<IVec> {
        let mut out = Vec::new();
        for x in self.x.0..=self.x.1 {
            for y in self.y.0..=self.y.1 {
                out.push(ivec::gather(x, y, 0));
            }
        }
        out
    }

    /// The published pages' union in EPSG:3857, `[minx, miny, maxx, maxy]`.
    pub fn rect(&self) -> [f64; 4] {
        let [a0, a1] = epsg3857::get_page_bbox(ivec::gather(self.x.0, self.y.0, 0));
        let [b0, b1] = epsg3857::get_page_bbox(ivec::gather(self.x.1, self.y.1, 0));
        [a0[0].min(b0[0]), a0[1].min(b0[1]), a1[0].max(b1[0]), a1[1].max(b1[1])]
    }

    /// Pages past the published ones that hold the farthest an Overture read
    /// goes: a page's `DRAWN_REACH_U`, or `NEAREST_REACH_M` where k is largest.
    pub fn margin_pages(&self) -> i64 {
        let [_, y0, _, y1] = self.rect();
        let label = NEAREST_REACH_M * u_per_m(from_db(y0.abs().max(y1.abs())));
        (DRAWN_REACH_U.max(label) / L3_RANGE as f64).ceil() as i64
    }

    /// The published pages grown by `margin_pages`, in degrees: page edges, a
    /// margin from any row a page reads.
    pub fn ingest_bbox(&self) -> [f64; 4] {
        let m = self.margin_pages();
        let [a, _] = epsg3857::get_page_bbox(ivec::gather(self.x.0 - m, self.y.0 - m, 0));
        let [_, b] = epsg3857::get_page_bbox(ivec::gather(self.x.1 + m, self.y.1 + m, 0));
        let (mut sw, mut ne) = ([0.0; 3], [0.0; 3]);
        epsg4326::from_mercator(&mut sw, &[a[0].min(b[0]), a[1].min(b[1]), 0.0]);
        epsg4326::from_mercator(&mut ne, &[a[0].max(b[0]), a[1].max(b[1]), 0.0]);
        [sw[0], sw[1], ne[0], ne[1]]
    }
}

/// As `--bbox` takes it and `overture_catalog.region_code` records it.
pub fn bbox_arg(b: [f64; 4]) -> String {
    format!("{},{},{},{}", b[0], b[1], b[2], b[3])
}

/// The `overture_*` tables a set of statements names, outside string
/// literals and `--` comments, without the catalog; sorted, each once.
pub fn tables_of<'a>(reads: &[&[&'a str]]) -> Vec<&'a str> {
    let mut out: Vec<&str> = Vec::new();
    for sql in reads.iter().flat_map(|r| r.iter()) {
        let b = sql.as_bytes();
        let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
        let (mut i, mut quoted) = (0, false);
        while i < b.len() {
            if b[i] == b'\'' {
                quoted = !quoted;
            } else if !quoted && b[i..].starts_with(b"--") {
                i += b[i..].iter().position(|&c| c == b'\n').unwrap_or(b.len() - i);
                continue;
            } else if !quoted && b[i..].starts_with(b"overture_") && (i == 0 || !word(b[i - 1])) {
                let end = i + b[i..].iter().position(|&c| !word(c)).unwrap_or(b.len() - i);
                out.push(&sql[i..end]);
                i = end;
                continue;
            }
            i += 1;
        }
    }
    out.retain(|t| *t != CATALOG);
    out.sort_unstable();
    out.dedup();
    out
}

/// The geo DB, opened only when every table a bake reads is one release over
/// exactly [`Coverage::ingest_bbox`]: exactly, since some reads are unbounded
/// (the place dictionary, a deck's structure).
pub struct Overture {
    url: String,
    q: [f64; 4],
    pub release: String,
    /// Each table read, with its row count.
    pub tables: Vec<(String, i64)>,
}

impl Overture {
    /// For a bake whose statements are `reads`.
    pub fn reading(reads: &[&[&str]]) -> Result<(Overture, Client), String> {
        Overture::open(&tables_of(reads))
    }

    pub fn open(tables: &[&str]) -> Result<(Overture, Client), String> {
        let url = std::env::var("DB_GEO_URL").map_err(|_| "env:DB_GEO_URL:missing")?;
        let q = Coverage::of_source()?.ingest_bbox();
        let ingest = format!("ingest with --bbox {} (bin ingest_bbox)", bbox_arg(q));
        let mut client = Client::connect(&url, NoTls).map_err(|e| e.to_string())?;
        let rows = client
            .query(
                &format!("SELECT file_name, region_code, data_date::text, total_count FROM {CATALOG} WHERE file_name = ANY($1)"),
                &[&tables],
            )
            .map_err(|e| format!("{CATALOG}: {e}: {ingest}"))?;
        let mut release: Option<(String, String)> = None;
        let mut out = Vec::with_capacity(tables.len());
        for &table in tables {
            let row = rows.iter().find(|r| r.get::<_, &str>(0) == table).ok_or_else(|| format!("{table} has no catalog row: {ingest}"))?;
            let (region, date): (&str, String) = (row.get(1), row.get(2));
            let have: Option<Vec<f64>> = region.split(',').map(|v| v.trim().parse::<f64>().ok()).collect();
            if have.as_deref() != Some(q.as_slice()) {
                return Err(format!("{table} was ingested over {region}, not the coverage's bbox: {ingest}"));
            }
            match &release {
                None => release = Some((table.to_string(), date)),
                Some((first, r)) if *r != date => return Err(format!("{table} is release {date} and {first} is {r}: one bake reads one release; {ingest}")),
                Some(_) => {}
            }
            out.push((table.to_string(), row.get::<_, i64>(3)));
        }
        let (_, release) = release.ok_or("a bake that reads no Overture table")?;
        Ok((Overture { url, q, release, tables: out }, client))
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn connect(&self) -> Result<Client, String> {
        Client::connect(&self.url, NoTls).map_err(|e| e.to_string())
    }

    /// No bake opens `table` until [`Overture::stamp`]: a solve stopped partway leaves it unread.
    pub fn forget(&self, client: &mut Client, table: &str) -> Result<(), String> {
        client.execute(&format!("DELETE FROM {CATALOG} WHERE file_name = $1"), &[&table]).map(drop).map_err(|e| e.to_string())
    }

    /// Records `table` as the release and bbox it was made from.
    pub fn stamp(&self, client: &mut Client, table: &str) -> Result<(), String> {
        let count: i64 = client.query_one(&format!("SELECT count(*) FROM {table}"), &[]).map_err(|e| e.to_string())?.get(0);
        client
            .execute(
                &format!(
                    "INSERT INTO {CATALOG} (file_name, region_code, data_date, total_count) VALUES ($1, $2, $3, $4)
                     ON CONFLICT (file_name) DO UPDATE SET region_code = excluded.region_code, data_date = excluded.data_date,
                       imported_at = now(), status = 'COMPLETED', total_count = excluded.total_count"
                ),
                &[&table, &bbox_arg(self.q), &self.release, &count],
            )
            .map(drop)
            .map_err(|e| e.to_string())
    }

    /// What a manifest records of the source.
    pub fn manifest(&self) -> serde_json::Value {
        let tables: serde_json::Map<String, serde_json::Value> = self.tables.iter().map(|(t, n)| (t.clone(), serde_json::json!({ "release": self.release, "row_count": n }))).collect();
        serde_json::Value::Object(tables)
    }
}
