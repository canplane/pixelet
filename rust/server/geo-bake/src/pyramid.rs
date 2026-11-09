// pxl-geo-bake/pyramid.rs

//! A layer's pyramid on disk, flat under each tile's name, folded by the same one-zoom
//! function the client runs.

use fnv::{FnvHashMap, FnvHashSet};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use pxl_spec::bake::publication::BaseLayer;
use pxl_spec::bake::tile::Occupancy;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::lattice::{coarsest, Lattice};
use pxl_spec::space::layer::quadrant::{quad_rows, Quadrant, LEAF_ZOOM};

use crate::util;
use crate::util::{pack, unpack, write_atomic, Level};

/// A layer's tile as the pyramid holds it. One fold makes each zoom's tiles
/// from the next finer one's; a parent reads its four children unless
/// `sources` says otherwise.
pub trait Base: Clone + PartialEq + Send + Sync + 'static {
    /// The kind it is published as, whose `FORMAT` it is written in.
    const LAYER: BaseLayer;

    /// The zoom its tiles are made at; every coarser one is folded, up to
    /// `COARSEST`.
    const LEAF: Zoom = LEAF_ZOOM;

    /// Its tiles' cells, as the client's layer reads them.
    const LATTICE: Lattice;

    /// The coarsest zoom it is folded to: the coarsest the client reads it
    /// at (pxl-spec lattice.rs `coarsest`).
    const COARSEST: Zoom = coarsest(Self::LATTICE);

    /// The tile as it is published at `at`.
    fn encode(&self, at: Quadrant) -> Result<Vec<u8>, String>;

    /// As tile `at` is stored: its packet, as `encode` makes it, and its fit
    /// for `check_pool` (`FITS`), where the kind has one; one pass makes both.
    fn encode_with_fit(&self, at: Quadrant) -> Result<(Vec<u8>, Option<String>), String> {
        Ok((self.encode(at)?, None))
    }
    fn decode(raw: &[u8]) -> Result<Self, String>;

    fn sources(p: Quadrant) -> Vec<Quadrant> {
        p.children().to_vec()
    }

    /// `p` from its sources; None where a required one is not held.
    fn fold_tile<'a>(p: Quadrant, get: impl Fn(Quadrant) -> Option<&'a Self>) -> Option<Self>;

    /// Which of its children are tiles, as the tile says.
    fn occupancy(&self) -> Occupancy;

    /// The fits of the set as it will be (`set`, `FITS`), refused where the set holds what the
    /// client's pool cannot. It is a fact of the whole set, not of a tile, asked of each tile's fit
    /// (`encode_with_fit`) before a commit moves anything, `worker_cnt` tiles at once. None for a kind
    /// with no fits.
    fn check_pool(
        _set: &Staged<'_>,
        _worker_cnt: usize,
    ) -> Result<Option<Vec<(Quadrant, String)>>, String> {
        Ok(None)
    }
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

/// Inside a set, each of its tiles' fit (`Base::encode_with_fit`), a line a tile with its name
/// and its fit, so a commit asks what the set will hold of these, never of the tiles read and
/// made again. It is not published. A stage has each staged tile's fit beside it,
/// `<name>.fit`. No tile is named so.
pub(crate) const FITS: &str = "fits";

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

    fn fit_path(&self, p: Quadrant) -> PathBuf {
        self.root.join(format!("{}.fit", p.key()))
    }

    /// A tile as `Base::encode_with_fit` made it, and beside it its fit, or none:
    /// the fit first, so a tile written is never beside another's.
    pub fn write_with_fit(
        &self,
        p: Quadrant,
        packed: &[u8],
        fit: Option<&str>,
    ) -> Result<(), String> {
        fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let path = self.fit_path(p);
        match fit {
            Some(fit) => write_atomic(&path, fit.as_bytes())
                .map_err(|e| format!("{}: {e}", path.display()))?,
            None => remove_file(&path)?,
        }
        self.write(p, packed)
    }

    /// A tile's fit (`write_with_fit`), none where it has none.
    pub fn fit(&self, p: Quadrant) -> Result<Option<String>, String> {
        let path = self.fit_path(p);
        match fs::read_to_string(&path) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// The tile, then its fit: a tile gone first leaves a fit no tile reads.
    pub fn remove(&self, p: Quadrant) -> Result<(), String> {
        remove_file(&self.path(p))?;
        remove_file(&self.fit_path(p))
    }

    /// The set's fits (`FITS`), by tile; none for a set with none.
    pub(crate) fn fits(&self) -> Result<Option<FnvHashMap<Quadrant, String>>, String> {
        let path = self.root.join(FITS);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        let mut out = FnvHashMap::default();
        for line in text.lines() {
            let (name, fit) = line.split_once(' ').unwrap_or((line, ""));
            let p = Quadrant::of_key(name)
                .ok_or_else(|| format!("{}: {name} names no tile", path.display()))?;
            out.insert(p, fit.to_string());
        }
        Ok(Some(out))
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

    /// `p`'s fit in the set as it will be (`Base::encode_with_fit`), as `content`
    /// reads its tile: the stage's beside a staged tile, none for one gone, else
    /// the set's (`base`, its `fits`).
    pub fn fit_of(
        &self,
        p: Quadrant,
        base: &FnvHashMap<Quadrant, String>,
    ) -> Result<Option<String>, String> {
        if (p.zoom != self.leaf || self.whole.contains(&p.shard())) && self.stage.path(p).exists() {
            return self.stage.fit(p);
        }
        if self.removed.contains(&p) {
            return Ok(None);
        }
        Ok(base.get(&p).cloned())
    }

    /// Every tile it may hold, the set's and the stage's, each zoom's
    /// sorted; one gone or not taken up reads none (`content`).
    pub fn tiles(&self) -> Result<BTreeMap<Zoom, Vec<Quadrant>>, String> {
        let mut all = self.base.by_zoom()?;
        for (zoom, tiles) in self.stage.by_zoom()? {
            let at = all.entry(zoom).or_default();
            at.extend(tiles);
            at.sort_unstable();
            at.dedup();
        }
        Ok(all)
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
        let (written, removed) = fold_zoom(set, &readers::<L>(&below), absent, worker_cnt)?;
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

/// Every tile its parent marks as no tile (`Base::occupancy`), under
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

/// Folds `targets`, a zoom's parents in rows; returns those staged and those
/// gone. Each worker keeps only the child rows its current parent row reads.
fn fold_zoom<L: Base>(
    set: &Staged<'_>,
    targets: &[Quadrant],
    absent: &impl Absent<L>,
    worker_cnt: usize,
) -> Result<(Vec<Quadrant>, Vec<Quadrant>), String> {
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
    Ok(bands
        .into_iter()
        .fold((Vec::new(), Vec::new()), |(mut w, mut g), (bw, bg)| {
            w.extend(bw);
            g.extend(bg);
            (w, g)
        }))
}

fn fold_band<L: Base>(
    set: &Staged<'_>,
    rows: &[&[Quadrant]],
    absent: &impl Absent<L>,
) -> Result<(Vec<Quadrant>, Vec<Quadrant>), String> {
    let mut sources: FnvHashMap<Quadrant, Option<L>> = FnvHashMap::default();
    let (mut written, mut removed) = (Vec::new(), Vec::new());
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
                .map(|t| {
                    t.encode_with_fit(parent)
                        .map_err(|e| format!("{}: {e}", parent.key()))
                })
                .transpose()?;
            if set.content(parent)?.as_deref() == after.as_ref().map(|(raw, _)| raw.as_slice()) {
                continue;
            }
            match &after {
                Some((raw, fit)) => {
                    set.stage.write_with_fit(
                        parent,
                        &pack(raw, Level::Stored).map_err(|e| e.to_string())?,
                        fit.as_deref(),
                    )?;
                    written.push(parent);
                }
                None => removed.push(parent),
            }
        }
    }
    Ok((written, removed))
}

#[cfg(test)]
#[path = "pyramid.test.rs"]
mod test;
