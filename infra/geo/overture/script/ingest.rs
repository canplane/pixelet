#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2021"

[dependencies]
arrow = { version = "60", default-features = false }
bytes = "1"
futures = "0.3"
object_store = { version = "0.14", features = ["aws", "http"] }
parquet = { version = "60", default-features = false, features = ["arrow", "async", "object_store", "zstd", "snap"] }
pxl-geo-bake = { path = "../../../../crates/pxl/server/geo-bake" }
pxl-spec = { path = "../../../../crates/pxl/spec" }
xpute-core = { path = "../../../../crates/xpute/core" }
tokio-postgres = "0.7"
serde_json = "1"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
---

//! Every Overture type into db-geo, a shard at a time: a table a type, a row
//! a feature and a shard it meets, with its properties as the release holds
//! them and its geometry. A shard is a tile at `SHARD_Z`, the coverage's
//! (`pxl-geo-bake coverage.rs`); what it holds depends on it alone, so shards
//! ingested one by one are the shards ingested together. A column a reader
//! filters on is generated from `properties`. `overture_catalog` keeps each
//! shard's release, written once the shard is whole. The release's GeoParquet
//! is read where it is published, the row groups whose bbox statistics can
//! meet a shard only.
//!
//!   ./infra/geo/overture/script/ingest.rs

use std::collections::{BTreeMap, HashMap};
use std::ops::Range;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;

use arrow::array::{Array, AsArray, RecordBatch};
use arrow::compute::cast;
use arrow::datatypes::{DataType, Float64Type, Int64Type, UInt64Type};
use futures::future::BoxFuture;
use futures::{FutureExt, StreamExt, TryStreamExt};
use object_store::aws::AmazonS3Builder;
use object_store::http::HttpBuilder;
use object_store::path::Path as StorePath;
use object_store::{ObjectMeta, ObjectStore, ObjectStoreExt};
use parquet::arrow::arrow_reader::ArrowReaderOptions;
use parquet::arrow::async_reader::{AsyncFileReader, ParquetRecordBatchStreamBuilder};
use parquet::errors::ParquetError;
use parquet::file::metadata::{ParquetMetaData, ParquetMetaDataReader, RowGroupMetaData};
use parquet::file::statistics::Statistics;
use pxl_geo_bake::coverage::{Coverage, Node};
use pxl_spec::space::coord::adapter::{epsg3857, epsg4326};
use pxl_spec::space::coord::world::REACH;
use serde_json::{Map, Number, Value};
use tokio_postgres::types::ToSql;
use tokio_postgres::{Client, NoTls};
use xpute_core::math::scalar::round;

/// An Overture type, the theme it is published under, its table, its generated columns, its indexes past the geometry's.
struct Type {
    overture: &'static str,
    theme: &'static str,
    table: &'static str,
    columns: &'static [(&'static str, &'static str)],
    indexes: &'static [&'static str],
}

const SUBTYPE: (&str, &str) = ("subtype", "TEXT GENERATED ALWAYS AS (properties->>'subtype') STORED");
const CLASS: (&str, &str) = ("class", "TEXT GENERATED ALWAYS AS (properties->>'class') STORED");
const NAME: (&str, &str) = ("name", "TEXT GENERATED ALWAYS AS (properties->'names'->>'primary') STORED");

const TYPES: &[Type] = &[
    Type { overture: "address", theme: "addresses", table: "overture_addresses", columns: &[], indexes: &[] },
    Type { overture: "bathymetry", theme: "base", table: "overture_base_bathymetry", columns: &[SUBTYPE, CLASS], indexes: &[] },
    Type {
        overture: "building",
        theme: "buildings",
        table: "overture_buildings",
        columns: &[
            SUBTYPE,
            CLASS,
            ("height", "DOUBLE PRECISION GENERATED ALWAYS AS ((properties->>'height')::float8) STORED"),
            ("num_floors", "INTEGER GENERATED ALWAYS AS (trunc((properties->>'num_floors')::float8)::int) STORED"),
        ],
        indexes: &["class"],
    },
    Type { overture: "building_part", theme: "buildings", table: "overture_building_parts", columns: &[], indexes: &[] },
    Type { overture: "division", theme: "divisions", table: "overture_division_points", columns: &[SUBTYPE, NAME], indexes: &["subtype"] },
    Type { overture: "division_area", theme: "divisions", table: "overture_divisions", columns: &[SUBTYPE, NAME], indexes: &["subtype"] },
    Type { overture: "division_boundary", theme: "divisions", table: "overture_division_boundaries", columns: &[], indexes: &[] },
    Type {
        overture: "place",
        theme: "places",
        table: "overture_places",
        columns: &[("name", "TEXT NOT NULL GENERATED ALWAYS AS (COALESCE(properties->'names'->>'primary', '')) STORED")],
        indexes: &["USING GIN (properties)", "USING GIN (name gin_trgm_ops)"],
    },
    Type {
        overture: "segment",
        theme: "transportation",
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
    Type { overture: "connector", theme: "transportation", table: "overture_connectors", columns: &[], indexes: &[] },
    Type { overture: "infrastructure", theme: "base", table: "overture_base_infrastructure", columns: &[SUBTYPE, CLASS], indexes: &["class"] },
    Type { overture: "land", theme: "base", table: "overture_base_land", columns: &[SUBTYPE, CLASS], indexes: &["class"] },
    Type { overture: "land_cover", theme: "base", table: "overture_base_land_cover", columns: &[SUBTYPE], indexes: &[] },
    Type { overture: "land_use", theme: "base", table: "overture_base_land_use", columns: &[SUBTYPE, CLASS], indexes: &["class"] },
    Type { overture: "water", theme: "base", table: "overture_base_water", columns: &[SUBTYPE, CLASS], indexes: &["class"] },
];


/// The release's bucket, open to anyone, and the catalog that names its releases.
const BUCKET: &str = "overturemaps-us-west-2";
const STAC: &str = "https://stac.overturemaps.org";

/// Files read at once, as the CLI's dataset read ahead of itself.
const READ_AHEAD: usize = 8;

const BATCH: usize = 300;

/// A row group's statistics are the bbox's f32s; a shard's edges are within this of them.
const SLACK_DEG: f64 = 1e-4;

/// id, properties, geometry, shard.
type Row = (String, String, Vec<u8>, String);

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[ingest] {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    if std::env::args().len() > 1 {
        return Err("no flags: the shards are the coverage's (infra/geo/elevation/metadata.json)".to_string());
    }
    let shards: Vec<Node> = Coverage::of_source()?.shards().collect();
    let (client, connection) = tokio_postgres::connect(&db_geo_url()?, NoTls).await.map_err(|e| e.to_string())?;
    tokio::spawn(connection);
    let store: Arc<dyn ObjectStore> =
        Arc::new(AmazonS3Builder::new().with_bucket_name(BUCKET).with_region("us-west-2").with_skip_signature(true).build().map_err(|e| e.to_string())?);
    let release = latest_release().await?;
    println!("[ingest] release {release}, {} shards", shards.len());
    client.batch_execute(CATALOG_SQL).await.map_err(|e| e.to_string())?;
    keyed_by_shard(&client, "overture_catalog", "file_name").await?;

    for ty in TYPES {
        client.batch_execute(&schema(ty)).await.map_err(|e| e.to_string())?;
        keyed_by_shard(&client, ty.table, "gers_id").await?;
        let done: Vec<String> = client
            .query("SELECT node FROM overture_catalog WHERE file_name = $1 AND data_date = $2", &[&ty.table, &release])
            .await
            .map_err(|e| e.to_string())?
            .iter()
            .map(|r| r.get(0))
            .collect();
        let to_do: Vec<Node> = shards.iter().copied().filter(|n| !done.contains(&n.name())).collect();
        if to_do.is_empty() {
            println!("[ingest/{}] every shard already release {release}", ty.table);
            continue;
        }
        let names: Vec<String> = to_do.iter().map(|n| n.name()).collect();
        // The catalog row goes first, so a shard is never marked whole while its rows are partial.
        client.execute("DELETE FROM overture_catalog WHERE file_name = $1 AND node = ANY($2)", &[&ty.table, &names]).await.map_err(|e| e.to_string())?;
        client.execute(&format!("DELETE FROM {} WHERE node = ANY($1)", ty.table), &[&names]).await.map_err(|e| e.to_string())?;
        let counts = ingest(&client, &store, &release, ty, &to_do).await?;
        for name in &names {
            let count = counts.get(name).copied().unwrap_or(0);
            client
                .execute("INSERT INTO overture_catalog (file_name, node, data_date, total_count) VALUES ($1, $2, $3, $4)", &[&ty.table, name, &release, &count])
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// A table keyed otherwise than by (node, `id`) was made before shards and cannot be filled a shard at a time.
async fn keyed_by_shard(client: &Client, table: &str, id: &str) -> Result<(), String> {
    let key: Vec<String> = client
        .query(
            "SELECT a.attname FROM pg_index i JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey)
             WHERE i.indrelid = to_regclass($1::text) AND i.indisprimary ORDER BY array_position(i.indkey, a.attnum)",
            &[&table],
        )
        .await
        .map_err(|e| e.to_string())?
        .iter()
        .map(|r| r.get(0))
        .collect();
    if key != ["node", id] {
        return Err(format!("{table} is keyed by {key:?}, made before shards: drop it, and the catalog, and ingest again"));
    }
    Ok(())
}

/// The release the catalog names as latest.
async fn latest_release() -> Result<String, String> {
    let stac = HttpBuilder::new().with_url(STAC).build().map_err(|e| e.to_string())?;
    let bytes = stac.get(&StorePath::from("catalog.json")).await.map_err(|e| e.to_string())?.bytes().await.map_err(|e| e.to_string())?;
    let catalog: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    catalog["latest"].as_str().map(str::to_string).ok_or_else(|| format!("{STAC}/catalog.json names no latest release"))
}

/// A shard as a feature is tested against: its edges in EPSG:3857 meters, each
/// rounded half to even, `[left, bottom, right, top]`, the right and top
/// edges the next shard's; and the same in degrees, for row groups.
struct Shard {
    name: String,
    m: [f64; 4],
    deg: [f64; 4],
}

impl Shard {
    fn of(node: Node) -> Shard {
        let m = node.rect().map(|u| round(epsg3857::to_db(u)));
        let [w, s, _] = epsg4326::from_mercator(&[m[0], m[1], 0.0]);
        let [e, n, _] = epsg4326::from_mercator(&[m[2], m[3], 0.0]);
        // The east edge at the seam reads back as -180.
        Shard { name: node.name(), m, deg: [w, s, if e <= w { 180.0 } else { e }, n] }
    }

    /// Whether a feature whose vertices span `[lo_x, lo_y, hi_x, hi_y]` in
    /// meters meets the shard. Both are half open: a span ending on an edge
    /// stops short of it, and the edge is the shard's whose left or bottom it
    /// is; a single point on it lies there alone.
    fn meets(&self, [lo_x, lo_y, hi_x, hi_y]: [f64; 4]) -> bool {
        let [left, bottom, right, top] = self.m;
        let axis = |lo: f64, hi: f64, from: f64, to: f64| lo < to && (hi > from || (lo == hi && lo >= from));
        axis(lo_x, hi_x, left, right) && axis(lo_y, hi_y, bottom, top)
    }

    /// Whether a row group spanning `[w, s, e, n]` in degrees can hold a feature that meets the shard.
    fn may_hold(&self, [w, s, e, n]: [f64; 4]) -> bool {
        let [sw, ss, se, sn] = self.deg;
        w <= se + SLACK_DEG && e >= sw - SLACK_DEG && s <= sn + SLACK_DEG && n >= ss - SLACK_DEG
    }
}

/// A row group's bbox statistics, `[w, s, e, n]`; the whole world where one is missing.
fn span(group: &RowGroupMetaData) -> [f64; 4] {
    let bound = |path: &str, min: bool, or: f64| -> f64 {
        let stat = group.columns().iter().find(|c| c.column_path().string() == path).and_then(|c| c.statistics());
        let v = match stat {
            Some(Statistics::Float(s)) => (if min { s.min_opt() } else { s.max_opt() }).map(|&v| f64::from(v)),
            Some(Statistics::Double(s)) => (if min { s.min_opt() } else { s.max_opt() }).copied(),
            _ => None,
        };
        v.unwrap_or(or)
    };
    [bound("bbox.xmin", true, -180.0), bound("bbox.ymin", true, -90.0), bound("bbox.xmax", false, 180.0), bound("bbox.ymax", false, 90.0)]
}

/// One type's rows of the shards `to_do` into its table, counted by shard. A
/// feature with no id or geometry fails the run rather than being dropped.
async fn ingest(client: &Client, store: &Arc<dyn ObjectStore>, release: &str, ty: &Type, to_do: &[Node]) -> Result<BTreeMap<String, i64>, String> {
    let started = std::time::Instant::now();
    let shards: Arc<Vec<Shard>> = Arc::new(to_do.iter().map(|&n| Shard::of(n)).collect());
    let prefix = StorePath::from(format!("release/{release}/theme={}/type={}", ty.theme, ty.overture));
    let files: Vec<_> = store.list(Some(&prefix)).try_collect().await.map_err(|e| e.to_string())?;
    if files.is_empty() {
        return Err(format!("s3://{BUCKET}/{prefix}: no files"));
    }
    let mut reads = futures::stream::iter(files)
        .map(|file| {
            let (store, shards) = (Arc::clone(store), Arc::clone(&shards));
            async move {
                let builder = ParquetRecordBatchStreamBuilder::new(File { store, meta: file.clone() }).await.map_err(|e| format!("{}: {e}", file.location))?;
                let groups: Vec<usize> =
                    builder.metadata().row_groups().iter().enumerate().filter(|(_, g)| shards.iter().any(|s| s.may_hold(span(g)))).map(|(i, _)| i).collect();
                if groups.is_empty() {
                    return Ok(Vec::new());
                }
                builder.with_row_groups(groups).build().map_err(|e| e.to_string())?.try_collect::<Vec<_>>().await.map_err(|e| format!("{}: {e}", file.location))
            }
        })
        .buffer_unordered(READ_AHEAD);
    let mut batch: Vec<Row> = Vec::with_capacity(BATCH);
    let mut counts: BTreeMap<String, i64> = BTreeMap::new();
    let mut total: u64 = 0;
    while let Some(read) = reads.next().await {
        for group in read? {
            for row in rows(&group, &shards)? {
                *counts.entry(row.3.clone()).or_default() += 1;
                batch.push(row);
                if batch.len() == BATCH {
                    total += flush(client, ty.table, &mut batch).await?;
                    if total.is_multiple_of(BATCH as u64 * 100) {
                        println!("[ingest/{}] {total} rows in {:.0}s", ty.table, started.elapsed().as_secs_f64());
                    }
                }
            }
        }
    }
    total += flush(client, ty.table, &mut batch).await?;
    client.batch_execute(&format!("ANALYZE {}", ty.table)).await.map_err(|e| e.to_string())?;
    let names: Vec<String> = shards.iter().map(|s| s.name.clone()).collect();
    let stored: i64 = client.query_one(&format!("SELECT count(*) FROM {} WHERE node = ANY($1)", ty.table), &[&names]).await.map_err(|e| e.to_string())?.get(0);
    if stored as u64 != total {
        return Err(format!("{}: {total} rows read, {stored} stored", ty.table));
    }
    println!("[ingest/{}] {total} rows in {} shards in {:.0}s", ty.table, to_do.len(), started.elapsed().as_secs_f64());
    Ok(counts)
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
fn rows(group: &RecordBatch, shards: &[Shard]) -> Result<Vec<Row>, String> {
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
        let met: Vec<&Shard> = shards.iter().filter(|s| s.meets(span)).collect();
        if met.is_empty() {
            continue;
        }
        let mut object = Map::new();
        for (field, column) in &properties {
            if !column.is_null(i) {
                object.insert(field.name().clone(), json(column.as_ref(), i)?);
            }
        }
        let properties = Value::Object(object).to_string();
        for shard in met {
            out.push((id.value(i).to_string(), properties.clone(), geometry.value(i).to_vec(), shard.name.clone()));
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

/// A release file, its size known from the listing, read a range at a time.
struct File {
    store: Arc<dyn ObjectStore>,
    meta: ObjectMeta,
}

impl AsyncFileReader for File {
    fn get_bytes(&mut self, range: Range<u64>) -> BoxFuture<'_, parquet::errors::Result<bytes::Bytes>> {
        async move { self.store.get_range(&self.meta.location, range).await.map_err(|e| ParquetError::External(Box::new(e))) }.boxed()
    }

    fn get_byte_ranges(&mut self, ranges: Vec<Range<u64>>) -> BoxFuture<'_, parquet::errors::Result<Vec<bytes::Bytes>>> {
        async move { self.store.get_ranges(&self.meta.location, &ranges).await.map_err(|e| ParquetError::External(Box::new(e))) }.boxed()
    }

    fn get_metadata<'a>(&'a mut self, options: Option<&'a ArrowReaderOptions>) -> BoxFuture<'a, parquet::errors::Result<Arc<ParquetMetaData>>> {
        async move {
            let size = self.meta.size;
            Ok(Arc::new(ParquetMetaDataReader::new().with_arrow_reader_options(options).load_and_finish(self, size).await?))
        }
        .boxed()
    }
}

/// A WKB parameter in EPSG:3857. Mercator ends short of the poles, where the
/// world's edge clamps latitude (`epsg4326`), so what lies past it is cut off
/// rather than refused by the projection.
fn projected(param: usize) -> String {
    let lat = epsg4326::Y_TO_LAT(epsg3857::to_db(REACH.max[1]));
    format!("ST_Transform(ST_ClipByBox2D(ST_GeomFromWKB(${param}, 4326), ST_MakeEnvelope(-180, -{lat:?}, 180, {lat:?}, 4326)), 3857)")
}


async fn flush(client: &Client, table: &str, batch: &mut Vec<Row>) -> Result<u64, String> {
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
    let done = client.execute(sql.as_str(), &params).await.map_err(|e| match e.as_db_error() {
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

/// A feature over a shard's edge is a row in each shard it meets.
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
