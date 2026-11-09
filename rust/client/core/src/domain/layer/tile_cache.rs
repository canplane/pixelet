// pxl-core/domain/layer/tile_cache.rs

//! A layer's tile cache. It records which of the layer's tiles are held, in
//! which slot, and what answers a tile that is not held. Any tile fits any
//! slot. A tile the current demand asks for is held hard and is never
//! evicted; a held tile nothing asks for is cache, bytes kept in case it is
//! asked for again, and it answers nothing.
//!
//! A tile is readable only while it is both held and asked for, and each tile
//! is judged on its own, so a missing ancestor never hides a resident
//! descendant. What a reader sees therefore depends only on what the current
//! observation asks for, never on where the eye has been. A tile that is
//! asked for and not held is a miss, and is drawn from the nearest resident
//! tile over it (`finest_resident`).
//!
//! The slot count is checked at compile time to cover every tile held hard
//! plus every tile the demand can name at once, so a view at rest always
//! converges to exact tiles.

use fnv::FnvHashMap;
use pxl_spec::bake::tile::Occupancy;

use pxl_spec::space::coord::zoom::LEAF_ZOOM;
use pxl_spec::space::layer::demand::demand_tiles;
use pxl_spec::space::layer::quadrant::Quadrant;

use crate::domain::layer::floor;
use crate::domain::layer::{AskScope, Base};

/// The number of observers whose demand the cache is sized for: the eye alone.
const OBSERVERS: u32 = 1;

/// A held tile. An empty tile takes no slot, since its body holds all of it.
/// A tile marked `refetch` is fetched once more and stays readable until the
/// new copy arrives.
struct Resident<L> {
    body: L,
    slot: Option<u32>,
    refetch: bool,
}

struct Entry<L> {
    resident: Option<Resident<L>>,
    asked: bool,
    /// The bucket holds no object for this tile, so the tile over it gives
    /// its ground.
    answered: bool,
}

impl<L> Entry<L> {
    const NONE: Entry<L> = Entry {
        resident: None,
        asked: false,
        answered: false,
    };

    fn idle(&self) -> bool {
        self.resident.is_none() && !self.asked
    }

    /// Whether the tile is held in a slot of its own rather than as an empty
    /// tile.
    fn is_slotted(&self) -> bool {
        self.resident.as_ref().is_some_and(|h| h.slot.is_some())
    }
}

pub struct TileCache<L: Base> {
    tiles: FnvHashMap<Quadrant, Entry<L>>,
    /// The tile each slot holds. It is the inverse of `Resident::slot`, and
    /// only `put` and `remove` write either side, so the two always agree.
    /// The GPU hand-off starts from a slot and walks this side (earth.rs
    /// `upload`).
    in_slot: Vec<Option<Quadrant>>,
    free: Vec<u32>,
    /// A buffer kept between calls so that `ask` allocates nothing.
    going: Vec<Quadrant>,
    /// The tiles whose answer changed since the last `take_changed`: a tile
    /// came to be held or was let go, or came to be answered from above or
    /// stopped being. The content of a given tile never changes, so these are
    /// the only changes a reader has to follow.
    changed: Vec<Quadrant>,
    /// A counter that advances whenever what the cache holds changes: a tile
    /// held or let go, a held tile asked for or no longer asked for, a tile
    /// answered from above. A change reported through `changed_at` does not
    /// advance it, since the cache itself holds nothing new then.
    revision: u32,
    /// A counter that advances on every `ask`. On an ask, `revision` advances
    /// only where a held tile's answer changed.
    ask_revision: u32,
    /// The last `count_shown` and the two revisions it was counted at. Read it
    /// only through `shown`, which counts again once either revision moved.
    shown: core::cell::Cell<Option<([u32; 2], [u32; 2])>>,
    refusals: u32,
    /// The tiles taken in since the last `take_loads`, for the profiler.
    loads: u32,
}

/// The cache's capacity, derived from what the layer asks for
/// (`Base::ASK_SCOPE`). A layer holds only what it asks for.
impl<L: Base> TileCache<L> {
    /// The number of zooms in the layer's pyramid, from its leaves up to the
    /// coarsest zoom it is read at.
    pub const ZOOMS: u32 = (L::LEAF - L::COARSEST + 1);

    /// The tiles on a destination's line: one at each zoom from its leaf to
    /// the top.
    pub const DST_TILES: u32 = Self::ZOOMS;

    /// The most tiles the demand at one zoom names, as the layer's lattice
    /// bounds it.
    pub const ZOOM_DEMAND_TILES: u32 = demand_tiles(L::LATTICE);

    const DEMAND_TILES: u32 = if matches!(L::ASK_SCOPE, AskScope::View) {
        OBSERVERS * Self::ZOOM_DEMAND_TILES * Self::ZOOMS
    } else {
        0
    };

    /// The most tiles held hard on the lines over the misses: up to one zoom's
    /// demand at every zoom but the finest.
    const ANCESTOR_TILES: u32 = if matches!(L::ASK_SCOPE, AskScope::View) {
        OBSERVERS * Self::ZOOM_DEMAND_TILES * (Self::ZOOMS - 1)
    } else {
        0
    };

    /// The tiles held hard apart from the view's demand: the floor
    /// (layer/floor.rs), a destination's line, and the tiles a reader
    /// that cannot take a coarser answer declares (`Base::PIN_CAP`).
    const PINNED_TILES: u32 = match L::ASK_SCOPE {
        AskScope::Floor => floor::tiles(L::LATTICE),
        AskScope::Line => Self::DST_TILES + L::PIN_CAP,
        AskScope::View => floor::tiles(L::LATTICE) + Self::DST_TILES + L::PIN_CAP,
    };

    /// The most tiles held hard at once.
    const HARD_TILES: u32 = Self::PINNED_TILES + Self::ANCESTOR_TILES;

    /// One spare slot, so that a tile nothing holds hard still has a slot to
    /// land in. A layer that asks for the floor alone needs none: it holds
    /// hard every tile it asks for and asks for at most `floor::tiles`, so
    /// each of them finds a slot that is free or held only as cache.
    const WORKSPACE: u32 = matches!(L::ASK_SCOPE, AskScope::View) as u32;

    /// The misses and the lines over them share each zoom's demand, so a
    /// tile the view reads never takes the slot of another tile the view
    /// reads, and the view converges.
    pub const SLOTS: u32 = Self::PINNED_TILES + Self::DEMAND_TILES;

    /// The most tiles the cache keeps an entry for: every slotted tile and
    /// every asked one. No more tiles are asked for than there are slots.
    const ENTRIES: u32 = 2 * Self::SLOTS;

    const SAFE: () = assert!(
        Self::SLOTS >= Self::HARD_TILES + Self::WORKSPACE,
        "safety: no slot for a tile nothing holds hard to land in"
    );
}

impl<L: Base> Default for TileCache<L> {
    fn default() -> Self {
        let () = Self::SAFE;
        TileCache {
            tiles: FnvHashMap::with_capacity_and_hasher(Self::ENTRIES as usize, Default::default()),
            in_slot: vec![None; Self::SLOTS as usize],
            // Reversed, so that `pop` hands out the lowest slot first.
            free: (0..Self::SLOTS).rev().collect(),
            going: Vec::with_capacity(Self::ENTRIES as usize),
            changed: Vec::new(),
            revision: 0,
            ask_revision: 0,
            shown: core::cell::Cell::new(None),
            refusals: 0,
            loads: 0,
        }
    }
}

/// What the cache knows of a tile from its line: the tile and every tile
/// over it up to the layer's top. A child's line is computed from its
/// parent's line and the child's own entry alone.
///
/// A tile is absent when it has no data of its own: the bucket answered that
/// it holds nothing for it, or its parent's occupancy has no child there. A
/// tile over it then gives its ground.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line {
    pub tile: Quadrant,
    /// The finest resident tile on the line, which the tile is drawn from.
    pub finest_resident: Option<Quadrant>,
    /// The resident tile that gives this tile's ground exactly: the tile
    /// itself, or a tile over it that an absent tile defers to.
    pub src: Option<Quadrant>,
    /// Whether some tile on the line is absent.
    pub settled: bool,
    absent: bool,
    /// The tile that gives this tile's ground if it is absent: the nearest
    /// resident tile over it, reached through absent tiles only.
    fallback: Option<Quadrant>,
    /// The `fallback` of the nearest absent tile at or over this one.
    absent_fallback: Option<Quadrant>,
    /// Which children have tiles of their own, when the tile is resident.
    children: Option<Occupancy>,
}

/// The lines of the last tile a walk reached and of every tile over it, by
/// zoom. A walk to a nearby tile then steps down only from where the two
/// lines part. It stays valid only while the tile cache it was walked over is
/// unchanged.
#[derive(Clone, Copy)]
pub struct Path {
    at: [Option<Line>; LEAF_ZOOM as usize + 1],
}

impl Default for Path {
    fn default() -> Self {
        Path {
            at: [None; LEAF_ZOOM as usize + 1],
        }
    }
}

impl Line {
    /// The line over the layer's top, where nothing is known.
    const ABOVE: Line = Line {
        tile: Quadrant {
            zoom: 0,
            at: [0, 0],
        },
        finest_resident: None,
        src: None,
        settled: false,
        absent: false,
        fallback: None,
        absent_fallback: None,
        children: None,
    };

    pub fn resident(&self) -> bool {
        self.children.is_some()
    }
}

impl<L: Base> TileCache<L> {
    fn entry(&self, p: Quadrant) -> Option<&Entry<L>> {
        self.tiles.get(&p)
    }

    /// A new entry past `ENTRIES` means a tile is kept for a reason the
    /// capacity bound does not count, and the core stops as unrecoverable.
    fn entry_mut(&mut self, p: Quadrant) -> &mut Entry<L> {
        let full = self.tiles.len() >= Self::ENTRIES as usize;
        self.tiles.entry(p).or_insert_with(|| {
            xpute_kit::ensure!(!full, ENOTRECOVERABLE, p.zoom);
            Entry::NONE
        })
    }

    /// The tile a reader may read: one both held and asked for.
    fn readable(&self, p: Quadrant) -> Option<&Resident<L>> {
        self.entry(p).filter(|e| e.asked)?.resident.as_ref()
    }

    /// Whether `p` is readable and not marked `refetch`, so that nothing more
    /// will be fetched for it.
    pub fn resident(&self, p: Quadrant) -> bool {
        self.readable(p).is_some_and(|h| !h.refetch)
    }

    fn is_slotted(&self, p: Quadrant) -> bool {
        self.entry(p).is_some_and(Entry::is_slotted)
    }

    /// The slot a held tile is in; none for an empty one.
    pub(in crate::domain::layer) fn slot(&self, p: Quadrant) -> Option<u32> {
        self.readable(p)?.slot
    }

    pub(in crate::domain::layer) fn get(&self, p: Quadrant) -> Option<&L> {
        self.readable(p).map(|h| &h.body)
    }

    /// `p`'s line, computed down from the top one tile at a time.
    pub fn line(&self, p: Quadrant) -> Line {
        self.seek(&mut Path::default(), p)
    }

    /// `p`'s line, computed down from the finest tile over `p` whose line
    /// `path` still holds. Afterwards `path` holds `p`'s line too.
    pub fn seek(&self, path: &mut Path, p: Quadrant) -> Line {
        let line_at = |q: Quadrant| path.at[q.zoom as usize].filter(|l| l.tile == q);
        let line = match line_at(p) {
            Some(l) => l,
            None if p.zoom > L::COARSEST => {
                let parent = self.seek(path, p.parent());
                self.step(&parent, p)
            }
            None => self.step(&Line::ABOVE, p),
        };
        path.at[p.zoom as usize] = Some(line);
        line
    }

    /// `child`'s line from its parent's line. It reads only `child`'s own
    /// entry, so a descent costs one lookup per zoom and never walks back up.
    pub(in crate::domain::layer) fn step(&self, parent: &Line, child: Quadrant) -> Line {
        let e = self.entry(child);
        let resident = e.filter(|e| e.asked).and_then(|e| e.resident.as_ref());
        let absent = e.is_some_and(|e| e.answered)
            || parent.children.is_some_and(|o| !o.has_child(child.slot()));
        let fallback = match parent.resident() {
            true => Some(parent.tile),
            false if parent.absent => parent.fallback,
            false => None,
        };
        let absent_fallback = if absent {
            fallback
        } else {
            parent.absent_fallback
        };
        Line {
            tile: child,
            finest_resident: if resident.is_some() {
                Some(child)
            } else {
                parent.finest_resident
            },
            src: if resident.is_some() {
                Some(child)
            } else {
                absent_fallback
            },
            settled: absent || parent.settled,
            absent,
            fallback,
            absent_fallback,
            children: resident.map(|h| h.body.occupancy()),
        }
    }

    /// The finest resident tile on `p`'s line, which answers `p` when `p` is
    /// a miss.
    pub fn finest_resident(&self, p: Quadrant) -> Option<Quadrant> {
        self.line(p).finest_resident
    }

    /// The resident tile that gives `p`'s ground exactly, or none where `p`
    /// is a miss. A resident tile over `p` that does not give its ground only
    /// draws `p` coarser.
    pub fn src(&self, p: Quadrant) -> Option<Quadrant> {
        self.line(p).src
    }

    /// Sets the tiles asked for to the keys of `asks`; the values are the
    /// asker's own. Empty tiles and tiles answered from above are dropped as
    /// soon as nothing asks for them: they take no slot, so eviction would
    /// never reach them. A slotted tile stays as cache until its slot is
    /// needed.
    pub fn ask<A>(&mut self, asks: &FnvHashMap<Quadrant, A>) {
        xpute_kit::ensure!(
            asks.len() <= Self::SLOTS as usize,
            ENOTRECOVERABLE,
            asks.len()
        );
        self.ask_revision = self.ask_revision.wrapping_add(1);
        for (&p, e) in &mut self.tiles {
            let was = e.asked;
            e.asked = asks.contains_key(&p);
            // A held tile that starts or stops being asked for starts or stops
            // being readable, which changes its answer as much as its arriving
            // or leaving would.
            let read = e.resident.is_some() && was != e.asked;
            if read | (!e.asked && core::mem::take(&mut e.answered)) {
                self.changed.push(p);
                self.revision = self.revision.wrapping_add(1);
            }
        }
        let mut going = core::mem::take(&mut self.going);
        going.clear();
        going.extend(
            self.tiles
                .iter()
                .filter(|&(_, e)| !e.asked && e.resident.as_ref().is_some_and(|h| h.slot.is_none()))
                .map(|(&p, _)| p),
        );
        for &p in &going {
            self.remove(p);
        }
        self.going = going;
        self.tiles.retain(|_, e| !e.idle());
        for &p in asks.keys() {
            self.entry_mut(p).asked = true;
        }
    }

    fn is_asked(&self, p: Quadrant) -> bool {
        self.entry(p).is_some_and(|e| e.asked)
    }

    /// A slot for arriving tile `p`: the slot it already holds, a free one, or
    /// one taken from a victim held only as cache. It returns none only for a
    /// tile no longer asked for, and counts that as a refusal. For a tile that
    /// is asked for, finding no slot means the capacity bound is broken, and
    /// the core stops.
    fn slot_for(
        &mut self,
        p: Quadrant,
        victim: impl FnOnce(&mut dyn Iterator<Item = Quadrant>) -> Option<Quadrant>,
    ) -> Option<u32> {
        if self.is_slotted(p) {
            return self.remove(p);
        }
        if let Some(free) = self.free.pop() {
            return Some(free);
        }
        let unasked = !self.is_asked(p);
        let Some(q) = victim(
            &mut self
                .tiles
                .iter()
                .filter(|(_, e)| e.is_slotted() && !e.asked)
                .map(|(&q, _)| q),
        ) else {
            xpute_kit::ensure!(unasked, ENOTRECOVERABLE, p.zoom);
            self.refusals += 1;
            return None;
        };
        self.remove(q)
    }

    pub fn take_refusals(&mut self) -> u32 {
        core::mem::take(&mut self.refusals)
    }

    /// The number of tiles taken in since the last call, in a slot or as an
    /// empty tile, for the profiler (perf/report.rs `perf_frame`).
    pub fn take_loads(&mut self) -> u32 {
        core::mem::take(&mut self.loads)
    }

    /// Takes arriving tile `p` into a slot that `receive` fills. The slot is
    /// emptied before `receive` runs, so a body that fails to decode leaves
    /// the slot free and no other tile reads half-overwritten bytes.
    ///
    /// When no slot is free, `victim` picks the tile to evict from those held
    /// only as cache, or none to refuse. The candidates come in hash order,
    /// so a policy must pick by a total order of its own.
    pub fn admit<E>(
        &mut self,
        p: Quadrant,
        victim: impl FnOnce(&mut dyn Iterator<Item = Quadrant>) -> Option<Quadrant>,
        receive: impl FnOnce(u32) -> Result<L, E>,
    ) -> Result<(), E> {
        let Some(slot) = self.slot_for(p, victim) else {
            return Ok(());
        };
        match receive(slot) {
            Ok(tile) => {
                self.put(p, tile, Some(slot));
                Ok(())
            }
            Err(e) => {
                self.free.push(slot);
                Err(e)
            }
        }
    }

    /// Holds `p` as an empty tile, which takes no slot. A tile nothing asks
    /// for is not held.
    pub fn set(&mut self, p: Quadrant, tile: L) {
        if !self.is_asked(p) {
            return;
        }
        if let Some(slot) = self.remove(p) {
            self.free.push(slot);
        }
        self.put(p, tile, None);
    }

    /// Whether `p` is resident or answered by the tile over it, so that
    /// nothing more will come for it.
    pub fn has_answer(&self, p: Quadrant) -> bool {
        self.resident(p) || self.entry(p).is_some_and(|e| e.answered)
    }

    /// Whether every tile asked for shows. A tile shows when it has its
    /// answer, or when a resident tile over it draws it coarser
    /// (`finest_resident`), as a finer tile asked for while the eye moves is
    /// drawn.
    pub fn is_shown(&self) -> bool {
        let [asked, shown] = self.shown();
        shown == asked
    }

    /// The share of the tiles asked for that show, as `is_shown` counts them,
    /// from 0 to 1. It is 0 while nothing is asked for, and it falls when
    /// more is asked for.
    pub fn shown_share(&self) -> f64 {
        match self.shown() {
            [0, _] => 0.0,
            [asked, shown] => f64::from(shown) / f64::from(asked),
        }
    }

    /// The number of tiles asked for and the number of them that show,
    /// counted once for each state of the asks and of what is held.
    fn shown(&self) -> [u32; 2] {
        let now = [self.revision, self.ask_revision];
        match self.shown.get() {
            Some((at, cnt)) if at == now => cnt,
            _ => {
                let cnt = self.count_shown();
                self.shown.set(Some((now, cnt)));
                cnt
            }
        }
    }

    fn count_shown(&self) -> [u32; 2] {
        let (mut path, mut cnt) = (Path::default(), [0u32; 2]);
        for p in self.asked() {
            cnt[0] += 1;
            if self.has_answer(p) || self.seek(&mut path, p).finest_resident.is_some() {
                cnt[1] += 1;
            }
        }
        cnt
    }

    fn asked(&self) -> impl Iterator<Item = Quadrant> + '_ {
        self.tiles.iter().filter(|(_, e)| e.asked).map(|(&p, _)| p)
    }

    pub fn answer_above(&mut self, p: Quadrant) {
        if !self.is_asked(p) {
            return;
        }
        if !core::mem::replace(&mut self.entry_mut(p).answered, true) {
            self.changed.push(p);
            self.revision = self.revision.wrapping_add(1);
        }
    }

    /// The cache's revision. Two readings are equal only if nothing the cache
    /// holds changed between them.
    pub fn revision(&self) -> u32 {
        self.revision
    }

    /// Moves into `out` the tiles whose answer changed since the last call.
    pub fn take_changed(&mut self, out: &mut Vec<Quadrant>) {
        out.clear();
        core::mem::swap(out, &mut self.changed);
    }

    fn put(&mut self, p: Quadrant, body: L, slot: Option<u32>) {
        if let Some(was) = self
            .entry_mut(p)
            .resident
            .replace(Resident {
                body,
                slot,
                refetch: false,
            })
            .and_then(|h| h.slot)
        {
            self.in_slot[was as usize] = None;
        }
        if let Some(f) = slot {
            self.in_slot[f as usize] = Some(p);
        }
        self.changed.push(p);
        self.revision = self.revision.wrapping_add(1);
        self.loads = self.loads.wrapping_add(1);
    }

    /// Marks held slotted tile `p` to be fetched once more; it stays readable
    /// until the new copy arrives. A layer that keeps nothing of a packet
    /// calls this when it needs the packet again. A newly marked tile is
    /// reported as changed, so the layer's next observation asks for it.
    pub fn refetch(&mut self, p: Quadrant) {
        if let Some(h) = self
            .tiles
            .get_mut(&p)
            .and_then(|e| e.resident.as_mut())
            .filter(|h| h.slot.is_some())
        {
            if !core::mem::replace(&mut h.refetch, true) {
                self.changed.push(p);
            }
        }
    }

    pub fn needs_refetch(&self, p: Quadrant) -> bool {
        self.readable(p).is_some_and(|h| h.refetch)
    }

    /// The number of readable tiles marked `refetch`, whose new copy has not
    /// come yet, among those `counts` accepts.
    pub fn asked_again(&self, counts: impl Fn(Quadrant) -> bool) -> u32 {
        self.tiles
            .iter()
            .filter(|(&p, e)| {
                e.asked && e.resident.as_ref().is_some_and(|h| h.refetch) && counts(p)
            })
            .count() as u32
    }

    /// Reports node `p` as changed when something read over it changed
    /// outside the cache, such as a clipmap brick, which is a node finer than
    /// a tile, as a tile's arriving would be reported.
    pub fn changed_at(&mut self, p: Quadrant) {
        self.changed.push(p);
    }

    /// The tile held in slot `slot` and what it holds.
    pub(in crate::domain::layer) fn in_slot(&mut self, slot: u32) -> Option<(Quadrant, &mut L)> {
        let p = self.in_slot[slot as usize]?;
        let held = self.tiles.get_mut(&p)?.resident.as_mut()?;
        Some((p, &mut held.body))
    }

    /// Returns the slot `p` was in, if any, for the caller to fill or free.
    fn remove(&mut self, p: Quadrant) -> Option<u32> {
        let std::collections::hash_map::Entry::Occupied(mut e) = self.tiles.entry(p) else {
            return None;
        };
        let resident = e.get_mut().resident.take()?;
        if e.get().idle() {
            e.remove();
        }
        if let Some(f) = resident.slot {
            self.in_slot[f as usize] = None;
        }
        self.changed.push(p);
        self.revision = self.revision.wrapping_add(1);
        resident.slot
    }
}

#[cfg(test)]
#[path = "tile_cache.test.rs"]
mod test;
