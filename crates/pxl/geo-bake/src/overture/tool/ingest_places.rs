// pxl-geo-bake/overture/tool/ingest_places.rs

//! Overture's Places theme into `overture_places`, the one table the POI
//! provider and forward search read.
//!
//! Deliberately its own small pipeline rather than a generic ogr2ogr-style
//! flat-field mapping: Overture's per-place payload is a rich nested object
//! (categories/names/addresses/sources/...), a poor fit for that, and this
//! pipeline is meant to stay structurally independent of the terrain one
//! rather than layered on top of it
//! (issue/20260908-overture-poi-adoption.md).
//!
//! `name` is lifted out of the payload into a column of its own so that the
//! trigram index has something to be on — search matches a name, and a GIN
//! index over the whole JSONB cannot serve `ILIKE`. It is `names.primary`,
//! the same access the POI provider's own `name_of` does.
//!
//! Attribution: Places is CDLA-Permissive-2.0, which imposes no attribution
//! requirement as a condition of use — still shown in-app as a courtesy.
//!
//!   deno task ingest:places [--bbox ...] [--out dir]

use pxl_geo_bake::overture::ingest::{self, Cell, Geom, GeomValue, Row, Table};
use pxl_geo_bake::overture::tool::args::Args;

const SCHEMA: &str = "
CREATE EXTENSION IF NOT EXISTS postgis;
CREATE EXTENSION IF NOT EXISTS pg_trgm;
DROP TABLE IF EXISTS overture_places;
CREATE TABLE overture_places (
  gers_id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  properties JSONB NOT NULL,
  geom GEOMETRY(Point, 3857) NOT NULL
);
CREATE INDEX idx_overture_places_geom ON overture_places USING GIST (geom);
CREATE INDEX idx_overture_places_properties ON overture_places USING GIN (properties);
CREATE INDEX idx_overture_places_name_trgm ON overture_places USING GIN (name gin_trgm_ops);
";

fn main() -> std::process::ExitCode {
    match Args::parse().and_then(|a| run(&a)) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => Args::fail(&e),
    }
}

fn run(args: &Args) -> Result<(), String> {
    let mut client = args.connect()?;
    // A point batches larger than a polygon: what bounds an insert is the
    // payload's size, and a place is a coordinate pair plus its properties.
    let table = Table {
        name: "overture_places",
        columns: &["gers_id", "name", "properties"],
        geom: Geom::Point,
        schema: SCHEMA.to_string(),
        batch: 500,
    };

    let count = ingest::ingest(&mut client, &args.bbox, &args.out, "place", &table, |f| {
        let name = f.prop("names").and_then(|n| n.get("primary")).and_then(|v| v.as_str()).unwrap_or("");
        // The point's own two numbers, not its text: they are bound to
        // ST_MakePoint as doubles and never pass through a decimal rendering.
        let geometry: serde_json::Value = serde_json::from_str(f.geometry()?).ok()?;
        let coords = geometry.get("coordinates")?.as_array()?;
        let (lon, lat) = (coords.first()?.as_f64()?, coords.get(1)?.as_f64()?);
        Some(Row {
            cells: vec![
                Cell::Text(Some(f.id()?.to_string())),
                Cell::Text(Some(name.to_string())),
                Cell::Json(f.properties_raw().map(str::to_string)),
            ],
            geom: GeomValue::Point(lon, lat),
        })
    })?;

    args.record(&mut client, "overture_places", count)
}
