// pxl-geo-bake/bin/fill_digest.rs

//! Every brick of a terrain set filled and its ways' tree built as a client
//! fills and builds them, in the bake's own pass (pxl-spec
//! `terrain::fill_fit`), each one line of what a reader reads of it: its
//! levels' words, the child groups its tree took and holds, and its tree under
//! each of its own nodes along z as a reader steps down it
//! (pxl-engine `way::tree::cursor`), each node's holding, folding and
//! word, no child group's id. Two checkouts' lines over one set are a proof that
//! a change left what the fill makes as it was.
//!
//! The lines are a function of the sources that make them and the set they
//! read, so they are kept once a pair, under the digest of both, where every
//! checkout, export or worktree finds them.
//!
//!   cargo run --profile server -p pxl-geo-bake --bin fill_digest -- [set]
//!       the lines of `set` (default out/terrain), made or found: their path
//!   cargo run --profile server -p pxl-geo-bake --bin fill_digest -- --diff <a> <b>
//!       the bricks two such files differ in, and what each holds

use std::collections::BTreeMap;
use std::hash::Hasher;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use fnv::FnvHasher;
use pxl_engine::object::{self, NodeSlot, Table};
use pxl_engine::way::tree::{self, Read};
use pxl_engine::{clipmap, fill};
use pxl_geo_bake::pyramid::{Base, TileSet};
use pxl_geo_bake::util::code_digest;
use pxl_spec::bake::tile::terrain::{self, FilledBrick, TerrainTile};
use pxl_spec::directory;
use pxl_spec::space::coord::zoom::Zoom;

/// The set a proof reads when none is named: the finished one, which no
/// bake writes while it is read (out/terrain.stage is).
const SET: &str = "out/terrain";

/// Where the lines are kept, a file a digest.
fn lines_dir() -> PathBuf {
    directory::tmp_dir("fill")
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

fn key(set: &Path) -> Result<String, String> {
    let mut data = FnvHasher::default();
    digest_set(set, &mut data)?;
    Ok(format!("{:016x}{:016x}", code_digest()?, data.finish()))
}

/// The node `at` of `zoom` and every node under it a reader steps to, into
/// `h`: its zoom, whether it holds, whether folded, and its word.
fn walk(
    l: &impl Table,
    slot: NodeSlot,
    (zoom, cell_zoom): (Zoom, Zoom),
    at: [i32; 3],
    h: &mut FnvHasher,
) {
    h.write_u32(zoom);
    if !slot.holds() {
        h.write_u32(0);
        return;
    }
    h.write_u32(1 + u32::from(slot.is_folded()));
    h.write_u32(l.data(slot.slot()));
    if slot.is_folded() && zoom < cell_zoom {
        for k in 0..8 {
            let c = [
                2 * at[0] + (k & 1),
                2 * at[1] + (k >> 1 & 1),
                2 * at[2] + (k >> 2),
            ];
            walk(
                l,
                object::child(l, slot, zoom + 1, c),
                (zoom + 1, cell_zoom),
                c,
                h,
            );
        }
    }
}

/// A brick's line: its levels' words, the child groups its build took and
/// holds, and its tree under each of its own nodes along z its ways meet
/// and two past either end.
fn line(name: &str, k: u32, b: &FilledBrick<'_>) -> String {
    let at = clipmap::BOUNDS_AT as usize;
    let mut levels = FnvHasher::default();
    for w in &b.column[at..at + clipmap::BOUND_WORDS as usize] {
        levels.write_u32(*w);
    }
    let (z0, nodes) = fill::root_span(b.column, 0, b.cell_zoom);
    let top = b.cell_zoom - clipmap::BRICK_SHIFT;
    let (l, mut sub) = (Read(b.pool), FnvHasher::default());
    if nodes > 0 {
        for z in z0 - 2..z0 + nodes as i32 + 2 {
            let slot = tree::cursor(&l, b.root, b.cell_zoom, top, [0, 0, z]);
            walk(&l, slot, (top, b.cell_zoom), [0, 0, z], &mut sub);
        }
    }
    format!(
        "{name} {k} levels={:016x} taken={} held={} sub={:016x}",
        levels.finish(),
        b.peak_group_cnt,
        b.group_cnt,
        sub.finish()
    )
}

/// The lines of every brick of the set's tiles with ways, sorted.
fn lines(set: &TileSet, worker_cnt: usize) -> Result<Vec<String>, String> {
    let tiles: Vec<_> = set.by_zoom()?.into_values().flatten().collect();
    let parts: Vec<Result<Vec<String>, String>> = std::thread::scope(|s| {
        let runs: Vec<_> = (0..worker_cnt)
            .map(|w| {
                let part: Vec<_> = tiles.iter().skip(w).step_by(worker_cnt).copied().collect();
                s.spawn(move || {
                    let mut out = Vec::new();
                    for p in part {
                        let Some(raw) = set.content(p)? else {
                            continue;
                        };
                        let tile = TerrainTile::decode(&raw)?;
                        let name = p.key();
                        terrain::fill_fit(p, &tile, |k, b| out.push(line(&name, k, b)))
                            .map_err(|(at, why)| format!("{name}: brick {at:?} {why:?}"))?;
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

/// The lines of `set`, made where none of this digest are kept:
/// their path.
fn lines_path(set: &Path, worker_cnt: usize) -> Result<PathBuf, String> {
    let dir = lines_dir();
    let path = dir.join(key(set)?);
    if path.is_file() {
        return Ok(path);
    }
    let text = lines(&TileSet::new(set), worker_cnt)?.join("\n") + "\n";
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let part = path.with_extension(format!("{}.part", std::process::id()));
    std::fs::write(&part, text)
        .and_then(|()| std::fs::rename(&part, &path))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// A file's lines by brick, each its fields.
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

/// The bricks `a` and `b` both hold that differ, a field at a time, and
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
        "{both} bricks in both, {differ} differ; {} in the first alone, {} in the second",
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
    let worker_cnt = std::thread::available_parallelism().map_or(4, |n| n.get());
    println!("{}", lines_path(Path::new(set), worker_cnt)?.display());
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
