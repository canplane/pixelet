#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2021"

[dependencies]
arrow = { version = "60", default-features = false }
parquet = { version = "60", default-features = false, features = ["arrow", "snap", "zstd"] }
postgres = "0.19"
pxl-geo-bake = { path = "../../../../crates/server/geo-bake" }
pxl-spec = { path = "../../../../crates/spec" }
xpute-core = { path = "../../../../xpute/rust/core" }
serde_json = "1"
---

//! Every Overture type into db-geo, a shard at a time: a table a type, a row
//! a feature, in each shard whose apron it meets, with its properties as the release holds
//! them and its geometry. A shard is a tile at `SHARD_ZOOM`, the coverage's
//! (`pxl-geo-bake coverage.rs`); what it holds depends on it alone, so shards
//! ingested one by one are the shards ingested together. The official
//! `overturemaps` CLI, by its public commands alone, downloads a type's
//! GeoParquet over the shards to do, kept in `raw/` under a name that says its
//! release and its bbox, so a shard added fetches only itself. The extract is
//! read once into a staging table, each row with its shard, and each shard is
//! written from it in a transaction of its own, its catalog row last, so it
//! is whole or absent. A type whose every shard is to do is emptied first. A
//! column a reader filters on is generated from `properties`.
//!
//!   ./infra/geo/overture/script/ingest.rs

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output, Stdio};
use std::sync::Mutex;

use arrow::array::{Array, AsArray, RecordBatch};
use arrow::compute::cast;
use arrow::datatypes::{DataType, Float64Type, Int64Type, UInt64Type};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use postgres::binary_copy::BinaryCopyInWriter;
use postgres::types::Type as Pg;
use postgres::{Client, NoTls};
use pxl_geo_bake::coverage::{Coverage, Node};
use pxl_geo_bake::util::fnv1a;
use pxl_spec::space::coord::adapter::{epsg3857, epsg4326};
use pxl_spec::space::coord::world::REACH;
use serde_json::{Map, Number, Value};
use xpute_core::math::scalar::round;

/// An Overture type, its table, its generated columns, its indexes past the geometry's.
struct Type {
    overture: &'static str,
    table: &'static str,
    columns: &'static [(&'static str, &'static str)],
    indexes: &'static [&'static str],
}

const SUBTYPE: (&str, &str) = ("subtype", "TEXT GENERATED ALWAYS AS (properties->>'subtype') STORED");
const CLASS: (&str, &str) = ("class", "TEXT GENERATED ALWAYS AS (properties->>'class') STORED");
const NAME: (&str, &str) = ("name", "TEXT GENERATED ALWAYS AS (properties->'names'->>'primary') STORED");

const TYPES: &[Type] = &[
    Type { overture: "address", table: "overture_addresses", columns: &[], indexes: &[] },
    Type { overture: "bathymetry", table: "overture_base_bathymetry", columns: &[SUBTYPE, CLASS], indexes: &[] },
    Type {
        overture: "building",
        table: "overture_buildings",
        columns: &[
            SUBTYPE,
            CLASS,
            ("height", "DOUBLE PRECISION GENERATED ALWAYS AS ((properties->>'height')::float8) STORED"),
            ("num_floors", "INTEGER GENERATED ALWAYS AS (trunc((properties->>'num_floors')::float8)::int) STORED"),
        ],
        indexes: &["class"],
    },
    Type { overture: "building_part", table: "overture_building_parts", columns: &[], indexes: &[] },
    Type { overture: "division", table: "overture_division_points", columns: &[SUBTYPE, NAME], indexes: &["subtype"] },
    Type { overture: "division_area", table: "overture_divisions", columns: &[SUBTYPE, NAME], indexes: &["subtype"] },
    Type { overture: "division_boundary", table: "overture_division_boundaries", columns: &[], indexes: &[] },
    Type {
        overture: "place",
        table: "overture_places",
        columns: &[("name", "TEXT NOT NULL GENERATED ALWAYS AS (COALESCE(properties->'names'->>'primary', '')) STORED")],
        indexes: &["USING GIN (properties)", "USING GIN (name gin_trgm_ops)"],
    },
    Type {
        overture: "segment",
        table: "overture_transportation",
        columns: &[
            SUBTYPE,
            CLASS,
            ("road_flags", "JSONB GENERATED ALWAYS AS (NULLIF(properties->'road_flags', 'null')) STORED"),
            ("rail_flags", "JSONB GENERATED ALWAYS AS (NULLIF(properties->'rail_flags', 'null')) STORED"),
            ("level_rules", "JSONB GENERATED ALWAYS AS (NULLIF(properties->'level_rules', 'null')) STORED"),
            ("width_rules", "JSONB GENERATED ALWAYS AS (NULLIF(properties->'width_rules', 'null')) STORED"),
            ("access_restrictions", "JSONB GENERATED ALWAYS AS (NULLIF(properties->'access_restrictions', 'null')) STORED"),
            ("subclass", "TEXT GENERATED ALWAYS AS (properties->>'subclass') STORED"),
            ("subclass_rules", "JSONB GENERATED ALWAYS AS (NULLIF(properties->'subclass_rules', 'null')) STORED"),
        ],
        indexes: &["(subtype, class)"],
    },
    Type { overture: "connector", table: "overture_connectors", columns: &[], indexes: &[] },
    Type { overture: "infrastructure", table: "overture_base_infrastructure", columns: &[SUBTYPE, CLASS], indexes: &["class"] },
    Type { overture: "land", table: "overture_base_land", columns: &[SUBTYPE, CLASS], indexes: &["class"] },
    Type { overture: "land_cover", table: "overture_base_land_cover", columns: &[SUBTYPE], indexes: &[] },
    Type { overture: "land_use", table: "overture_base_land_use", columns: &[SUBTYPE, CLASS], indexes: &["class"] },
    Type { overture: "water", table: "overture_base_water", columns: &[SUBTYPE, CLASS], indexes: &["class"] },
];

/// The official CLI, a tool regenerable under `cache/`, and what it downloads,
/// the source, kept beside this script.
const VENV: &str = "./infra/geo/overture/.venv";

/// The CLI's release this script was written against: a new one is taken by
/// changing this, not by whatever `pip` finds that day, so a change in the
/// CLI's behaviour arrives with a commit rather than unnoticed.
const CLI_VERSION: &str = "1.0.2";
const EXTRACTS: &str = "./infra/geo/overture/raw";

/// Shards written at once, a connection each. A type is fetched by one CLI
/// call and staged by one connection before any shard is written.
const WORKERS: usize = 6;

/// The CLI's bbox test is strict, so the shards are asked for this much wider.
const SLACK_DEG: f64 = 1e-4;

/// A shard's edges are taken this far out, in meters, so a feature on one is in both.
const EPS_M: f64 = 1.0;

/// id, properties, geometry, shard.
type Row = (String, String, Vec<u8>, String);

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[ingest] {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    if std::env::args().len() > 1 {
        return Err("no flags: the shards are the coverage's (infra/geo/elevation/metadata.json)".to_string());
    }
    let shards: Vec<Shard> = Coverage::of_source()?.shards().map(Shard::of).collect();
    let mut client = Client::connect(&db_geo_url()?, NoTls).map_err(|e| e.to_string())?;
    let cli = ensure_cli()?;
    let release = release(&cli)?;
    println!("[ingest] release {release}, {} shards", shards.len());
    client.batch_execute(CATALOG_SQL).map_err(|e| e.to_string())?;
    keyed_by_shard(&mut client, "overture_catalog", "file_name")?;

    for ty in TYPES {
        client.batch_execute(&schema(ty)).map_err(|e| e.to_string())?;
        keyed_by_shard(&mut client, ty.table, "gers_id")?;
        let done: Vec<String> = client
            .query("SELECT node FROM overture_catalog WHERE file_name = $1 AND data_date = $2", &[&ty.table, &release])
            .map_err(|e| e.to_string())?
            .iter()
            .map(|r| r.get(0))
            .collect();
        let to_do: Vec<String> = shards.iter().map(|s| s.name.clone()).filter(|n| !done.contains(n)).collect();
        if to_do.is_empty() {
            println!("[ingest/{}] every shard already release {release}", ty.table);
            continue;
        }
        let started = std::time::Instant::now();
        let to_do: Vec<&Shard> = shards.iter().filter(|s| to_do.contains(&s.name)).collect();
        let extract = download(&cli, ty, &to_do, &release)?;
        let fetched = started.elapsed().as_secs_f64();
        stage(&mut client, ty, &extract, &to_do)?;
        // Every shard to do: the table emptied whole, which a delete a shard would leave bloated.
        let whole = to_do.len() == shards.len();
        if whole {
            println!("[ingest/{}] emptying it, once its readers (a running tile server) let go", ty.table);
            let mut tx = client.transaction().map_err(|e| e.to_string())?;
            tx.batch_execute(&format!("TRUNCATE {}", ty.table)).map_err(|e| e.to_string())?;
            tx.execute("DELETE FROM overture_catalog WHERE file_name = $1", &[&ty.table]).map_err(|e| e.to_string())?;
            tx.commit().map_err(|e| e.to_string())?;
        }
        let rows = in_pool(&to_do, |client, shard| ingest(client, ty, shard, &release, !whole))?.into_iter().sum::<i64>();
        client.batch_execute(&format!("DROP TABLE {}", staging(ty))).map_err(|e| e.to_string())?;
        // Shards replaced in place leave their old rows dead in the indexes.
        let settle = if whole { "ANALYZE" } else { "VACUUM (ANALYZE)" };
        client.batch_execute(&format!("{settle} {}", ty.table)).map_err(|e| e.to_string())?;
        println!("[ingest/{}] {rows} rows in {} shards in {:.0}s, {fetched:.0}s of it fetching", ty.table, to_do.len(), started.elapsed().as_secs_f64());
    }
    Ok(())
}

/// A table keyed otherwise than by (node, `id`) was made before shards and cannot be filled a shard at a time.
fn keyed_by_shard(client: &mut Client, table: &str, id: &str) -> Result<(), String> {
    let key: Vec<String> = client
        .query(
            "SELECT a.attname FROM pg_index i JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey)
             WHERE i.indrelid = to_regclass($1::text) AND i.indisprimary ORDER BY array_position(i.indkey, a.attnum)",
            &[&table],
        )
        .map_err(|e| e.to_string())?
        .iter()
        .map(|r| r.get(0))
        .collect();
    if key != ["node", id] {
        return Err(format!("{table} is keyed by {key:?}, made before shards: drop it, and the catalog, and ingest again"));
    }
    Ok(())
}

/// One type's extract over the shards to do, one CLI call: every call reads
/// Overture's STAC catalog, and a call a shard was refused for load (HTTP
/// 429). Its name says the release and the bbox it was asked for, and the
/// CLI's output takes that name only once the CLI has finished, so an extract
/// under it is current and whole; the CLI's own state file is its private
/// format, not read. The type's other files, another release's or bbox's
/// and that state beside them, are removed.
fn download(cli: &Path, ty: &Type, shards: &[&Shard], release: &str) -> Result<PathBuf, String> {
    // What the shards hold, their aprons with them (`Shard::deg`), not their bare tiles.
    let [w, s, e, n] = shards.iter().fold([f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY], |b, sh| {
        [b[0].min(sh.deg[0]), b[1].min(sh.deg[1]), b[2].max(sh.deg[2]), b[3].max(sh.deg[3])]
    });
    let bbox = format!("{},{},{},{}", w - SLACK_DEG, s - SLACK_DEG, e + SLACK_DEG, n + SLACK_DEG);
    std::fs::create_dir_all(EXTRACTS).map_err(|e| e.to_string())?;
    let name = format!("{}.{release}.{:016x}.parquet", ty.overture, fnv1a(bbox.as_bytes()));
    let extract = Path::new(EXTRACTS).join(&name);
    if !extract.exists() {
        let part = Path::new(EXTRACTS).join(format!("{name}.part"));
        let args = ["download", &format!("--bbox={bbox}"), "-f", "geoparquet", &format!("--type={}", ty.overture), &format!("--release={release}"), "-o", &part.to_string_lossy()];
        overture(cli, &args)?;
        std::fs::rename(&part, &extract).map_err(|e| e.to_string())?;
    }
    for entry in std::fs::read_dir(EXTRACTS).map_err(|e| e.to_string())?.flatten() {
        let other = entry.file_name().to_string_lossy().to_string();
        if other != name && (other == ty.overture || other.starts_with(&format!("{}.", ty.overture))) {
            let path = entry.path();
            std::fs::remove_dir_all(&path).or_else(|_| std::fs::remove_file(&path)).map_err(|e| format!("{}: {e}", path.display()))?;
        }
    }
    Ok(extract)
}

/// The staging table a type's shards are written from.
fn staging(ty: &Type) -> String {
    format!("{}_staging", ty.table)
}

/// The type's extract read once, each feature a row for each shard of
/// `shards` it meets, into the staging table; a feature with no id or
/// geometry fails the run rather than being dropped.
fn stage(client: &mut Client, ty: &Type, extract: &Path, shards: &[&Shard]) -> Result<(), String> {
    let t = staging(ty);
    client
        .batch_execute(&format!("DROP TABLE IF EXISTS {t}; CREATE UNLOGGED TABLE {t} (node TEXT NOT NULL, gers_id TEXT NOT NULL, properties TEXT NOT NULL, wkb BYTEA NOT NULL)"))
        .map_err(|e| e.to_string())?;
    let file = std::fs::File::open(extract).map_err(|e| format!("{}: {e}", extract.display()))?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file).map_err(|e| e.to_string())?.build().map_err(|e| e.to_string())?;
    let mut copy = BinaryCopyInWriter::new(client.copy_in(&format!("COPY {t} (gers_id, properties, wkb, node) FROM STDIN (FORMAT binary)")).map_err(|e| e.to_string())?, &[Pg::TEXT, Pg::TEXT, Pg::BYTEA, Pg::TEXT]);
    for group in reader {
        for (id, properties, wkb, node) in rows(&group.map_err(|e| e.to_string())?, shards)? {
            copy.write(&[&id, &properties, &wkb, &node]).map_err(|e| e.to_string())?;
        }
    }
    copy.finish().map_err(|e| e.to_string())?;
    client.batch_execute(&format!("CREATE INDEX ON {t} (node)")).map_err(|e| e.to_string())?;
    Ok(())
}

/// `each` over `items`, `WORKERS` at once, each worker with its own
/// connection; the first error is the run's.
fn in_pool<T: Sync, R: Send>(items: &[T], each: impl Fn(&mut Client, &T) -> Result<R, String> + Sync) -> Result<Vec<R>, String> {
    let next = Mutex::new(0usize);
    let out: Mutex<Vec<R>> = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..WORKERS.min(items.len()))
            .map(|_| {
                scope.spawn(|| -> Result<(), String> {
                    let mut client = Client::connect(&db_geo_url()?, NoTls).map_err(|e| e.to_string())?;
                    loop {
                        let at = {
                            let mut n = next.lock().expect("the queue");
                            if *n == items.len() {
                                return Ok(());
                            }
                            *n += 1;
                            *n - 1
                        };
                        let r = each(&mut client, &items[at])?;
                        out.lock().expect("the results").push(r);
                    }
                })
            })
            .collect();
        workers.into_iter().try_for_each(|w| w.join().expect("a worker"))
    })?;
    Ok(out.into_inner().expect("the results"))
}

/// One shard of one type from the staging table, replacing what it held
/// unless the table was emptied, through the same SQL as ever, its catalog
/// row last, in one transaction.
fn ingest(client: &mut Client, ty: &Type, shard: &Shard, release: &str, replace: bool) -> Result<i64, String> {
    let db = |e: postgres::Error| match e.as_db_error() {
        Some(d) => format!("{} {}: {} {}{}", ty.table, shard.name, d.code().code(), d.message(), d.detail().map(|s| format!(" ({s})")).unwrap_or_default()),
        None => format!("{} {}: {e}", ty.table, shard.name),
    };
    let mut tx = client.transaction().map_err(db)?;
    if replace {
        tx.execute("DELETE FROM overture_catalog WHERE file_name = $1 AND node = $2", &[&ty.table, &shard.name]).map_err(db)?;
        tx.execute(&format!("DELETE FROM {} WHERE node = $1", ty.table), &[&shard.name]).map_err(db)?;
    }
    let count = tx
        .execute(
            &format!("INSERT INTO {} (gers_id, properties, geom, node) SELECT gers_id, properties::jsonb, {}, node FROM {} WHERE node = $1", ty.table, projected("wkb"), staging(ty)),
            &[&shard.name],
        )
        .map_err(db)? as i64;
    tx.execute("INSERT INTO overture_catalog (file_name, node, data_date, total_count) VALUES ($1, $2, $3, $4)", &[&ty.table, &shard.name, &release, &count]).map_err(db)?;
    tx.commit().map_err(db)?;
    Ok(count)
}

/// A shard as a feature is tested against: what it holds (`Node::held`, its
/// apron included) in EPSG:3857 meters, each rounded half to even,
/// `[left, bottom, right, top]`; and the same in degrees, for the CLI.
struct Shard {
    name: String,
    m: [f64; 4],
    deg: [f64; 4],
}

impl Shard {
    fn of(node: Node) -> Shard {
        let m = node.held().map(|u| round(epsg3857::mer_from_u(u)));
        let [w, s, _] = epsg4326::from_mercator(&[m[0], m[1], 0.0]);
        let [e, n, _] = epsg4326::from_mercator(&[m[2], m[3], 0.0]);
        // The east edge at the seam reads back as -180.
        Shard { name: node.name(), m, deg: [w, s, if e <= w { 180.0 } else { e }, n] }
    }

    /// Whether a feature whose vertices span `[lo_x, lo_y, hi_x, hi_y]` in
    /// meters lies in the shard grown by `EPS_M`.
    fn meets(&self, [lo_x, lo_y, hi_x, hi_y]: [f64; 4]) -> bool {
        let [left, bottom, right, top] = self.m;
        lo_x <= right + EPS_M && hi_x >= left - EPS_M && lo_y <= top + EPS_M && hi_y >= bottom - EPS_M
    }
}

/// Longitude to x is linear and latitude to y increasing, so the extremes' projections are the projected extremes.
fn span_m(wkb: &[u8]) -> Result<[f64; 4], String> {
    let mut deg = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
    vertices(wkb, &mut 0, &mut |x, y| deg = [deg[0].min(x), deg[1].min(y), deg[2].max(x), deg[3].max(y)])?;
    if !deg[0].is_finite() {
        return Err("a geometry with no vertex".to_string());
    }
    let [lo_x, lo_y, _] = epsg4326::to_mercator(&[deg[0], deg[1], 0.0]);
    let [hi_x, hi_y, _] = epsg4326::to_mercator(&[deg[2], deg[3], 0.0]);
    Ok([lo_x, lo_y, hi_x, hi_y].map(round))
}

/// Every vertex of a WKB geometry, ISO's 2D, Z, M and ZM.
fn vertices(b: &[u8], at: &mut usize, each: &mut dyn FnMut(f64, f64)) -> Result<(), String> {
    let end = || "WKB ends early".to_string();
    let le = *b.get(*at).ok_or_else(end)? == 1;
    *at += 1;
    let word = |at: &mut usize| -> Result<u32, String> {
        let s: [u8; 4] = b.get(*at..*at + 4).ok_or_else(end)?.try_into().map_err(|_| end())?;
        *at += 4;
        Ok(if le { u32::from_le_bytes(s) } else { u32::from_be_bytes(s) })
    };
    let real = |at: &mut usize| -> Result<f64, String> {
        let s: [u8; 8] = b.get(*at..*at + 8).ok_or_else(end)?.try_into().map_err(|_| end())?;
        *at += 8;
        Ok(if le { f64::from_le_bytes(s) } else { f64::from_be_bytes(s) })
    };
    let code = word(at)?;
    let dims = match code / 1000 {
        0 => 2,
        1 | 2 => 3,
        3 => 4,
        _ => return Err(format!("WKB type {code} is not ISO WKB")),
    };
    let mut points = |at: &mut usize, n: u32| -> Result<(), String> {
        for _ in 0..n {
            let (x, y) = (real(at)?, real(at)?);
            *at += 8 * (dims - 2);
            each(x, y);
        }
        Ok(())
    };
    match code % 1000 {
        1 => points(at, 1),
        2 => {
            let n = word(at)?;
            points(at, n)
        }
        3 => {
            for _ in 0..word(at)? {
                let n = word(at)?;
                points(at, n)?;
            }
            Ok(())
        }
        4..=7 => {
            for _ in 0..word(at)? {
                vertices(b, at, each)?;
            }
            Ok(())
        }
        kind => Err(format!("WKB type {kind} is not a geometry this reads")),
    }
}

/// A row for each feature and each shard of `shards` it meets. The properties
/// are every column but the id, the geometry and the bbox, a null one left
/// out; the geometry is the release's WKB, its doubles as written.
fn rows(group: &RecordBatch, shards: &[&Shard]) -> Result<Vec<Row>, String> {
    let id = cast(group.column_by_name("id").ok_or("no id column")?, &DataType::Utf8).map_err(|e| e.to_string())?;
    let id = id.as_string::<i32>();
    let geometry = cast(group.column_by_name("geometry").ok_or("no geometry column")?, &DataType::LargeBinary).map_err(|e| e.to_string())?;
    let geometry = geometry.as_binary::<i64>();
    let schema = group.schema();
    let properties: Vec<_> = schema.fields().iter().zip(group.columns()).filter(|(f, _)| !matches!(f.name().as_str(), "id" | "geometry" | "bbox")).collect();
    let mut out = Vec::new();
    for i in 0..group.num_rows() {
        if id.is_null(i) || geometry.is_null(i) {
            return Err(format!("{}: a feature with no id or geometry", if id.is_null(i) { "?" } else { id.value(i) }));
        }
        let span = span_m(geometry.value(i)).map_err(|e| format!("{}: {e}", id.value(i)))?;
        let nodes: Vec<&str> = shards.iter().filter(|s| s.meets(span)).map(|s| s.name.as_str()).collect();
        if nodes.is_empty() {
            continue;
        }
        let mut object = Map::new();
        for (field, column) in &properties {
            if !column.is_null(i) {
                object.insert(field.name().clone(), json(column.as_ref(), i)?);
            }
        }
        let properties = Value::Object(object).to_string();
        for node in nodes {
            out.push((id.value(i).to_string(), properties.clone(), geometry.value(i).to_vec(), node.to_string()));
        }
    }
    Ok(out)
}

/// A value as JSON, a map as an object; a type with no JSON reading fails the run.
fn json(column: &dyn Array, i: usize) -> Result<Value, String> {
    if column.is_null(i) {
        return Ok(Value::Null);
    }
    let one = |to: &DataType| cast(&column.slice(i, 1), to).map_err(|e| e.to_string());
    Ok(match column.data_type() {
        DataType::Boolean => Value::Bool(column.as_boolean().value(i)),
        DataType::Int8 | DataType::Int16 | DataType::Int32 | DataType::Int64 => Value::from(one(&DataType::Int64)?.as_primitive::<Int64Type>().value(0)),
        DataType::UInt8 | DataType::UInt16 | DataType::UInt32 | DataType::UInt64 => Value::from(one(&DataType::UInt64)?.as_primitive::<UInt64Type>().value(0)),
        // A float32 widens to a double, its value unchanged.
        DataType::Float16 | DataType::Float32 | DataType::Float64 => {
            Number::from_f64(one(&DataType::Float64)?.as_primitive::<Float64Type>().value(0)).map_or(Value::Null, Value::Number)
        }
        DataType::Utf8 => Value::from(column.as_string::<i32>().value(i)),
        DataType::LargeUtf8 => Value::from(column.as_string::<i64>().value(i)),
        DataType::Utf8View => Value::from(column.as_string_view().value(i)),
        DataType::List(_) => items(column.as_list::<i32>().value(i).as_ref())?,
        DataType::LargeList(_) => items(column.as_list::<i64>().value(i).as_ref())?,
        DataType::Struct(fields) => {
            let mut object = Map::new();
            for (field, child) in fields.iter().zip(column.as_struct().columns()) {
                object.insert(field.name().clone(), json(child.as_ref(), i)?);
            }
            Value::Object(object)
        }
        DataType::Map(_, _) => {
            let entries = column.as_map().value(i);
            let mut object = Map::new();
            for k in 0..entries.len() {
                let Value::String(key) = json(entries.column(0).as_ref(), k)? else { return Err("a map key that is not a string".to_string()) };
                object.insert(key, json(entries.column(1).as_ref(), k)?);
            }
            Value::Object(object)
        }
        other => return Err(format!("no JSON reading for {other}")),
    })
}

fn items(list: &dyn Array) -> Result<Value, String> {
    (0..list.len()).map(|k| json(list, k)).collect::<Result<Vec<_>, _>>().map(Value::Array)
}

/// A WKB value in EPSG:3857. Mercator ends short of the poles, where the
/// world's edge clamps latitude (`epsg4326`), so what lies past it is cut off
/// rather than refused by the projection.
fn projected(wkb: &str) -> String {
    let lat = epsg4326::Y_TO_LAT(epsg3857::mer_from_u(REACH.max[1]));
    format!("ST_Transform(ST_ClipByBox2D(ST_GeomFromWKB({wkb}, 4326), ST_MakeEnvelope(-180, -{lat:?}, 180, {lat:?}, 4326)), 3857)")
}

const CATALOG_SQL: &str = "
CREATE TABLE IF NOT EXISTS overture_catalog (
  file_name TEXT NOT NULL,
  node TEXT NOT NULL,
  data_date TEXT NOT NULL,
  imported_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  total_count BIGINT NOT NULL,
  PRIMARY KEY (node, file_name)
);
";

fn schema(ty: &Type) -> String {
    let t = ty.table;
    let mut sql = format!(
        "CREATE EXTENSION IF NOT EXISTS postgis;\nCREATE EXTENSION IF NOT EXISTS pg_trgm;\nCREATE TABLE IF NOT EXISTS {t} (\n  node TEXT NOT NULL,\n  gers_id TEXT NOT NULL,\n  properties JSONB NOT NULL,\n"
    );
    for (name, def) in ty.columns {
        sql.push_str(&format!("  {name} {def},\n"));
    }
    sql.push_str(&format!(
        "  geom GEOMETRY(Geometry, 3857) NOT NULL,\n  PRIMARY KEY (node, gers_id)\n);\nCREATE INDEX IF NOT EXISTS {t}_geom ON {t} USING GIST (geom);\nCREATE INDEX IF NOT EXISTS {t}_gers_id ON {t} (gers_id);\n"
    ));
    for (k, index) in ty.indexes.iter().enumerate() {
        let on = if index.starts_with("USING") || index.starts_with('(') { index.to_string() } else { format!("({index})") };
        sql.push_str(&format!("CREATE INDEX IF NOT EXISTS {t}_{k} ON {t} {on};\n"));
    }
    sql
}

/// The official CLI by a public command: what it prints to stderr is shown
/// as it runs, and a failure is the run's.
fn overture(cli: &Path, args: &[&str]) -> Result<Output, String> {
    let out = Command::new(cli).args(args).stderr(Stdio::inherit()).output().map_err(|e| format!("{}: {e}", cli.display()))?;
    if !out.status.success() {
        return Err(format!("{} {} exited {}", cli.display(), args.join(" "), out.status.code().unwrap_or(-1)));
    }
    Ok(out)
}

fn run_cmd(cmd: &Path, args: &[&str]) -> Result<(), String> {
    let status = Command::new(cmd).args(args).status().map_err(|e| format!("{}: {e}", cmd.display()))?;
    if !status.success() {
        return Err(format!("{} {} exited {}", cmd.display(), args.join(" "), status.code().unwrap_or(-1)));
    }
    Ok(())
}
fn ensure_cli() -> Result<PathBuf, String> {
    let cli = Path::new(VENV).join("bin/overturemaps");
    // Run it rather than stat it: a venv that moved keeps a shebang to an
    // interpreter that is gone, and exists without being able to run. Its
    // `--version` is public: "overturemaps, version 1.0.2".
    let version = Command::new(&cli).arg("--version").output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    if version.as_deref().is_some_and(|v| v.ends_with(&format!("version {CLI_VERSION}"))) {
        return Ok(cli);
    }
    if Path::new(VENV).exists() {
        std::fs::remove_dir_all(VENV).map_err(|e| e.to_string())?;
    }
    println!("[ingest] creating {VENV} and installing overturemaps {CLI_VERSION}...");
    std::fs::create_dir_all(Path::new(VENV).parent().unwrap_or(Path::new("."))).map_err(|e| e.to_string())?;
    run_cmd(Path::new("python3"), &["-m", "venv", VENV])?;
    run_cmd(&Path::new(VENV).join("bin/pip"), &["install", "--quiet", &format!("overturemaps=={CLI_VERSION}")])?;
    Ok(cli)
}
/// `overturemaps releases latest`, the snapshot's own date.
fn release(cli: &Path) -> Result<String, String> {
    let out = overture(cli, &["releases", "latest"])?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
/// `DB_GEO_URL` from the environment, or `.env` with its `${...}` expanded.
fn db_geo_url() -> Result<String, String> {
    if let Ok(url) = std::env::var("DB_GEO_URL") {
        return Ok(url);
    }
    let dotenv = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../.env");
    let text = std::fs::read_to_string(&dotenv).map_err(|_| format!("env:DB_GEO_URL:missing, and no {}", dotenv.display()))?;
    let mut vars: HashMap<&str, &str> = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        let line = line.strip_prefix("export ").unwrap_or(line);
        if line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let value = value.trim();
        vars.insert(key.trim(), value.strip_prefix('"').and_then(|v| v.strip_suffix('"')).unwrap_or(value));
    }
    let raw = vars.get("DB_GEO_URL").ok_or_else(|| format!("env:DB_GEO_URL:missing, and {} does not set it", dotenv.display()))?;
    let mut out = String::with_capacity(raw.len());
    let mut rest = *raw;
    while let Some(at) = rest.find("${") {
        out.push_str(&rest[..at]);
        let Some(end) = rest[at..].find('}') else { break };
        out.push_str(vars.get(&rest[at + 2..at + end]).copied().unwrap_or(""));
        rest = &rest[at + end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}
