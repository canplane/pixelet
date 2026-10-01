// pxl-kernel/world/paging.rs

//! Residency: which of a layer's pages are held, in which frame, and what a
//! page that is not held is answered by. A mechanism, with one policy inside
//! it — which frame goes when one must (`Victim`) — and nothing of the view:
//! what is observed, what is fetched and in what order are the demand's, the
//! prefetch's and the scheduler's, and each hands this a page and a reason
//! (`Ask`) and nothing more.
//!
//! A frame is a record of the layer's room (layer/terrain/fixed.rs for
//! terrain; POI's pages are on the heap, and a frame is only its place in the
//! count). Any page can sit in any frame: a frame is a page's room, not its
//! address. `ABSENT` is the zero page — what every page the bake left as no
//! object reads, the open ocean or a page of no places — written once and
//! shared, so such a page is held the way any page is and costs no frame.
//!
//! **What is held is two sets.** Held hard: what the fallback asks for
//! (floor.rs). The rest is a cache: the pages the view reads now, the
//! speculated, the recent. Only the hard set is correctness. Every hold is
//! as long as its asking: a page is held for the fallback or the view only
//! while the latest observation asks for it, and is cache the moment it does
//! not.
//!
//! **A held page is read on its own.** A page carries the index of
//! everything under it in its own lanes, so nothing over it is needed to
//! read it: what is held is what is resident, in whatever order it arrived.
//! The page over a missing one is the fallback's and the query's, which
//! descend from the nearest resident page, never a condition on reading one
//! — or a missing ancestor would make a resident descendant unreachable.
//!
//! **A fault does not stall.** A page not resident is answered by the
//! nearest resident page over it (`drawn_from`), coarser, and the caller goes
//! on; the fallback's lease is what makes some page over every observed piece
//! of ground resident, so an observation always has an answer once it lands.
//!
//! **Capacity is asserted, not observed.** Thrash here is silent — a fault
//! falls back and nothing fails — so it is made impossible by size:
//!
//! - *Safety*: a frame for everything held hard, and a workspace, one frame,
//!   for anything else to land in, so progress is never blocked.
//! - *Convergence*: a frame for everything held hard and everything demand
//!   can name at once — the fault bound, the structure's theorem about
//!   faults (pxl-spec space/lod.rs). With that, a page the view reads never
//!   has to give up another the view reads, and a view at rest converges to
//!   exact.
//!
//! What speculation asks for enters neither: it is given only a frame nothing
//! else holds (`Hold::Cache`), and however eager it is, it only pushes
//! itself out of the cache.
//!
//! **Every table is as large as the bound says and no larger.** What is known
//! of a page — the frame it is in, why it is asked for, whether the bucket
//! said it has no object — is one entry, kept only while one of those is
//! so, so the entries are at most the frames and the asks (`ENTRIES`), and
//! the table never grows past them.

pub mod floor;

use std::collections::HashMap;

use pxl_spec::space::lod::FAULT_PAGES;
use pxl_spec::space::pyramid::{LevelPage, TOP_LEVEL};

use crate::io::demand::{Ask, Why};
use crate::world::pyramid::Layer;
use floor::{Fallback, Floor};

/// The record every page the bake left as no object shares: what such a
/// page reads as (a layer's `absent`). A layer's room like any other record,
/// written once, never a frame of a page of its own and never taken back,
/// however many pages read it.
pub const ABSENT: u32 = 0;

/// The pyramid's levels.
pub const LEVELS: u32 = TOP_LEVEL + 1;

/// The observers whose faults the view reads: the eye. A point observer —
/// physics, a pick far from the eye — adds faults of its own, and how far
/// from the eye one may be is a product number to say aloud.
const OBSERVERS: u32 = 1;

/// What demand names a level about one observer: an observation's faults
/// name at most `FAULT_PAGES`, and its estimator no more (pxl-spec
/// space/observation.rs `ByBounds`), a function of the criterion and the
/// page alone. Nothing of how the answers are drawn enters it.
pub const LEVEL_DEMAND_PAGES: u32 = FAULT_PAGES;

/// What the view can read at once, with every page over what it reads: the
/// demand's pages a level, every level, each observer.
pub const DEMAND_PAGES: u32 = OBSERVERS * LEVEL_DEMAND_PAGES * LEVELS;

/// What a destination holds hard: the line from its z16 page to the top, a
/// page a level (world/pyramid.rs `Pyramid::step`).
pub const DESTINATION_PAGES: u32 = LEVELS;

/// What is held hard at most: the fallback's pages, and a destination's.
/// Every reader of the ground takes the fallback's answer where the exact
/// page is not resident, the device's puck too; one that cannot is a
/// destination, which waits for its ground exactly and holds the pages that
/// say it while it does. Another such consumer — physics under a moving
/// body, a placement, an edit — would declare the pages it needs as these do,
/// and be counted here beside them.
pub const HARD_PAGES: u32 = Floor::PAGES + DESTINATION_PAGES;

/// Frames progress needs past what is held hard: one, for a page nothing
/// holds hard to land in while everything that is holds its own.
const WORKSPACE: u32 = 1;

/// Frames a layer pages into: the least that holds everything held hard and
/// everything the view can read at once.
pub const FRAMES: u32 = HARD_PAGES + DEMAND_PAGES;

/// Records a layer holds its pages in: `ABSENT`, then the frames.
pub const RECORDS: u32 = 1 + FRAMES;

/// Pages asked for at once: everything held hard, everything the view reads,
/// and speculation, which is asked for no further than there are frames
/// (world/pyramid.rs `Pyramid::observe`).
pub const ASKS: u32 = HARD_PAGES + DEMAND_PAGES + FRAMES;

/// Pages residency knows anything of at once: every page in a frame and
/// every page asked for. A page held in `ABSENT` or said by the page over it
/// is one asked for, since either is kept only while it is.
const ENTRIES: u32 = FRAMES + ASKS;

const _: () = assert!(FRAMES >= HARD_PAGES + WORKSPACE, "safety: no frame for a page nothing holds hard to land in");
const _: () = assert!(FRAMES >= HARD_PAGES + DEMAND_PAGES, "convergence: a page the view reads could have to give up another the view reads");

/// How a held page is held, strongest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Hold {
    /// Asked for by the fallback: never given up.
    Hard,
    /// The latest observation reads it: given up for nothing, which capacity
    /// makes possible.
    Demand,
    /// Speculation, or nothing asks for it: given up for whatever needs the
    /// frame.
    Cache,
}

/// A frame that may be given up, for a victim policy to choose among.
#[derive(Clone, Copy, Debug)]
pub struct Candidate {
    pub page: LevelPage,
    /// Whether speculation asks for it now.
    pub hinted: bool,
}

/// Which frame goes when one must: chosen only among frames that may
/// (`Candidate`), so a policy moves what is held next and never what is
/// correct.
pub trait Victim {
    /// The candidate to give up, none to give up none.
    fn victim(&self, candidates: &[Candidate]) -> Option<usize>;
}

/// A held page: the page, the record it lives in — `ABSENT` for a page the
/// bake left as no object, which shares it with every other — and which of
/// its four children it settles, by `children`' order.
struct Held<L> {
    body: L,
    record: u32,
    settles: [bool; 4],
}

/// Everything residency knows of one page.
struct Entry<L> {
    held: Option<Held<L>>,
    /// Why the latest observation asks for it.
    ask: Option<Ask>,
    /// The bucket has no object for it, under the top: the page over it says
    /// its ground. Known only while it is asked for.
    answered: bool,
}

impl<L> Entry<L> {
    const NONE: Entry<L> = Entry {
        held: None,
        ask: None,
        answered: false,
    };

    /// Whether nothing is known of it, and the entry goes.
    fn idle(&self) -> bool {
        self.held.is_none() && self.ask.is_none()
    }
}

pub struct Paging<L: Layer> {
    /// Every page residency knows anything of, never more than `ENTRIES`.
    pages: HashMap<LevelPage, Entry<L>>,
    /// The frames no page is in.
    free: Vec<u32>,
    /// The pages an observation's asking lets go, gathered before they go:
    /// held here so asking allocates nothing (`ask`).
    going: Vec<LevelPage>,
    /// Moves whenever a page is held or let go, or a page is answered as
    /// said by the one over it: everything
    /// what is read and what is asked for are functions of.
    revision: u32,
    refused: u32,
    admitted: u32,
    evicted: u32,
}

impl<L: Layer> Default for Paging<L> {
    fn default() -> Self {
        Paging {
            pages: HashMap::with_capacity(ENTRIES as usize),
            // Lowest first, as the frames are taken.
            free: (ABSENT + 1..RECORDS).rev().collect(),
            going: Vec::with_capacity(ENTRIES as usize),
            revision: 0,
            refused: 0,
            admitted: 0,
            evicted: 0,
        }
    }
}

/// `p` and every page over it to the top, finest first.
fn line(p: LevelPage) -> impl Iterator<Item = LevelPage> {
    std::iter::successors(Some(p), |q| (q.level < TOP_LEVEL).then(|| q.parent()))
}

impl<L: Layer> Paging<L> {
    fn entry(&self, p: LevelPage) -> Option<&Entry<L>> {
        self.pages.get(&p)
    }

    /// `p`'s entry, made where there is none. Past `ENTRIES` is a page kept
    /// for no reason the bound counts.
    fn entry_mut(&mut self, p: LevelPage) -> &mut Entry<L> {
        let full = self.pages.len() >= ENTRIES as usize;
        self.pages.entry(p).or_insert_with(|| {
            xpute_core::ensure!(!full, ENOTRECOVERABLE, p.level);
            Entry::NONE
        })
    }

    fn held_as(&self, p: LevelPage) -> Option<&Held<L>> {
        self.entry(p)?.held.as_ref()
    }

    // ---- what is resident ----

    /// Whether `p` is held, and so read.
    pub fn resident(&self, p: LevelPage) -> bool {
        self.held_as(p).is_some()
    }

    /// A resident page. None past the pyramid's top too, which is what a top
    /// page's parent is.
    pub fn get(&self, p: LevelPage) -> Option<&L> {
        self.held_as(p).map(|h| &h.body)
    }

    /// Every held page.
    pub fn held(&self) -> impl Iterator<Item = (LevelPage, &L)> {
        self.pages.iter().filter_map(|(&p, e)| Some((p, &e.held.as_ref()?.body)))
    }

    /// Every held page of `level`.
    pub fn level(&self, level: u32) -> impl Iterator<Item = (LevelPage, &L)> {
        self.held().filter(move |(p, _)| p.level == level)
    }

    /// Frames in use, `ABSENT` apart.
    pub fn frames_used(&self) -> u32 {
        FRAMES - self.free.len() as u32
    }

    /// Which resident set this is: equal between two reads only if nothing
    /// it is a function of moved between them.
    pub fn revision(&self) -> u32 {
        self.revision
    }

    /// Whether the page over `q` says `q`'s ground: it is held and settles
    /// `q`, or the bucket has no object for `q`. Never of the top.
    fn said(&self, q: LevelPage) -> bool {
        if q.level >= TOP_LEVEL {
            return false;
        }
        let over = q.parent();
        let settled = self.held_as(over).is_some_and(|h| over.children().iter().zip(h.settles).any(|(&c, s)| c == q && s));
        settled || self.entry(q).is_some_and(|e| e.answered)
    }

    /// Whether `p` is no page because one over it says its ground: it or a
    /// page between it and the top is settled by the page over it, or has no
    /// object in the bucket.
    pub fn settled(&self, p: LevelPage) -> bool {
        line(p).any(|q| self.said(q))
    }

    /// The finest resident page on `p`'s line: `p` itself, or the nearest
    /// resident page over it, which draws its ground coarser — a count in its
    /// own lattice, terrain in its own cells upsampled — until `p` comes.
    /// None where nothing over it is resident. The fault's answer.
    pub fn drawn_from(&self, p: LevelPage) -> Option<LevelPage> {
        line(p).find(|&q| self.resident(q))
    }

    /// The resident page that says `p`'s ground exactly: `p` itself, or where
    /// it is no page, the page over the highest page between that is none,
    /// where that page is resident — or, where it is no page either, the page
    /// that says its ground. None where `p` is a fault: a page over it that
    /// is resident but does not say its ground draws it only coarser.
    pub fn source(&self, p: LevelPage) -> Option<LevelPage> {
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

    // ---- what is asked for ----

    /// Why each page is asked for from now. What is held in `ABSENT` and what
    /// is answered by the page over it is known only while it is asked for:
    /// a page the bake has no object
    /// for costs no frame, so nothing would ever want its room and it would
    /// sit in the table for the session. A page in a frame of its own is left
    /// to the frames' own turnover: the view leaving a page is no reason to
    /// throw it away.
    pub fn ask(&mut self, asks: &HashMap<LevelPage, Ask>) {
        xpute_core::ensure!(asks.len() <= ASKS as usize, ENOTRECOVERABLE, asks.len());
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

    /// Whether holding `p` takes a frame: it is a page, and not one held in
    /// `ABSENT`. One not held yet is counted as taking one, since whether the
    /// bake left it as no object is known only once it is asked.
    pub fn takes_frame(&self, p: LevelPage) -> bool {
        !self.settled(p) && self.held_as(p).is_none_or(|h| h.record != ABSENT)
    }

    /// Why `p` is asked for now, if it is.
    pub fn asked(&self, p: LevelPage) -> Option<Ask> {
        self.entry(p)?.ask
    }

    fn hold(&self, p: LevelPage) -> Hold {
        let Some(e) = self.entry(p) else { return Hold::Cache };
        match e.ask.map(|a| a.why) {
            Some(Why::Floor | Why::Destination) => Hold::Hard,
            Some(Why::Fault) => Hold::Demand,
            Some(Why::Hint) | None => Hold::Cache,
        }
    }

    fn hinted(&self, p: LevelPage) -> bool {
        self.asked(p).is_some_and(|a| a.why == Why::Hint)
    }

    // ---- frames ----

    /// A frame for `p`, which has arrived, emptied: `p`'s own, a free one, or
    /// one the victim policy gives up among those that may go for it. What
    /// may go is a frame nothing holds hard and the view does not read; for
    /// speculation, one speculation does not ask
    /// for either, so speculation never turns itself over. None only for a
    /// page held by nothing stronger than speculation, counted, since whether
    /// the frames are too few for what is speculated is a reading and not an
    /// argument. For anything stronger capacity makes one exist, and none is
    /// the bound broken.
    fn frame_for(&mut self, p: LevelPage, victim: &impl Victim) -> Option<u32> {
        if self.held_as(p).is_some_and(|h| h.record != ABSENT) {
            return self.remove(p);
        }
        if let Some(free) = self.free.pop() {
            return Some(free);
        }
        let speculative = self.hold(p) == Hold::Cache;
        let mut candidates: Vec<Candidate> = self
            .pages
            .iter()
            .filter(|&(&q, e)| e.held.as_ref().is_some_and(|h| h.record != ABSENT) && self.hold(q) == Hold::Cache)
            .map(|(&q, _)| Candidate { page: q, hinted: self.hinted(q) })
            .filter(|c| !(speculative && c.hinted))
            .collect();
        // In the pages' own order, so the policy sees the same list for the
        // same state whatever order the table keeps them in.
        candidates.sort_unstable_by_key(|c| c.page);
        let Some(at) = victim.victim(&candidates) else {
            xpute_core::ensure!(speculative, ENOTRECOVERABLE, p.level);
            self.refused += 1;
            return None;
        };
        self.evicted = self.evicted.wrapping_add(1);
        self.remove(candidates[at].page)
    }

    /// Pages taken into a frame, and pages given up for another's frame,
    /// since load: what a view's turnover is read by, for the profiler.
    pub fn churn(&self) -> [u32; 2] {
        [self.admitted, self.evicted]
    }

    /// Arrivals given no frame since the last take, for the profiler.
    pub fn take_refusals(&mut self) -> u32 {
        core::mem::take(&mut self.refused)
    }

    // ---- arriving and leaving ----

    /// `p` has arrived: `receive` makes it the page a frame holds, and it is
    /// held there. The frame is emptied first, so a body that does not
    /// decode leaves it free and never leaves another page reading bytes
    /// half overwritten.
    pub fn admit<E>(&mut self, p: LevelPage, victim: &impl Victim, receive: impl FnOnce(u32) -> Result<L, E>) -> Result<(), E> {
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

    /// Holds a page the bake left as no object, in `ABSENT`, while something
    /// asks for it: nothing will want the room of it, so
    /// that is all that lets it go (`ask`). One nothing keeps is not held.
    pub fn set(&mut self, p: LevelPage, page: L) {
        if self.asked(p).is_none() {
            return;
        }
        if let Some(record) = self.remove(p) {
            self.free.push(record);
        }
        self.put(p, page, ABSENT);
    }

    /// The bucket has no object for `p`, under the top: a page over it says
    /// its ground, while something asks for it.
    pub fn answer_above(&mut self, p: LevelPage) {
        if self.asked(p).is_none() {
            return;
        }
        let e = self.entry_mut(p);
        if !core::mem::replace(&mut e.answered, true) {
            self.revision = self.revision.wrapping_add(1);
        }
    }

    fn put(&mut self, p: LevelPage, body: L, record: u32) {
        let settles = if p.level > 0 { p.children().map(|c| body.settles(p, c)) } else { [false; 4] };
        self.entry_mut(p).held = Some(Held { body, record, settles });
        self.revision = self.revision.wrapping_add(1);
        L::changed(p, true);
    }

    /// Takes `p` out: given up, or about to be held again in another way.
    /// The frame it was in, where it had one, for the caller to fill or free.
    fn remove(&mut self, p: LevelPage) -> Option<u32> {
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
