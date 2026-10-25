// pxl-geo-bake/bin/ingest_overture.rs

//! Every Overture type into db-geo, a shard at a time: a table a type, a row
//! a feature, once, with its properties as the release holds them and its
//! geometry; the table holds every feature that meets an ingested shard's
//! apron, and which shards are ingested is the catalog's alone. A shard is a tile at `SHARD_ZOOM`, the coverage's
//! (`pxl-geo-bake coverage.rs`); what it holds depends on it alone, so shards
//! ingested one by one are the shards ingested together. The official
//! `overturemaps` CLI, by its public commands alone, downloads a type's
//! GeoParquet over the shards to do, a rectangle of them a call, kept in
//! `raw/` under a name that says its release and its bbox, so a shard added
//! fetches only itself and the ground between shards is never fetched. The extract is
//! read once into a staging table, each row with its shard, and the run is
//! written from it in one transaction, its catalog rows with it, so it is
//! whole or absent. A run takes the types it is given (`--type`, every one
//! when none is), each its own table and catalog rows, so one type moves to
//! a new release while the others stay. It adds the shards it is given to
//! each table as it is, leaving a feature it holds already; `--whole` makes
//! each table anew from every shard, beside the live one, and swaps it in.
//! When to run which is the user's to say: the catalog records each shard's
//! release and when it was ingested, and decides nothing. The release is the
//! one `source.json` pins, never the CLI's latest. A column a reader
//! filters on is generated from `properties`.
//! A division names its parent, which may stand past every shard: the
//! divisions are taken with their ancestors, each fetched by its id, so a
//! name's chain never breaks at the coverage's edge.
//!
//!   make ingest-overture ARGS="[--type <type>]… [--shard <name>]… [--whole]"

use std::collections::BTreeMap;

use fnv::{FnvHashMap, FnvHashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output, Stdio};

use arrow::array::{Array, AsArray, RecordBatch};
use arrow::compute::cast;
use arrow::datatypes::{DataType, Float64Type, Int64Type, UInt64Type};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use postgres::binary_copy::BinaryCopyInWriter;
use postgres::types::Type as Pg;
use postgres::{Client, NoTls, Transaction};
use pxl_geo_bake::coverage::{Coverage, Node};
use pxl_geo_bake::util::fnv1a;
use pxl_spec::space::coord::adapter::{epsg3857, epsg4326};
use pxl_spec::space::coord::world::REACH;
use serde_json::{Map, Number, Value};
use xpute_kit::math::scalar::round;

/// An Overture type, its table, its generated columns, its indexes past the geometry's.
#[derive(Debug)]
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
    Type {
        overture: "address",
        table: "overture_addresses",
        columns: &[],
        indexes: &[],
    },
    Type {
        overture: "bathymetry",
        table: "overture_base_bathymetry",
        columns: &[SUBTYPE, CLASS],
        indexes: &[],
    },
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
    Type {
        overture: "building_part",
        table: "overture_building_parts",
        columns: &[],
        indexes: &[],
    },
    Type {
        overture: "division",
        table: "overture_divisions",
        columns: &[SUBTYPE, NAME],
        indexes: &["subtype"],
    },
    // `division_id` keyed, so a bake tests a division for an area by its id, not by a scan.
    Type {
        overture: "division_area",
        table: "overture_division_areas",
        columns: &[SUBTYPE, NAME],
        indexes: &["subtype", "((properties->>'division_id'))"],
    },
    Type {
        overture: "division_boundary",
        table: "overture_division_boundaries",
        columns: &[],
        indexes: &[],
    },
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
    Type {
        overture: "connector",
        table: "overture_connectors",
        columns: &[],
        indexes: &[],
    },
    Type {
        overture: "infrastructure",
        table: "overture_base_infrastructure",
        columns: &[SUBTYPE, CLASS],
        indexes: &["class"],
    },
    Type {
        overture: "land",
        table: "overture_base_land",
        columns: &[SUBTYPE, CLASS],
        indexes: &["class"],
    },
    Type {
        overture: "land_cover",
        table: "overture_base_land_cover",
        columns: &[SUBTYPE],
        indexes: &[],
    },
    Type {
        overture: "land_use",
        table: "overture_base_land_use",
        columns: &[SUBTYPE, CLASS],
        indexes: &["class"],
    },
    Type {
        overture: "water",
        table: "overture_base_water",
        columns: &[SUBTYPE, CLASS],
        indexes: &["class"],
    },
];

/// What runs the official CLI, at the release `tools/Dockerfile` pins, and
/// what it downloads, the source, kept beside this script.
const TOOLS: &str = "./infra/geo/tools/run";
const EXTRACTS: &str = "./infra/geo/overture/raw";
const SOURCE: &str = "./infra/geo/overture/source.json";

/// How long the swap waits for the live table's readers, and how often it
/// tries: waiting longer would queue every new reader behind it.
const SWAP_WAIT: &str = "5s";
const SWAP_TRIES: u32 = 12;

/// The CLI's bbox test is strict, so the shards are asked for this much wider.
const SLACK_DEG: f64 = 1e-4;

/// id, properties, geometry, shard.
type Row = (String, String, Vec<u8>, String);

/// The type closed under a parent, and the property naming it.
const PARENT: (&str, &str) = ("division", "parent_division_id");

/// The shard an ancestor is staged under: none, so the catalog counts it in none.
const ANCESTOR: &str = "";

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
    pxl_geo_bake::cli::enter()?;
    // `--type <type>`, repeated, for those types' tables alone; `--shard <name>`, repeated, for those
    // shards alone; `--whole` for each table made anew from every shard.
    let (mut types, mut named, mut whole) = (Vec::<String>::new(), Vec::<String>::new(), false);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--type" => types.push(args.next().ok_or("--type names an Overture type")?),
            "--shard" => named.push(args.next().ok_or("--shard names a shard")?),
            "--whole" => whole = true,
            other => return Err(format!("unknown flag: {other}")),
        }
    }
    if let Some(t) = types.iter().find(|t| !TYPES.iter().any(|ty| ty.overture == t.as_str())) {
        return Err(format!("{t}: no Overture type this ingests"));
    }
    let all: Vec<Shard> = Coverage::of_source()?.shards().map(Shard::of).collect();
    if let Some(n) = named.iter().find(|n| !all.iter().any(|s| s.name == **n)) {
        return Err(format!("{n}: no shard of the coverage (infra/geo/coverage.json)"));
    }
    if whole && !named.is_empty() {
        return Err("--whole makes each table from every shard; it takes no --shard".into());
    }
    let shards: Vec<&Shard> = all.iter().filter(|s| named.is_empty() || named.contains(&s.name)).collect();
    let mut client = Client::connect(&std::env::var("DB_GEO_URL").map_err(|_| "env:DB_GEO_URL:missing")?, NoTls).map_err(|e| e.to_string())?;
    let (release, accessed_at) = (pinned("version")?, pinned("accessed_at")?);
    println!("[ingest] release {release}, {} shards{}", shards.len(), if whole { ", whole" } else { "" });
    client
        .batch_execute(&format!("CREATE EXTENSION IF NOT EXISTS postgis;\nCREATE EXTENSION IF NOT EXISTS pg_trgm;\n{CATALOG_SQL}"))
        .map_err(|e| e.to_string())?;

    for ty in TYPES.iter().filter(|ty| types.is_empty() || types.iter().any(|t| t == ty.overture)) {
        if !whole {
            // A shard is added to the table as it is; one of another shape is made anew only on --whole.
            let exists: bool = client.query_one("SELECT to_regclass($1::text) IS NOT NULL", &[&ty.table]).map_err(|e| e.to_string())?.get(0);
            if exists && !keyed(&mut client, ty.table, &["gers_id"])? {
                return Err(format!("{}: keyed otherwise than by gers_id; run with --whole", ty.table));
            }
            client.batch_execute(&format!("{}{}", table(ty, ty.table), indexes(ty, ty.table))).map_err(|e| e.to_string())?;
        }
        let started = std::time::Instant::now();
        let extracts = download(ty, &groups(&shards), &release)?;
        let fetched = started.elapsed().as_secs_f64();
        stage(&mut client, ty, &extracts)?;
        if ty.overture == PARENT.0 {
            let fetched = ancestors(&mut client, ty, &release, !whole)?;
            println!("[ingest/{}] {fetched} ancestors past the shards", ty.table);
        }
        let rows = if whole {
            build_and_swap(&mut client, ty, &shards, &release, &accessed_at)?
        } else {
            add(&mut client, ty, &shards, &release, &accessed_at)?
        };
        client.batch_execute(&format!("DROP TABLE {}; ANALYZE {}", staging(ty), ty.table)).map_err(|e| e.to_string())?;
        println!(
            "[ingest/{}] {rows} features in {} shards in {:.0}s, {fetched:.0}s of it fetching",
            ty.table,
            shards.len(),
            started.elapsed().as_secs_f64()
        );
    }
    Ok(())
}

/// Whether a table is keyed as this script keys it.
fn keyed(client: &mut Client, table: &str, want: &[&str]) -> Result<bool, String> {
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
    Ok(key == want)
}

/// The shards as rectangles of the shard grid, so a download asks for little
/// past them: each row's shards in runs of neighbours, and a run joined to
/// the one under it when the two span the same columns. A full coverage is
/// one rectangle.
fn groups<'a>(shards: &[&'a Shard]) -> Vec<Vec<&'a Shard>> {
    let mut rows: BTreeMap<i32, Vec<&Shard>> = BTreeMap::new();
    for &s in shards {
        rows.entry(s.at[1]).or_default().push(s);
    }
    // Each rectangle open: its columns, its last row, its shards.
    let mut out: Vec<([i32; 2], i32, Vec<&Shard>)> = Vec::new();
    for (y, mut row) in rows {
        row.sort_by_key(|s| s.at[0]);
        for run in row.chunk_by(|a, b| b.at[0] == a.at[0] + 1) {
            let cols = [run[0].at[0], run[run.len() - 1].at[0]];
            match out.iter_mut().find(|(c, last, _)| *c == cols && *last == y - 1) {
                Some((_, last, held)) => {
                    *last = y;
                    held.extend_from_slice(run);
                }
                None => out.push((cols, y, run.to_vec())),
            }
        }
    }
    out.into_iter().map(|(_, _, held)| held).collect()
}

/// A type's extracts, a CLI call a rectangle of shards, called one after
/// another: every call reads Overture's STAC catalog, and a call a shard, six
/// at once, was refused for load (HTTP 429). Each is named by the release and
/// the bbox it was asked for, and the CLI's output takes that name only once
/// the CLI has finished, so an extract under it is current and whole; the
/// CLI's own state file is its private format, not read. The type's other
/// files, another release's or bbox's and that state beside them, are
/// removed.
fn download<'a>(ty: &Type, groups: &[Vec<&'a Shard>], release: &str) -> Result<Vec<(PathBuf, Vec<&'a Shard>)>, String> {
    std::fs::create_dir_all(EXTRACTS).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for shards in groups {
        // What the shards hold, their aprons with them (`Shard::deg`), not their bare tiles.
        let [w, s, e, n] = shards.iter().fold([f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY], |b, sh| {
            [b[0].min(sh.deg[0]), b[1].min(sh.deg[1]), b[2].max(sh.deg[2]), b[3].max(sh.deg[3])]
        });
        let bbox = format!("{},{},{},{}", w - SLACK_DEG, s - SLACK_DEG, e + SLACK_DEG, n + SLACK_DEG);
        let name = format!("{}.{release}.{:016x}.parquet", ty.overture, fnv1a(bbox.as_bytes()));
        let extract = Path::new(EXTRACTS).join(&name);
        if !extract.exists() {
            let part = Path::new(EXTRACTS).join(format!("{name}.part"));
            let args = [
                "download",
                &format!("--bbox={bbox}"),
                "-f",
                "geoparquet",
                &format!("--type={}", ty.overture),
                &format!("--release={release}"),
                "-o",
                &part.to_string_lossy(),
            ];
            overture(&args)?;
            std::fs::rename(&part, &extract).map_err(|e| e.to_string())?;
        }
        out.push((extract, shards.clone()));
    }
    for entry in std::fs::read_dir(EXTRACTS).map_err(|e| e.to_string())?.flatten() {
        let other = entry.file_name().to_string_lossy().to_string();
        let kept = out.iter().any(|(p, _)| p.file_name().is_some_and(|n| n.to_string_lossy() == other));
        if !kept && (other == ty.overture || other.starts_with(&format!("{}.", ty.overture))) {
            let path = entry.path();
            std::fs::remove_dir_all(&path)
                .or_else(|_| std::fs::remove_file(&path))
                .map_err(|e| format!("{}: {e}", path.display()))?;
        }
    }
    Ok(out)
}

/// The staging table a type's shards are written from.
fn staging(ty: &Type) -> String {
    format!("{}_staging", ty.table)
}

/// The type's extracts read once, each feature a row for each shard of its
/// own extract's rectangle it meets, so a feature two rectangles' bboxes
/// both reach is staged once a shard; a feature with no id or geometry fails
/// the run rather than being dropped.
fn stage(client: &mut Client, ty: &Type, extracts: &[(PathBuf, Vec<&Shard>)]) -> Result<(), String> {
    let t = staging(ty);
    client
        .batch_execute(&format!(
            "DROP TABLE IF EXISTS {t}; CREATE UNLOGGED TABLE {t} (shard TEXT NOT NULL, gers_id TEXT NOT NULL, properties TEXT NOT NULL, wkb BYTEA NOT NULL)"
        ))
        .map_err(|e| e.to_string())?;
    copy_extracts(client, &t, extracts.iter().map(|(p, s)| (p.as_path(), Some(s.as_slice()))))?;
    client
        .batch_execute(&format!("CREATE INDEX ON {t} (shard); CREATE INDEX ON {t} (gers_id)"))
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Extracts' rows into the staging table `t`, each under the shards it meets.
fn copy_extracts<'a>(client: &mut Client, t: &str, extracts: impl Iterator<Item = (&'a Path, Option<&'a [&'a Shard]>)>) -> Result<(), String> {
    let mut copy = BinaryCopyInWriter::new(
        client
            .copy_in(&format!("COPY {t} (gers_id, properties, wkb, shard) FROM STDIN (FORMAT binary)"))
            .map_err(|e| e.to_string())?,
        &[Pg::TEXT, Pg::TEXT, Pg::BYTEA, Pg::TEXT],
    );
    for (extract, shards) in extracts {
        let file = std::fs::File::open(extract).map_err(|e| format!("{}: {e}", extract.display()))?;
        let reader = ParquetRecordBatchReaderBuilder::try_new(file).map_err(|e| e.to_string())?.build().map_err(|e| e.to_string())?;
        for batch in reader {
            for (id, properties, wkb, node) in rows(&batch.map_err(|e| e.to_string())?, shards)? {
                copy.write(&[&id, &properties, &wkb, &node]).map_err(|e| e.to_string())?;
            }
        }
    }
    copy.finish().map_err(|e| e.to_string())?;
    Ok(())
}

/// The parents the staged divisions name and neither the staging table nor,
/// for a partial run, the live one holds, fetched by id with the CLI's
/// `gers` and staged under no shard, until every parent is held. A feature
/// the registry finds in another release than the run's fails the run. An
/// ancestor's extract is named by its release and id, so a run fetches it
/// once.
fn ancestors(client: &mut Client, ty: &Type, release: &str, live: bool) -> Result<usize, String> {
    let t = staging(ty);
    let held = if live {
        format!(" AND NOT EXISTS (SELECT 1 FROM {} l WHERE l.gers_id = p.id)", ty.table)
    } else {
        String::new()
    };
    let missing_sql = format!(
        "SELECT DISTINCT p.id FROM (SELECT properties::jsonb->>'{}' AS id FROM {t}) p WHERE p.id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM {t} s WHERE s.gers_id = p.id){held} ORDER BY p.id",
        PARENT.1
    );
    let prefix = format!("{}-ancestor.", ty.overture);
    let mut kept = Vec::new();
    let mut asked = FnvHashSet::default();
    loop {
        let missing: Vec<String> = client.query(&missing_sql, &[]).map_err(|e| e.to_string())?.iter().map(|r| r.get(0)).collect();
        if missing.is_empty() {
            break;
        }
        // The registry answering another feature than the one asked would ask forever.
        if let Some(id) = missing.iter().find(|id| !asked.insert((*id).clone())) {
            return Err(format!("gers {id}: fetched, and still not held"));
        }
        let mut extracts = Vec::new();
        for id in &missing {
            let extract = Path::new(EXTRACTS).join(format!("{prefix}{release}.{id}.parquet"));
            if !extract.exists() {
                let part = extract.with_extension("parquet.part");
                let out = cli()
                    .args(["gers", id, "-f", "geoparquet", "-o", &part.to_string_lossy()])
                    .output()
                    .map_err(|e| format!("{TOOLS}: {e}"))?;
                let said = String::from_utf8_lossy(&out.stderr);
                if !out.status.success() {
                    return Err(format!("gers {id}: {}", said.trim()));
                }
                // The registry says the release it found the feature in, on stderr.
                if !said.contains(&format!("in release {release}")) {
                    return Err(format!("gers {id}: not found in release {release}: {}", said.trim()));
                }
                std::fs::rename(&part, &extract).map_err(|e| e.to_string())?;
            }
            extracts.push(extract);
        }
        copy_extracts(client, &t, extracts.iter().map(|p| (p.as_path(), None)))?;
        kept.extend(extracts);
    }
    for entry in std::fs::read_dir(EXTRACTS).map_err(|e| e.to_string())?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(&prefix) && !name.starts_with(&format!("{prefix}{release}.")) {
            std::fs::remove_file(entry.path()).map_err(|e| e.to_string())?;
        }
    }
    Ok(kept.len())
}

/// Each feature of the staging table once, in key order, made into a row.
fn features(ty: &Type, into: &str) -> String {
    format!(
        "INSERT INTO {into} (gers_id, properties, geom) SELECT gers_id, properties::jsonb, {} FROM (SELECT DISTINCT ON (gers_id) gers_id, properties, wkb FROM {} ORDER BY gers_id) f",
        projected("wkb"),
        staging(ty)
    )
}

/// The catalog rows of the shards written: a shard's count is the features
/// meeting it, so their sum over shards exceeds the table's rows.
/// Each shard's row, what it says read and never decided on: the release
/// it holds, when it was ingested, and its features. A shard ingested again
/// replaces its row.
fn catalog(tx: &mut Transaction, ty: &Type, shards: &[&Shard], release: &str, accessed_at: &str) -> Result<(), postgres::Error> {
    let counts: FnvHashMap<String, i64> = tx
        .query(&format!("SELECT shard, count(*) FROM {} GROUP BY shard", staging(ty)), &[])?
        .iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect();
    for s in shards {
        let count = counts.get(&s.name).copied().unwrap_or(0);
        tx.execute(
            "INSERT INTO overture_catalog (table_name, shard, version, accessed_at, total_count) VALUES ($1, $2, $3, $4::text::timestamptz, $5)
             ON CONFLICT (shard, table_name) DO UPDATE SET version = EXCLUDED.version, accessed_at = EXCLUDED.accessed_at, total_count = EXCLUDED.total_count",
            &[&ty.table, &s.name, &release, &accessed_at, &count],
        )?;
    }
    Ok(())
}

/// A whole run: the table built anew beside the live one, its readers
/// answered from the live one meanwhile, then swapped in with the catalog in
/// one short transaction. Its indexes are built after it is filled.
fn build_and_swap(client: &mut Client, ty: &Type, shards: &[&Shard], release: &str, accessed_at: &str) -> Result<i64, String> {
    let (t, next) = (ty.table, format!("{}_next", ty.table));
    let db = |e: postgres::Error| format!("{t}: {e}");
    client.batch_execute(&format!("DROP TABLE IF EXISTS {next}; {}", table(ty, &next))).map_err(db)?;
    let rows = client.execute(&features(ty, &next), &[]).map_err(db)? as i64;
    client.batch_execute(&indexes(ty, &next)).map_err(db)?;
    for attempt in 1..=SWAP_TRIES {
        let mut tx = client.transaction().map_err(db)?;
        tx.batch_execute(&format!("SET LOCAL lock_timeout = '{SWAP_WAIT}'")).map_err(db)?;
        match tx.batch_execute(&format!("DROP TABLE IF EXISTS {t}; ALTER TABLE {next} RENAME TO {t}")) {
            Err(e) if e.code() == Some(&postgres::error::SqlState::LOCK_NOT_AVAILABLE) => {
                println!("[ingest/{t}] a reader holds it; swap {attempt} of {SWAP_TRIES} waits");
                continue;
            }
            r => r.map_err(db)?,
        }
        // The indexes keep the names they were built under; schema() would add a second set.
        for name in tx
            .query("SELECT indexrelid::regclass::text FROM pg_index WHERE indrelid = $1::text::regclass", &[&t])
            .map_err(db)?
            .iter()
            .map(|r| r.get::<_, String>(0))
        {
            if let Some(rest) = name.strip_prefix(&next) {
                tx.batch_execute(&format!("ALTER INDEX {name} RENAME TO {t}{rest}")).map_err(db)?;
            }
        }
        tx.execute("DELETE FROM overture_catalog WHERE table_name = $1", &[&t]).map_err(db)?;
        catalog(&mut tx, ty, shards, release, accessed_at).map_err(db)?;
        tx.commit().map_err(db)?;
        return Ok(rows);
    }
    Err(format!("{t}: its readers held it through {SWAP_TRIES} tries; {next} is built and left beside it"))
}

/// The shards added to the table as it is: a feature held already is left,
/// with their catalog rows, in one transaction.
fn add(client: &mut Client, ty: &Type, shards: &[&Shard], release: &str, accessed_at: &str) -> Result<i64, String> {
    let db = |e: postgres::Error| format!("{}: {e}", ty.table);
    let mut tx = client.transaction().map_err(db)?;
    let rows = tx.execute(&format!("{} ON CONFLICT (gers_id) DO NOTHING", features(ty, ty.table)), &[]).map_err(db)? as i64;
    catalog(&mut tx, ty, shards, release, accessed_at).map_err(db)?;
    tx.commit().map_err(db)?;
    Ok(rows)
}

/// A shard as a feature is tested against: what it holds (`Node::held`, its
/// apron included) in EPSG:3857 meters, each rounded half to even,
/// `[left, bottom, right, top]`; and the same in degrees, for the CLI.
struct Shard {
    name: String,
    /// Its place on the shard grid.
    at: [i32; 2],
    m: [f64; 4],
    deg: [f64; 4],
}

impl Shard {
    fn of(node: Node) -> Shard {
        let m = node.held().map(|u| round(epsg3857::mer_from_u(u)));
        let [w, s, _] = epsg4326::from_mercator(&[m[0], m[1], 0.0]);
        let [e, n, _] = epsg4326::from_mercator(&[m[2], m[3], 0.0]);
        // The east edge at the seam reads back as -180.
        Shard {
            name: node.name(),
            at: node.at,
            m,
            deg: [w, s, if e <= w { 180.0 } else { e }, n],
        }
    }

    /// Whether a feature whose vertices span `[lo_x, lo_y, hi_x, hi_y]` in
    /// meters meets what the shard holds; its edges, rounded to the meter,
    /// are already `EPS` out (`SHARD_APRON`), so a feature on one is in both.
    fn meets(&self, [lo_x, lo_y, hi_x, hi_y]: [f64; 4]) -> bool {
        let [left, bottom, right, top] = self.m;
        lo_x <= right && hi_x >= left && lo_y <= top && hi_y >= bottom
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

/// A row for each feature and each shard of `shards` it meets, or one under
/// no shard where `shards` is none. The properties
/// are every column but the id, the geometry and the bbox, a null one left
/// out; the geometry is the release's WKB, its doubles as written.
fn rows(group: &RecordBatch, shards: Option<&[&Shard]>) -> Result<Vec<Row>, String> {
    let id = cast(group.column_by_name("id").ok_or("no id column")?, &DataType::Utf8).map_err(|e| e.to_string())?;
    let id = id.as_string::<i32>();
    let geometry = cast(group.column_by_name("geometry").ok_or("no geometry column")?, &DataType::LargeBinary).map_err(|e| e.to_string())?;
    let geometry = geometry.as_binary::<i64>();
    let schema = group.schema();
    let properties: Vec<_> = schema
        .fields()
        .iter()
        .zip(group.columns())
        .filter(|(f, _)| !matches!(f.name().as_str(), "id" | "geometry" | "bbox"))
        .collect();
    let mut out = Vec::new();
    for i in 0..group.num_rows() {
        if id.is_null(i) || geometry.is_null(i) {
            return Err(format!("{}: a feature with no id or geometry", if id.is_null(i) { "?" } else { id.value(i) }));
        }
        let span = span_m(geometry.value(i)).map_err(|e| format!("{}: {e}", id.value(i)))?;
        let nodes: Vec<&str> = match shards {
            Some(shards) => shards.iter().filter(|s| s.meets(span)).map(|s| s.name.as_str()).collect(),
            None => vec![ANCESTOR],
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
        DataType::Float16 | DataType::Float32 | DataType::Float64 => Number::from_f64(one(&DataType::Float64)?.as_primitive::<Float64Type>().value(0)).map_or(Value::Null, Value::Number),
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
                let Value::String(key) = json(entries.column(0).as_ref(), k)? else {
                    return Err("a map key that is not a string".to_string());
                };
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
  table_name TEXT NOT NULL,
  shard TEXT NOT NULL,
  version TEXT NOT NULL,
  accessed_at TIMESTAMPTZ NOT NULL,
  total_count BIGINT NOT NULL,
  PRIMARY KEY (shard, table_name)
);
";

fn table(ty: &Type, t: &str) -> String {
    let mut sql = format!("CREATE TABLE IF NOT EXISTS {t} (\n  gers_id TEXT NOT NULL PRIMARY KEY,\n  properties JSONB NOT NULL,\n");
    for (name, def) in ty.columns {
        sql.push_str(&format!("  {name} {def},\n"));
    }
    sql.push_str("  geom GEOMETRY(Geometry, 3857) NOT NULL\n);\n");
    sql
}

fn indexes(ty: &Type, t: &str) -> String {
    let mut sql = format!("CREATE INDEX IF NOT EXISTS {t}_geom ON {t} USING GIST (geom);\n");
    for (k, index) in ty.indexes.iter().enumerate() {
        let on = if index.starts_with("USING") || index.starts_with('(') {
            index.to_string()
        } else {
            format!("({index})")
        };
        sql.push_str(&format!("CREATE INDEX IF NOT EXISTS {t}_{k} ON {t} {on};\n"));
    }
    sql
}

/// The official CLI, in the tools' image.
fn cli() -> Command {
    let mut cmd = Command::new(TOOLS);
    cmd.arg("overturemaps");
    cmd
}

/// The official CLI by a public command: what it prints to stderr is shown
/// as it runs, and a failure is the run's.
fn overture(args: &[&str]) -> Result<Output, String> {
    let out = cli().args(args).stderr(Stdio::inherit()).output().map_err(|e| format!("{TOOLS}: {e}"))?;
    if !out.status.success() {
        return Err(format!("overturemaps {} exited {}", args.join(" "), out.status.code().unwrap_or(-1)));
    }
    Ok(out)
}
/// A field of `source.json`'s pin: the release (Overture's name for its
/// version) and when it was taken, which the catalog repeats.
fn pinned(field: &str) -> Result<String, String> {
    let path = Path::new(SOURCE);
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let meta: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    meta["pin"][field].as_str().map(str::to_string).ok_or_else(|| format!("{}: no pin.{field}", path.display()))
}
