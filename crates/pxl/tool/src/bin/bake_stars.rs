// pxl-tool/bin/bake_stars.rs

//! The night sky's stars, baked from the Yale Bright Star Catalog into the
//! eight bytes a star the renderer reads (pxl-render's gpu/stars.rs).
//!
//!   deno task bake:stars                  # fetch the catalog from CDS
//!   deno task bake:stars catalog.gz       # or read one already here
//!
//! The catalog is Hoffleit and Warren's fifth revised edition (1991), every
//! star to about magnitude 6.5, which is what an eye sees on a dark night:
//! CDS publishes it as V/50, a fixed-width table whose ReadMe gives the
//! columns read below. It is a public catalog, credited here and where it is
//! drawn rather than in THIRD_PARTY_NOTICES.md, which covers copied source.
//!
//! A star is two little-endian words: its right ascension as a u16 of a turn
//! and its declination as an i16 of a quarter turn, then its visual magnitude
//! and its B−V color as i16 thousandths. The quantization is 20″ in right
//! ascension and 10″ in declination, under a pixel at any field of view the
//! camera has. The few entries with no position — novae and objects the
//! catalog has retired — are left out; a star with no color index takes
//! the Sun's. Brightest first, so a prefix of the file is the sky to a
//! magnitude.

use std::io::Read;
use std::process::ExitCode;

use flate2::read::GzDecoder;

use pxl_tool::root;

const SOURCE: &str = "https://cdsarc.cds.unistra.fr/ftp/V/50/catalog.gz";
const OUT: &str = "crates/pxl/web/render/src/frame/shader/bright-stars.bin";
/// The Sun's B−V, for a star the catalog gives no color.
const SOLAR_BV: f64 = 0.65;

struct Star {
    ra: f64,
    dec: f64,
    vmag: f64,
    bv: f64,
}

fn catalog(path: Option<&str>) -> Result<String, String> {
    let packed = match path {
        Some(path) => std::fs::read(path).map_err(|e| format!("{path}: {e}"))?,
        None => {
            let mut body = Vec::new();
            ureq::get(SOURCE).call().map_err(|e| e.to_string())?.into_reader().read_to_end(&mut body).map_err(|e| e.to_string())?;
            body
        }
    };
    let mut text = String::new();
    GzDecoder::new(packed.as_slice()).read_to_string(&mut text).map_err(|e| format!("catalog: {e}"))?;
    Ok(text)
}

/// A line's star, by the ReadMe's byte columns (1-based, inclusive); none
/// where it has no position or no magnitude.
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

/// Half up. `f64::round` takes a half away from zero, which would move a
/// negative declination or magnitude that lands on one a step from where the
/// committed catalog has it.
fn round(x: f64) -> f64 {
    (x + 0.5).floor()
}

fn run() -> Result<(), String> {
    let arg = std::env::args().nth(1);
    let text = catalog(arg.as_deref())?;
    let mut stars = Vec::new();
    for line in text.lines() {
        stars.extend(star(line)?);
    }
    stars.sort_by(|a, b| a.vmag.total_cmp(&b.vmag));

    let mut out = Vec::with_capacity(stars.len() * 8);
    for s in &stars {
        out.extend_from_slice(&((round(s.ra / 360.0 * 65536.0) as u32 % 65536) as u16).to_le_bytes());
        out.extend_from_slice(&(round(s.dec / 90.0 * 32767.0) as i16).to_le_bytes());
        out.extend_from_slice(&(round(s.vmag * 1000.0) as i16).to_le_bytes());
        out.extend_from_slice(&(round(s.bv * 1000.0) as i16).to_le_bytes());
    }
    let path = root().join(OUT);
    std::fs::write(&path, &out).map_err(|e| format!("{}: {e}", path.display()))?;
    let (first, last) = (stars.first().ok_or("no stars")?, stars.last().ok_or("no stars")?);
    println!("[bake-stars] {} stars, magnitude {} to {} -> {}", stars.len(), first.vmag, last.vmag, path.display());
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
