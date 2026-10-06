// pxl-geo-bake/pyramid.rs

//! A layer's pyramid on disk, a directory a zoom, folded by the same
//! one-zoom function the client runs.

use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pyramid::{Folds, Tile, LEAF_Z, TOP_Z};
use pxl_spec::space::zoom::Zoom;

use crate::util::{measure_output, pack, unpack, write_atomic, Level};

pub trait Layer: Folds + Clone + PartialEq + Send + Sync + 'static {
    fn encode(&self) -> Result<Vec<u8>, String>;
    fn decode(raw: &[u8]) -> Result<Self, String>;
}

/// What a tile the set holds no object for is. The pyramid is correct only
/// if a parent's is the fold of its children's, as every other tile is.
pub trait Absent<L: Layer>: Sync {
    fn at(&self, p: Tile) -> Cow<'_, L>;
}

/// One tile for every absence, which folds to itself.
impl<L: Layer> Absent<L> for L {
    fn at(&self, _p: Tile) -> Cow<'_, L> {
        Cow::Borrowed(self)
    }
}

pub struct TileSet {
    pub root: PathBuf,
}

impl TileSet {
    pub fn new(root: impl Into<PathBuf>) -> TileSet {
        TileSet { root: root.into() }
    }

    pub fn zoom_dir(&self, zoom: Zoom) -> PathBuf {
        self.root.join(format!("z{zoom}"))
    }

    pub fn path(&self, p: Tile) -> PathBuf {
        self.zoom_dir(p.zoom).join(p.tile.to_string())
    }

    pub fn write(&self, p: Tile, packed: &[u8]) -> Result<(), String> {
        let path = self.path(p);
        fs::create_dir_all(self.zoom_dir(p.zoom)).map_err(|e| e.to_string())?;
        write_atomic(&path, packed).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn remove(&self, p: Tile) -> Result<(), String> {
        let path = self.path(p);
        match fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("{}: {e}", path.display())),
            _ => Ok(()),
        }
    }

    pub fn read<L: Layer>(&self, p: Tile) -> Result<Option<L>, String> {
        let Some(raw) = self.content(p)? else { return Ok(None) };
        L::decode(&raw).map(Some).map_err(|e| format!("{}: {e}", self.path(p).display()))
    }

    /// A tile's content, unpacked.
    pub fn content(&self, p: Tile) -> Result<Option<Vec<u8>>, String> {
        let path = self.path(p);
        if !path.exists() {
            return Ok(None);
        }
        let stored = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        unpack(&stored).map(Some).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn tiles(&self, zoom: Zoom) -> Result<Vec<Tile>, String> {
        let dir = self.zoom_dir(zoom);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))? {
            let name = entry.map_err(|e| e.to_string())?.file_name();
            if let Some(tile) = name.to_str().and_then(|n| n.parse::<IVec>().ok()) {
                out.push(Tile::new(zoom, tile));
            }
        }
        out.sort_unstable();
        Ok(out)
    }
}

pub fn parents(children: &[Tile]) -> Vec<Tile> {
    let mut out: Vec<Tile> = children.iter().map(|c| c.parent()).collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// None where it has no child: a parent exists exactly where a child does.
pub fn fold_parent<'a, L: Layer>(parent: Tile, read: impl Fn(Tile) -> Option<&'a L>, absent: &impl Absent<L>) -> Option<L> {
    if parent.children().iter().all(|&k| read(k).is_none()) {
        return None;
    }
    let filled: HashMap<Tile, Cow<'_, L>> = L::sources(parent).into_iter().filter(|&p| read(p).is_none()).map(|p| (p, absent.at(p))).collect();
    L::fold_tile(parent, |p| read(p).or_else(|| filled.get(&p).map(|c| c.as_ref())))
}

pub fn fold_stored<L: Layer>(set: &TileSet, parent: Tile, absent: &impl Absent<L>) -> Result<Option<L>, String> {
    let mut read = HashMap::new();
    for p in L::sources(parent) {
        if let Some(tile) = set.read::<L>(p)? {
            read.insert(p, tile);
        }
    }
    Ok(fold_parent(parent, |p| read.get(&p), absent))
}

/// Folds every zoom up to `TOP_Z` or the first empty one.
pub fn build<L: Layer>(set: &TileSet, absent: &impl Absent<L>, workers: usize) -> Result<Zoom, String> {
    let mut top = LEAF_Z;
    for zoom in (TOP_Z..LEAF_Z).rev() {
        let children = set.tiles(zoom + 1)?;
        if children.is_empty() {
            break;
        }
        let at = std::time::Instant::now();
        let n = fold_zoom(set, zoom, &children, absent, workers)?;
        println!("[bake/pyramid] z{zoom}: {n} tiles in {:.1}s", at.elapsed().as_secs_f64());
        top = zoom;
    }
    for stale in (0..top).rev().map(|z| set.zoom_dir(z)).take_while(|d| d.exists()) {
        fs::remove_dir_all(&stale).map_err(|e| format!("{}: {e}", stale.display()))?;
    }
    Ok(top)
}

/// Folds again only what a change reaches: at each zoom, the parents whose
/// `sources` read a tile changed below, until a zoom changes nothing. A tile
/// is changed when its content differs, not its packed bytes, which a codec
/// update re-packs alike.
pub fn refold<L: Layer>(set: &TileSet, mut changed: HashSet<Tile>, absent: &impl Absent<L>) -> Result<usize, String> {
    let mut written = 0;
    for zoom in (TOP_Z..LEAF_Z).rev() {
        if changed.is_empty() {
            break;
        }
        let mut candidates: BTreeSet<Tile> = set.tiles(zoom)?.into_iter().collect();
        candidates.extend(changed.iter().map(|c| c.parent()));
        let mut next = HashSet::new();
        for p in candidates {
            if !L::sources(p).iter().any(|s| changed.contains(s)) {
                continue;
            }
            let before = set.content(p)?;
            let after = fold_stored(set, p, absent)?.map(|tile| tile.encode()).transpose()?;
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

/// Drops every tile its parent settles (`Folds::settles`), judged over the
/// whole set before any is removed. Returns how many.
pub fn prune<L: Layer>(set: &TileSet, top: Zoom) -> Result<usize, String> {
    let mut gone = Vec::new();
    for zoom in top + 1..=LEAF_Z {
        let mut parents: HashMap<Tile, Option<L>> = HashMap::new();
        for p in set.tiles(zoom)? {
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
pub fn write_manifest(set: &TileSet, top: Zoom, built: Option<&str>, layer: serde_json::Value) -> Result<(), String> {
    let (mut objects, mut bytes) = measure_output(&set.root).map_err(|e| e.to_string())?;
    let mut zooms = Vec::new();
    for zoom in (top..=LEAF_Z).rev() {
        let (n, b) = measure_output(&set.zoom_dir(zoom)).map_err(|e| e.to_string())?;
        zooms.push(serde_json::json!({ "z": zoom, "objects": n, "bytes": b }));
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
fn fold_zoom<L: Layer>(set: &TileSet, zoom: Zoom, children: &[Tile], absent: &impl Absent<L>, workers: usize) -> Result<usize, String> {
    let here = set.zoom_dir(zoom);
    if here.exists() {
        fs::remove_dir_all(&here).map_err(|e| format!("{}: {e}", here.display()))?;
    }
    fs::create_dir_all(&here).map_err(|e| e.to_string())?;
    let targets = parents(children);
    let mut rows: Vec<Vec<Tile>> = Vec::new();
    for p in &targets {
        match rows.last_mut() {
            Some(row) if ivec::Y(row[0].tile) == ivec::Y(p.tile) => row.push(*p),
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

fn fold_band<L: Layer>(set: &TileSet, rows: &[Vec<Tile>], absent: &impl Absent<L>) -> Result<(), String> {
    let mut held: HashMap<Tile, Option<L>> = HashMap::new();
    for row in rows {
        let y = ivec::Y(row[0].tile);
        held.retain(|p, _| ivec::Y(p.tile) >= 2 * y - 1);
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
