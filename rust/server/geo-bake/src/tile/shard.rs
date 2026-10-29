// pxl-geo-bake/tile/shard.rs

//! A bake's one set, `out/<kind>/`: every zoom of the pyramid, flat under
//! the tiles' names and never pruned, which `dev:baked` reads. A bake
//! changes it only through `<set>.stage/`, a shard at a time, as the ingest
//! keeps its rows. Which shards are baked, and when, is the operator's to
//! say (`--shard`); a change of source, code or coverage is a purge, the set
//! deleted by hand. Every step can stop and run again:
//!
//! - `leaf` stages a shard's leaves, then `<shard>.done`, the shard's leaves
//!   the set holds and the bake no longer makes;
//! - `fold` folds again what the whole shards change, reading the set as it
//!   will be, and stages the parents that changed, then `fold.done`, those
//!   that folded to none;
//! - `commit`, only once folded, turns `fold.done` into `commit.done`,
//!   writing the shards the set will hold into its manifest first, moves
//!   what is staged into the set and takes away what each `.done` lists,
//!   each `.done` last. While a commit is begun, a leaf or a fold is
//!   refused: it would fold from a set half moved.

use fnv::FnvHashSet;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::Flags;
use crate::coverage::{src_of, Coverage, Node};
use crate::pyramid::{
    ancestor, fold, unasked, write_manifest, Absent, Base, Staged, TileSet, FITS, MANIFEST,
};
use crate::source;
use crate::util;
use crate::util::git_sha;
use pxl_spec::bake::published::FORMAT;
use pxl_spec::space::coord::zoom::{Zoom, SHARD_ZOOM};
use pxl_spec::space::layer::tile::Tile;

/// The fold's unit in the stage, and the same once its commit has begun; no
/// shard is named so.
const FOLD: &str = "fold";
const COMMIT: &str = "commit";

pub struct Stage {
    pub set: TileSet,
    stage: TileSet,
}

fn io(p: &Path, e: std::io::Error) -> String {
    format!("{}: {e}", p.display())
}

fn shard_tile(shard: Node) -> Tile {
    Tile::new(SHARD_ZOOM, shard.at)
}

impl Stage {
    pub fn of(set: PathBuf) -> Stage {
        let mut stage = set.clone().into_os_string();
        stage.push(".stage");
        Stage {
            set: TileSet::new(set),
            stage: TileSet::new(PathBuf::from(stage)),
        }
    }

    fn done(&self, unit: &str) -> PathBuf {
        self.stage.root.join(format!("{unit}.done"))
    }

    fn write_done(&self, unit: &str, gone: impl Iterator<Item = Tile>) -> Result<(), String> {
        let path = self.done(unit);
        let names: Vec<String> = gone.map(Tile::name).collect();
        util::write_atomic(&path, names.join("\n").as_bytes()).map_err(|e| io(&path, e))
    }

    fn read_done(&self, unit: &str) -> Result<Vec<Tile>, String> {
        let path = self.done(unit);
        match fs::read_to_string(&path) {
            Ok(text) => text
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| {
                    Tile::of_name(l).ok_or_else(|| format!("{}: {l}: no tile", path.display()))
                })
                .collect(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(io(&path, e)),
        }
    }

    fn remove_done(&self, unit: &str) -> Result<(), String> {
        let path = self.done(unit);
        match fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(io(&path, e)),
            _ => Ok(()),
        }
    }

    /// The shards staged whole.
    fn whole(&self) -> Result<Vec<Node>, String> {
        Ok(Coverage::of_source()?
            .shards()
            .filter(|s| self.done(&s.name()).exists())
            .collect())
    }

    /// The shards the set holds once what is staged is in: those its
    /// manifest names and those staged whole. None where it holds tiles and
    /// its manifest names no shards, which nothing then knows.
    fn will_hold(&self) -> Result<Option<BTreeSet<Tile>>, String> {
        let held = match source::manifest(&self.set.root)?
            .as_ref()
            .map(source::held)
            .transpose()?
            .flatten()
        {
            Some(held) => held,
            None if self.set.by_zoom()?.is_empty() => BTreeSet::new(),
            None => return Ok(None),
        };
        Ok(Some(
            held.into_iter()
                .chain(self.whole()?.into_iter().map(shard_tile))
                .collect(),
        ))
    }

    /// The fold's unit let go, so a fold runs again from the set as it is.
    fn clear_fold(&self, leaf: Zoom) -> Result<(), String> {
        self.remove_done(FOLD)?;
        for p in self
            .stage
            .by_zoom()?
            .into_iter()
            .filter(|(z, _)| *z != leaf)
            .flat_map(|(_, v)| v)
        {
            self.stage.remove(p)?;
        }
        Ok(())
    }

    /// The shard's leaves staged anew by `fill`, then its `.done`. A leaf
    /// stage makes any fold stale, so the fold's unit goes first.
    pub fn leaf<L: Base>(
        &self,
        shard: Node,
        fill: impl FnOnce(&TileSet) -> Result<(), String>,
    ) -> Result<(), String> {
        let at = shard_tile(shard);
        fs::create_dir_all(&self.stage.root).map_err(|e| io(&self.stage.root, e))?;
        self.not_committing()?;
        self.remove_done(&shard.name())?;
        self.clear_fold(L::LEAF)?;
        let under = |set: &TileSet| -> Result<Vec<Tile>, String> {
            Ok(set
                .by_zoom()?
                .remove(&L::LEAF)
                .unwrap_or_default()
                .into_iter()
                .filter(|&p| ancestor(p, SHARD_ZOOM) == at)
                .collect())
        };
        for p in under(&self.stage)? {
            self.stage.remove(p)?;
        }
        fill(&self.stage)?;
        let made: FnvHashSet<Tile> = under(&self.stage)?.into_iter().collect();
        let held = under(&self.set)?;
        self.write_done(
            &shard.name(),
            held.into_iter().filter(|p| !made.contains(p)),
        )
    }

    /// Folds again what the whole shards change.
    pub fn fold<L: Base>(&self, absent: &impl Absent<L>, workers: usize) -> Result<(), String> {
        self.not_committing()?;
        self.clear_fold(L::LEAF)?;
        let whole = self.whole()?;
        let tiles: FnvHashSet<Tile> = whole.iter().copied().map(shard_tile).collect();
        let mut changed: FnvHashSet<Tile> = self
            .stage
            .by_zoom()?
            .remove(&L::LEAF)
            .unwrap_or_default()
            .into_iter()
            .filter(|&p| tiles.contains(&ancestor(p, SHARD_ZOOM)))
            .collect();
        let leaves_gone = self.leaves_gone(&whole)?;
        changed.extend(leaves_gone.iter().copied());
        let mut staged = Staged {
            base: &self.set,
            stage: &self.stage,
            leaf: L::LEAF,
            whole: &tiles,
            gone: leaves_gone.clone(),
        };
        fold(&mut staged, changed, absent, workers)?;
        self.write_done(
            FOLD,
            staged.gone.into_iter().filter(|p| !leaves_gone.contains(p)),
        )
    }

    /// The leaves the whole shards' stages take away.
    fn leaves_gone(&self, whole: &[Node]) -> Result<FnvHashSet<Tile>, String> {
        let mut gone = FnvHashSet::default();
        for shard in whole {
            gone.extend(self.read_done(&shard.name())?);
        }
        Ok(gone)
    }

    /// Moves what is staged into the set and takes away what each `.done`
    /// lists, each `.done` last; only once folded, else the set would hold
    /// leaves under parents not folded from them.
    pub fn commit<L: Base>(&self, workers: usize) -> Result<(), String> {
        if !self.done(COMMIT).exists() {
            if !self.done(FOLD).exists() {
                return Err(format!(
                    "{}: nothing folded; run --stage fold first",
                    self.stage.root.display()
                ));
            }
            // The set as it will be, refused before anything moves.
            let whole = self.whole()?;
            let tiles: FnvHashSet<Tile> = whole.iter().copied().map(shard_tile).collect();
            let mut gone = self.leaves_gone(&whole)?;
            gone.extend(self.read_done(FOLD)?);
            let view = Staged {
                base: &self.set,
                stage: &self.stage,
                leaf: L::LEAF,
                whole: &tiles,
                gone,
            };
            let fits = L::fits_set(&view, workers)?;
            // Before anything moves: a commit begun is finished, never undone, so
            // the set will hold these, and a commit cut short leaves them said.
            if let Some(held) = self.will_hold()? {
                let path = self.set.root.join(MANIFEST);
                let mut manifest =
                    source::manifest(&self.set.root)?.unwrap_or_else(|| serde_json::json!({}));
                manifest["shard"] =
                    serde_json::json!(held.iter().map(|t| t.name()).collect::<Vec<_>>());
                fs::create_dir_all(&self.set.root).map_err(|e| io(&self.set.root, e))?;
                util::write_atomic(
                    &path,
                    serde_json::to_string_pretty(&manifest)
                        .map_err(|e| e.to_string())?
                        .as_bytes(),
                )
                .map_err(|e| io(&path, e))?;
            }
            // What the set will keep of its tiles, beside it until they are
            // all moved (`finish_commit`).
            if let Some(fits) = fits {
                let text: String = fits
                    .iter()
                    .map(|(p, fit)| format!("{} {fit}\n", p.name()))
                    .collect();
                let path = self.next_fits();
                fs::create_dir_all(&self.set.root).map_err(|e| io(&self.set.root, e))?;
                util::write_atomic(&path, text.as_bytes()).map_err(|e| io(&path, e))?;
            }
            fs::rename(self.done(FOLD), self.done(COMMIT)).map_err(|e| io(&self.done(FOLD), e))?;
        }
        self.finish_commit::<L>()
    }

    fn not_committing(&self) -> Result<(), String> {
        match self.done(COMMIT).exists() {
            true => Err(format!(
                "{}: a commit is begun; finish it with --stage commit",
                self.stage.root.display()
            )),
            false => Ok(()),
        }
    }

    fn finish_commit<L: Base>(&self) -> Result<(), String> {
        fs::create_dir_all(&self.set.root).map_err(|e| io(&self.set.root, e))?;
        let staged = self.stage.by_zoom()?;
        let put = |p: Tile| {
            fs::rename(self.stage.path(p), self.set.path(p)).map_err(|e| io(&self.stage.path(p), e))
        };
        for shard in self.whole()? {
            let at = shard_tile(shard);
            for &p in staged
                .get(&L::LEAF)
                .into_iter()
                .flatten()
                .filter(|&&p| ancestor(p, SHARD_ZOOM) == at)
            {
                put(p)?;
            }
            for p in self.read_done(&shard.name())? {
                self.set.remove(p)?;
            }
            self.remove_done(&shard.name())?;
        }
        for &p in staged
            .iter()
            .filter(|(z, _)| **z != L::LEAF)
            .flat_map(|(_, v)| v)
        {
            put(p)?;
        }
        for p in self.read_done(COMMIT)? {
            self.set.remove(p)?;
        }
        // The set's tiles all moved, what it keeps of them is theirs; the
        // stage's kept beside its tiles, read into it, go.
        let next = self.next_fits();
        if next.exists() {
            fs::rename(&next, self.set.root.join(FITS)).map_err(|e| io(&next, e))?;
        }
        for entry in fs::read_dir(&self.stage.root).map_err(|e| io(&self.stage.root, e))? {
            let path = entry.map_err(|e| io(&self.stage.root, e))?.path();
            if path.extension().is_some_and(|e| e == "fit") {
                fs::remove_file(&path).map_err(|e| io(&path, e))?;
            }
        }
        self.remove_done(COMMIT)
    }

    /// What the set will keep of its tiles (`FITS`) once a commit begun has
    /// moved them.
    fn next_fits(&self) -> PathBuf {
        self.set.root.join(format!("{FITS}.next"))
    }

    /// The set as a version is uploaded, into `to`, made empty first: every
    /// tile, less where `pruned` those no reader asks for.
    pub fn publish<L: Base>(&self, to: &Path, pruned: bool) -> Result<(), String> {
        if to.exists() {
            fs::remove_dir_all(to).map_err(|e| io(to, e))?;
        }
        fs::create_dir_all(to).map_err(|e| io(to, e))?;
        // A reader starts at the kind's coarsest, so no tile there goes, and
        // none coarser was ever read: a set baked before the fold stopped
        // there still holds them.
        let skip = if pruned {
            unasked::<L>(&self.set, L::COARSEST)?
        } else {
            FnvHashSet::default()
        };
        for p in self
            .set
            .by_zoom()?
            .into_values()
            .flatten()
            .filter(|p| p.zoom >= L::COARSEST && !skip.contains(p))
        {
            fs::hard_link(self.set.path(p), to.join(p.name()))
                .map_err(|e| io(&self.set.path(p), e))?;
        }
        // What the copy is, which the set may stop being once it changes again.
        let manifest = self.set.root.join(MANIFEST);
        fs::copy(&manifest, to.join(MANIFEST)).map_err(|e| io(&manifest, e))?;
        println!(
            "[bake] {}: {} tiles no reader asks for left out",
            to.display(),
            skip.len()
        );
        Ok(())
    }
}

/// What a bake is told to do: its stages (`--stage leaf`, `fold`, `commit`,
/// all three where none is named, and `publish` only where named), the
/// shards to bake (`--shard <name>`, repeated; every shard of the coverage
/// where none is), its set (`--out`) and how many tiles at once
/// (`--concurrency`).
pub struct Run {
    pub leaf: bool,
    pub fold: bool,
    pub commit: bool,
    pub publish: bool,
    pub shards: Vec<Node>,
    pub stage: Stage,
    pub concurrency: usize,
}

impl Run {
    pub fn parse(kind: &str) -> Result<Run, String> {
        let coverage = Coverage::of_source()?;
        let (mut stages, mut named, mut out, mut concurrency) = (
            Vec::new(),
            Vec::new(),
            PathBuf::from(format!("./out/{kind}")),
            8,
        );
        let mut flags = Flags::from_env();
        while let Some(flag) = flags.arg() {
            match flag.as_str() {
                "--stage" => stages.push(flags.val()?),
                "--shard" => named.push(flags.val()?),
                "--out" => out = PathBuf::from(flags.val()?),
                "--concurrency" => concurrency = flags.parse()?,
                other => return Err(format!("unknown flag: {other}")),
            }
        }
        if let Some(s) = stages
            .iter()
            .find(|s| !matches!(s.as_str(), "leaf" | "fold" | "commit" | "publish"))
        {
            return Err(format!("unknown stage: {s}"));
        }
        if let Some(n) = named
            .iter()
            .find(|n| !coverage.shards().any(|s| s.name() == **n))
        {
            return Err(format!(
                "{n}: no shard of the coverage (infra/geo/coverage.json)"
            ));
        }
        let all = stages.is_empty();
        let has = |s: &str| stages.iter().any(|t| t == s);
        Ok(Run {
            leaf: all || has("leaf"),
            fold: all || has("fold"),
            commit: all || has("commit"),
            publish: has("publish"),
            shards: coverage
                .shards()
                .filter(|s| named.is_empty() || named.contains(&s.name()))
                .collect(),
            stage: Stage::of(out),
            concurrency,
        })
    }

    /// The shards the set holds once what is staged is in: what its fold
    /// reads as baked, and nothing else.
    pub fn read_baked(&self) -> Result<Coverage, String> {
        let held = self.stage.will_hold()?.ok_or(format!(
            "{}: its manifest names no shards it holds, so which are baked is unknown",
            self.stage.set.root.display()
        ))?;
        Ok(Coverage::of_shards(held))
    }

    /// Every stage after the leaves; the set's manifest says what it was
    /// made from once it is committed.
    /// `absent` is asked only by a fold.
    pub fn finish<L: Base, A: Absent<L>>(
        &self,
        absent: impl FnOnce(&Run) -> Result<A, String>,
        reads: &[&[&str]],
        pruned: bool,
    ) -> Result<(), String> {
        if self.fold {
            self.stage.fold::<L>(&absent(self)?, self.concurrency)?;
        }
        if self.commit {
            self.stage.commit::<L>(self.concurrency)?;
            let mut fields =
                serde_json::json!({ "source": src_of(reads)?, "format": FORMAT.of(L::LAYER) });
            // The commit wrote them before it moved anything.
            if let Some(held) = source::manifest(&self.stage.set.root)?
                .as_ref()
                .map(source::held)
                .transpose()?
                .flatten()
            {
                fields["shard"] =
                    serde_json::json!(held.iter().map(|t| t.name()).collect::<Vec<_>>());
            }
            write_manifest(&self.stage.set, L::COARSEST, git_sha().as_deref(), fields)?;
        }
        if self.publish {
            let mut to = self.stage.set.root.clone().into_os_string();
            to.push(".publish");
            self.stage.publish::<L>(Path::new(&to), pruned)?;
        }
        Ok(())
    }
}
