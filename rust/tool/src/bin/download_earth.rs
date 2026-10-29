// pxl-tool/bin/download_earth.rs

//! The Earth's picture as NASA gives it, each file `infra/earth/source.json`
//! names fetched into `infra/earth/raw/`, for `make bake-earth`.
//!
//!   make download-earth

use std::process::ExitCode;

use pxl_tool::fetch;
use pxl_tool::root;

fn run() -> Result<(), String> {
    let path = root().join("infra/earth/source.json");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let src: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let night = src["night"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|row| row.as_array().into_iter().flatten());
    let urls: Vec<&str> = std::iter::once(&src["day"])
        .chain(night)
        .filter_map(serde_json::Value::as_str)
        .collect();
    let raw = root().join("infra/earth/raw");
    for url in &urls {
        fetch::fetch_kept(url, &raw)?;
    }
    println!("[earth] {} files in {}", urls.len(), raw.display());
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[earth] {e}");
            ExitCode::FAILURE
        }
    }
}
