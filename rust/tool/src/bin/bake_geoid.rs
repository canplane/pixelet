// pxl-tool/bin/bake_geoid.rs

//! EGM2008's geoid heights over the WGS84 ellipsoid, every `STEP_DEG`, from
//! GeographicLib's 5′ grid of it, into spec/spec/space/geoid.json. A GPS
//! height is the ellipsoid's and the terrain's is EGM2008's (GEDTM30, EPSG:3855);
//! the geoid is between them.
//!
//!   make bake-geoid ARGS=egm2008-5.pgm
//!
//! The grid is egm2008-5.tar.bz2 from
//! https://sourceforge.net/projects/geographiclib/files/geoids-distrib/,
//! a PGM whose header gives `Offset` and `Scale` (GeographicLib's geoid.html).

use std::process::ExitCode;

use pxl_tool::root;

const OUT: &str = "spec/spec/space/geoid.json";
/// Coarse beside GPS's vertical error: over Korea bilinear between these
/// misses the 5′ grid by 3.9 m at most, 1.4 m RMS, against a geoid there
/// from 15 m to 33 m. The world's worst and RMS are measured and written
/// beside it.
const STEP_DEG: usize = 5;

/// Rows from 90°N down, columns from 0°E east, in meters.
struct Grid {
    width: usize,
    height: usize,
    m: Vec<f64>,
}

impl Grid {
    /// At whole multiples of the grid's spacing, X wrapping.
    fn at(&self, row: usize, col: usize) -> f64 {
        self.m[row * self.width + col % self.width]
    }

    /// Bilinear at `lat`, `lon` in degrees, `lon` from 0 east.
    fn bilinear(&self, lat: f64, lon: f64) -> f64 {
        let per = (self.height - 1) as f64 / 180.0;
        let (r, c) = ((90.0 - lat) * per, lon.rem_euclid(360.0) * per);
        let (r0, c0) = (r.floor() as usize, c.floor() as usize);
        let (fr, fc) = (r - r0 as f64, c - c0 as f64);
        let r1 = (r0 + 1).min(self.height - 1);
        let top = self.at(r0, c0) * (1.0 - fc) + self.at(r0, c0 + 1) * fc;
        let bottom = self.at(r1, c0) * (1.0 - fc) + self.at(r1, c0 + 1) * fc;
        top * (1.0 - fr) + bottom * fr
    }
}

fn read(path: &str) -> Result<Grid, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let (mut off, mut scale) = (None, None);
    let mut fields: Vec<usize> = Vec::new();
    let mut at = 0;
    // Header: "P5", comments, width, height, maxval, then one whitespace byte.
    while fields.len() < 4 {
        let end = at
            + bytes[at..]
                .iter()
                .position(|&b| b == b'\n')
                .ok_or("a header past the file's end")?;
        let line = std::str::from_utf8(&bytes[at..end])
            .map_err(|e| e.to_string())?
            .trim()
            .to_string();
        at = end + 1;
        if let Some(c) = line.strip_prefix('#') {
            let mut words = c.split_whitespace();
            match (words.next(), words.next()) {
                (Some("Offset"), Some(v)) => off = v.parse::<f64>().ok(),
                (Some("Scale"), Some(v)) => scale = v.parse::<f64>().ok(),
                _ => {}
            }
            continue;
        }
        for w in line.split_whitespace() {
            fields.push(if w == "P5" {
                5
            } else {
                w.parse().map_err(|e| format!("{w:?}: {e}"))?
            });
        }
    }
    let (off, scale) = (off.ok_or("no Offset")?, scale.ok_or("no Scale")?);
    let [_, width, height, maxval] = [fields[0], fields[1], fields[2], fields[3]];
    if maxval != 65535 {
        return Err(format!("maxval {maxval}: two bytes a sample expected"));
    }
    let body = bytes
        .get(at..at + width * height * 2)
        .ok_or("samples past the file's end")?;
    let m = body
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&b| off + scale * f64::from(u16::from_be_bytes(b)))
        .collect();
    Ok(Grid { width, height, m })
}

/// Half to even, as everything rounds; the tool links no xpute.
fn round(x: f64) -> f64 {
    x.round_ties_even()
}

fn run() -> Result<(), String> {
    // the grid's path is the checkout's, as a bake's paths are (pxl-geo-bake cli.rs `enter`)
    std::env::set_current_dir(root()).map_err(|e| format!("{}: {e}", root().display()))?;
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: bake_geoid <egm2008-5.pgm>")?;
    let fine = read(&path)?;
    let per = (fine.height - 1) / 180;
    let (rows, cols) = (180 / STEP_DEG + 1, 360 / STEP_DEG);
    // Decimeters: finer than that is lost under GPS's vertical error.
    let m: Vec<f64> = (0..rows)
        .flat_map(|r| (0..cols).map(move |c| (r, c)))
        .map(|(r, c)| round(fine.at(r * STEP_DEG * per, c * STEP_DEG * per) * 10.0) / 10.0)
        .collect();
    let coarse = Grid {
        width: cols,
        height: rows,
        m: m.clone(),
    };
    let (mut worst, mut bricks): (f64, f64) = (0.0, 0.0);
    for r in 0..fine.height {
        for c in 0..fine.width {
            let (lat, lon) = (90.0 - r as f64 / per as f64, c as f64 / per as f64);
            let miss = (coarse.bilinear(lat, lon) - fine.at(r, c)).abs();
            (worst, bricks) = (worst.max(miss), bricks + miss * miss);
        }
    }
    let rms = (bricks / (fine.width * fine.height) as f64).sqrt();
    let vals: Vec<String> = m.iter().map(|v| format!("{v}")).collect();
    let json = format!(
        "{{\n  \"$doc\": [\n    \"EGM2008's geoid height over the WGS84 ellipsoid, in meters to a tenth: what a GPS height,\",\n    \"the ellipsoid's, is over the terrain's, EGM2008's (GEDTM30, EPSG:3855). From GeographicLib's\",\n    \"5' grid (egm2008-5) by pxl-tool/bin/bake_geoid.rs; bilinear between these misses it by at\",\n    \"most {worst:.1} m, {rms:.1} m RMS.\"\n  ],\n  \"GEOID_STEP_DEG\": {{\n    \"$val\": {STEP_DEG},\n    \"$doc\": [\"The grid's spacing; rows from 90N down, {cols} columns from 0E east.\"]\n  }},\n  \"GEOID_M\": {{\n    \"$type\": \"f32\",\n    \"$val\": [\n      {}\n    ]\n  }}\n}}\n",
        // One a line.
        vals.join(",\n      ")
    );
    std::fs::write(root().join(OUT), json).map_err(|e| format!("{OUT}: {e}"))?;
    println!("[bake_geoid] {} points every {STEP_DEG}°, bilinear off the 5' grid by at most {worst:.2} m, {rms:.2} m RMS", m.len());
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[bake_geoid] {e}");
            ExitCode::FAILURE
        }
    }
}
