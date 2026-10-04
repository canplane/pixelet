// pxl-geo-bake/bin/ingest_bbox.rs

//! The bbox the Overture ingest must take, derived from the coverage; the
//! derivation goes to stderr and the bbox alone to stdout:
//!
//!   ./infra/geo/overture/script/ingest.rs --bbox "$(cargo run -q --manifest-path crates/Cargo.toml --bin ingest_bbox)"

use std::process::ExitCode;

use pxl_geo_bake::coverage::{bbox_arg, source_bbox, Coverage};

fn run() -> Result<(), String> {
    let coverage = Coverage::of_source()?;
    let [sw, ne] = source_bbox()?;
    let [x0, y0, x1, y1] = coverage.rect();
    eprintln!("[ingest_bbox] raster bbox_4326 {sw:?} {ne:?}");
    eprintln!(
        "[ingest_bbox] pages x[{}..{}] y[{}..{}], EPSG:3857 [{x0}, {y0}, {x1}, {y1}]",
        coverage.x.0, coverage.x.1, coverage.y.0, coverage.y.1
    );
    eprintln!("[ingest_bbox] {} page(s) past them on each side", coverage.margin_pages());
    println!("{}", bbox_arg(coverage.ingest_bbox()));
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[ingest_bbox] {e}");
            ExitCode::FAILURE
        }
    }
}
