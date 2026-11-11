// pxl-geo-bake/pyramid.rs

//! A layer's pyramid on disk, flat under each tile's name, folded by the same one-zoom
//! function the client runs.

use fnv::{FnvHashMap, FnvHashSet};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use pxl_spec::bake::tile::Tile;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::quadrant::{quad_rows, Quadrant};

use crate::util;
use crate::util::{pack, unpack, write_atomic, Level};

/// A layer's tile as the pyramid holds it. One fold makes each zoom's tiles
/// from the next finer one's; a parent reads its four children unless
/// `sources` says otherwise.
pub trait Base: Tile + Clone + PartialEq + Send + Sync + 'static {
    /// Whether a version leaves out the tiles their parent marks as no tile
    /// (`Tile::occupancy`), which no reader asks for.
    const PRUNED: bool;

    /// The tile as it is published at `at`; an error names the tile.
    fn encode(&self, at: Quadrant) -> Result<Vec<u8>, String>;

    fn decode(raw: &[u8]) -> Result<Self, String>;

    fn sources(p: Quadrant) -> Vec<Quadrant> {
        p.children().to_vec()
    }

    /// `p` from its sources; None where a required one is not held.
    fn fold_tile<'a>(p: Quadrant, get: impl Fn(Quadrant) -> Option<&'a Self>) -> Option<Self>;
}

/// What a tile the set holds no object for is. The pyramid is correct only
/// if a parent's is the fold of its children's, as every other tile is.
pub trait Absent<L: Base>: Sync {
    fn at(&self, p: Quadrant) -> Cow<'_, L>;
}

/// One tile for every absence, which folds to itself.
impl<L: Base> Absent<L> for L {
    fn at(&self, _p: Quadrant) -> Cow<'_, L> {
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
    pub fn path(&self, p: Quadrant) -> PathBuf {
        self.root.join(p.key())
    }

    pub fn write(&self, p: Quadrant, packed: &[u8]) -> Result<(), String> {
        let path = self.path(p);
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        write_atomic(&path, packed).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn remove(&self, p: Quadrant) -> Result<(), String> {
        remove_file(&self.path(p))
    }

    pub fn read<L: Base>(&self, p: Quadrant) -> Result<Option<L>, String> {
        let Some(raw) = self.content(p)? else {
            return Ok(None);
        };
        L::decode(&raw)
            .map(Some)
            .map_err(|e| format!("{}: {e}", self.path(p).display()))
    }

    /// A tile's content, unpacked.
    pub fn content(&self, p: Quadrant) -> Result<Option<Vec<u8>>, String> {
        let path = self.path(p);
        if !path.exists() {
            return Ok(None);
        }
        let stored = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        unpack(&stored)
            .map(Some)
            .map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The set as a version is uploaded, `<set>.publish/`, made empty first: the tiles `keep`
    /// passes, linked, and the manifest copied, which the set may stop being once it changes
    /// again.
    pub fn publish(&self, keep: impl Fn(Quadrant) -> bool) -> Result<PathBuf, String> {
        let mut to = self.root.clone().into_os_string();
        to.push(".publish");
        let to = PathBuf::from(to);
        if to.exists() {
            fs::remove_dir_all(&to).map_err(|e| format!("{}: {e}", to.display()))?;
        }
        fs::create_dir_all(&to).map_err(|e| format!("{}: {e}", to.display()))?;
        for p in self.by_zoom()?.into_values().flatten().filter(|&p| keep(p)) {
            fs::hard_link(self.path(p), to.join(p.key()))
                .map_err(|e| format!("{}: {e}", self.path(p).display()))?;
        }
        let manifest = self.root.join(MANIFEST);
        fs::copy(&manifest, to.join(MANIFEST))
            .map_err(|e| format!("{}: {e}", manifest.display()))?;
        Ok(to)
    }

    /// Every tile, each zoom's sorted, from one listing: a name's length is its zoom.
    pub fn by_zoom(&self) -> Result<BTreeMap<Zoom, Vec<Quadrant>>, String> {
        let mut out: BTreeMap<Zoom, Vec<Quadrant>> = BTreeMap::new();
        if self.root.exists() {
            for entry in
                fs::read_dir(&self.root).map_err(|e| format!("{}: {e}", self.root.display()))?
            {
                let name = entry.map_err(|e| e.to_string())?.file_name();
                if let Some(p) = name.to_str().and_then(Quadrant::of_key) {
                    out.entry(p.zoom).or_default().push(p);
                }
            }
        }
        out.values_mut().for_each(|v| v.sort_unstable());
        Ok(out)
    }
}

fn remove_file(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(format!("{}: {e}", path.display()))
        }
        _ => Ok(()),
    }
}

/// The parent folded from its children; none where it has no child, since a parent exists
/// exactly where a child does.
pub(crate) fn fold_parent<'a, L: Base>(
    parent: Quadrant,
    read: impl Fn(Quadrant) -> Option<&'a L>,
    absent: &impl Absent<L>,
) -> Option<L> {
    if parent.children().iter().all(|&k| read(k).is_none()) {
        return None;
    }
    let filled: FnvHashMap<Quadrant, Cow<'_, L>> = L::sources(parent)
        .into_iter()
        .filter(|&p| read(p).is_none())
        .map(|p| (p, absent.at(p)))
        .collect();
    L::fold_tile(parent, |p| {
        read(p).or_else(|| filled.get(&p).map(|c| c.as_ref()))
    })
}

/// A set with a change staged beside it, read as the set it will be: a
/// staged tile first, then none where the change takes one away, then the
/// set's own. A staged leaf counts only once its shard is staged whole.
pub struct Staged<'a> {
    pub base: &'a TileSet,
    pub stage: &'a TileSet,
    pub leaf: Zoom,
    pub whole: &'a FnvHashSet<Quadrant>,
    pub(crate) removed: FnvHashSet<Quadrant>,
}

impl Staged<'_> {
    pub fn content(&self, p: Quadrant) -> Result<Option<Vec<u8>>, String> {
        if p.zoom != self.leaf || self.whole.contains(&p.shard()) {
            if let Some(c) = self.stage.content(p)? {
                return Ok(Some(c));
            }
        }
        if self.removed.contains(&p) {
            return Ok(None);
        }
        self.base.content(p)
    }

    pub fn read<L: Base>(&self, p: Quadrant) -> Result<Option<L>, String> {
        let Some(raw) = self.content(p)? else {
            return Ok(None);
        };
        L::decode(&raw)
            .map(Some)
            .map_err(|e| format!("{}: {e}", p.key()))
    }
}

/// Folds again what `changed` reaches: at each zoom, every parent whose
/// `sources` read a tile changed below, until a zoom changes nothing. A
/// parent whose content differs is staged, one that folds to none is gone.
/// `changed` may hold tiles of any zoom, each taken up at its own: one with
/// no object is changed when what its absence is has.
pub fn fold<L: Base>(
    set: &mut Staged<'_>,
    mut changed: FnvHashSet<Quadrant>,
    absent: &impl Absent<L>,
    worker_cnt: usize,
) -> Result<(), String> {
    for zoom in (L::COARSEST..L::LEAF).rev() {
        if changed.is_empty() {
            break;
        }
        let below: Vec<Quadrant> = changed
            .iter()
            .copied()
            .filter(|c| c.zoom == zoom + 1)
            .collect();
        let at = std::time::Instant::now();
        let (written, removed, failed) = fold_zoom(set, &readers::<L>(&below), absent, worker_cnt)?;
        if failed > 0 {
            return Err(format!("z{zoom}: {failed} tiles failed"));
        }
        println!(
            "[bake/pyramid] z{zoom}: {} tiles changed in {:.1}s",
            written.len() + removed.len(),
            at.elapsed().as_secs_f64()
        );
        changed.retain(|c| c.zoom <= zoom);
        changed.extend(written.iter().chain(&removed));
        set.removed.extend(removed);
    }
    Ok(())
}

/// Every tile its parent marks as no tile (`Tile::occupancy`), under
/// `top`: what a reader never asks for.
pub(crate) fn orphan_tiles<L: Base>(
    set: &TileSet,
    top: Zoom,
) -> Result<FnvHashSet<Quadrant>, String> {
    let mut out = FnvHashSet::default();
    let by_zoom = set.by_zoom()?;
    for zoom in top + 1..=L::LEAF {
        let mut parents: FnvHashMap<Quadrant, Option<L>> = FnvHashMap::default();
        for &p in by_zoom.get(&zoom).into_iter().flatten() {
            let parent = p.parent();
            if let std::collections::hash_map::Entry::Vacant(e) = parents.entry(parent) {
                e.insert(set.read::<L>(parent)?);
            }
            if parents[&parent]
                .as_ref()
                .is_some_and(|q| !q.occupancy().has_child(p.slot()))
            {
                out.insert(p);
            }
        }
    }
    Ok(out)
}

/// Writes the set's manifest, measured from disk, not from the run: a resumed run that
/// skipped everything would describe an empty set.
pub fn write_manifest(
    set: &TileSet,
    top: Zoom,
    git_sha: Option<&str>,
    layer: serde_json::Value,
) -> Result<(), String> {
    // The tiles, a zoom at a time; the set is its tiles and the manifest.
    let by_zoom = set.by_zoom()?;
    let (mut objects, mut bytes) = (0u64, 0u64);
    let mut zooms = Vec::new();
    for zoom in (top..=by_zoom.keys().last().copied().unwrap_or(top)).rev() {
        let (mut n, mut b) = (0u64, 0u64);
        for &p in by_zoom.get(&zoom).into_iter().flatten() {
            n += 1;
            b += fs::metadata(set.path(p)).map_err(|e| e.to_string())?.len();
        }
        (objects, bytes) = (objects + n, bytes + b);
        zooms.push(serde_json::json!({ "z": zoom, "objects": n, "bytes": b }));
    }
    let mut fields = serde_json::json!({ "objects": objects, "bytes": bytes, "zooms": zooms });
    if let (Some(out), serde_json::Value::Object(layer)) = (fields.as_object_mut(), layer) {
        out.extend(layer);
    }
    util::write_manifest(&set.root.join(MANIFEST), git_sha, fields)
}

/// The parents whose `sources` read any of `below`, sorted by row. A
/// layer's sources lie under the 3 × 3 tiles around a parent.
fn readers<L: Base>(below: &[Quadrant]) -> Vec<Quadrant> {
    let mut out = BTreeSet::new();
    for &c in below {
        let q = c.parent();
        let [x, y] = q.at;
        for (dx, dy) in (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy))) {
            if !quad_rows(q.zoom).contains(&(y + dy)) {
                continue;
            }
            let p = Quadrant::new(q.zoom, [x + dx, y + dy]);
            if L::sources(p).contains(&c) {
                out.insert((y + dy, p.at[0], p));
            }
        }
    }
    out.into_iter().map(|(_, _, p)| p).collect()
}

/// Folds `targets`, a zoom's parents in rows; returns those staged, those gone and how many
/// failed, each said as it failed, as a leaf's walk says it (tile/walk.rs). Each worker keeps
/// only the child rows its current parent row reads.
fn fold_zoom<L: Base>(
    set: &Staged<'_>,
    targets: &[Quadrant],
    absent: &impl Absent<L>,
    worker_cnt: usize,
) -> Result<(Vec<Quadrant>, Vec<Quadrant>, u64), String> {
    let rows: Vec<&[Quadrant]> = targets.chunk_by(|a, b| a.at[1] == b.at[1]).collect();
    let band = rows.len().div_ceil(worker_cnt.max(1)).max(1);
    let bands = std::thread::scope(|scope| {
        let handles: Vec<_> = rows
            .chunks(band)
            .map(|band| scope.spawn(move || fold_band(set, band, absent)))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("a fold worker"))
            .collect::<Result<Vec<_>, _>>()
    })?;
    Ok(bands.into_iter().fold(
        (Vec::new(), Vec::new(), 0),
        |(mut w, mut g, n), (bw, bg, bn)| {
            w.extend(bw);
            g.extend(bg);
            (w, g, n + bn)
        },
    ))
}

fn fold_band<L: Base>(
    set: &Staged<'_>,
    rows: &[&[Quadrant]],
    absent: &impl Absent<L>,
) -> Result<(Vec<Quadrant>, Vec<Quadrant>, u64), String> {
    let mut sources: FnvHashMap<Quadrant, Option<L>> = FnvHashMap::default();
    let (mut written, mut removed, mut failed) = (Vec::new(), Vec::new(), 0);
    for row in rows {
        let y = row[0].at[1];
        sources.retain(|p, _| p.at[1] >= 2 * y - 1);
        for &parent in row.iter() {
            for p in L::sources(parent) {
                if let std::collections::hash_map::Entry::Vacant(e) = sources.entry(p) {
                    e.insert(set.read::<L>(p)?);
                }
            }
            let after = fold_parent(parent, |p| sources.get(&p).and_then(Option::as_ref), absent)
                .map(|t| t.encode(parent))
                .transpose();
            let after = match after {
                Ok(after) => after,
                Err(err) => {
                    eprintln!("[bake/fold/{}] failed: {err}", parent.key());
                    failed += 1;
                    continue;
                }
            };
            if set.content(parent)?.as_deref() == after.as_deref() {
                continue;
            }
            match &after {
                Some(raw) => {
                    set.stage.write(
                        parent,
                        &pack(raw, Level::Stored).map_err(|e| e.to_string())?,
                    )?;
                    written.push(parent);
                }
                None => removed.push(parent),
            }
        }
    }
    Ok((written, removed, failed))
}

#[cfg(test)]
#[path = "pyramid.test.rs"]
mod test;
