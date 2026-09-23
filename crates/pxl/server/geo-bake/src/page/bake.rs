// pxl-geo-bake/page/bake.rs

//! The rest of a page bake: the walk, then the manifest beside the version
//! directory.
//!
//! The POI and reverse-geocode bakes differ in their provider, their encoding
//! and the tables they read — this is everything else, which is most of both.

use std::path::PathBuf;

use std::path::Path;

use crate::page::catalog;
use crate::page::walk::{self, Args, Page, Target};
use crate::util::{fmt_bytes, git_sha, measure_output, now_iso8601, refuse_mixed, CONTENT_ENCODING};

/// What a page bake is, apart from the pages it writes: where they go, how
/// they are served, and which tables they were read from.
pub struct Bake<'a> {
    pub kind: &'a str,
    pub out: PathBuf,
    /// How often the walk reports progress, in pages. A POI page is a packet
    /// and a geocode page is one label, so they are not the same cost.
    pub every: u64,
    /// How many levels above a page one object of this bake covers. Zero is
    /// one object a page.
    pub group_shamt: u32,
    pub content_type: &'a str,
    pub format: Option<&'a str>,
    /// The `overture_catalog` rows this bake read from, named rather than
    /// taken whole: the catalog holds terrain's tables too.
    pub sources: &'a [&'a str],
}

/// Walks the land pages, writes the objects, then writes the manifest beside
/// the version directory.
///
/// The two page bakes differ in their provider, their encoding and their
/// tables, and in nothing else — this is the rest of both.
pub fn bake<T>(
    b: &Bake,
    prelude: impl FnOnce(&Path) -> Result<(), String>,
    connect: impl Fn() -> Result<T, String> + Sync,
    one: impl Fn(&mut T, &Target) -> Result<Page, String> + Sync,
) -> Result<(), String> {
    let args = Args::parse(b.out.clone())?;
    println!(
        "[bake/{}] out={} land_cache={} concurrency={} limit={} force={}",
        b.kind,
        args.out.display(),
        args.land_cache.display(),
        args.concurrency,
        args.limit.map(|n| n.to_string()).unwrap_or_else(|| "none".into()),
        args.force
    );

    std::fs::create_dir_all(&args.out).map_err(|e| e.to_string())?;
    if !args.force {
        refuse_mixed(&args.out)?;
    }
    prelude(&args.out)?;

    let land = walk::land_pages(&args.land_cache)?;
    println!("[bake/{}] {} land pages from {}", b.kind, land.len(), args.land_cache.display());
    let mut targets = walk::targets_of(&land, b.group_shamt);
    if let Some(n) = args.limit {
        targets.truncate(n);
    }

    let walked = walk::walk(&args, b.kind, b.every, &targets, connect, one)?;
    let t = &walked.tally;
    println!("[bake/{}] {} items across {} non-empty pages, {}", b.kind, t.items, t.baked, fmt_bytes(t.bytes as f64));

    let url = std::env::var("DB_GEO_URL").map_err(|_| "env:DB_GEO_URL:missing")?;
    let mut client = postgres::Client::connect(&url, postgres::NoTls).map_err(|e| e.to_string())?;
    let (objects, bytes) = measure_output(&args.out).map_err(|e| e.to_string())?;
    let mut manifest = serde_json::json!({
        "version": args.out.file_name().map(|n| n.to_string_lossy().to_string()),
        "updated_at": now_iso8601(),
        "git_sha": git_sha(),
        "land_pages": land.len(),

        "objects": objects,
        "bytes": bytes,

        // This invocation only — a resumed run reports what it re-did, so a
        // zero here means "nothing to do" rather than "nothing wrong".
        "run": { "baked": t.baked, "empty": t.empty, "skipped": t.skipped, "failed": t.failed, "items": t.items, "bytes": t.bytes },

        "content_type": b.content_type,
        "content_encoding": CONTENT_ENCODING,

        "source": catalog::sources(&mut client, b.sources),
    });
    if let Some(format) = b.format {
        manifest["format"] = serde_json::Value::String(format.to_string());
    }

    let path = args.out.join("../manifest.json");
    std::fs::write(&path, serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    println!("[bake/{}] manifest -> {}", b.kind, path.display());
    Ok(())
}
