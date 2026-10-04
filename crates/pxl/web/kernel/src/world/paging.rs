// pxl-kernel/world/paging.rs

//! Residency: which of a layer's pages are held, in which frame, and what
//! answers a page that is not. Any page fits any frame. Pages the fallback
//! asks for are held hard; the rest is cache. A held page is readable on its
//! own, so a missing ancestor never hides a resident descendant. A fault
//! falls back to the nearest resident page over it (`drawn_from`).
//!
//! Capacity is asserted, not observed, since thrash here would be silent:
//! frames cover everything held hard plus everything demand can name at once
//! (the fault bound), so a view at rest converges to exact. Speculation only
//! ever takes frames nothing else holds.

pub mod floor;

use std::collections::HashMap;

use pxl_spec::space::lod::FAULT_PAGES;
use pxl_spec::space::pyramid::{Page, GROUND_Z, TOP_Z};

use crate::io::demand::{Ask, Why};
use crate::world::pyramid::Layer;
use floor::{Fallback, Floor};

/// The record shared by every page the bake left empty; written once, never a frame.
pub const ABSENT: u32 = 0;

/// The zooms the pyramid holds, z16 to its top.
pub const PYRAMID_ZOOMS: u32 = GROUND_Z - TOP_Z + 1;

/// Only the eye. Another point observer would add its own faults.
const OBSERVERS: u32 = 1;

pub const ZOOM_DEMAND_PAGES: u32 = FAULT_PAGES;

/// A destination's line from its z16 page to the top.
pub const DESTINATION_PAGES: u32 = PYRAMID_ZOOMS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Hold {
    Hard,
    Demand,
    Cache,
}

#[derive(Clone, Copy, Debug)]
pub struct Candidate {
    pub page: Page,
    pub hinted: bool,
}

/// Which frame goes when one must, chosen only among those that may, so a
/// policy changes what is held next and never what is correct.
pub trait Victim {
    fn victim(&self, candidates: &[Candidate]) -> Option<usize>;
}

/// `settles`: which of its four children it says the ground of.
struct Held<L> {
    body: L,
    record: u32,
    settles: [bool; 4],
}

struct Entry<L> {
    held: Option<Held<L>>,
    ask: Option<Ask>,
    /// The bucket has no object for it, so the page over it says its ground.
    answered: bool,
    /// The bucket has no object for it, and it reads the body every such
    /// page shares, once that has come.
    absent: bool,
}

impl<L> Entry<L> {
    const NONE: Entry<L> = Entry {
        held: None,
        ask: None,
        answered: false,
        absent: false,
    };

    fn idle(&self) -> bool {
        self.held.is_none() && self.ask.is_none()
    }
}

pub struct Paging<L: Layer> {
    pages: HashMap<Page, Entry<L>>,
    /// The body every absent page reads, decoded once into `ABSENT`.
    shared: Option<L>,
    free: Vec<u32>,
    /// Kept so `ask` allocates nothing.
    going: Vec<Page>,
    /// Moves whenever a page is held, let go, or answered by the page over it.
    revision: u32,
    refused: u32,
    admitted: u32,
    evicted: u32,
}

/// What a layer's residency holds at once, from what it asks for. A layer
/// that asks only for the floor (`Layer::DEMANDS` false) asks for no fault, no
/// line over one, no destination and nothing speculative.
impl<L: Layer> Paging<L> {
    const DEMAND_PAGES: u32 = if L::DEMANDS { OBSERVERS * ZOOM_DEMAND_PAGES * PYRAMID_ZOOMS } else { 0 };

    /// The lines over the faults, every zoom's but the finest.
    const ANCESTOR_PAGES: u32 = if L::DEMANDS { OBSERVERS * ZOOM_DEMAND_PAGES * (PYRAMID_ZOOMS - 1) } else { 0 };

    /// Held hard apart from the view: the floor and a destination's line. A new
    /// consumer that cannot take a coarser answer would declare its pages here.
    const PINNED_PAGES: u32 = Floor::PAGES + if L::DEMANDS { DESTINATION_PAGES } else { 0 };

    /// What is held hard at most.
    const HARD_PAGES: u32 = Self::PINNED_PAGES + Self::ANCESTOR_PAGES;

    /// A frame for a page nothing holds hard to land in. The floor alone holds
    /// every page it asks for hard, and asks for at most `Floor::PAGES`, so a
    /// page it asks for finds a frame free or held only as cache.
    const WORKSPACE: u32 = L::DEMANDS as u32;

    /// Convergence: the faults and their lines share each zoom's ring, so a page
    /// the view reads never gives up another the view reads.
    pub const FRAMES: u32 = Self::PINNED_PAGES + Self::DEMAND_PAGES;

    pub const RECORDS: u32 = 1 + Self::FRAMES;

    /// A fault's line is among the demand's asks; speculation takes at most the frames.
    pub const ASKS: u32 = Self::PINNED_PAGES + Self::DEMAND_PAGES + if L::DEMANDS { Self::FRAMES } else { 0 };

    /// Pages residency knows of at once: every framed page and every asked one.
    const ENTRIES: u32 = Self::FRAMES + Self::ASKS;

    const SAFE: () = assert!(Self::FRAMES >= Self::HARD_PAGES + Self::WORKSPACE, "safety: no frame for a page nothing holds hard to land in");
}

impl<L: Layer> Default for Paging<L> {
    fn default() -> Self {
        let () = Self::SAFE;
        Paging {
            pages: HashMap::with_capacity(Self::ENTRIES as usize),
            shared: None,
            // Lowest first, as the frames are taken.
            free: (ABSENT + 1..Self::RECORDS).rev().collect(),
            going: Vec::with_capacity(Self::ENTRIES as usize),
            revision: 0,
            refused: 0,
            admitted: 0,
            evicted: 0,
        }
    }
}

fn line(p: Page) -> impl Iterator<Item = Page> {
    std::iter::successors(Some(p), |q| (q.z > TOP_Z).then(|| q.parent()))
}

impl<L: Layer> Paging<L> {
    fn entry(&self, p: Page) -> Option<&Entry<L>> {
        self.pages.get(&p)
    }

    /// Past `ENTRIES` is a page kept for no reason the bound counts.
    fn entry_mut(&mut self, p: Page) -> &mut Entry<L> {
        let full = self.pages.len() >= Self::ENTRIES as usize;
        self.pages.entry(p).or_insert_with(|| {
            xpute_core::ensure!(!full, ENOTRECOVERABLE, p.z);
            Entry::NONE
        })
    }

    fn held_as(&self, p: Page) -> Option<&Held<L>> {
        self.entry(p)?.held.as_ref()
    }

    pub fn resident(&self, p: Page) -> bool {
        self.held_as(p).is_some()
    }

    /// Held in a frame of its own, not the shared `ABSENT` record.
    fn framed(&self, p: Page) -> bool {
        self.held_as(p).is_some_and(|h| h.record != ABSENT)
    }

    /// The frame a held page is in; none for one that reads `ABSENT`.
    pub fn frame(&self, p: Page) -> Option<u32> {
        self.held_as(p).map(|h| h.record).filter(|&r| r != ABSENT)
    }

    pub fn get(&self, p: Page) -> Option<&L> {
        self.held_as(p).map(|h| &h.body)
    }

    pub fn held(&self) -> impl Iterator<Item = (Page, &L)> {
        self.pages.iter().filter_map(|(&p, e)| Some((p, &e.held.as_ref()?.body)))
    }

    pub fn frames_used(&self) -> u32 {
        Self::FRAMES - self.free.len() as u32
    }

    pub fn revision(&self) -> u32 {
        self.revision
    }

    fn said(&self, q: Page) -> bool {
        if q.z <= TOP_Z {
            return false;
        }
        let over = q.parent();
        let settled = self.held_as(over).is_some_and(|h| over.children().iter().zip(h.settles).any(|(&c, s)| c == q && s));
        settled || self.entry(q).is_some_and(|e| e.answered)
    }

    pub fn settled(&self, p: Page) -> bool {
        line(p).any(|q| self.said(q))
    }

    /// The finest resident page on `p`'s line: the fault's answer.
    pub fn drawn_from(&self, p: Page) -> Option<Page> {
        line(p).find(|&q| self.resident(q))
    }

    /// The resident page that says `p`'s ground exactly; none where `p` is a
    /// fault. A resident page above that does not say `p` only draws it coarser.
    pub fn source(&self, p: Page) -> Option<Page> {
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

    /// Pages held in `ABSENT` or answered above are kept only while asked:
    /// they cost no frame, so nothing would ever evict them. Framed pages are
    /// left to normal turnover.
    pub fn ask(&mut self, asks: &HashMap<Page, Ask>) {
        xpute_core::ensure!(asks.len() <= Self::ASKS as usize, ENOTRECOVERABLE, asks.len());
        let mut changed = false;
        for (p, e) in &mut self.pages {
            e.ask = asks.get(p).copied();
            if e.ask.is_none() && e.answered {
                e.answered = false;
                changed = true;
            }
        }
        let mut going = core::mem::take(&mut self.going);
        going.clear();
        going.extend(
            self.pages
                .iter()
                .filter(|&(_, e)| e.ask.is_none() && e.held.as_ref().is_some_and(|h| h.record == ABSENT))
                .map(|(&p, _)| p),
        );
        for &p in &going {
            self.remove(p);
            changed = true;
        }
        self.going = going;
        self.pages.retain(|_, e| !e.idle());
        for (&p, &a) in asks {
            self.entry_mut(p).ask = Some(a);
        }
        if changed {
            self.revision = self.revision.wrapping_add(1);
        }
    }

    /// A page not yet held counts as taking one: whether it is empty is known only on arrival.
    pub fn takes_frame(&self, p: Page) -> bool {
        !self.settled(p) && (self.framed(p) || !self.resident(p))
    }

    pub fn asked(&self, p: Page) -> Option<Ask> {
        self.entry(p)?.ask
    }

    fn hold(&self, p: Page) -> Hold {
        let Some(e) = self.entry(p) else { return Hold::Cache };
        match e.ask.map(|a| a.why) {
            Some(Why::Floor | Why::Destination | Why::Ancestor) => Hold::Hard,
            Some(Why::Fault) => Hold::Demand,
            Some(Why::Hint) | None => Hold::Cache,
        }
    }

    fn hinted(&self, p: Page) -> bool {
        self.asked(p).is_some_and(|a| a.why == Why::Hint)
    }

    /// A frame for arriving `p`: its own, a free one, or a victim among frames
    /// held only as cache (speculation never evicts other speculation). None
    /// only for a speculative page, counted; for anything stronger, none means
    /// the capacity bound is broken.
    fn frame_for(&mut self, p: Page, victim: &impl Victim) -> Option<u32> {
        if self.framed(p) {
            return self.remove(p);
        }
        if let Some(free) = self.free.pop() {
            return Some(free);
        }
        let speculative = self.hold(p) == Hold::Cache;
        let mut candidates: Vec<Candidate> = self
            .pages
            .iter()
            .filter(|&(&q, _)| self.framed(q) && self.hold(q) == Hold::Cache)
            .map(|(&q, _)| Candidate { page: q, hinted: self.hinted(q) })
            .filter(|c| !(speculative && c.hinted))
            .collect();
        // Sorted, so the policy sees the same list for the same state.
        candidates.sort_unstable_by_key(|c| c.page);
        let Some(at) = victim.victim(&candidates) else {
            xpute_core::ensure!(speculative, ENOTRECOVERABLE, p.z);
            self.refused += 1;
            return None;
        };
        self.evicted = self.evicted.wrapping_add(1);
        self.remove(candidates[at].page)
    }

    pub fn churn(&self) -> [u32; 2] {
        [self.admitted, self.evicted]
    }

    pub fn take_refusals(&mut self) -> u32 {
        core::mem::take(&mut self.refused)
    }

    /// The frame is emptied first, so a body that fails to decode leaves it free
    /// rather than another page reading half-overwritten bytes.
    pub fn admit<E>(&mut self, p: Page, victim: &impl Victim, receive: impl FnOnce(u32) -> Result<L, E>) -> Result<(), E> {
        let Some(record) = self.frame_for(p, victim) else { return Ok(()) };
        match receive(record) {
            Ok(page) => {
                self.admitted = self.admitted.wrapping_add(1);
                self.put(p, page, record);
                Ok(())
            }
            Err(e) => {
                self.free.push(record);
                Err(e)
            }
        }
    }

    /// Held only while something asks for it: nothing else would ever let it go.
    pub fn set(&mut self, p: Page, page: L) {
        if self.asked(p).is_none() {
            return;
        }
        if let Some(record) = self.remove(p) {
            self.free.push(record);
        }
        self.put(p, page, ABSENT);
    }

    /// `p` has no object: it reads the shared body, or waits for it.
    pub fn absent(&mut self, p: Page) {
        if self.asked(p).is_none() {
            return;
        }
        match &self.shared {
            Some(body) => self.set(p, body.clone()),
            None => self.entry_mut(p).absent = true,
        }
    }

    /// Whether an asked page waits for the shared body.
    pub fn awaits_shared(&self) -> bool {
        self.shared.is_none() && self.pages.values().any(|e| e.absent && e.ask.is_some())
    }

    pub fn has_shared(&self) -> bool {
        self.shared.is_some()
    }

    /// The shared body has come: every page waiting for it reads it.
    pub fn adopt_shared(&mut self, body: L) {
        let waiting: Vec<Page> = self.pages.iter().filter(|(_, e)| e.absent && e.held.is_none()).map(|(&p, _)| p).collect();
        for p in waiting {
            self.set(p, body.clone());
        }
        self.shared = Some(body);
    }

    pub fn answer_above(&mut self, p: Page) {
        if self.asked(p).is_none() {
            return;
        }
        let e = self.entry_mut(p);
        if !core::mem::replace(&mut e.answered, true) {
            self.revision = self.revision.wrapping_add(1);
        }
    }

    fn put(&mut self, p: Page, body: L, record: u32) {
        let settles = if p.z < GROUND_Z { p.children().map(|c| body.settles(p, c)) } else { [false; 4] };
        self.entry_mut(p).held = Some(Held { body, record, settles });
        self.revision = self.revision.wrapping_add(1);
        L::changed(p, true);
    }

    /// Returns the frame `p` was in, if any, for the caller to fill or free.
    fn remove(&mut self, p: Page) -> Option<u32> {
        let std::collections::hash_map::Entry::Occupied(mut e) = self.pages.entry(p) else { return None };
        let held = e.get_mut().held.take()?;
        if e.get().idle() {
            e.remove();
        }
        self.revision = self.revision.wrapping_add(1);
        L::changed(p, false);
        (held.record != ABSENT).then_some(held.record)
    }
}

#[cfg(test)]
#[path = "paging.test.rs"]
mod test;
