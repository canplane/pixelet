// pxl-tool/download/imagery.rs

//! The imagery as NASA gives it, each file `infra/imagery/source.json`
//! names fetched into `infra/imagery/raw/`, for `make bake-imagery`.
//!
//!   make download-imagery

use crate::{fetch, root};

pub fn run() -> Result<(), String> {
    let path = root().join("infra/imagery/source.json");
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
    let raw = root().join("infra/imagery/raw");
    for url in &urls {
        fetch::download(url, &raw)?;
    }
    println!("[imagery] {} files in {}", urls.len(), raw.display());
    Ok(())
}
