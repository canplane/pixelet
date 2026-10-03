// pxl-geo-bake/page/bake.rs

use pxl_spec::bake::quadkey::PAGE_KEY_DEPTH;
use std::path::PathBuf;

use std::path::Path;

use crate::coverage::{Coverage, Overture};
use crate::page::walk::{self, Args, Page, Target};
use crate::terrain::leaf::land_pages;
use crate::util::{fmt_bytes, git_sha, measure_output, now_iso8601, refuse_mixed, CONTENT_ENCODING};

pub struct Bake<'a> {
    pub kind: &'a str,
    pub out: PathBuf,
    pub every: u64,
    /// Levels above a page one object covers; zero is one object a page.
    pub group_shamt: u32,
    pub content_type: &'a str,
    pub format: Option<&'a str>,
    /// Opened for what the bake reads, the land list's classification too.
    pub source: &'a Overture,
}

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

    let land = land_pages(&args.land_cache, &Coverage::of_source()?, Some(&b.source.release))?;
    println!("[bake/{}] {} land pages from {}", b.kind, land.len(), args.land_cache.display());
    let mut targets = walk::targets_of(&land, b.group_shamt);
    if let Some(n) = args.limit {
        targets.truncate(n);
    }

    let walked = walk::walk(&args, b.kind, PAGE_KEY_DEPTH - b.group_shamt, b.every, &targets, connect, one)?;
    let t = &walked.tally;
    println!("[bake/{}] {} items across {} non-empty pages, {}", b.kind, t.items, t.baked, fmt_bytes(t.bytes as f64));

    let (objects, bytes) = measure_output(&args.out).map_err(|e| e.to_string())?;
    let mut manifest = serde_json::json!({
        "version": args.out.file_name().map(|n| n.to_string_lossy().to_string()),
        "updated_at": now_iso8601(),
        "git_sha": git_sha(),
        "land_pages": land.len(),

        "objects": objects,
        "bytes": bytes,

        // This invocation only, so a resumed run's zero means "nothing to do".
        "run": { "baked": t.baked, "empty": t.empty, "skipped": t.skipped, "failed": t.failed, "items": t.items, "bytes": t.bytes },

        "content_type": b.content_type,
        "content_encoding": CONTENT_ENCODING,

        "source": b.source.manifest(),
    });
    if let Some(format) = b.format {
        manifest["format"] = serde_json::Value::String(format.to_string());
    }

    let path = args.out.join("../manifest.json");
    std::fs::write(&path, serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    println!("[bake/{}] manifest -> {}", b.kind, path.display());
    Ok(())
}
