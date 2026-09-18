// pxl-geo-bake/overture/tool/ingest_divisions.rs

//! Overture's Divisions theme into two tables — reverse geocoding's own
//! source, the same shape Places has for POI:
//!
//! - `overture_divisions` — `division_area`, real boundary polygons
//! - `overture_division_points` — `division`, the entity's own representative
//!   point, present even where nobody has mapped its polygon yet
//!
//! `country` is the only subtype dropped from both. `region` (도/특별시) and
//! `county` (시/구) are area-only: Korea's provinces and cities always have a
//! mapped polygon, so a point-fallback row for them would only ever be
//! redundant. `locality`/`neighborhood`/`microhood`/`macrohood` — Overture's
//! four sub-city subtypes, all roughly 동 scale or finer — go into both,
//! because for much of Korea a polygon simply does not exist yet at this
//! level even though the named place does.
//!
//! An unnamed feature is skipped: a label with no name is not a label.
//!
//!   deno task ingest:divisions [--bbox ...] [--out dir]

use pxl_geo_bake::overture::ingest::{self, Cell, Geom, GeomValue, Row, Table};
use pxl_geo_bake::overture::tool::args::Args;

const AREA_KEPT: &[&str] = &["region", "county", "locality", "neighborhood", "microhood", "macrohood"];
const POINT_KEPT: &[&str] = &["locality", "neighborhood", "microhood", "macrohood"];

/// The UI's own languages. Overture's `names.common` carries many more, and
/// storing the rest would be dead weight in every row.
const SUPPORTED_LANGS: &[&str] = &["en", "ko"];

fn schema(table: &str, geom_type: &str) -> String {
    format!(
        "
CREATE EXTENSION IF NOT EXISTS postgis;
DROP TABLE IF EXISTS {table};
CREATE TABLE {table} (
  gers_id TEXT PRIMARY KEY,
  subtype TEXT NOT NULL,
  name TEXT NOT NULL,
  names_common JSONB,
  geom GEOMETRY({geom_type}, 3857) NOT NULL
);
CREATE INDEX idx_{table}_geom ON {table} USING GIST (geom);
CREATE INDEX idx_{table}_subtype ON {table} (subtype);
"
    )
}

/// `names.common` narrowed to the languages the UI has, as a JSON object —
/// `null` when none of them is present, which is most rows.
fn common_names(f: &ingest::Feature) -> Option<String> {
    let common = f.prop("names")?.get("common")?.as_array()?;
    let mut picked = serde_json::Map::new();
    for pair in common {
        let (Some(lang), Some(name)) = (pair.get(0).and_then(|v| v.as_str()), pair.get(1).and_then(|v| v.as_str())) else {
            continue;
        };
        if SUPPORTED_LANGS.contains(&lang) && !name.is_empty() {
            picked.insert(lang.to_string(), serde_json::Value::String(name.to_string()));
        }
    }
    (!picked.is_empty()).then(|| serde_json::Value::Object(picked).to_string())
}

fn main() -> std::process::ExitCode {
    match Args::parse().and_then(|a| run(&a)) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => Args::fail(&e),
    }
}

fn run(args: &Args) -> Result<(), String> {
    let mut client = args.connect()?;
    let themes: [(&str, &str, &str, &[&str]); 2] = [
        ("division_area", "overture_divisions", "Geometry", AREA_KEPT),
        ("division", "overture_division_points", "Point", POINT_KEPT),
    ];

    let mut counts: Vec<(&str, i64)> = Vec::new();
    for (overture_type, name, geom_type, kept) in themes {
        let table = Table {
            name,
            columns: &["gers_id", "subtype", "name", "names_common"],
            geom: Geom::GeoJson,
            schema: schema(name, geom_type),
            batch: 200,
        };
        let count = ingest::ingest(&mut client, &args.bbox, &args.out, overture_type, &table, |f| {
            let subtype = f.prop_str("subtype")?;
            if !kept.contains(&subtype) {
                return None;
            }
            let primary = f.prop("names")?.get("primary")?.as_str()?;
            Some(Row {
                cells: vec![
                    Cell::Text(Some(f.id()?.to_string())),
                    Cell::Text(Some(subtype.to_string())),
                    Cell::Text(Some(primary.to_string())),
                    Cell::Json(common_names(f)),
                ],
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
