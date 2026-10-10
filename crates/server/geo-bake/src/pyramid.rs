// pxl-geo-bake/pyramid.rs

//! A layer's pyramid on disk, a directory a zoom, folded by the same
//! one-zoom function the client runs.

use pxl_spec::space::layer::lattice::GROUND;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use pxl_spec::bake::tile::Occupancy;
use pxl_spec::space::coord::pvf3::ivec;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::coord::zoom::SHARD_ZOOM;
use pxl_spec::space::layer::tile::{tile_at, tile_rows, Tile, LEAF_ZOOM};

use crate::util::{pack, unpack, write_atomic, Level};

/// A layer's tile as the pyramid holds it. One fold makes each zoom's tiles
/// from the next finer one's; a parent reads its four children unless
/// `sources` says otherwise.
pub trait Base: Clone + PartialEq + Send + Sync + 'static {
    /// The zoom its tiles are made at; every coarser one is folded.
    const LEAF: Zoom = LEAF_ZOOM;

    fn encode(&self) -> Result<Vec<u8>, String>;
    fn decode(raw: &[u8]) -> Result<Self, String>;

    fn sources(p: Tile) -> Vec<Tile> {
        p.children().to_vec()
    }

    /// `p` from its sources; None where a required one is not held.
    fn fold_tile<'a>(p: Tile, held: impl Fn(Tile) -> Option<&'a Self>) -> Option<Self>;

    /// Which of its children are tiles, as the tile says.
    fn occupancy(&self) -> Occupancy;
}

/// What a tile the set holds no object for is. The pyramid is correct only
/// if a parent's is the fold of its children's, as every other tile is.
pub trait Absent<L: Base>: Sync {
    fn at(&self, p: Tile) -> Cow<'_, L>;
}

/// One tile for every absence, which folds to itself.
impl<L: Base> Absent<L> for L {
    fn at(&self, _p: Tile) -> Cow<'_, L> {
        Cow::Borrowed(self)
    }
}

/// Inside a set, beside its tiles; no tile is named so.
pub const MANIFEST: &str = "manifest.json";

pub struct TileSet {
    pub root: PathBuf,
}

impl TileSet {
    pub fn new(root: impl Into<PathBuf>) -> TileSet {
        TileSet { root: root.into() }
    }

    /// Flat under its name, as the bucket holds it.
    pub fn path(&self, p: Tile) -> PathBuf {
        self.root.join(p.name())
    }

    pub fn write(&self, p: Tile, packed: &[u8]) -> Result<(), String> {
        let path = self.path(p);
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        write_atomic(&path, packed).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn remove(&self, p: Tile) -> Result<(), String> {
        let path = self.path(p);
        match fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("{}: {e}", path.display())),
            _ => Ok(()),
        }
    }

    pub fn read<L: Base>(&self, p: Tile) -> Result<Option<L>, String> {
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

    /// Every tile, each zoom's sorted, from one listing: a name's length is its zoom.
    pub fn by_zoom(&self) -> Result<BTreeMap<Zoom, Vec<Tile>>, String> {
        let mut out: BTreeMap<Zoom, Vec<Tile>> = BTreeMap::new();
        if self.root.exists() {
            for entry in fs::read_dir(&self.root).map_err(|e| format!("{}: {e}", self.root.display()))? {
                let name = entry.map_err(|e| e.to_string())?.file_name();
                if let Some(p) = name.to_str().and_then(Tile::of_name) {
                    out.entry(p.zoom).or_default().push(p);
                }
            }
        }
        out.values_mut().for_each(|v| v.sort_unstable());
        Ok(out)
    }
}

/// None where it has no child: a parent exists exactly where a child does.
pub fn fold_parent<'a, L: Base>(parent: Tile, read: impl Fn(Tile) -> Option<&'a L>, absent: &impl Absent<L>) -> Option<L> {
    if parent.children().iter().all(|&k| read(k).is_none()) {
        return None;
    }
    let filled: HashMap<Tile, Cow<'_, L>> = L::sources(parent).into_iter().filter(|&p| read(p).is_none()).map(|p| (p, absent.at(p))).collect();
    L::fold_tile(parent, |p| read(p).or_else(|| filled.get(&p).map(|c| c.as_ref())))
}

pub fn fold_stored<L: Base>(set: &TileSet, parent: Tile, absent: &impl Absent<L>) -> Result<Option<L>, String> {
    let mut read = HashMap::new();
    for p in L::sources(parent) {
        if let Some(tile) = set.read::<L>(p)? {
            read.insert(p, tile);
        }
    }
    Ok(fold_parent(parent, |p| read.get(&p), absent))
}

/// A set with a change staged beside it, read as the set it will be: a
/// staged tile first, then none where the change takes one away, then the
/// set's own. A staged leaf counts only once its shard is staged whole.
pub struct Staged<'a> {
    pub base: &'a TileSet,
    pub stage: &'a TileSet,
    pub leaf: Zoom,
    pub whole: &'a HashSet<Tile>,
    pub gone: HashSet<Tile>,
}

impl Staged<'_> {
    pub fn content(&self, p: Tile) -> Result<Option<Vec<u8>>, String> {
        if p.zoom != self.leaf || self.whole.contains(&ancestor(p, SHARD_ZOOM)) {
            if let Some(c) = self.stage.content(p)? {
                return Ok(Some(c));
            }
        }
        if self.gone.contains(&p) {
            return Ok(None);
        }
        self.base.content(p)
    }

    fn read<L: Base>(&self, p: Tile) -> Result<Option<L>, String> {
        let Some(raw) = self.content(p)? else { return Ok(None) };
        L::decode(&raw).map(Some).map_err(|e| format!("{}: {e}", p.name()))
    }
}

/// The tile over `p` at `zoom`.
pub fn ancestor(mut p: Tile, zoom: Zoom) -> Tile {
    while p.zoom > zoom {
        p = p.parent();
    }
    p
}

/// Folds again what `changed` reaches: at each zoom, every parent whose
/// `sources` read a tile changed below, until a zoom changes nothing. A
/// parent whose content differs is staged, one that folds to none is gone.
/// `changed` may hold tiles of any zoom, each taken up at its own: one with
/// no object is changed when what its absence is has.
pub fn fold<L: Base>(set: &mut Staged<'_>, mut changed: HashSet<Tile>, absent: &impl Absent<L>, workers: usize) -> Result<(), String> {
    for zoom in (GROUND.top()..L::LEAF).rev() {
        if changed.is_empty() {
            break;
        }
        let below: Vec<Tile> = changed.iter().copied().filter(|c| c.zoom == zoom + 1).collect();
        let at = std::time::Instant::now();
        let (written, gone) = fold_zoom(set, &readers::<L>(&below), absent, workers)?;
        println!("[bake/pyramid] z{zoom}: {} tiles changed in {:.1}s", written.len() + gone.len(), at.elapsed().as_secs_f64());
        changed.retain(|c| c.zoom <= zoom);
        changed.extend(written.iter().chain(&gone));
        set.gone.extend(gone);
    }
    Ok(())
}

/// Every tile its parent marks as no tile (`Base::occupancy`), under
/// `top`: what a reader never asks for.
pub fn unasked<L: Base>(set: &TileSet, top: Zoom) -> Result<HashSet<Tile>, String> {
    let mut out = HashSet::new();
    let listed = set.by_zoom()?;
    for zoom in top + 1..=L::LEAF {
        let mut parents: HashMap<Tile, Option<L>> = HashMap::new();
        for &p in listed.get(&zoom).into_iter().flatten() {
            let parent = p.parent();
            if let std::collections::hash_map::Entry::Vacant(e) = parents.entry(parent) {
                e.insert(set.read::<L>(parent)?);
            }
            if parents[&parent].as_ref().is_some_and(|q| !q.occupancy().has_child(p.slot())) {
                out.insert(p);
            }
        }
    }
    Ok(out)
}

/// Measured from disk, not the run: a resumed run that skipped everything
/// would describe an empty set.
pub fn write_manifest(set: &TileSet, top: Zoom, built: Option<&str>, layer: serde_json::Value) -> Result<(), String> {
    // The tiles, a zoom at a time; the set is its tiles and the manifest.
    let listed = set.by_zoom()?;
    let (mut objects, mut bytes) = (0u64, 0u64);
    let mut zooms = Vec::new();
    for zoom in (top..=listed.keys().last().copied().unwrap_or(top)).rev() {
        let (mut n, mut b) = (0u64, 0u64);
        for &p in listed.get(&zoom).into_iter().flatten() {
            n += 1;
            b += fs::metadata(set.path(p)).map_err(|e| e.to_string())?.len();
        }
        (objects, bytes) = (objects + n, bytes + b);
        zooms.push(serde_json::json!({ "z": zoom, "objects": n, "bytes": b }));
    }
    let mut fields = serde_json::json!({ "objects": objects, "bytes": bytes, "zooms": zooms, "content_type": "application/octet-stream" });
    if let (Some(out), serde_json::Value::Object(layer)) = (fields.as_object_mut(), layer) {
        out.extend(layer);
    }
    crate::util::write_manifest(&set.root.join(MANIFEST), built, fields)
}

/// The parents whose `sources` read any of `below`, sorted by row. A
/// layer's sources lie under the 3 × 3 tiles around a parent.
fn readers<L: Base>(below: &[Tile]) -> Vec<Tile> {
    let mut out = BTreeSet::new();
    for &c in below {
        let q = c.parent();
        let [x, y] = [ivec::X(q.tile), ivec::Y(q.tile)];
        for (dx, dy) in (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy))) {
            if !tile_rows(q.zoom).contains(&(y + dy)) {
                continue;
            }
            let p = tile_at(q.zoom, [x + dx, y + dy]);
            if L::sources(p).contains(&c) {
                out.insert((y + dy, ivec::X(p.tile), p));
            }
        }
    }
    out.into_iter().map(|(_, _, p)| p).collect()
}

/// Folds `targets`, a zoom's parents in rows; returns those staged and those
/// gone. Each worker keeps only the child rows its current parent row reads.
fn fold_zoom<L: Base>(set: &Staged<'_>, targets: &[Tile], absent: &impl Absent<L>, workers: usize) -> Result<(Vec<Tile>, Vec<Tile>), String> {
    let rows: Vec<&[Tile]> = targets.chunk_by(|a, b| ivec::Y(a.tile) == ivec::Y(b.tile)).collect();
    let band = rows.len().div_ceil(workers.max(1)).max(1);
    let bands = std::thread::scope(|scope| {
        let handles: Vec<_> = rows.chunks(band).map(|band| scope.spawn(move || fold_band(set, band, absent))).collect();
        handles.into_iter().map(|h| h.join().expect("a fold worker")).collect::<Result<Vec<_>, _>>()
    })?;
    Ok(bands.into_iter().fold((Vec::new(), Vec::new()), |(mut w, mut g), (bw, bg)| {
        w.extend(bw);
        g.extend(bg);
        (w, g)
    }))
}

fn fold_band<L: Base>(set: &Staged<'_>, rows: &[&[Tile]], absent: &impl Absent<L>) -> Result<(Vec<Tile>, Vec<Tile>), String> {
    let mut held: HashMap<Tile, Option<L>> = HashMap::new();
    let (mut written, mut gone) = (Vec::new(), Vec::new());
    for row in rows {
        let y = ivec::Y(row[0].tile);
        held.retain(|p, _| ivec::Y(p.tile) >= 2 * y - 1);
        for &parent in row.iter() {
            for p in L::sources(parent) {
                if let std::collections::hash_map::Entry::Vacant(e) = held.entry(p) {
                    e.insert(set.read::<L>(p)?);
                }
            }
            let after = fold_parent(parent, |p| held.get(&p).and_then(Option::as_ref), absent).map(|t| t.encode()).transpose()?;
            if set.content(parent)? == after {
                continue;
            }
            match &after {
                Some(raw) => {
                    set.stage.write(parent, &pack(raw, Level::Stored).map_err(|e| e.to_string())?)?;
                    written.push(parent);
                }
                None => gone.push(parent),
            }
        }
    }
    Ok((written, gone))
}
