// pxl-geo-bake/bin/declare_source.rs

//! Writes the published declaration (`source`) out of a record of what the
//! bucket serves, read from stdin (`source::recorded`), to stdout, for
//! script/publish.ts to upload.

use std::io::Read;
use std::process::ExitCode;

use pxl_geo_bake::source::recorded;

fn run() -> Result<(), String> {
    let mut text = String::new();
    std::io::stdin().read_to_string(&mut text).map_err(|e| format!("stdin: {e}"))?;
    let record: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("stdin: {e}"))?;
    println!("{}", recorded(&record)?);
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
