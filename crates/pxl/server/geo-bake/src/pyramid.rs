// pxl-geo-bake/pyramid.rs

//! A layer's pyramid on disk, and the fold that builds it. A version is a
//! directory a level and an object a page named by its key, as the bucket
//! holds it; the levels above the leaves are folded from the level under them
//! by the layer's one-level function (pxl-spec space/pyramid.rs), the one the
//! client runs, so the bake and the client compose the same function. The
//! offline bake writes a version whole; the dev tile server fills one a page
//! at a time as pages are wanted.
//!
//! A layer supplies its payload's codec, and the page a page it left as no
//! object reads as — open water for terrain, no places for POI. Everything
//! else here is the pyramid's.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pyramid::{Folds, LevelPage, TOP_LEVEL};

use crate::util::{git_sha, measure_output, now_iso8601, pack, unpack, write_atomic, Level, CONTENT_ENCODING};

/// A layer the bake holds as a pyramid: its page, folded by `Folds`, and the
/// bytes a page is stored as.
pub trait Layer: Folds + PartialEq + Send + Sync {
    fn encode(&self) -> Result<Vec<u8>, String>;
    fn decode(raw: &[u8]) -> Result<Self, String>;
}

/// The marker a page that does not exist leaves, so that asking again does
/// not ask the source again. Local to a page set and never uploaded.
const ABSENT: &str = "empty";

pub struct PageSet {
    pub root: PathBuf,
}

impl PageSet {
    pub fn new(root: impl Into<PathBuf>) -> PageSet {
        PageSet { root: root.into() }
    }

    pub fn level_dir(&self, level: u32) -> PathBuf {
        self.root.join(level.to_string())
    }

    pub fn path(&self, p: LevelPage) -> PathBuf {
        self.level_dir(p.level).join(p.page.to_string())
    }

    /// Stores a page's packed object.
    pub fn write(&self, p: LevelPage, packed: &[u8]) -> Result<(), String> {
        let path = self.path(p);
        fs::create_dir_all(self.level_dir(p.level)).map_err(|e| e.to_string())?;
        write_atomic(&path, packed).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Records that a page does not exist.
    pub fn write_absent(&self, p: LevelPage) -> Result<(), String> {
        fs::create_dir_all(self.level_dir(p.level)).map_err(|e| e.to_string())?;
        let path = self.path(p).with_extension(ABSENT);
        fs::write(&path, b"").map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Whether a page is settled on disk: some when it is (whether it exists),
    /// none when nothing has been decided about it yet.
    pub fn settled(&self, p: LevelPage) -> Option<bool> {
        let path = self.path(p);
        if path.exists() {
            Some(true)
        } else if path.with_extension(ABSENT).exists() {
            Some(false)
        } else {
            None
        }
    }

    /// A stored page, decoded, or none where the page does not exist.
    pub fn read<L: Layer>(&self, p: LevelPage) -> Result<Option<L>, String> {
        let path = self.path(p);
        if !path.exists() {
            return Ok(None);
        }
        let stored = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let raw = unpack(&stored).map_err(|e| format!("{}: {e}", path.display()))?;
        L::decode(&raw).map(Some).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The pages a level holds on disk, in key order.
    pub fn pages(&self, level: u32) -> Result<Vec<LevelPage>, String> {
        let dir = self.level_dir(level);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))? {
            let name = entry.map_err(|e| e.to_string())?.file_name();
            if let Some(page) = name.to_str().and_then(|n| n.parse::<IVec>().ok()) {
                out.push(LevelPage::new(level, page));
            }
        }
        out.sort_unstable();
        Ok(out)
    }
}

/// Every parent a level has: one for each child that exists, in key order.
pub fn parents(children: &[LevelPage]) -> Vec<LevelPage> {
    let mut out: Vec<LevelPage> = children.iter().map(|c| c.parent()).collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// A parent folded from the level under it, or none where it has no child: a
/// parent exists exactly where one of its four children does, which is the
/// rule `parents` walks by and the dev server faults by. `read` answers for
/// a page of the level below; one it has no page for reads as `absent`.
pub fn fold_parent<'a, L: Layer>(parent: LevelPage, read: impl Fn(LevelPage) -> Option<&'a L>, absent: &'a L) -> Option<L> {
    if parent.children().iter().all(|&k| read(k).is_none()) {
        return None;
    }
    L::fold_page(parent, |p| Some(read(p).unwrap_or(absent)))
}

/// A parent folded from the level under it as a page set holds it.
pub fn fold_stored<L: Layer>(set: &PageSet, parent: LevelPage, absent: &L) -> Result<Option<L>, String> {
    let mut read = HashMap::new();
    for p in L::sources(parent) {
        if let Some(page) = set.read::<L>(p)? {
            read.insert(p, page);
        }
    }
    Ok(fold_parent(parent, |p| read.get(&p), absent))
}

/// Every level above the leaves folded, the finest first, up to `TOP_LEVEL`
/// or the first level with nothing under it; a level this run did not reach
/// is not part of the set. The top level folded.
///
/// Every level reads the one absent page for what it left as no object,
/// which holds only if a parent of four absent pages is the absent page.
pub fn build<L: Layer>(set: &PageSet, absent: &L, workers: usize) -> Result<u32, String> {
    let fixed = L::fold_page(LevelPage::new(1, 0), |_| Some(absent)).ok_or("no parent over four absent children")?;
    if fixed != *absent {
        return Err("the absent page does not fold to itself: one page cannot stand for every level's absence".into());
    }
    let mut top = 0;
    for level in 1..=TOP_LEVEL {
        let children = set.pages(level - 1)?;
        if children.is_empty() {
            break;
        }
        let at = std::time::Instant::now();
        let n = fold_level(set, level, &children, absent, workers)?;
        println!("[bake/pyramid] level {level}: {n} pages in {:.1}s", at.elapsed().as_secs_f64());
        top = level;
    }
    for stale in (top + 1..).map(|l| set.level_dir(l)).take_while(|d| d.exists()) {
        fs::remove_dir_all(&stale).map_err(|e| format!("{}: {e}", stale.display()))?;
    }
    Ok(top)
}

/// Writes the manifest beside a version: what the set holds, level by level,
/// how its objects are served, and what the layer says it was made from.
///
/// Measured from disk rather than counted by the run: a bake resumes, and a
/// re-run that skips everything would otherwise describe an empty set.
pub fn write_manifest(set: &PageSet, top: u32, layer: serde_json::Value) -> Result<(), String> {
    let (mut objects, mut bytes) = measure_output(&set.root).map_err(|e| e.to_string())?;
    let mut levels = Vec::new();
    for l in 0..=top {
        let (n, b) = measure_output(&set.level_dir(l)).map_err(|e| e.to_string())?;
        levels.push(serde_json::json!({ "level": l, "objects": n, "bytes": b }));
        objects += n;
        bytes += b;
    }
    let mut manifest = serde_json::json!({
        "version": set.root.file_name().map(|n| n.to_string_lossy().to_string()),
        "updated_at": now_iso8601(),
        // The layer's source says which *data* this was baked from; this says
        // which *code* did the baking.
        "git_sha": git_sha(),
        "objects": objects,
        "bytes": bytes,
        "levels": levels,
        // Every object is stored packed: upload with this content-type and
        // this content-encoding, so whatever fetches it decodes it
        // transparently. The upload takes them from here, not from memory.
        "content_type": "application/octet-stream",
        "content_encoding": CONTENT_ENCODING,
    });
    if let (Some(out), serde_json::Value::Object(extra)) = (manifest.as_object_mut(), layer) {
        out.extend(extra);
    }
    let path = set.root.join("../manifest.json");
    fs::write(&path, serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    println!("[bake/pyramid] manifest -> {}", path.display());
    Ok(())
}

/// Folds `level` from `children` and returns how many pages it wrote.
///
/// A parent reads the 4×4 pages around its children, so its row reads four
/// rows of the level below and the next row shares three of them. Each worker
/// folds a band of consecutive parent rows and keeps only the four child rows
/// its current parent row reads, so a page is decoded about once and what is
/// held is four rows, whatever the layer's page weighs.
fn fold_level<L: Layer>(set: &PageSet, level: u32, children: &[LevelPage], absent: &L, workers: usize) -> Result<usize, String> {
    let here = set.level_dir(level);
    // A parent that no longer has a child must not outlive it.
    if here.exists() {
        fs::remove_dir_all(&here).map_err(|e| format!("{}: {e}", here.display()))?;
    }
    fs::create_dir_all(&here).map_err(|e| e.to_string())?;
    let targets = parents(children);
    let mut rows: Vec<Vec<LevelPage>> = Vec::new();
    for p in &targets {
        match rows.last_mut() {
            Some(row) if ivec::Y(row[0].page) == ivec::Y(p.page) => row.push(*p),
            _ => rows.push(vec![*p]),
        }
    }
    let band = rows.len().div_ceil(workers.max(1)).max(1);
    std::thread::scope(|scope| {
        let handles: Vec<_> = rows.chunks(band).map(|band| scope.spawn(move || fold_band(set, band, absent))).collect();
        handles.into_iter().try_for_each(|h| h.join().expect("a fold worker"))
    })?;
    Ok(targets.len())
}

fn fold_band<L: Layer>(set: &PageSet, rows: &[Vec<LevelPage>], absent: &L) -> Result<(), String> {
    let mut held: HashMap<LevelPage, Option<L>> = HashMap::new();
    for row in rows {
        let y = ivec::Y(row[0].page);
        held.retain(|p, _| ivec::Y(p.page) >= 2 * y - 1);
        for &parent in row {
            for p in L::sources(parent) {
                if let std::collections::hash_map::Entry::Vacant(e) = held.entry(p) {
                    e.insert(set.read::<L>(p)?);
                }
            }
            let folded = fold_parent(parent, |p| held.get(&p).and_then(Option::as_ref), absent).ok_or("a parent of an existing child folded to nothing")?;
            set.write(parent, &pack(&folded.encode()?, Level::Stored).map_err(|e| e.to_string())?)?;
        }
    }
    Ok(())
}
