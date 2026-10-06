#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2021"

[dependencies]
arrow = { version = "60", default-features = false }
parquet = { version = "60", default-features = false, features = ["arrow", "snap", "zstd"] }
postgres = "0.19"
pxl-geo-bake = { path = "../../../../crates/pxl/server/geo-bake" }
pxl-spec = { path = "../../../../crates/pxl/spec" }
xpute-core = { path = "../../../../crates/xpute/core" }
serde_json = "1"
---

//! Every Overture type into db-geo, a shard at a time: a table a type, a row
//! a feature, in the shard holding its bbox's lower corner, with its properties as the release holds
//! them and its geometry. A shard is a tile at `SHARD_ZOOM`, the coverage's
//! (`pxl-geo-bake coverage.rs`); what it holds depends on it alone, so shards
//! ingested one by one are the shards ingested together. The official
//! `overturemaps` CLI downloads a type's GeoParquet over every shard, kept
//! under `cache/` until the release changes; the type's shards are then
//! written in one transaction, their catalog rows with them. A column a reader filters on is generated from
//! `properties`.
//!
//!   ./infra/geo/overture/script/ingest.rs

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use arrow::array::{Array, AsArray, RecordBatch};
use arrow::compute::cast;
use arrow::datatypes::{DataType, Float64Type, Int64Type, UInt64Type};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use postgres::types::ToSql;
use postgres::{Client, NoTls, Transaction};
use pxl_geo_bake::coverage::{Coverage, Node, ROOT, WHOLE};
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

/// The official CLI and what it downloads: regenerable, so under `cache/`.
const VENV: &str = "./cache/overture/.venv";
const EXTRACTS: &str = "./cache/overture";

const BATCH: usize = 300;

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
        let whole = ty.overture == WHOLE.overture;
        let to_do: Vec<String> = shards.iter().map(|s| s.name.clone()).chain(whole.then(|| ROOT.to_string())).filter(|n| !done.contains(n)).collect();
        if to_do.is_empty() {
            println!("[ingest/{}] every shard already release {release}", ty.table);
            continue;
        }
        let started = std::time::Instant::now();
        let extract = download(&cli, ty, &shards, whole, &release)?;
        let mut tx = client.transaction().map_err(|e| e.to_string())?;
        let rows = ingest(&mut tx, ty, &shards, whole, &to_do, &extract, &release)?;
        tx.commit().map_err(|e| e.to_string())?;
        client.batch_execute(&format!("ANALYZE {}", ty.table)).map_err(|e| e.to_string())?;
        println!("[ingest/{}] {rows} rows in {} shards in {:.0}s", ty.table, to_do.len(), started.elapsed().as_secs_f64());
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

/// One type's extract over every shard, or the world's for a type taken whole,
/// downloaded unless it is already this release: one call a type.
fn download(cli: &Path, ty: &Type, shards: &[Shard], whole: bool, release: &str) -> Result<PathBuf, String> {
    std::fs::create_dir_all(EXTRACTS).map_err(|e| e.to_string())?;
    let extract = Path::new(EXTRACTS).join(format!("{}.parquet", ty.overture));
    let [w, s, e, n] = shards.iter().fold([f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY], |b, sh| {
        [b[0].min(sh.deg[0]), b[1].min(sh.deg[1]), b[2].max(sh.deg[2]), b[3].max(sh.deg[3])]
    });
    let bbox = if whole { None } else { Some(format!("{},{},{},{}", w - SLACK_DEG, s - SLACK_DEG, e + SLACK_DEG, n + SLACK_DEG)) };
    if !is_current(&extract, ty.overture, bbox.as_deref(), release) {
        let mut args = vec!["download".to_string(), "-f".into(), "geoparquet".into(), format!("--type={}", ty.overture), format!("--release={release}"), "-o".into(), extract.to_string_lossy().into()];
        args.extend(bbox.map(|b| format!("--bbox={b}")));
        run_cmd(cli, &args.iter().map(String::as_str).collect::<Vec<_>>())?;
    }
    Ok(extract)
}

/// The nodes `names` of one type, replacing what they held, their catalog
/// rows with them. A feature with no id or geometry fails the run rather than
/// being dropped.
fn ingest(tx: &mut Transaction, ty: &Type, shards: &[Shard], whole: bool, names: &[String], extract: &Path, release: &str) -> Result<i64, String> {
    tx.execute("DELETE FROM overture_catalog WHERE file_name = $1 AND node = ANY($2)", &[&ty.table, &names]).map_err(|e| e.to_string())?;
    tx.execute(&format!("DELETE FROM {} WHERE node = ANY($1)", ty.table), &[&names]).map_err(|e| e.to_string())?;
    let file = std::fs::File::open(extract).map_err(|e| format!("{}: {e}", extract.display()))?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file).map_err(|e| e.to_string())?.build().map_err(|e| e.to_string())?;
    let mut counts: HashMap<String, i64> = HashMap::new();
    let mut batch: Vec<Row> = Vec::with_capacity(BATCH);
    for group in reader {
        for row in rows(&group.map_err(|e| e.to_string())?, shards, whole)? {
            if !names.contains(&row.3) {
                continue;
            }
            *counts.entry(row.3.clone()).or_default() += 1;
            batch.push(row);
            if batch.len() == BATCH {
                flush(tx, ty.table, &mut batch)?;
            }
        }
    }
    flush(tx, ty.table, &mut batch)?;
    for name in names {
        let count = counts.get(name).copied().unwrap_or(0);
        tx.execute("INSERT INTO overture_catalog (file_name, node, data_date, total_count) VALUES ($1, $2, $3, $4)", &[&ty.table, name, &release, &count])
            .map_err(|e| e.to_string())?;
    }
    Ok(counts.values().sum())
}

/// A shard as a feature is tested against: its edges in EPSG:3857 meters, each
/// rounded half to even, `[left, bottom, right, top]`, the right and top
/// edges the next shard's; and the same in degrees, for the CLI.
struct Shard {
    name: String,
    m: [f64; 4],
    deg: [f64; 4],
}

impl Shard {
    fn of(node: Node) -> Shard {
        let m = node.rect().map(|u| round(epsg3857::mer_from_u(u)));
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

/// Each feature with the shard of `shards` holding it; one none holds is past them. The properties are every column but the
/// id, the geometry and the bbox, a null one left out; the geometry is the
/// release's WKB, its doubles as written.
fn rows(group: &RecordBatch, shards: &[Shard], whole: bool) -> Result<Vec<Row>, String> {
    let subtype = match (whole, group.column_by_name("subtype")) {
        (true, Some(c)) => Some(cast(c, &DataType::Utf8).map_err(|e| e.to_string())?),
        (true, None) => return Err(format!("{}: no subtype column", WHOLE.overture)),
        (false, _) => None,
    };
    let subtype = subtype.as_ref().map(|c| c.as_string::<i32>());
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
        let nodes: Vec<&str> = if subtype.is_some_and(|s| !s.is_null(i) && s.value(i) == WHOLE.subtype) {
            vec![ROOT]
        } else {
            let span = span_m(geometry.value(i)).map_err(|e| format!("{}: {e}", id.value(i)))?;
            shards.iter().filter(|s| s.meets(span)).map(|s| s.name.as_str()).collect()
        };
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

/// A WKB parameter in EPSG:3857. Mercator ends short of the poles, where the
/// world's edge clamps latitude (`epsg4326`), so what lies past it is cut off
/// rather than refused by the projection.
fn projected(param: usize) -> String {
    let lat = epsg4326::Y_TO_LAT(epsg3857::mer_from_u(REACH.max[1]));
    format!("ST_Transform(ST_ClipByBox2D(ST_GeomFromWKB(${param}, 4326), ST_MakeEnvelope(-180, -{lat:?}, 180, {lat:?}, 4326)), 3857)")
}

fn flush(tx: &mut Transaction, table: &str, batch: &mut Vec<Row>) -> Result<u64, String> {
    if batch.is_empty() {
        return Ok(0);
    }
    let mut sql = format!("INSERT INTO {table} (gers_id, properties, geom, node) VALUES ");
    for i in 0..batch.len() {
        let n = i * 4;
        if i > 0 {
            sql.push(',');
        }
        // `::text::jsonb`, not `::jsonb`, so the parameter is bound as text.
        sql.push_str(&format!("(${}, ${}::text::jsonb, {}, ${})", n + 1, n + 2, projected(n + 3), n + 4));
    }
    let params: Vec<&(dyn ToSql + Sync)> = batch.iter().flat_map(|(id, properties, wkb, node)| [id as &(dyn ToSql + Sync), properties, wkb, node]).collect();
    let done = tx.execute(sql.as_str(), &params).map_err(|e| match e.as_db_error() {
        Some(d) => format!("{table}: {} {}{}", d.code().code(), d.message(), d.detail().map(|s| format!(" ({s})")).unwrap_or_default()),
        None => format!("{table}: {e}"),
    })?;
    batch.clear();
    Ok(done)
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
    // interpreter that is gone, and exists without being able to run.
    if cli.exists() && Command::new(&cli).arg("--version").output().is_ok_and(|o| o.status.success()) {
        return Ok(cli);
    }
    if cli.exists() {
        std::fs::remove_dir_all(VENV).map_err(|e| e.to_string())?;
    }
    println!("[ingest] creating {VENV} and installing overturemaps...");
    std::fs::create_dir_all(Path::new(VENV).parent().unwrap_or(Path::new("."))).map_err(|e| e.to_string())?;
    run_cmd(Path::new("python3"), &["-m", "venv", VENV])?;
    run_cmd(&Path::new(VENV).join("bin/pip"), &["install", "--quiet", "overturemaps"])?;
    Ok(cli)
}
/// `overturemaps releases latest`, the snapshot's own date.
fn release(cli: &Path) -> Result<String, String> {
    let out = Command::new(cli).args(["releases", "latest"]).output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("overturemaps releases latest exited {}", out.status.code().unwrap_or(-1)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
/// By the state the CLI writes when a download finishes; a stopped download
/// leaves the old state beside a truncated file, so it counts only if newer.
fn is_current(extract: &Path, overture_type: &str, bbox: Option<&str>, release: &str) -> bool {
    let state_path = PathBuf::from(format!("{}.state", extract.display()));
    let (Ok(extract_meta), Ok(state_meta)) = (std::fs::metadata(extract), std::fs::metadata(&state_path)) else {
        return false;
    };
    if !matches!((extract_meta.modified(), state_meta.modified()), (Ok(e), Ok(s)) if s >= e) {
        return false;
    }
    let Some(state) = std::fs::read_to_string(&state_path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else {
        return false;
    };
    let want: Vec<f64> = bbox.map_or(Vec::new(), |b| b.split(',').filter_map(|v| v.trim().parse().ok()).collect());
    let have: Vec<f64> = ["xmin", "ymin", "xmax", "ymax"].iter().filter_map(|k| state["bbox"][k].as_f64()).collect();
    state["last_release"] == release && state["type"] == overture_type && want == have
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
