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
tokio-postgres = "0.7"
serde_json = "1"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
---

//! Every Overture type, every feature in the bbox, into db-geo: a table a
//! type, a row its id, its properties as the release holds them and its
//! geometry. A column a reader filters on is generated from `properties`.
//! Each run replaces the tables; `overture_catalog` keeps each one's bbox and
//! release. The release's GeoParquet is read where it is published, the
//! row groups whose bbox statistics meet the query's only.
//!
//!   ./infra/geo/overture/script/ingest.rs --bbox "$(cargo run -q --manifest-path crates/Cargo.toml --bin ingest_bbox)"

use std::collections::HashMap;
use std::path::Path;
use std::ops::Range;
use std::process::ExitCode;
use std::sync::Arc;

use arrow::array::{Array, ArrayRef, AsArray, BooleanArray, RecordBatch};
use arrow::compute::{cast, filter_record_batch};
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
use parquet::file::metadata::{ParquetMetaData, ParquetMetaDataReader};
use parquet::file::metadata::RowGroupMetaData;
use parquet::file::statistics::Statistics;
use serde_json::{Map, Number, Value};
use tokio_postgres::types::ToSql;
use tokio_postgres::{Client, NoTls};

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

type Bbox = [f64; 4];
type Row = (String, String, Vec<u8>);

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
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let bbox = match argv.as_slice() {
        [flag, value] if flag == "--bbox" => value.clone(),
        _ => return Err("--bbox is required: `cargo run -q --manifest-path crates/Cargo.toml --bin ingest_bbox` prints it".to_string()),
    };
    let corners: Vec<f64> = bbox.split(',').map(|v| v.trim().parse::<f64>()).collect::<Result<_, _>>().map_err(|e| format!("--bbox {bbox}: {e}"))?;
    let corners: Bbox = corners.try_into().map_err(|_| format!("--bbox {bbox}: four numbers, xmin,ymin,xmax,ymax"))?;
    let (client, connection) = tokio_postgres::connect(&db_geo_url()?, NoTls).await.map_err(|e| e.to_string())?;
    tokio::spawn(connection);
    let store: Arc<dyn ObjectStore> =
        Arc::new(AmazonS3Builder::new().with_bucket_name(BUCKET).with_region("us-west-2").with_skip_signature(true).build().map_err(|e| e.to_string())?);
    let release = latest_release().await?;
    println!("[ingest] release {release}, bbox={bbox}");
    client.batch_execute(CATALOG_SQL).await.map_err(|e| e.to_string())?;

    for ty in TYPES {
        // A catalog row is written only once its table is whole, so it marks one done.
        let done = client
            .query_opt("SELECT 1 FROM overture_catalog WHERE file_name = $1 AND region_code = $2 AND data_date = $3", &[&ty.table, &bbox, &release])
            .await
            .map_err(|e| e.to_string())?;
        if done.is_some() {
            println!("[ingest/{}] already release {release} over this bbox", ty.table);
            continue;
        }
        client.execute("DELETE FROM overture_catalog WHERE file_name = $1", &[&ty.table]).await.map_err(|e| e.to_string())?;
        let count = ingest(&client, &store, corners, &release, ty).await?;
        client
            .execute(
                "INSERT INTO overture_catalog (file_name, region_code, data_date, total_count) VALUES ($1, $2, $3, $4)
                 ON CONFLICT (file_name) DO UPDATE SET region_code = excluded.region_code, data_date = excluded.data_date,
                   imported_at = now(), status = 'COMPLETED', total_count = excluded.total_count",
                &[&ty.table, &bbox, &release, &count],
            )
            .await
            .map_err(|e| e.to_string())?;
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

/// One type into its table. A feature with no id or geometry fails the run
/// rather than being dropped.
async fn ingest(client: &Client, store: &Arc<dyn ObjectStore>, bbox: Bbox, release: &str, ty: &Type) -> Result<i64, String> {
    let started = std::time::Instant::now();
    let prefix = StorePath::from(format!("release/{release}/theme={}/type={}", ty.theme, ty.overture));
    let files: Vec<_> = store.list(Some(&prefix)).try_collect().await.map_err(|e| e.to_string())?;
    if files.is_empty() {
        return Err(format!("s3://{BUCKET}/{prefix}: no files"));
    }
    client.batch_execute(&schema(ty)).await.map_err(|e| e.to_string())?;
    let mut reads = futures::stream::iter(files)
        .map(|file| {
            let store = Arc::clone(store);
            async move {
                let builder = ParquetRecordBatchStreamBuilder::new(File { store, meta: file.clone() }).await.map_err(|e| format!("{}: {e}", file.location))?;
                let groups: Vec<usize> = builder.metadata().row_groups().iter().enumerate().filter(|(_, g)| may_meet(g, bbox)).map(|(i, _)| i).collect();
                if groups.is_empty() {
                    return Ok(Vec::new());
                }
                builder.with_row_groups(groups).build().map_err(|e| e.to_string())?.try_collect::<Vec<_>>().await.map_err(|e| format!("{}: {e}", file.location))
            }
        })
        .buffer_unordered(READ_AHEAD);
    let mut batch: Vec<Row> = Vec::with_capacity(BATCH);
    let mut total: u64 = 0;
    while let Some(read) = reads.next().await {
        for group in read? {
            let group = filter_record_batch(&group, &meets(&group, bbox)?).map_err(|e| e.to_string())?;
            for row in rows(&group)? {
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
    let count: i64 = client.query_one(&format!("SELECT count(*) FROM {}", ty.table), &[]).await.map_err(|e| e.to_string())?.get(0);
    if count as u64 != total {
        return Err(format!("{}: {total} features read, {count} rows stored", ty.table));
    }
    println!("[ingest/{}] {count} rows in {:.0}s", ty.table, started.elapsed().as_secs_f64());
    Ok(count)
}

/// Whether a row group's bbox statistics leave room for a row in the query's.
fn may_meet(group: &RowGroupMetaData, [xmin, ymin, xmax, ymax]: Bbox) -> bool {
    let bound = |path: &str, min: bool| -> Option<f64> {
        let column = group.columns().iter().find(|c| c.column_path().string() == path)?;
        match column.statistics()? {
            Statistics::Float(s) => (if min { s.min_opt() } else { s.max_opt() }).map(|&v| f64::from(v)),
            Statistics::Double(s) => (if min { s.min_opt() } else { s.max_opt() }).copied(),
            _ => None,
        }
    };
    !(bound("bbox.xmin", true).is_some_and(|v| v >= xmax)
        || bound("bbox.xmax", false).is_some_and(|v| v <= xmin)
        || bound("bbox.ymin", true).is_some_and(|v| v >= ymax)
        || bound("bbox.ymax", false).is_some_and(|v| v <= ymin))
}

/// The CLI's test: a row's bbox overlaps the query's, strictly.
fn meets(group: &RecordBatch, [xmin, ymin, xmax, ymax]: Bbox) -> Result<BooleanArray, String> {
    let bbox = group.column_by_name("bbox").ok_or("no bbox column")?.as_struct();
    let side = |name: &str| -> Result<ArrayRef, String> {
        let column = bbox.column_by_name(name).ok_or_else(|| format!("no bbox.{name}"))?;
        cast(column, &DataType::Float64).map_err(|e| e.to_string())
    };
    let (x0, x1, y0, y1) = (side("xmin")?, side("xmax")?, side("ymin")?, side("ymax")?);
    let [x0, x1, y0, y1] = [&x0, &x1, &y0, &y1].map(|c| c.as_primitive::<Float64Type>());
    Ok((0..group.num_rows())
        .map(|i| Some(!x0.is_null(i) && !x1.is_null(i) && !y0.is_null(i) && !y1.is_null(i) && x0.value(i) < xmax && x1.value(i) > xmin && y0.value(i) < ymax && y1.value(i) > ymin))
        .collect())
}

/// The properties are every column but the id, the geometry and the bbox,
/// a null one left out; the geometry is the release's WKB, its doubles as
/// written.
fn rows(group: &RecordBatch) -> Result<Vec<Row>, String> {
    let id = cast(group.column_by_name("id").ok_or("no id column")?, &DataType::Utf8).map_err(|e| e.to_string())?;
    let id = id.as_string::<i32>();
    let geometry = cast(group.column_by_name("geometry").ok_or("no geometry column")?, &DataType::LargeBinary).map_err(|e| e.to_string())?;
    let geometry = geometry.as_binary::<i64>();
    let schema = group.schema();
    let properties: Vec<_> = schema.fields().iter().zip(group.columns()).filter(|(f, _)| !matches!(f.name().as_str(), "id" | "geometry" | "bbox")).collect();
    (0..group.num_rows())
        .map(|i| {
            if id.is_null(i) || geometry.is_null(i) {
                return Err(format!("{}: a feature with no id or no geometry", if id.is_null(i) { "?" } else { id.value(i) }));
            }
            let mut object = Map::new();
            for (field, column) in &properties {
                if !column.is_null(i) {
                    object.insert(field.name().clone(), json(column.as_ref(), i)?);
                }
            }
            Ok((id.value(i).to_string(), Value::Object(object).to_string(), geometry.value(i).to_vec()))
        })
        .collect()
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

async fn flush(client: &Client, table: &str, batch: &mut Vec<Row>) -> Result<u64, String> {
    if batch.is_empty() {
        return Ok(0);
    }
    let mut sql = format!("INSERT INTO {table} (gers_id, properties, geom) VALUES ");
    for i in 0..batch.len() {
        let n = i * 3;
        if i > 0 {
            sql.push(',');
        }
        // `::text::jsonb`, not `::jsonb`, so the parameter is bound as text.
        sql.push_str(&format!("(${}, ${}::text::jsonb, ST_Transform(ST_GeomFromWKB(${}, 4326), 3857))", n + 1, n + 2, n + 3));
    }
    let params: Vec<&(dyn ToSql + Sync)> = batch.iter().flat_map(|(id, properties, wkb)| [id as &(dyn ToSql + Sync), properties, wkb]).collect();
    let done = client.execute(sql.as_str(), &params).await.map_err(|e| format!("{table}: {e}"))?;
    batch.clear();
    Ok(done)
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

fn schema(ty: &Type) -> String {
    let t = ty.table;
    let mut sql = format!(
        "CREATE EXTENSION IF NOT EXISTS postgis;\nCREATE EXTENSION IF NOT EXISTS pg_trgm;\nDROP TABLE IF EXISTS {t};\nCREATE TABLE {t} (\n  gers_id TEXT PRIMARY KEY,\n  properties JSONB NOT NULL,\n"
    );
    for (name, def) in ty.columns {
        sql.push_str(&format!("  {name} {def},\n"));
    }
    sql.push_str(&format!("  geom GEOMETRY(Geometry, 3857) NOT NULL\n);\nCREATE INDEX ON {t} USING GIST (geom);\n"));
    for index in ty.indexes {
        if index.starts_with("USING") || index.starts_with('(') {
            sql.push_str(&format!("CREATE INDEX ON {t} {index};\n"));
        } else {
            sql.push_str(&format!("CREATE INDEX ON {t} ({index});\n"));
        }
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
