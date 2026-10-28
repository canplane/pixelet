// pxl-geo-bake/bin/fill_digest.rs

//! Every square of a terrain set filled and its ways' tree built as a client
//! fills and builds them, in the bake's own pass (pxl-spec
//! `terrain::fill_fit`), each one line of what a reader reads of it: its
//! levels' words, the families its tree took and holds, and its tree under
//! each of its own nodes along z as a reader steps down it
//! (pxl-spec-core `way::tree::cursor`), each node's holding, folding and
//! word, no family's id. Two checkouts' lines over one set are a proof that
//! a change left what the fill makes as it was.
//!
//! The lines are a function of the sources that make them and the set they
//! read, so they are kept once a pair, under the digest of both, where every
//! checkout, export or worktree finds them, as the shaders are (pxl-tool
//! bin/shader_wgsl.rs): a proof's side made once is made for all.
//!
//!   cargo run --profile server -p pxl-geo-bake --bin fill_digest -- [set]
//!       the lines of `set` (default out/terrain), made or found: their path
//!   cargo run --profile server -p pxl-geo-bake --bin fill_digest -- --diff <a> <b>
//!       the squares two such files differ in, and what each holds

use std::collections::BTreeMap;
use std::hash::Hasher;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use fnv::FnvHasher;
use pxl_geo_bake::pyramid::{Base, TileSet};
use pxl_spec::bake::tile::terrain::{self, Built, TerrainTile};
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec_core::overwrite::{self, Held, Lanes};
use pxl_spec_core::way::tree::{self, Read};
use pxl_spec_core::{fill, window};

/// The set a proof reads when none is named: the finished one, which no
/// bake writes while it is read (out/terrain.stage is).
const SET: &str = "out/terrain";

/// What makes the lines, read off the disk of the checkout the bin was
/// built in: the crates it compiles of this tree, and the workspace's
/// manifest and lock.
const SOURCES: [&str; 6] = [
    "rust/spec/core",
    "rust/spec",
    "rust/server/geo-bake",
    "xpute/rust/kit",
    "rust/Cargo.toml",
    "rust/Cargo.lock",
];

/// The checkout the bin was built in.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// Where the machine keeps the lines, a file a digest.
fn kept_home() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or("no $HOME")?;
    let caches = if cfg!(target_os = "macos") {
        PathBuf::from(home).join("Library/Caches")
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(home).join(".cache"))
    };
    Ok(caches.join("pixelet/fill"))
}

/// Every file under `path`, a directory's in name order, `target`
/// directories left out, each its path and bytes into `h`.
fn digest_tree(path: &Path, h: &mut FnvHasher) -> Result<(), String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.is_file() {
        h.write(path.to_string_lossy().as_bytes());
        h.write(&std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?);
        return Ok(());
    }
    let mut names: Vec<_> = std::fs::read_dir(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .filter_map(|e| e.ok().map(|e| e.file_name()))
        .filter(|n| n != "target")
        .collect();
    names.sort();
    for n in names {
        digest_tree(&path.join(n), h)?;
    }
    Ok(())
}

/// The set's tiles as listed, each its name, length and time: what the
/// lines read of it.
fn digest_set(set: &Path, h: &mut FnvHasher) -> Result<(), String> {
    let mut files: Vec<_> = std::fs::read_dir(set)
        .map_err(|e| format!("{}: {e}", set.display()))?
        .filter_map(|e| e.ok())
        .collect();
    files.sort_by_key(|e| e.file_name());
    for e in files {
        let meta = e.metadata().map_err(|e| e.to_string())?;
        let time = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());
        h.write(e.file_name().as_encoded_bytes());
        h.write_u64(meta.len());
        h.write_u64(time);
    }
    Ok(())
}

/// The digest of the sources at `root` and the set: the lines' name.
fn key(root: &Path, set: &Path) -> Result<String, String> {
    let (mut sources, mut data) = (FnvHasher::default(), FnvHasher::default());
    for s in SOURCES {
        digest_tree(&root.join(s), &mut sources)?;
    }
    digest_set(set, &mut data)?;
    Ok(format!("{:016x}{:016x}", sources.finish(), data.finish()))
}

/// The node `at` of `zoom` and every node under it a reader steps to, into
/// `h`: its zoom, whether it holds, whether folded, and its word.
fn walk(l: &impl Lanes, held: Held, (zoom, cell): (Zoom, Zoom), at: [i32; 3], h: &mut FnvHasher) {
    h.write_u32(zoom);
    if !held.holds() {
        h.write_u32(0);
        return;
    }
    h.write_u32(1 + u32::from(held.folded()));
    h.write_u32(l.data(held.slot()));
    if held.folded() && zoom < cell {
        for k in 0..8 {
            let c = [
                2 * at[0] + (k & 1),
                2 * at[1] + (k >> 1 & 1),
                2 * at[2] + (k >> 2),
            ];
            walk(
                l,
                overwrite::child(l, held, zoom + 1, c),
                (zoom + 1, cell),
                c,
                h,
            );
        }
    }
}

/// A square's line: its levels' words, the families its build took and
/// holds, and its tree under each of its own nodes along z its ways meet
/// and two past either end.
fn line(name: &str, k: u32, b: &Built<'_>) -> String {
    let at = window::LEVELS_AT as usize;
    let mut levels = FnvHasher::default();
    for w in &b.lane[at..at + window::LEVEL_WORDS as usize] {
        levels.write_u32(*w);
    }
    let (z0, nodes) = fill::root_span(b.lane, 0, b.cell);
    let top = b.cell - window::SQUARE_SHIFT;
    let (l, mut sub) = (Read(b.pool), FnvHasher::default());
    if nodes > 0 {
        for z in z0 - 2..z0 + nodes as i32 + 2 {
            let held = tree::cursor(&l, b.root, b.cell, top, [0, 0, z]);
            walk(&l, held, (top, b.cell), [0, 0, z], &mut sub);
        }
    }
    format!(
        "{name} {k} levels={:016x} taken={} held={} sub={:016x}",
        levels.finish(),
        b.taken,
        b.held,
        sub.finish()
    )
}

/// The lines of every square of the set's tiles with ways, sorted.
fn lines(set: &TileSet, workers: usize) -> Result<Vec<String>, String> {
    let tiles: Vec<_> = set.by_zoom()?.into_values().flatten().collect();
    let parts: Vec<Result<Vec<String>, String>> = std::thread::scope(|s| {
        let runs: Vec<_> = (0..workers)
            .map(|w| {
                let part: Vec<_> = tiles.iter().skip(w).step_by(workers).copied().collect();
                s.spawn(move || {
                    let mut out = Vec::new();
                    for p in part {
                        let Some(raw) = set.content(p)? else {
                            continue;
                        };
                        let tile = TerrainTile::decode(&raw)?;
                        let name = p.name();
                        terrain::fill_fit(p, &tile, |k, b| out.push(line(&name, k, b)))
                            .map_err(|(at, why)| format!("{name}: square {at:?} {why:?}"))?;
                    }
                    Ok(out)
                })
            })
            .collect();
        runs.into_iter()
            .map(|r| r.join().unwrap_or_else(|_| Err("a worker panicked".into())))
            .collect()
    });
    let mut all = Vec::new();
    for part in parts {
        all.extend(part?);
    }
    all.sort_unstable();
    Ok(all)
}

/// The lines of `set`, made where the machine keeps none of this digest:
/// their path.
fn made(set: &Path, workers: usize) -> Result<PathBuf, String> {
    let home = kept_home()?;
    let path = home.join(key(&root(), set)?);
    if path.is_file() {
        return Ok(path);
    }
    let text = lines(&TileSet::new(set), workers)?.join("\n") + "\n";
    std::fs::create_dir_all(&home).map_err(|e| format!("{}: {e}", home.display()))?;
    let part = path.with_extension(format!("{}.part", std::process::id()));
    std::fs::write(&part, text)
        .and_then(|()| std::fs::rename(&part, &path))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// A file's lines by square, each its fields.
fn read(path: &str) -> Result<BTreeMap<String, Vec<String>>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    Ok(text
        .lines()
        .filter_map(|l| {
            let mut f = l.split(' ');
            let at = format!("{} {}", f.next()?, f.next()?);
            Some((at, f.map(str::to_owned).collect()))
        })
        .collect())
}

/// The squares `a` and `b` both hold that differ, a field at a time, and
/// how many each holds alone and in all.
fn diff(a: &str, b: &str) -> Result<bool, String> {
    let (a, b) = (read(a)?, read(b)?);
    let (mut differ, mut both) = (0, 0);
    for (at, fa) in &a {
        let Some(fb) = b.get(at) else { continue };
        both += 1;
        if fa != fb {
            if differ < 10 {
                println!("{at}: {} | {}", fa.join(" "), fb.join(" "));
            }
            differ += 1;
        }
    }
    println!(
        "{both} squares in both, {differ} differ; {} in the first alone, {} in the second",
        a.len() - both,
        b.len() - both
    );
    Ok(differ == 0)
}

fn run() -> Result<bool, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--diff") {
        let [_, a, b] = args.as_slice() else {
            return Err("--diff <a> <b>".into());
        };
        return diff(a, b);
    }
    let set = args.first().map_or(SET, String::as_str);
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get());
    println!("{}", made(Path::new(set), workers)?.display());
    Ok(true)
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("[fill_digest] {e}");
            ExitCode::FAILURE
        }
    }
}
