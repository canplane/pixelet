// pxl-tool/bin/download_star.rs

//! The Yale Bright Star Catalog (V/50, CDS) as CDS gives it, kept in
//! `asset/raw/sky/star/`, for `make bake-star`.
//!
//!   make download-star

use std::process::ExitCode;

use pxl_tool::root;

/// What it fetches; pxl-geo-bake bin/bake_star.rs reads it by this name.
const SOURCE: &str = "https://cdsarc.cds.unistra.fr/ftp/V/50/catalog.gz";

fn run() -> Result<(), String> {
    let dir = root().join("asset/raw/sky/star");
    let bytes = pxl_tool::fetch::kept(SOURCE, &dir)?;
    println!("[star] {} bytes in {}", bytes.len(), dir.display());
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[star] {e}");
            ExitCode::FAILURE
        }
    }
}
