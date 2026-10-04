// pxl-geo-bake/pyramid.rs

//! A layer's pyramid on disk, a directory a zoom, folded by the same
//! one-zoom function the client runs.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pyramid::{Folds, Page, GROUND_Z, TOP_Z};

use crate::util::{measure_output, pack, unpack, write_atomic, Level};

pub trait Layer: Folds + PartialEq + Send + Sync {
    fn encode(&self) -> Result<Vec<u8>, String>;
    fn decode(raw: &[u8]) -> Result<Self, String>;
}

pub struct PageSet {
    pub root: PathBuf,
}

impl PageSet {
    pub fn new(root: impl Into<PathBuf>) -> PageSet {
        PageSet { root: root.into() }
    }

    pub fn zoom_dir(&self, z: u32) -> PathBuf {
        self.root.join(format!("z{z}"))
    }

    pub fn path(&self, p: Page) -> PathBuf {
        self.zoom_dir(p.z).join(p.page.to_string())
    }

    pub fn write(&self, p: Page, packed: &[u8]) -> Result<(), String> {
        let path = self.path(p);
        fs::create_dir_all(self.zoom_dir(p.z)).map_err(|e| e.to_string())?;
        write_atomic(&path, packed).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn remove(&self, p: Page) -> Result<(), String> {
        let path = self.path(p);
        match fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("{}: {e}", path.display())),
            _ => Ok(()),
        }
    }

    pub fn read<L: Layer>(&self, p: Page) -> Result<Option<L>, String> {
        let Some(raw) = self.content(p)? else { return Ok(None) };
        L::decode(&raw).map(Some).map_err(|e| format!("{}: {e}", self.path(p).display()))
    }

    /// A page's content, unpacked.
    pub fn content(&self, p: Page) -> Result<Option<Vec<u8>>, String> {
        let path = self.path(p);
        if !path.exists() {
            return Ok(None);
        }
        let stored = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        unpack(&stored).map(Some).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn pages(&self, z: u32) -> Result<Vec<Page>, String> {
        let dir = self.zoom_dir(z);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))? {
            let name = entry.map_err(|e| e.to_string())?.file_name();
            if let Some(page) = name.to_str().and_then(|n| n.parse::<IVec>().ok()) {
                out.push(Page::new(z, page));
            }
        }
        out.sort_unstable();
        Ok(out)
    }
}

pub fn parents(children: &[Page]) -> Vec<Page> {
    let mut out: Vec<Page> = children.iter().map(|c| c.parent()).collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// None where it has no child: a parent exists exactly where a child does.
pub fn fold_parent<'a, L: Layer>(parent: Page, read: impl Fn(Page) -> Option<&'a L>, absent: &'a L) -> Option<L> {
    if parent.children().iter().all(|&k| read(k).is_none()) {
        return None;
    }
    L::fold_page(parent, |p| Some(read(p).unwrap_or(absent)))
}

pub fn fold_stored<L: Layer>(set: &PageSet, parent: Page, absent: &L) -> Result<Option<L>, String> {
    let mut read = HashMap::new();
    for p in L::sources(parent) {
        if let Some(page) = set.read::<L>(p)? {
            read.insert(p, page);
        }
    }
    Ok(fold_parent(parent, |p| read.get(&p), absent))
}

/// Folds every zoom up to `TOP_Z` or the first empty one. Correct only if
/// a parent of four absent pages is the absent page.
pub fn build<L: Layer>(set: &PageSet, absent: &L, workers: usize) -> Result<u32, String> {
    let fixed = L::fold_page(Page::new(GROUND_Z - 1, 0), |_| Some(absent)).ok_or("no parent over four absent children")?;
    if fixed != *absent {
        return Err("the absent page does not fold to itself: one page cannot stand for every zoom's absence".into());
    }
    let mut top = GROUND_Z;
    for z in (TOP_Z..GROUND_Z).rev() {
        let children = set.pages(z + 1)?;
        if children.is_empty() {
            break;
        }
        let at = std::time::Instant::now();
        let n = fold_zoom(set, z, &children, absent, workers)?;
        println!("[bake/pyramid] z{z}: {n} pages in {:.1}s", at.elapsed().as_secs_f64());
        top = z;
    }
    for stale in (0..top).rev().map(|z| set.zoom_dir(z)).take_while(|d| d.exists()) {
        fs::remove_dir_all(&stale).map_err(|e| format!("{}: {e}", stale.display()))?;
    }
    Ok(top)
}

/// Folds again only what a change reaches: at each zoom, the parents whose
/// `sources` read a page changed below, until a zoom changes nothing. A page
/// is changed when its content differs, not its packed bytes, which a codec
/// update re-packs alike.
pub fn refold<L: Layer>(set: &PageSet, mut changed: HashSet<Page>, absent: &L) -> Result<usize, String> {
    let mut written = 0;
    for z in (TOP_Z..GROUND_Z).rev() {
        if changed.is_empty() {
            break;
        }
        let mut candidates: BTreeSet<Page> = set.pages(z)?.into_iter().collect();
        candidates.extend(changed.iter().map(|c| c.parent()));
        let mut next = HashSet::new();
        for p in candidates {
            if !L::sources(p).iter().any(|s| changed.contains(s)) {
                continue;
            }
            let before = set.content(p)?;
            let after = fold_stored(set, p, absent)?.map(|page| page.encode()).transpose()?;
            if before == after {
                continue;
            }
            match &after {
                Some(raw) => set.write(p, &pack(raw, Level::Stored).map_err(|e| e.to_string())?)?,
                None => set.remove(p)?,
            }
            next.insert(p);
            written += 1;
        }
        changed = next;
    }
    Ok(written)
}

/// Drops every page its parent settles (`Folds::settles`), judged over the
/// whole set before any is removed. Returns how many.
pub fn prune<L: Layer>(set: &PageSet, top: u32) -> Result<usize, String> {
    let mut gone = Vec::new();
    for z in top + 1..=GROUND_Z {
        let mut parents: HashMap<Page, Option<L>> = HashMap::new();
        for p in set.pages(z)? {
            let parent = p.parent();
            if let std::collections::hash_map::Entry::Vacant(e) = parents.entry(parent) {
                e.insert(set.read::<L>(parent)?);
            }
            if parents[&parent].as_ref().is_some_and(|q| q.settles(parent, p)) {
                gone.push(set.path(p));
            }
        }
    }
    for path in &gone {
        fs::remove_file(path).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(gone.len())
}

/// Measured from disk, not the run: a resumed run that skipped everything
/// would describe an empty set.
pub fn write_manifest(set: &PageSet, top: u32, built: Option<&str>, layer: serde_json::Value) -> Result<(), String> {
    let (mut objects, mut bytes) = measure_output(&set.root).map_err(|e| e.to_string())?;
    let mut zooms = Vec::new();
    for z in (top..=GROUND_Z).rev() {
        let (n, b) = measure_output(&set.zoom_dir(z)).map_err(|e| e.to_string())?;
        zooms.push(serde_json::json!({ "z": z, "objects": n, "bytes": b }));
        objects += n;
        bytes += b;
    }
    let mut fields = serde_json::json!({ "objects": objects, "bytes": bytes, "zooms": zooms, "content_type": "application/octet-stream" });
    if let (Some(out), serde_json::Value::Object(layer)) = (fields.as_object_mut(), layer) {
        out.extend(layer);
    }
    crate::util::write_manifest(&set.root, built, fields)
}

/// A parent reads the 4×4 children around it, so each worker keeps only the
/// four child rows its current parent row reads.
fn fold_zoom<L: Layer>(set: &PageSet, z: u32, children: &[Page], absent: &L, workers: usize) -> Result<usize, String> {
    let here = set.zoom_dir(z);
    if here.exists() {
        fs::remove_dir_all(&here).map_err(|e| format!("{}: {e}", here.display()))?;
    }
    fs::create_dir_all(&here).map_err(|e| e.to_string())?;
    let targets = parents(children);
    let mut rows: Vec<Vec<Page>> = Vec::new();
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

fn fold_band<L: Layer>(set: &PageSet, rows: &[Vec<Page>], absent: &L) -> Result<(), String> {
    let mut held: HashMap<Page, Option<L>> = HashMap::new();
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
