// pxl-geo-bake/bin/bake_poi_tiles.rs

//! The POI pyramid: every leaf page Overture's places fill, then the levels
//! above folded by counting (pxl-geo-bake pyramid.rs), into
//! `out/poi/<version>` as the bucket holds it once renamed.
//!
//!   --out <dir>          where the version goes (default out/poi/<published>)
//!   --concurrency <n>    parallel folds (default 8)

use std::fs;
use std::path::PathBuf;

use pxl_geo_bake::poi::leaves;
use pxl_geo_bake::pyramid::{build, Layer, PageSet};
use pxl_geo_bake::util::{pack, refuse_mixed, Level};
use pxl_spec::bake::poi_page::PoiPage;
use pxl_spec::bake::published::PUBLISHED_VERSION;
use pxl_spec::space::pyramid::LevelPage;

fn run() -> Result<(), String> {
    let mut out = PathBuf::from(format!("out/poi/{}", PUBLISHED_VERSION.poi));
    let mut concurrency = 8;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or("missing flag value")?;
        match flag.as_str() {
            "--out" => out = PathBuf::from(value),
            "--concurrency" => concurrency = value.parse().map_err(|_| "bad --concurrency")?,
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    let set = PageSet::new(&out);
    if set.root.exists() {
        refuse_mixed(&set.root)?;
        fs::remove_dir_all(&set.root).map_err(|e| e.to_string())?;
    }
    let url = std::env::var("DB_GEO_URL").map_err(|_| "DB_GEO_URL required")?;
    let mut client = postgres::Client::connect(&url, postgres::NoTls).map_err(|e| e.to_string())?;
    let pages = leaves(&mut client, None)?;
    for (page, poi) in &pages {
        set.write(LevelPage::new(0, *page), &pack(&poi.encode()?, Level::Stored).map_err(|e| e.to_string())?)?;
    }
    println!("[bake/poi] level 0: {} pages", pages.len());
    let top = build(&set, &PoiPage::default(), concurrency)?;
    println!("[bake/poi] complete to level {top}: {}", out.display());
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[bake/poi] {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
