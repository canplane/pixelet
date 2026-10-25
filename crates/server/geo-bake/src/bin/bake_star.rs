// pxl-geo-bake/bin/bake_star.rs

//! The night sky's stars from the Yale Bright Star Catalog (V/50, CDS), as
//! `deno task download star` keeps it in `asset/raw/sky/star/`, into the
//! records pxl-render's gpu/star.rs reads (pxl-spec asset/star.rs),
//! brightest first.
//!
//!   deno task bake star

use std::io::Read;
use std::process::ExitCode;

use flate2::read::GzDecoder;

use pxl_spec::asset::star::{record, Star, RECORD_BYTES};

/// The catalog as pxl-tool bin/download_star.rs keeps it.
const SOURCE: &str = "asset/raw/sky/star/catalog.gz";
const OUT: &str = "crates/client/render/src/frame/shader/bright-star.bin";
const SOLAR_BV: f64 = 0.65;

fn catalog() -> Result<String, String> {
    let packed = std::fs::read(SOURCE).map_err(|e| format!("{SOURCE}: {e} — run `deno task download star`"))?;
    let mut text = String::new();
    GzDecoder::new(packed.as_slice()).read_to_string(&mut text).map_err(|e| format!("catalog: {e}"))?;
    Ok(text)
}

/// Byte columns per the catalog's ReadMe (1-based, inclusive).
fn star(line: &str) -> Result<Option<Star>, String> {
    let at = |from: usize, to: usize| line.get(from - 1..to.min(line.len())).unwrap_or("").trim();
    let num = |s: &str| s.parse::<f64>().map_err(|e| format!("{s:?} in {line:?}: {e}"));
    let (rah, ram, ras) = (at(76, 77), at(78, 79), at(80, 83));
    let (sign, ded, dem, des) = (at(84, 84), at(85, 86), at(87, 88), at(89, 90));
    let (vmag, bv) = (at(103, 107), at(110, 114));
    if rah.is_empty() || ded.is_empty() || vmag.is_empty() {
        return Ok(None);
    }
    let ra = (num(rah)? + num(ram)? / 60.0 + num(ras)? / 3600.0) * 15.0;
    let dec = if sign == "-" { -1.0 } else { 1.0 } * (num(ded)? + num(dem)? / 60.0 + num(des)? / 3600.0);
    let bv = if bv.is_empty() { SOLAR_BV } else { num(bv)? };
    Ok(Some(Star { ra, dec, vmag: num(vmag)?, bv }))
}

fn run() -> Result<(), String> {
    let text = catalog()?;
    let mut stars = Vec::new();
    for line in text.lines() {
        stars.extend(star(line)?);
    }
    stars.sort_by(|a, b| a.vmag.total_cmp(&b.vmag));
    let mut out = Vec::with_capacity(stars.len() * RECORD_BYTES);
    for s in &stars {
        out.extend_from_slice(&record(s));
    }
    std::fs::write(OUT, &out).map_err(|e| format!("{OUT}: {e}"))?;
    let (first, last) = (stars.first().ok_or("no stars")?, stars.last().ok_or("no stars")?);
    println!("[bake-stars] {} stars, magnitude {} to {} -> {OUT}", stars.len(), first.vmag, last.vmag);
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[bake-stars] {e}");
            ExitCode::FAILURE
        }
    }
}
