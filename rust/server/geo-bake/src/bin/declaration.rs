// pxl-geo-bake/bin/declaration.rs

//! Writes the published declaration (`source`) out of a record of what the
//! bucket serves, read from stdin (`source::recorded`), to stdout, for
//! script/publish.py to upload.

use std::io::Read;
use std::process::ExitCode;

use pxl_geo_bake::cli;
use pxl_geo_bake::source::decl_of;

fn run() -> Result<(), String> {
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .map_err(|e| format!("stdin: {e}"))?;
    let rec: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("stdin: {e}"))?;
    println!("{}", decl_of(&rec)?);
    Ok(())
}

fn main() -> ExitCode {
    cli::main("declaration", run)
}
