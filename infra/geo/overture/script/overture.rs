//! How an Overture theme reaches db-geo, shared by the five tools that pull
//! one.
//!
//! The five differ only in which type they download, what their table looks
//! like and how a feature's properties map onto its columns. Everything
//! around that — finding the CLI, the download, reading the extract a line at
//! a time, batching the insert, `ANALYZE`, and the catalog row that records
//! which Overture release the table is from — is the same path, and is here.
//!
//! **`-f geojsonseq`, never plain geojson.** South Korea's full Places
//! extract is 700K+ features; as one pretty-printed FeatureCollection it came
//! to ~900 MB and blew the runtime's own maximum string length when loaded
//! whole (confirmed live, in the TypeScript this is ported from). A line at a
//! time keeps memory flat whatever the country's size.
//!
//! **Full replace, never an incremental diff.** Overture cuts a new dated
//! release roughly monthly rather than streaming edits, so there is nothing to
//! merge against: each tool's schema drops its table and recreates it.
//! `overture_catalog` is the exception and accumulates, one upserted row per
//! table.

// Each ingest is its own script and takes a different part of this: Places
// binds no GeoJSON geometry, Base no jsonb column, Transportation no integer.
// What one of them does not reach is not unreachable, so the whole module
// would otherwise warn on every run of every script.
#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;

use postgres::types::ToSql;
use postgres::Client;
use serde_json::value::RawValue;
use serde_json::Value;

/// The official `overturemaps` Python CLI, installed on first use into a venv
/// beside the crate's own scratch directory. It is the one thing in this
/// pipeline that is neither Rust nor SQL, and it is Overture's own tool for
/// reading their release — reimplementing their S3 layout and GeoParquet
/// reader to avoid a Python dependency would be a worse trade than the venv.
const VENV: &str = "./out/overture/.venv";

fn venv_overturemaps() -> PathBuf {
    Path::new(VENV).join("bin/overturemaps")
}

fn run(cmd: &Path, args: &[&str]) -> Result<(), String> {
    let status = Command::new(cmd).args(args).status().map_err(|e| format!("{}: {e}", cmd.display()))?;
    if !status.success() {
        return Err(format!("{} {} exited {}", cmd.display(), args.join(" "), status.code().unwrap_or(-1)));
    }
    Ok(())
}

fn ensure_cli() -> Result<PathBuf, String> {
    let cli = venv_overturemaps();
    // Run it rather than stat it. A venv is not relocatable — its scripts
    // carry the absolute path of the interpreter they were made with in a
    // shebang — so a tree that moved leaves one that exists and cannot run,
    // and a check for the file alone reports it as ready and then fails at
    // the download. This is how a stale venv from an earlier layout survived
    // unnoticed until it was next used.
    if cli.exists() && Command::new(&cli).arg("--version").output().is_ok_and(|o| o.status.success()) {
        return Ok(cli);
    }
    if cli.exists() {
        println!("[ingest] the venv at {VENV} cannot run; rebuilding it");
        std::fs::remove_dir_all(VENV).map_err(|e| e.to_string())?;
    }
    println!("[ingest] no venv at {VENV}, creating one and installing overturemaps...");
    std::fs::create_dir_all(Path::new(VENV).parent().unwrap_or(Path::new("."))).map_err(|e| e.to_string())?;
    run(Path::new("python3"), &["-m", "venv", VENV])?;
    run(&Path::new(VENV).join("bin/pip"), &["install", "--quiet", "overturemaps"])?;
    Ok(cli)
}

/// `overturemaps releases latest`, e.g. `2026-08-19.0`.
///
/// Recorded as a table's `data_date` rather than the day the ingest ran: the
/// release id is the data snapshot's own as-of date, and the run date is only
/// when someone got around to pulling it.
pub fn release(cli: &Path) -> Result<String, String> {
    let out = Command::new(cli).args(["releases", "latest"]).output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("overturemaps releases latest exited {}", out.status.code().unwrap_or(-1)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

const CATALOG_SQL: &str = "
CREATE TABLE IF NOT EXISTS overture_catalog (
  file_name TEXT PRIMARY KEY,
  region_code TEXT NOT NULL,
  data_date TEXT NOT NULL,
  imported_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  status TEXT NOT NULL DEFAULT 'COMPLETED',
  total_count BIGINT NOT NULL DEFAULT 0
);
";

/// Provenance, per source table: which bbox it was pulled with and which
/// Overture release it is from. A version folder's name says nothing about
/// when the data under it is from; this is what answers that, from the
/// database rather than from a commit message. The bake's manifest reads it.
pub fn record_catalog(client: &mut Client, table: &str, bbox: &str, data_date: &str, total: i64) -> Result<(), String> {
    client.batch_execute(CATALOG_SQL).map_err(|e| e.to_string())?;
    client
        .execute(
            "INSERT INTO overture_catalog (file_name, region_code, data_date, total_count)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (file_name) DO UPDATE SET
               region_code = excluded.region_code,
               data_date = excluded.data_date,
               imported_at = now(),
               status = 'COMPLETED',
               total_count = excluded.total_count",
            &[&table, &bbox, &data_date, &total],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// One column's value in one row, as the insert binds it.
///
/// Absence is inside each variant rather than a `Null` of its own: a
/// parameter is serialized by its Rust type, so an untyped NULL reaches a
/// `double precision` column as text and the driver refuses it ("error
/// serializing parameter"). A missing height has to be a missing *float*.
pub enum Cell {
    Text(Option<String>),
    F64(Option<f64>),
    I32(Option<i32>),
    /// The source's own bytes, bound as text and parsed by Postgres.
    ///
    /// Never a re-serialized `Value`. jsonb keeps a number as `numeric`, so
    /// its decimal text is part of the value — and rendering a parsed double
    /// back to text does not reproduce it: serde_json writes
    /// `0.9018452233076095` as `0.9018452233076096`, the same double and a
    /// different numeric. The cast is `::text::jsonb` rather than `::jsonb`
    /// for the same reason it is bound as text: `$n::jsonb` would make
    /// Postgres infer the parameter itself as jsonb.
    Json(Option<String>),
}

impl Cell {
    fn as_sql(&self) -> &(dyn ToSql + Sync) {
        match self {
            Cell::Text(v) | Cell::Json(v) => v,
            Cell::F64(v) => v,
            Cell::I32(v) => v,
        }
    }

    fn cast(&self) -> &'static str {
        match self {
            Cell::Json(_) => "::text::jsonb",
            _ => "",
        }
    }
}

/// How a row's geometry reaches the `geom` column, which is always the last.
pub enum Geom {
    /// The feature's own `geometry`, as GeoJSON text: one parameter.
    GeoJson,
    /// A longitude and a latitude: two parameters. Places' geometry is a point
    /// and a point is cheaper to build than to parse.
    Point,
}

pub struct Table {
    pub name: &'static str,
    /// Every column but `geom`, in the order `row` returns them.
    pub columns: &'static [&'static str],
    pub geom: Geom,
    /// `DROP TABLE` and everything that recreates it, indexes included.
    pub schema: String,
    /// Rows per insert. Points batch larger than polygons: the limit is the
    /// payload's size, not the row count.
    pub batch: usize,
}

/// What a feature became, or `None` for one this theme does not keep.
pub struct Row {
    pub cells: Vec<Cell>,
    /// GeoJSON text for `Geom::GeoJson`, `(lon, lat)` for `Geom::Point`.
    pub geom: GeomValue,
}

pub enum GeomValue {
    GeoJson(String),
    Point(f64, f64),
}

/// One line of the extract: its fields left as the source wrote them, and
/// `properties` parsed on top for the tools that read a field out of it.
///
/// The geometry is never parsed. It is the largest field by far, nothing here
/// reads into it, and what reaches `ST_GeomFromGeoJSON` has to be the source's
/// own text — a re-rendered coordinate is a different `numeric` and, once
/// projected, a different point by an ULP.
pub struct Feature<'a> {
    raw: HashMap<&'a str, &'a RawValue>,
    properties: Value,
}

impl<'a> Feature<'a> {
    pub fn parse(line: &'a str) -> Result<Feature<'a>, String> {
        let raw: HashMap<&str, &RawValue> = serde_json::from_str(line).map_err(|e| e.to_string())?;
        let properties = match raw.get("properties") {
            Some(p) => serde_json::from_str::<Value>(p.get()).map_err(|e| e.to_string())?,
            None => Value::Null,
        };
        Ok(Feature { raw, properties })
    }

    /// The GERS id, which is every table's primary key.
    pub fn id(&self) -> Option<&'a str> {
        serde_json::from_str(self.raw.get("id")?.get()).ok()
    }

    /// `geometry`, exactly as the source wrote it.
    pub fn geometry(&self) -> Option<&'a str> {
        Some(self.raw.get("geometry")?.get())
    }

    /// `properties`, exactly as the source wrote it.
    pub fn properties_raw(&self) -> Option<&'a str> {
        Some(self.raw.get("properties")?.get())
    }

    /// One of `properties`' own fields, exactly as the source wrote it.
    pub fn prop_raw(&self, key: &str) -> Option<String> {
        let props: HashMap<&str, &RawValue> = serde_json::from_str(self.raw.get("properties")?.get()).ok()?;
        Some(props.get(key)?.get().to_string())
    }

    pub fn prop_str(&self, key: &str) -> Option<&str> {
        self.properties.get(key)?.as_str()
    }

    /// Overture writes some numbers as JSON numbers and some as strings
    /// depending on the theme's schema, so both are accepted rather than one
    /// being assumed. Only for a `double precision` or `integer` column, where
    /// the value is a number rather than its text.
    pub fn prop_num(&self, key: &str) -> Option<f64> {
        let v = self.properties.get(key)?;
        v.as_f64().or_else(|| v.as_str()?.parse().ok())
    }

    pub fn prop(&self, key: &str) -> Option<&Value> {
        self.properties.get(key)
    }
}

/// A text column from an optional property.
pub fn cell_text(v: Option<&str>) -> Cell {
    Cell::Text(v.map(str::to_string))
}

fn insert_sql(table: &Table, rows: usize) -> String {
    let mut sql = String::with_capacity(rows * 64);
    sql.push_str("INSERT INTO ");
    sql.push_str(table.name);
    sql.push_str(" (");
    for c in table.columns {
        sql.push_str(c);
        sql.push_str(", ");
    }
    sql.push_str("geom) VALUES ");
    sql
}

/// Downloads one Overture type into `out` and fills `table` from it.
///
/// `row` is called once per feature and returns `None` for one the theme does
/// not keep, which is how a type that carries more than this project uses is
/// narrowed without a second pass over the extract.
pub fn ingest(client: &mut Client, bbox: &str, out: &Path, overture_type: &str, table: &Table, mut row: impl FnMut(&Feature) -> Option<Row>) -> Result<i64, String> {
    let cli = ensure_cli()?;
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let extract = out.join(format!("{overture_type}.geojsonseq"));

    println!("[ingest/{overture_type}] downloading from Overture's public S3 release...");
    let started = std::time::Instant::now();
    run(
        &cli,
        &[
            "download",
            &format!("--bbox={bbox}"),
            "-f",
            "geojsonseq",
            &format!("--type={overture_type}"),
            "-o",
            &extract.to_string_lossy(),
        ],
    )?;

    println!("[ingest/{}] resetting the table...", table.name);
    client.batch_execute(&table.schema).map_err(|e| e.to_string())?;

    let file = std::fs::File::open(&extract).map_err(|e| format!("{}: {e}", extract.display()))?;
    let mut batch: Vec<Row> = Vec::with_capacity(table.batch);
    let mut total: u64 = 0;

    for line in BufReader::new(file).lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.is_empty() {
            continue;
        }
        let f = Feature::parse(&line).map_err(|e| format!("{}: {e}", &line[..line.len().min(120)]))?;
        let Some(r) = row(&f) else { continue };
        batch.push(r);
        if batch.len() >= table.batch {
            total += flush(client, table, &mut batch)?;
            if total.is_multiple_of(table.batch as u64 * 40) {
                println!("[ingest/{}] {} rows in {}", table.name, fmt_int(total), fmt_dur(started.elapsed().as_secs_f64() * 1000.0));
            }
        }
    }
    total += flush(client, table, &mut batch)?;

    println!("[ingest/{}] ANALYZE...", table.name);
    client.batch_execute(&format!("ANALYZE {}", table.name)).map_err(|e| e.to_string())?;

    let count: i64 = client.query_one(&format!("SELECT count(*) FROM {}", table.name), &[]).map_err(|e| e.to_string())?.get(0);
    println!(
        "[ingest/{}] {} rows ({} attempted) in {}",
        table.name,
        fmt_int(count as u64),
        fmt_int(total),
        fmt_dur(started.elapsed().as_secs_f64() * 1000.0)
    );
    Ok(count)
}

fn flush(client: &mut Client, table: &Table, batch: &mut Vec<Row>) -> Result<u64, String> {
    if batch.is_empty() {
        return Ok(0);
    }
    let mut sql = insert_sql(table, batch.len());
    let mut n = 0usize;
    for (i, r) in batch.iter().enumerate() {
        if i > 0 {
            sql.push(',');
        }
        sql.push('(');
        for c in &r.cells {
            n += 1;
            sql.push_str(&format!("${n}{}, ", c.cast()));
        }
        match &r.geom {
            GeomValue::GeoJson(_) => {
                n += 1;
                sql.push_str(&format!("ST_Transform(ST_SetSRID(ST_GeomFromGeoJSON(${n}), 4326), 3857)"));
            }
            GeomValue::Point(..) => {
                sql.push_str(&format!("ST_Transform(ST_SetSRID(ST_MakePoint(${}, ${}), 4326), 3857)", n + 1, n + 2));
                n += 2;
            }
        }
        sql.push(')');
    }
    sql.push_str(" ON CONFLICT (gers_id) DO NOTHING");

    // The geometry parameters have to outlive the slice of references below,
    // so they are materialized alongside the cells rather than formatted in
    // place.
    let mut geoms: Vec<Cell> = Vec::with_capacity(batch.len() * 2);
    for r in batch.iter() {
        match &r.geom {
            GeomValue::GeoJson(s) => geoms.push(Cell::Text(Some(s.clone()))),
            GeomValue::Point(lon, lat) => {
                geoms.push(Cell::F64(Some(*lon)));
                geoms.push(Cell::F64(Some(*lat)));
            }
        }
    }

    let mut params: Vec<&(dyn ToSql + Sync)> = Vec::with_capacity(n);
    let mut g = 0usize;
    for r in batch.iter() {
        for c in &r.cells {
            params.push(c.as_sql());
        }
        match &r.geom {
            GeomValue::GeoJson(_) => {
                params.push(geoms[g].as_sql());
                g += 1;
            }
            GeomValue::Point(..) => {
                params.push(geoms[g].as_sql());
                params.push(geoms[g + 1].as_sql());
                g += 2;
            }
        }
    }

    client.execute(sql.as_str(), &params).map_err(|e| format!("{}: {e}", table.name))?;
    let done = batch.len() as u64;
    batch.clear();
    Ok(done)
}

/// A count with thousands separators, which is how a row count reads at the
/// scale these tables come in — 1,234,567 rather than 1234567.
fn fmt_int(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn fmt_dur(ms: f64) -> String {
    let s = (ms / 1000.0).round() as u64;
    let (h, m) = (s / 3600, (s % 3600) / 60);
    if h > 0 {
        format!("{h}h{m}m")
    } else {
        format!("{m}m{}s", s % 60)
    }
}
