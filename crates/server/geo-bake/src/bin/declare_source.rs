// pxl-geo-bake/bin/declare_source.rs

//! Writes the published declaration (`source`) out of published.json's
//! `SOURCE` to stdout, for script/publish.ts to upload.
//!
//!   declare_source [spec/spec/bake/published.json]

use std::process::ExitCode;

use pxl_geo_bake::source::published;

fn run() -> Result<(), String> {
    let path = std::env::args().nth(1).unwrap_or_else(|| "spec/spec/bake/published.json".into());
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
    let spec: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    println!("{}", published(&spec)?);
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[declare_source] {e}");
            ExitCode::FAILURE
        }
    }
}
