#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2021"

[dependencies]
postgres = { version = "0.19", features = ["with-serde_json-1"] }
# Both features are load-bearing, not defaults: overture.rs's `Cell::Json` and
# `Feature::parse` say what each one prevents.
serde_json = { version = "1", features = ["float_roundtrip", "raw_value"] }
---

//! Overture's Transportation theme (`segment` only) into
//! `overture_transportation`.
//!
//! `connector` — pure routing-topology points — is skipped: this pipeline only
//! ever rasterizes real geometry and routes nothing.
//!
//! Confirmed live against a real Gangnam-area sample: unlike Buildings,
//! `subtype`/`class` are 100% populated here (subtype: road/rail; class:
//! residential/service/footway/primary/secondary/steps/tertiary/path/subway/
//! cycleway/standard_gauge/...). `road` — structured lane and width data,
//! when present — is kept as JSONB rather than flattened, because
//! `overture_policy` decides how much of it to use and a raw passthrough here
//! means that mapping can change without a re-ingest.
//!
//!   ./infra/geo/overture/script/ingest_transportation.rs [--bbox ...] [--out dir]

mod args;
mod overture;

use args::Args;
use overture::{Cell, Geom, GeomValue, Row, Table};

const SCHEMA: &str = "
CREATE EXTENSION IF NOT EXISTS postgis;
DROP TABLE IF EXISTS overture_transportation;
CREATE TABLE overture_transportation (
  gers_id TEXT PRIMARY KEY,
  subtype TEXT NOT NULL,
  class TEXT,
  road JSONB,
  geom GEOMETRY(LineString, 3857) NOT NULL
);
CREATE INDEX idx_overture_transportation_geom ON overture_transportation USING GIST (geom);
CREATE INDEX idx_overture_transportation_class ON overture_transportation (subtype, class);
";

fn main() -> std::process::ExitCode {
    match Args::parse().and_then(|a| run(&a)) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => Args::fail(&e),
    }
}

fn run(args: &Args) -> Result<(), String> {
    let mut client = args.connect()?;
    let table = Table {
        name: "overture_transportation",
        columns: &["gers_id", "subtype", "class", "road"],
        geom: Geom::GeoJson,
        schema: SCHEMA.to_string(),
        batch: 300,
    };

    let count = overture::ingest(&mut client, &args.bbox, &args.out, "segment", &table, |f| {
        Some(Row {
            cells: vec![
                Cell::Text(Some(f.id()?.to_string())),
                overture::cell_text(f.prop_str("subtype")),
                overture::cell_text(f.prop_str("class")),
                Cell::Json(f.prop_raw("road")),
            ],
            geom: GeomValue::GeoJson(f.geometry()?.to_string()),
        })
    })?;

    args.record(&mut client, "overture_transportation", count)
}
