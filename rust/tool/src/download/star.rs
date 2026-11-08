// pxl-tool/download/star.rs

//! The Yale Bright Star Catalog (V/50, CDS) as CDS gives it, kept in
//! `asset/raw/sky/star/`, for `make bake-star`.
//!
//!   make download-star

use crate::{fetch, root};

/// pxl-geo-bake bin/star.rs reads what this fetches by its name.
const SRC: &str = "https://cdsarc.cds.unistra.fr/ftp/V/50/catalog.gz";

pub fn run() -> Result<(), String> {
    let dir = root().join("asset/raw/sky/star");
    let bytes = fetch::download(SRC, &dir)?;
    println!("[star] {} bytes in {}", bytes.len(), dir.display());
    Ok(())
}
