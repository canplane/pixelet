// pxl-kernel/world/residency.rs

//! Residency: which of a layer's tiles are held, in which frame, and what
//! answers a tile that is not. Any tile fits any frame. Tiles the fallback
//! asks for are held hard; the rest is cache. A held tile is readable on its
//! own, so a missing ancestor never hides a resident descendant. A miss
//! falls back to the nearest resident tile over it (`drawn_from`).
//!
//! Capacity is asserted, not observed, since thrash here would be silent:
//! frames cover everything held hard plus everything demand can name at once
//! (the reach), so a view at rest converges to exact. Speculation only
//! ever takes frames nothing else holds.

pub mod floor;

use std::collections::HashMap;

use pxl_spec::space::lod::{reach_tiles, REACH_TILES};
use pxl_spec::space::pyramid::{Tile, LEAF_Z, TOP_Z};

use crate::io::demand::{Ask, Why};
use crate::world::pyramid::Layer;

/// The zooms the pyramid holds, z15 to its top.
pub const PYRAMID_ZOOMS: u32 = (LEAF_Z - TOP_Z + 1) as u32;

/// Only the eye. Another point observer would add its own misses.
const OBSERVERS: u32 = 1;

pub const ZOOM_DEMAND_TILES: u32 = REACH_TILES;

/// A destination's line from its leaf to the top.
pub const DESTINATION_TILES: u32 = PYRAMID_ZOOMS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Hold {
    Hard,
    Demand,
    Cache,
}

#[derive(Clone, Copy, Debug)]
pub struct Candidate {
    pub tile: Tile,
    pub hinted: bool,
}

/// Which frame goes when one must, chosen only among those that may, so a
/// policy changes what is held next and never what is correct.
pub trait Victim {
    fn victim(&self, candidates: &[Candidate]) -> Option<usize>;
}

/// `settles`: which of its four children it says the ground of. No frame
/// for an empty tile, whose body is its own.
struct Held<L> {
    body: L,
    frame: Option<u32>,
    settles: [bool; 4],
}

struct Entry<L> {
    held: Option<Held<L>>,
    ask: Option<Ask>,
    /// The bucket has no object for it, so the tile over it says its ground.
    answered: bool,
}

impl<L> Entry<L> {
    const NONE: Entry<L> = Entry {
        held: None,
        ask: None,
        answered: false,
    };

    fn idle(&self) -> bool {
        self.held.is_none() && self.ask.is_none()
    }
}

pub struct Residency<L: Layer> {
    tiles: HashMap<Tile, Entry<L>>,
    free: Vec<u32>,
    /// Kept so `ask` allocates nothing.
    going: Vec<Tile>,
    /// Moves whenever a tile is held, let go, or answered by the tile over it.
    revision: u32,
    refused: u32,
    admitted: u32,
    evicted: u32,
}

/// What a layer's residency holds at once, from what it asks for. A layer
/// that asks only for the floor (`Layer::DEMANDS` false) asks for no miss, no
/// line over one, no destination and nothing speculative.
impl<L: Layer> Residency<L> {
    const DEMAND_TILES: u32 = if L::DEMANDS { OBSERVERS * ZOOM_DEMAND_TILES * PYRAMID_ZOOMS } else { 0 };

    /// The lines over the misses, every zoom's but the finest.
    const ANCESTOR_TILES: u32 = if L::DEMANDS { OBSERVERS * ZOOM_DEMAND_TILES * (PYRAMID_ZOOMS - 1) } else { 0 };

    /// Held hard apart from the view: the floor and a destination's line. A new
    /// consumer that cannot take a coarser answer would declare its tiles here.
    const PINNED_TILES: u32 = reach_tiles(L::DRAWN) + if L::DEMANDS { DESTINATION_TILES } else { 0 };

    /// What is held hard at most.
    const HARD_TILES: u32 = Self::PINNED_TILES + Self::ANCESTOR_TILES;

    /// A frame for a tile nothing holds hard to land in. The floor alone holds
    /// every tile it asks for hard, and asks for at most `reach_tiles`, so a
    /// tile it asks for finds a frame free or held only as cache.
    const WORKSPACE: u32 = L::DEMANDS as u32;

    /// Convergence: the misses and their lines share each zoom's ring, so a tile
    /// the view reads never gives up another the view reads.
    pub const FRAMES: u32 = Self::PINNED_TILES + Self::DEMAND_TILES;

    /// A miss's line is among the demand's asks; speculation takes at most the frames.
    pub const ASKS: u32 = Self::PINNED_TILES + Self::DEMAND_TILES + if L::DEMANDS { Self::FRAMES } else { 0 };

    /// Tiles residency knows of at once: every framed tile and every asked one.
    const ENTRIES: u32 = Self::FRAMES + Self::ASKS;

    const SAFE: () = assert!(Self::FRAMES >= Self::HARD_TILES + Self::WORKSPACE, "safety: no frame for a tile nothing holds hard to land in");
}

impl<L: Layer> Default for Residency<L> {
    fn default() -> Self {
        let () = Self::SAFE;
        Residency {
            tiles: HashMap::with_capacity(Self::ENTRIES as usize),
            // Lowest first, as the frames are taken.
            free: (0..Self::FRAMES).rev().collect(),
            going: Vec::with_capacity(Self::ENTRIES as usize),
            revision: 0,
            refused: 0,
            admitted: 0,
            evicted: 0,
        }
    }
}

fn line(p: Tile) -> impl Iterator<Item = Tile> {
    std::iter::successors(Some(p), |q| (q.zoom > TOP_Z).then(|| q.parent()))
}

impl<L: Layer> Residency<L> {
    fn entry(&self, p: Tile) -> Option<&Entry<L>> {
        self.tiles.get(&p)
    }

    /// Past `ENTRIES` is a tile kept for no reason the bound counts.
    fn entry_mut(&mut self, p: Tile) -> &mut Entry<L> {
        let full = self.tiles.len() >= Self::ENTRIES as usize;
        self.tiles.entry(p).or_insert_with(|| {
            xpute_core::ensure!(!full, ENOTRECOVERABLE, p.zoom);
            Entry::NONE
        })
    }

    fn held_as(&self, p: Tile) -> Option<&Held<L>> {
        self.entry(p)?.held.as_ref()
    }

    pub fn resident(&self, p: Tile) -> bool {
        self.held_as(p).is_some()
    }

    /// Held in a frame of its own, not as an empty tile.
    fn framed(&self, p: Tile) -> bool {
        self.held_as(p).is_some_and(|h| h.frame.is_some())
    }

    /// The frame a held tile is in; none for an empty one.
    pub fn frame(&self, p: Tile) -> Option<u32> {
        self.held_as(p)?.frame
    }

    pub fn get(&self, p: Tile) -> Option<&L> {
        self.held_as(p).map(|h| &h.body)
    }

    pub fn held(&self) -> impl Iterator<Item = (Tile, &L)> {
        self.tiles.iter().filter_map(|(&p, e)| Some((p, &e.held.as_ref()?.body)))
    }

    pub fn frames_used(&self) -> u32 {
        Self::FRAMES - self.free.len() as u32
    }

    pub fn revision(&self) -> u32 {
        self.revision
    }

    /// Answered for itself, the top too, or settled by the tile over it.
    fn said(&self, q: Tile) -> bool {
        if self.entry(q).is_some_and(|e| e.answered) {
            return true;
        }
        if q.zoom <= TOP_Z {
            return false;
        }
        let over = q.parent();
        self.held_as(over).is_some_and(|h| over.children().iter().zip(h.settles).any(|(&c, s)| c == q && s))
    }

    pub fn settled(&self, p: Tile) -> bool {
        line(p).any(|q| self.said(q))
    }

    /// The finest resident tile on `p`'s line: the miss's answer.
    pub fn drawn_from(&self, p: Tile) -> Option<Tile> {
        line(p).find(|&q| self.resident(q))
    }

    /// The resident tile that says `p`'s ground exactly; none where `p` is a
    /// miss. A resident tile above that does not say `p` only draws it coarser.
    pub fn source(&self, p: Tile) -> Option<Tile> {
        if self.resident(p) {
            return Some(p);
        }
        let mut none = line(p).find(|&q| self.said(q))?;
        loop {
            let over = none.parent();
            if self.resident(over) {
                return Some(over);
            }
            if !self.said(over) {
                return None;
            }
            none = over;
        }
    }

    /// Empty tiles and tiles answered above are kept only while asked:
    /// they cost no frame, so nothing would ever evict them. Framed tiles are
    /// left to normal turnover.
    pub fn ask(&mut self, asks: &HashMap<Tile, Ask>) {
        xpute_core::ensure!(asks.len() <= Self::ASKS as usize, ENOTRECOVERABLE, asks.len());
        let mut changed = false;
        for (p, e) in &mut self.tiles {
            e.ask = asks.get(p).copied();
            if e.ask.is_none() && e.answered {
                e.answered = false;
                changed = true;
            }
        }
        let mut going = core::mem::take(&mut self.going);
        going.clear();
        going.extend(
            self.tiles
                .iter()
                .filter(|&(_, e)| e.ask.is_none() && e.held.as_ref().is_some_and(|h| h.frame.is_none()))
                .map(|(&p, _)| p),
        );
        for &p in &going {
            self.remove(p);
            changed = true;
        }
        self.going = going;
        self.tiles.retain(|_, e| !e.idle());
        for (&p, &a) in asks {
            self.entry_mut(p).ask = Some(a);
        }
        if changed {
            self.revision = self.revision.wrapping_add(1);
        }
    }

    /// A tile not yet held counts as taking one: whether it is empty is known only on arrival.
    pub fn takes_frame(&self, p: Tile) -> bool {
        !self.settled(p) && (self.framed(p) || !self.resident(p))
    }

    pub fn asked(&self, p: Tile) -> Option<Ask> {
        self.entry(p)?.ask
    }

    fn hold(&self, p: Tile) -> Hold {
        let Some(e) = self.entry(p) else { return Hold::Cache };
        match e.ask.map(|a| a.why) {
            Some(Why::Floor | Why::Destination | Why::Ancestor) => Hold::Hard,
            Some(Why::Demand) => Hold::Demand,
            Some(Why::Hint) | None => Hold::Cache,
        }
    }

    fn hinted(&self, p: Tile) -> bool {
        self.asked(p).is_some_and(|a| a.why == Why::Hint)
    }

    /// A frame for arriving `p`: its own, a free one, or a victim among frames
    /// held only as cache (speculation never evicts other speculation). None
    /// only for a speculative tile, counted; for anything stronger, none means
    /// the capacity bound is broken.
    fn frame_for(&mut self, p: Tile, victim: &impl Victim) -> Option<u32> {
        if self.framed(p) {
            return self.remove(p);
        }
        if let Some(free) = self.free.pop() {
            return Some(free);
        }
        let speculative = self.hold(p) == Hold::Cache;
        let mut candidates: Vec<Candidate> = self
            .tiles
            .iter()
            .filter(|&(&q, _)| self.framed(q) && self.hold(q) == Hold::Cache)
            .map(|(&q, _)| Candidate { tile: q, hinted: self.hinted(q) })
            .filter(|c| !(speculative && c.hinted))
            .collect();
        // Sorted, so the policy sees the same list for the same state.
        candidates.sort_unstable_by_key(|c| c.tile);
        let Some(at) = victim.victim(&candidates) else {
            xpute_core::ensure!(speculative, ENOTRECOVERABLE, p.zoom);
            self.refused += 1;
            return None;
        };
        self.evicted = self.evicted.wrapping_add(1);
        self.remove(candidates[at].tile)
    }

    pub fn churn(&self) -> [u32; 2] {
        [self.admitted, self.evicted]
    }

    pub fn take_refusals(&mut self) -> u32 {
        core::mem::take(&mut self.refused)
    }

    /// The frame is emptied first, so a body that fails to decode leaves it free
    /// rather than another tile reading half-overwritten bytes.
    pub fn admit<E>(&mut self, p: Tile, victim: &impl Victim, receive: impl FnOnce(u32) -> Result<L, E>) -> Result<(), E> {
        let Some(record) = self.frame_for(p, victim) else { return Ok(()) };
        match receive(record) {
            Ok(tile) => {
                self.admitted = self.admitted.wrapping_add(1);
                self.put(p, tile, Some(record));
                Ok(())
            }
            Err(e) => {
                self.free.push(record);
                Err(e)
            }
        }
    }

    /// Held only while something asks for it: nothing else would ever let it go.
    pub fn set(&mut self, p: Tile, tile: L) {
        if self.asked(p).is_none() {
            return;
        }
        if let Some(record) = self.remove(p) {
            self.free.push(record);
        }
        self.put(p, tile, None);
    }

    /// Held, or answered by the tile over it: nothing more will come for it.
    pub fn has_answer(&self, p: Tile) -> bool {
        self.resident(p) || self.entry(p).is_some_and(|e| e.answered)
    }

    pub fn answer_above(&mut self, p: Tile) {
        if self.asked(p).is_none() {
            return;
        }
        let e = self.entry_mut(p);
        if !core::mem::replace(&mut e.answered, true) {
            self.revision = self.revision.wrapping_add(1);
        }
    }

    fn put(&mut self, p: Tile, body: L, frame: Option<u32>) {
        let settles = if p.zoom < LEAF_Z { p.children().map(|c| body.settles(p, c)) } else { [false; 4] };
        self.entry_mut(p).held = Some(Held { body, frame, settles });
        self.revision = self.revision.wrapping_add(1);
        L::changed(p, true);
    }

    /// Returns the frame `p` was in, if any, for the caller to fill or free.
    fn remove(&mut self, p: Tile) -> Option<u32> {
        let std::collections::hash_map::Entry::Occupied(mut e) = self.tiles.entry(p) else { return None };
        let held = e.get_mut().held.take()?;
        if e.get().idle() {
            e.remove();
        }
        self.revision = self.revision.wrapping_add(1);
        L::changed(p, false);
        held.frame
    }
}

#[cfg(test)]
#[path = "residency.test.rs"]
mod test;
