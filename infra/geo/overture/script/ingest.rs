#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2021"

[dependencies]
postgres = "0.19"
serde_json = { version = "1", features = ["raw_value"] }
---

//! Every Overture type, every feature in the bbox, into db-geo: a table a
//! type, a row its id, its properties as written and its geometry. A column a
//! reader filters on is generated from `properties`. Each run replaces the
//! tables; `overture_catalog` keeps each one's bbox and release.
//!
//!   ./infra/geo/overture/script/ingest.rs --bbox "$(cargo run -q --manifest-path crates/Cargo.toml --bin ingest_bbox)"
//!
//! `geojsonseq`, never one FeatureCollection: Places alone came to ~900 MB
//! and blew the runtime's maximum string length.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use postgres::types::ToSql;
use postgres::{Client, NoTls};
use serde_json::value::RawValue;
use serde_json::Value;

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

/// The official `overturemaps` CLI, in a venv beside the extracts.
const VENV: &str = "./out/overture/.venv";

const BATCH: usize = 300;

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
    let (mut bbox, mut out) = (None, PathBuf::from("./out/overture"));
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut it = argv.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or_else(|| format!("{flag} wants a value"))?.clone();
        match flag.as_str() {
            "--bbox" => bbox = Some(value),
            "--out" => out = PathBuf::from(value),
            other => return Err(format!("unknown flag: {other}")),
        }
    }
    let bbox = bbox.ok_or("--bbox is required: `cargo run -q --manifest-path crates/Cargo.toml --bin ingest_bbox` prints it")?;
    let mut client = Client::connect(&db_geo_url()?, NoTls).map_err(|e| e.to_string())?;
    let cli = ensure_cli()?;
    let release = release(&cli)?;
    println!("[ingest] release {release}, bbox={bbox}, out={}", out.display());
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    client.batch_execute(CATALOG_SQL).map_err(|e| e.to_string())?;

    for ty in TYPES {
        // A catalog row is written only once its table is whole, so it marks one done.
        let done = client
            .query_opt("SELECT 1 FROM overture_catalog WHERE file_name = $1 AND region_code = $2 AND data_date = $3", &[&ty.table, &bbox, &release])
            .map_err(|e| e.to_string())?;
        if done.is_some() {
            println!("[ingest/{}] already release {release} over this bbox", ty.table);
            continue;
        }
        client.execute("DELETE FROM overture_catalog WHERE file_name = $1", &[&ty.table]).map_err(|e| e.to_string())?;
        let count = ingest(&mut client, &cli, &bbox, &out, &release, ty)?;
        client
            .execute(
                "INSERT INTO overture_catalog (file_name, region_code, data_date, total_count) VALUES ($1, $2, $3, $4)
                 ON CONFLICT (file_name) DO UPDATE SET region_code = excluded.region_code, data_date = excluded.data_date,
                   imported_at = now(), status = 'COMPLETED', total_count = excluded.total_count",
                &[&ty.table, &bbox, &release, &count],
            )
            .map_err(|e| e.to_string())?;
    }
    Ok(())
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

/// One type, downloaded unless the extract is already this release and bbox.
/// A feature with no id or geometry fails the run rather than being dropped.
fn ingest(client: &mut Client, cli: &Path, bbox: &str, out: &Path, release: &str, ty: &Type) -> Result<i64, String> {
    let started = std::time::Instant::now();
    let extract = out.join(format!("{}.geojsonseq", ty.overture));
    if is_current(&extract, ty.overture, bbox, release) {
        println!("[ingest/{}] {} is already release {release} over this bbox", ty.overture, extract.display());
    } else {
        println!("[ingest/{}] downloading release {release}...", ty.overture);
        let args = ["download", &format!("--bbox={bbox}"), "-f", "geojsonseq", &format!("--type={}", ty.overture), &format!("--release={release}"), "-o", &extract.to_string_lossy()];
        run_cmd(cli, &args)?;
    }

    client.batch_execute(&schema(ty)).map_err(|e| e.to_string())?;
    let file = std::fs::File::open(&extract).map_err(|e| format!("{}: {e}", extract.display()))?;
    let mut batch: Vec<[String; 3]> = Vec::with_capacity(BATCH);
    let mut total: u64 = 0;
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.is_empty() {
            continue;
        }
        batch.push(row(&line).ok_or_else(|| format!("{}: a feature with no id or no geometry", &line[..line.len().min(120)]))?);
        if batch.len() == BATCH {
            total += flush(client, ty.table, &mut batch)?;
            if total.is_multiple_of(BATCH as u64 * 100) {
                println!("[ingest/{}] {total} rows in {:.0}s", ty.table, started.elapsed().as_secs_f64());
            }
        }
    }
    total += flush(client, ty.table, &mut batch)?;
    client.batch_execute(&format!("ANALYZE {}", ty.table)).map_err(|e| e.to_string())?;
    let count: i64 = client.query_one(&format!("SELECT count(*) FROM {}", ty.table), &[]).map_err(|e| e.to_string())?.get(0);
    if count as u64 != total {
        return Err(format!("{}: {total} features read, {count} rows stored", ty.table));
    }
    println!("[ingest/{}] {count} rows in {:.0}s", ty.table, started.elapsed().as_secs_f64());
    Ok(count)
}

/// As the source wrote them: a re-rendered number is a different jsonb
/// `numeric` and, projected, a different point.
fn row(line: &str) -> Option<[String; 3]> {
    let raw: HashMap<&str, &RawValue> = serde_json::from_str(line).ok()?;
    let id: String = serde_json::from_str(raw.get("id")?.get()).ok()?;
    let properties = raw.get("properties").map_or("{}", |p| p.get()).to_string();
    Some([id, properties, raw.get("geometry")?.get().to_string()])
}

fn flush(client: &mut Client, table: &str, batch: &mut Vec<[String; 3]>) -> Result<u64, String> {
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
        sql.push_str(&format!("(${}, ${}::text::jsonb, ST_Transform(ST_SetSRID(ST_GeomFromGeoJSON(${}), 4326), 3857))", n + 1, n + 2, n + 3));
    }
    let params: Vec<&(dyn ToSql + Sync)> = batch.iter().flat_map(|r| r.iter().map(|c| c as &(dyn ToSql + Sync))).collect();
    let done = client.execute(sql.as_str(), &params).map_err(|e| format!("{table}: {e}"))?;
    batch.clear();
    Ok(done)
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
fn is_current(extract: &Path, overture_type: &str, bbox: &str, release: &str) -> bool {
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
    let want: Vec<f64> = bbox.split(',').filter_map(|v| v.trim().parse().ok()).collect();
    let have: Vec<f64> = ["xmin", "ymin", "xmax", "ymax"].iter().filter_map(|k| state["bbox"][k].as_f64()).collect();
    state["last_release"] == release && state["type"] == overture_type && want.len() == 4 && want == have
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
