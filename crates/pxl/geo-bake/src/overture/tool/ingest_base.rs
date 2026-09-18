// pxl-geo-bake/overture/tool/ingest_base.rs

//! Overture's Base theme into four tables.
//!
//! Base is fragmented across several `--type` values rather than one, unlike
//! Places, Divisions, Buildings and Transportation:
//!
//! - `land_use` — residential/park/commercial/school/hospital/pitch/..., this
//!   project's own main land-colouring signal
//! - `water` — river/lake/stream/canal
//! - `land` — tree/forest/wood/peninsula, still needed separately from
//!   land_use, since a green patch with no administrative land_use polygon
//!   over it is common
//! - `infrastructure` — mixed geometry, kept for parking and helipad only
//!
//! `land_cover` is deliberately not ingested at all.
//!
//!   deno task ingest:base [--bbox ...] [--out dir]

use pxl_geo_bake::overture::ingest::{self, Cell, Geom, GeomValue, Row, Table};
use pxl_geo_bake::overture::tool::args::Args;

/// infrastructure is otherwise near-entirely point-scale street furniture —
/// confirmed live: crossing, bus_stop, traffic_signals, gate, fence,
/// bicycle_rental, recycling, emergency, power. No ground channel has an
/// equivalent for any of those, so only the classes this pipeline can
/// actually paint are kept.
const INFRA_KEPT: &[&str] = &["parking", "helipad"];

fn schema(table: &str) -> String {
    format!(
        "
CREATE EXTENSION IF NOT EXISTS postgis;
DROP TABLE IF EXISTS {table};
CREATE TABLE {table} (
  gers_id TEXT PRIMARY KEY,
  subtype TEXT,
  class TEXT,
  geom GEOMETRY(Geometry, 3857) NOT NULL
);
CREATE INDEX idx_{table}_geom ON {table} USING GIST (geom);
CREATE INDEX idx_{table}_class ON {table} (class);
"
    )
}

/// The Overture type, the table it fills, and the classes kept from it —
/// empty meaning all of them.
const THEMES: &[(&str, &str, &[&str])] = &[
    ("land_use", "overture_base_land_use", &[]),
    ("water", "overture_base_water", &[]),
    ("land", "overture_base_land", &[]),
    ("infrastructure", "overture_base_infrastructure", INFRA_KEPT),
];

fn main() -> std::process::ExitCode {
    match Args::parse().and_then(|a| run(&a)) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => Args::fail(&e),
    }
}

fn run(args: &Args) -> Result<(), String> {
    let mut client = args.connect()?;
    let mut counts: Vec<(&str, i64)> = Vec::new();

    for (overture_type, name, kept) in THEMES {
        let table = Table {
            name,
            columns: &["gers_id", "subtype", "class"],
            geom: Geom::GeoJson,
            schema: schema(name),
            batch: 300,
        };
        let count = ingest::ingest(&mut client, &args.bbox, &args.out, overture_type, &table, |f| {
            let class = f.prop_str("class");
            if !kept.is_empty() && !class.is_some_and(|c| kept.contains(&c)) {
                return None;
            }
            Some(Row {
                cells: vec![Cell::Text(Some(f.id()?.to_string())), ingest::cell_text(f.prop_str("subtype")), ingest::cell_text(class)],
                geom: GeomValue::GeoJson(f.geometry()?.to_string()),
            })
        })?;
        counts.push((name, count));
    }

    for (name, count) in counts {
        args.record(&mut client, name, count)?;
    }
    Ok(())
}
