// pxl-geo-bake/bin/bake_poi_tiles.rs

//! The POI pyramid into `out/poi/<version>`.
//!
//!   --out <dir>          where the version goes (default out/poi/v<format>.<UTC day>.<n>, the day's next)
//!   --concurrency <n>    parallel folds (default 8)

use std::fs;
use std::path::PathBuf;

use pxl_geo_bake::cli::Flags;
use pxl_geo_bake::coverage::{Coverage, Overture};
use pxl_geo_bake::poi::{self, leaves};
use pxl_geo_bake::pyramid::{build, prune, write_manifest, Layer, PageSet};
use pxl_geo_bake::util::{git_sha, next_version, pack, refuse_mixed, Level};
use pxl_spec::bake::poi_page::PoiPage;
use pxl_spec::bake::published::FORMAT;
use pxl_spec::space::pyramid::{Page, LEAF_Z};

fn run() -> Result<(), String> {
    let built = git_sha();
    let mut out = None;
    let mut concurrency = 8;
    let mut flags = Flags::from_env();
    while let Some(flag) = flags.arg() {
        match flag.as_str() {
            "--out" => out = Some(PathBuf::from(flags.value()?)),
            "--concurrency" => concurrency = flags.parse()?,
            other => return Err(format!("unknown flag: {other}")),
        }
    }
    let (source, mut client) = Overture::reading(&[&poi::READS])?;
    let out = out.unwrap_or_else(|| next_version("./out/poi", FORMAT.poi));
    let set = PageSet::new(&out);
    if set.root.exists() {
        refuse_mixed(&set.root)?;
        fs::remove_dir_all(&set.root).map_err(|e| e.to_string())?;
    }
    let pages = leaves(&mut client, &Coverage::of_source()?, None)?;
    for (page, poi) in &pages {
        set.write(Page::new(LEAF_Z, *page), &pack(&poi.encode()?, Level::Stored).map_err(|e| e.to_string())?)?;
    }
    println!("[bake/poi] z{LEAF_Z}: {} pages", pages.len());
    let top = build(&set, &PoiPage::default(), concurrency)?;
    println!("[bake/poi] {} pages their parent settles let go", prune::<PoiPage>(&set, top)?);
    write_manifest(&set, top, built.as_deref(), serde_json::json!({ "release": source.release, "source": source.manifest() }))?;
    println!("[bake/poi] complete to z{top}: {}", out.display());
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
