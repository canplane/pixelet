// pxl-geo-bake/tile/bake.rs

use std::path::{Path, PathBuf};

use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

use crate::coverage::{holdings, Coverage, Overture};
use crate::terrain::leaf::land_tiles;
use crate::tile::walk::{self, Args, Target, Tile};
use crate::util::{fmt_bytes, git_sha, measure_output, refuse_mixed, write_manifest};

pub struct Bake<'a> {
    pub kind: &'a str,
    pub out: PathBuf,
    pub every: u64,
    /// Zooms coarser than a tile one object covers; zero is one object a tile.
    pub group_shamt: u32,
    pub content_type: &'a str,
    pub format: Option<&'a str>,
    /// Opened for what the bake reads, the land list's classification too.
    pub source: &'a Overture,
    pub reads: &'a [&'a [&'a str]],
}

pub fn bake<T>(
    b: &Bake,
    prelude: impl FnOnce(&Path) -> Result<(), String>,
    connect: impl Fn() -> Result<T, String> + Sync,
    one: impl Fn(&mut T, &Target) -> Result<Tile, String> + Sync,
) -> Result<(), String> {
    let built = git_sha();
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

    let ground = crate::terrain::leaf::ground_source(&mut b.source.connect()?)?;
    let land = land_tiles(&args.land_cache, &Coverage::of_source()?, Some(&ground))?;
    println!("[bake/{}] {} land tiles from {}", b.kind, land.len(), args.land_cache.display());
    let mut targets = walk::targets_of(&land, b.group_shamt);
    if let Some(n) = args.limit {
        targets.truncate(n);
    }

    let t = &walk::walk(&args, b.kind, LEAF_ZOOM - b.group_shamt as Zoom, b.every, &targets, connect, one)?;
    println!("[bake/{}] {} items across {} non-empty tiles, {}", b.kind, t.items, t.baked, fmt_bytes(t.bytes as f64));

    let (objects, bytes) = measure_output(&args.out).map_err(|e| e.to_string())?;
    let mut fields = serde_json::json!({
        "land_tiles": land.len(),
        "objects": objects,
        "bytes": bytes,
        // This invocation only, so a resumed run's zero means "nothing to do".
        "run": { "baked": t.baked, "empty": t.empty, "skipped": t.skipped, "failed": t.failed, "items": t.items, "bytes": t.bytes },
        "content_type": b.content_type,
        "release": b.source.release,
        "source": holdings(&mut b.source.connect()?, b.reads)?,
    });
    if let Some(format) = b.format {
        fields["format"] = serde_json::Value::String(format.to_string());
    }
    write_manifest(&args.out, built.as_deref(), fields)
}
