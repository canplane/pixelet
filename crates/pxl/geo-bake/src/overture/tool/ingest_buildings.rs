// pxl-geo-bake/overture/tool/ingest_buildings.rs

//! Overture's Buildings theme (`building` only) into `overture_buildings`.
//!
//! Confirmed live against a real Gangnam-area sample: only ~14% of Korean
//! building features carry `height`/`num_floors`/`class`/`subtype` at all.
//! `height`/`num_floors` are kept in the table regardless (a future consumer's
//! problem), but baking never reads them — `ground_baker` reads only
//! `computed.width`, for linear features, which buildings never are. A
//! structure cell needs to know it is a building and roughly which kind, for
//! `cube_types.structure`'s own small enum, not its real height, so an absent
//! `class`/`subtype` means "generic building" rather than "unusable row".
//!
//! `building_part` — sub-footprint height slices, a tower's individual setback
//! tiers — is skipped for the same reason: there is nowhere in this pipeline
//! to put finer height data even where it exists.
//!
//!   deno task ingest:buildings [--bbox minLon,minLat,maxLon,maxLat] [--out dir]

use pxl_geo_bake::overture::ingest::{self, Cell, Geom, GeomValue, Row, Table};
use pxl_geo_bake::overture::tool::args::Args;

const SCHEMA: &str = "
CREATE EXTENSION IF NOT EXISTS postgis;
DROP TABLE IF EXISTS overture_buildings;
CREATE TABLE overture_buildings (
  gers_id TEXT PRIMARY KEY,
  subtype TEXT,
  class TEXT,
  height DOUBLE PRECISION,
  num_floors INTEGER,
  geom GEOMETRY(Geometry, 3857) NOT NULL
);
CREATE INDEX idx_overture_buildings_geom ON overture_buildings USING GIST (geom);
CREATE INDEX idx_overture_buildings_class ON overture_buildings (class);
";

fn main() -> std::process::ExitCode {
    let args = match Args::parse() {
        Ok(a) => a,
        Err(e) => return Args::fail(&e),
    };
    match run(&args) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => Args::fail(&e),
    }
}

fn run(args: &Args) -> Result<(), String> {
    let mut client = args.connect()?;
    // Polygons are heavier payloads than Places' points, so the batch is
    // smaller: what bounds an insert is its size, not its row count.
    let table = Table {
        name: "overture_buildings",
        columns: &["gers_id", "subtype", "class", "height", "num_floors"],
        geom: Geom::GeoJson,
        schema: SCHEMA.to_string(),
        batch: 200,
    };

    let count = ingest::ingest(&mut client, &args.bbox, &args.out, "building", &table, |f| {
        Some(Row {
            cells: vec![
                Cell::Text(Some(f.id()?.to_string())),
                ingest::cell_text(f.prop_str("subtype")),
                ingest::cell_text(f.prop_str("class")),
                Cell::F64(f.prop_num("height")),
                Cell::I32(f.prop_num("num_floors").map(|n| n as i32)),
            ],
            geom: GeomValue::GeoJson(f.geometry()?.to_string()),
        })
    })?;

    args.record(&mut client, "overture_buildings", count)
}
