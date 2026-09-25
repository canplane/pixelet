// pxl-kernel/world/pyramid.rs

//! The pyramid as the client holds it, for every layer the ground carries
//! (pxl-spec space/pyramid.rs is the hierarchy itself). What a view wants of
//! it is one window of levels and a ring of pages at each, and the ground in
//! view once at the level it is wanted at (window.rs `Domain`). What the
//! resident pages draw of that ground is the layer's cover (`cover`); a page
//! the four under it are held for is stood up from them (`fold_one`), and
//! what nothing covers is fetched. A page lives in one of a fixed number of
//! records, and only another page needing the room takes a record from it.
//!
//! None of that is a layer's. A layer says what its page is and how it folds,
//! how a fetched body becomes one in a record, what a page with no object is,
//! and who reads its pages; the rest is here, once.

pub mod chunk;
pub mod window;

use core::fmt;

use pxl_spec::bake::quadkey::quadkey_of;
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pyramid::{zoom_of_level, Folds, LevelPage, TOP_LEVEL};
use std::collections::HashSet;
use xpute_runtime::abi::fetch::FetchFail;
use xpute_runtime::sched::edge::{notified, yield_if_spent, Notify};

use crate::io::demand::{Category, Phases};
use crate::io::policy;
use crate::io::sched::{declare, Rank};
use crate::world::change::{world_edge, WorldTopic};
use crate::world::page_table::PageTable;
use chunk::Chunk;
pub use window::Domain;
use window::{domain, eye, held, ring_of, window, window_of};
use window::{HELD_AXIS, WINDOW_LEVELS};

/// A layer the pyramid holds: its page is itself, folded by `Folds`.
pub trait Layer: Folds + Clone + 'static {
    /// The stream a page of this layer is asked for under.
    const CATEGORY: Category;
    /// The topic a change to a leaf is logged under, where a reader compares
    /// revisions (world/change.rs).
    const LEAF_TOPIC: Option<WorldTopic> = None;

    fn pyramid() -> &'static mut Pyramid<Self>;

    /// A fetched body, made the page `record` holds.
    fn receive(record: u32, pkt: &[u8], phases: &mut Phases) -> Result<Self, FetchFail>;

    /// A page the client folded, made the page `record` holds.
    fn keep(record: u32, page: &Self) -> Result<Self, FetchFail>;

    /// A page the bake left as no object: what it reads as, set now or once
    /// the layer has it.
    fn absent(p: LevelPage);

    /// A page came or went; whoever reads this layer's pages is told.
    fn changed(_p: LevelPage, _present: bool) {}

    /// What the layer draws of its cover. A layer whose drawing has limits
    /// of its own — the terrain's mesh cannot meet a chunk two levels from
    /// it, and its chunk table holds so many — turns what it cannot draw into
    /// gaps here; a gap is what the wishlist fetches, so what is on the
    /// screen and what is asked for are one cover.
    fn present(cover: Cover, _d: &Domain, _pyramid: &Pyramid<Self>) -> Cover {
        cover
    }
}

/// A page's name in a bake: the quadkey of the tile it stands on, at the zoom
/// its level is (pxl-spec bake/quadkey.rs). The name's length is the level.
pub struct PageName(pub LevelPage);

impl fmt::Display for PageName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", quadkey_of(zoom_of_level(self.0.level), self.0.page))
    }
}

/// What the resident pages can do for a page the view wants.
///
/// Which of the four it is decides what a fetch is *for*: a page whose ground
/// the cover leaves a gap in is coverage and a page the cover draws otherwise
/// is refinement, so latency shows up as a layer getting sharper rather than
/// as it appearing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolved {
    /// The page itself is resident.
    Exact,
    /// Not resident, but every page it folds from is: the client stands it
    /// up itself, and no request is made.
    Foldable,
    /// Not resident and not foldable, but the cover draws all of its ground
    /// from other pages. The fetch refines what is already on the screen.
    Covered,
    /// The cover leaves a gap in its ground. The fetch is what puts it on
    /// the screen.
    Missing,
}

/// Pages a layer keeps wherever the view is: the device's own leaf, so the
/// marker keeps the ground under it while the view is panned away.
const PINNED: u32 = 1;

/// Records a layer holds its pages in: a ring for every level of the widest
/// window and the pinned page, so every page a layer can want at once has
/// room — which is what lets a page wanted never be turned out for another
/// (`record_for`) — and what the window leaves over is room for pages the
/// camera has left.
pub const RECORDS: u32 = WINDOW_LEVELS * HELD_AXIS * HELD_AXIS + PINNED;

/// What the resident pages draw of a view's `Domain`: pieces of its ground,
/// none over another, and the gaps between them, the two together being
/// the domain's ground exactly.
///
/// Chosen a sibling group at a time. Where the view wants a chunk finer and
/// the four under it are not all drawn, the chunk itself is drawn if it is
/// resident and its finer pieces are not: a coarse page with one fine page
/// under it shows the coarse one, never both, so whatever is counted over
/// the cover is counted once. Where a wanted chunk is not resident, the
/// nearest resident chunk over it in the window draws its ground; where
/// none is, the resident pages under it stand in, as far down as there are
/// any (Cesium's `skipLevelOfDetail`, which stands a node in by its loaded
/// descendants where no ancestor is loaded), with gaps where there are none.
#[derive(Default, PartialEq)]
pub struct Cover {
    pub drawn: Vec<Chunk>,
    pub gaps: Vec<Chunk>,
}

/// What the cover and the wishlist are a function of: where the eye is and
/// the window it holds, whether the camera is moving, the resident set and
/// the pinned page. Two turns agreeing on all of it want the same pages of
/// the same ground, so the second decides nothing again.
type Wish = ([f64; 3], (u32, u32), bool, u32, Option<LevelPage>);

pub struct Pyramid<L: Layer> {
    tables: Vec<PageTable<L>>,
    /// Which page each record carries. A record is a page's room, not its
    /// address: any page can sit in any record, and which pages the range
    /// holds is `worth`'s policy rather than a coordinate's residue.
    records: Vec<Option<LevelPage>>,
    refused: u32,
    /// Moves whenever a page is set or cleared. The resident set is what the
    /// cover and every fold are functions of, so this one number is what
    /// makes an arrival, a fold or an eviction reach both.
    revision: u32,
    pinned: Option<LevelPage>,
    /// The view the layer last drew, what it draws of it, and which making
    /// of it this is: whoever draws it re-reads it when the number moves.
    domain: Option<Domain>,
    cover: Cover,
    covers: u32,
    last: Option<Wish>,
    folds: Notify,
}

impl<L: Layer> Default for Pyramid<L> {
    fn default() -> Self {
        Pyramid {
            tables: (0..=TOP_LEVEL).map(|level| PageTable::new(if level == 0 { L::LEAF_TOPIC } else { None })).collect(),
            records: vec![None; RECORDS as usize],
            refused: 0,
            revision: 0,
            pinned: None,
            domain: None,
            cover: Cover::default(),
            covers: 0,
            last: None,
            folds: Notify::new(),
        }
    }
}

impl<L: Layer> Pyramid<L> {
    /// A resident page. None past the pyramid's top too, which is what a top
    /// page's parent is.
    pub fn get(&self, p: LevelPage) -> Option<&L> {
        self.tables.get(p.level as usize)?.get(p.page)
    }

    /// Every resident page of `level`.
    pub fn level(&self, level: u32) -> &PageTable<L> {
        &self.tables[level as usize]
    }

    /// Sets a page, with or without a record: a page with none is one every
    /// such page shares, which nothing will want the room of.
    pub fn set(&mut self, p: LevelPage, page: L) {
        self.tables[p.level as usize].set(p.page, page);
        self.revision = self.revision.wrapping_add(1);
        L::changed(p, true);
    }

    pub fn clear(&mut self, p: LevelPage) -> Option<L> {
        let held = self.tables[p.level as usize].clear(p.page)?;
        self.revision = self.revision.wrapping_add(1);
        L::changed(p, false);
        Some(held)
    }

    /// Which resident set this is: equal between two reads only if no page
    /// came or went between them.
    pub fn revision(&self) -> u32 {
        self.revision
    }

    /// Keeps `p` wanted wherever the view is, and its record with it, until
    /// another page is pinned in its place.
    pub fn pin(&mut self, p: Option<LevelPage>) {
        self.pinned = p;
    }

    // ---- what is resident can answer ----

    /// A page's nearest resident ancestor inside `window`. A page resident
    /// above the window's top is one nothing draws — the range keeps it
    /// against a zoom back.
    fn ancestor(&self, p: LevelPage, window: &std::ops::Range<u32>) -> Option<LevelPage> {
        let mut up = p;
        while up.level + 1 < window.end {
            up = up.parent();
            if self.get(up).is_some() {
                return Some(up);
            }
        }
        None
    }

    fn children_resident(&self, p: LevelPage) -> bool {
        p.level > 0 && p.children().iter().all(|&c| self.get(c).is_some())
    }

    /// Whether letting `p` go leaves its ground drawn: something coarser
    /// over it is resident, which the cover falls back to once `p`'s sibling
    /// group is short of it, or the pages it folds from are.
    fn covered(&self, p: LevelPage, window: &std::ops::Range<u32>) -> bool {
        self.ancestor(p, window).is_some() || self.children_resident(p)
    }

    /// What the resident pages draw of `d` (`Cover`).
    pub fn cover_of(&self, d: &Domain) -> Cover {
        // The pages that are resident or have one under them, which is how
        // far down a stand-in is looked for without walking ground that has
        // nothing.
        let mut below: HashSet<LevelPage> = HashSet::new();
        for level in 0..=TOP_LEVEL {
            for page in self.tables[level as usize].keys() {
                let mut p = LevelPage::new(level, page);
                while below.insert(p) && p.level < TOP_LEVEL {
                    p = p.parent();
                }
            }
        }
        let mut out = Cover::default();
        for c in d.roots() {
            self.wanted(d, &below, c, false, &mut out);
        }
        out
    }

    /// `c` as the domain wants it, into `out`, `above` saying whether a
    /// resident chunk over it in the window draws its ground should `c` not
    /// be drawn whole. Whether it is.
    fn wanted(&self, d: &Domain, below: &HashSet<LevelPage>, c: Chunk, above: bool, out: &mut Cover) -> bool {
        let here = self.get(c.page).is_some();
        if d.splits(c) {
            let mut finer = Cover::default();
            let mut whole = true;
            for k in c.children() {
                whole &= self.wanted(d, below, k, above || here, &mut finer);
            }
            if here && !whole {
                out.drawn.push(c);
                return true;
            }
            out.drawn.append(&mut finer.drawn);
            out.gaps.append(&mut finer.gaps);
            return whole;
        }
        if here || above {
            if here {
                out.drawn.push(c);
            } else {
                out.gaps.push(c);
            }
            return here;
        }
        self.stand_in(below, c, out)
    }

    /// `c` from the resident pages under it, where nothing over it is;
    /// whether it is drawn whole.
    fn stand_in(&self, below: &HashSet<LevelPage>, c: Chunk, out: &mut Cover) -> bool {
        if self.get(c.page).is_some() {
            out.drawn.push(c);
            return true;
        }
        if c.level() == 0 || !below.contains(&c.children()[0].page) {
            out.gaps.push(c);
            return false;
        }
        let mut whole = true;
        for k in c.children() {
            whole &= self.stand_in(below, k, out);
        }
        whole
    }

    /// What the layer draws of the view, and which making of it this is.
    pub fn cover(&self) -> (&Cover, u32) {
        (&self.cover, self.covers)
    }

    /// The view the cover was made for.
    pub fn domain(&self) -> Option<&Domain> {
        self.domain.as_ref()
    }

    /// What the resident pages can do for `p`, by what the cover made of its
    /// ground.
    fn resolve(&self, p: LevelPage) -> Resolved {
        if self.get(p).is_some() {
            Resolved::Exact
        } else if self.children_resident(p) {
            Resolved::Foldable
        } else if self.cover.gaps.iter().any(|g| g.overlaps_page(p)) {
            Resolved::Missing
        } else {
            Resolved::Covered
        }
    }

    // ---- records ----

    fn record_of(&self, p: LevelPage) -> Option<u32> {
        self.records.iter().position(|held| *held == Some(p)).map(|at| at as u32)
    }

    /// How readily a resident page gives up its record, highest first. A page
    /// the camera does not want goes before one it does; among those, one
    /// something else already draws goes before one nothing covers; and among
    /// equals the one furthest out, the rim being margin. Ordered by
    /// comparison rather than a score, so a page the camera wants is never
    /// given up for one it does not, whatever the distance.
    fn worth(&self, p: LevelPage, anchor: IVec, view: &std::ops::Range<u32>) -> (bool, bool, u32) {
        let wanted = Some(p) == self.pinned || (view.contains(&p.level) && ring_of(anchor, p) <= HELD_AXIS / 2);
        (!wanted, self.covered(p, view), ring_of(anchor, p))
    }

    /// The record `p` goes into: its own, a free one, or the one whose page
    /// is worth least. None where every record carries a page worth more,
    /// which is an arrival the camera has moved past: dropped rather than
    /// turning out a page that is wanted, and counted, since whether the
    /// range is the wrong size is a reading and not an argument.
    fn record_for(&mut self, p: LevelPage, anchor: Option<IVec>, view: &std::ops::Range<u32>) -> Option<u32> {
        if let Some(record) = self.record_of(p) {
            return Some(record);
        }
        if let Some(free) = self.records.iter().position(Option::is_none) {
            return Some(free as u32);
        }
        let anchor = anchor?;
        let mine = self.worth(p, anchor, view);
        // The pinned page is never the one to go, whatever it is weighed at.
        let worst = (0..RECORDS)
            .filter_map(|r| {
                let held = self.records[r as usize].filter(|held| Some(*held) != self.pinned)?;
                Some((self.worth(held, anchor, view), r))
            })
            .max_by(|a, b| a.0.cmp(&b.0));
        let taken = worst.filter(|(w, _)| *w > mine).map(|(_, r)| r);
        if taken.is_none() {
            self.refused += 1;
        }
        taken
    }

    /// The record is `p`'s from here, and whatever it carried is gone.
    fn take(&mut self, p: LevelPage, record: u32, page: L) {
        if let Some(gone) = self.records[record as usize].replace(p).filter(|held| *held != p) {
            self.clear(gone);
        }
        self.set(p, page);
    }

    /// Arrivals dropped for want of a record since the last take, for the
    /// ledger.
    pub fn take_refusals(&mut self) -> u32 {
        core::mem::take(&mut self.refused)
    }

    // ---- arriving ----

    /// A page's fetched body, made resident.
    pub fn adopt(&mut self, p: LevelPage, pkt: &[u8], phases: &mut Phases) -> Result<(), FetchFail> {
        let Some(record) = self.record_for(p, eye().map(|e| e.foot), &window()) else { return Ok(()) };
        let page = L::receive(record, pkt, phases)?;
        self.take(p, record, page);
        Ok(())
    }

    /// Stands one page up from the four under it, if any of the window's wants
    /// can be. A level at a time from the finest, so what one fold makes feeds
    /// the next. Whether it found one.
    ///
    /// The ring around those four is read where it is held and stood in for
    /// where it is not: requiring it would put this at one page a level, and
    /// a zoom out would fetch pages whose ground is already held.
    fn fold_one(&mut self, anchor: IVec) -> bool {
        for level in window().filter(|&l| l > 0) {
            for p in held(level, anchor) {
                if self.get(p).is_some() {
                    continue;
                }
                let Some(folded) = L::fold_page(p, |q| self.get(q)) else { continue };
                let Some(record) = self.record_for(p, Some(anchor), &window()) else { return false };
                return match L::keep(record, &folded) {
                    Ok(page) => {
                        self.take(p, record, page);
                        true
                    }
                    Err(_) => false,
                };
            }
        }
        false
    }

    // ---- wanting ----

    /// Every page the view wants that the resident pages cannot answer, each
    /// ranked by what its absence costs, and the pinned page wherever the
    /// view is.
    fn wants(&self, anchor: IVec) -> Vec<(u64, Rank)> {
        let w = window();
        let mut wants = Vec::new();
        for level in w.clone().rev() {
            for p in held(level, anchor) {
                let resolved = self.resolve(p);
                if matches!(resolved, Resolved::Exact | Resolved::Foldable) {
                    continue;
                }
                // None of it drawn at its level: margin against a move that
                // has not happened.
                let (speculative, near) = self.domain.as_ref().map_or((false, 0.0), |d| (!d.reaches(p), d.page_nearest_to(p)));
                wants.push((p.subject(), policy::page(resolved, speculative, near)));
            }
        }
        if let Some(p) = self.pinned.filter(|p| self.get(*p).is_none() && !wants.iter().any(|(s, _)| *s == p.subject())) {
            wants.push((p.subject(), policy::pinned()));
        }
        wants
    }

    /// Lets go of the pages the camera has left that carry no record: a page
    /// the bake has no object for shares what it reads as with every other,
    /// so nothing will ever want its room and it would sit in the table for
    /// the session. A page with a record is left to `record_for`: the camera
    /// leaving a page is not a reason to throw it away.
    fn drop_unrecorded(&mut self, anchor: IVec) {
        let w = window();
        let wanted: Vec<LevelPage> = w.clone().flat_map(|level| held(level, anchor)).collect();
        for level in 0..=TOP_LEVEL {
            // Collected first: clearing moves the table's last page into the hole.
            let drop: Vec<LevelPage> = self.tables[level as usize]
                .keys()
                .map(|page| LevelPage::new(level, page))
                .filter(|p| Some(*p) != self.pinned && !wanted.contains(p) && self.record_of(*p).is_none())
                .collect();
            for p in drop {
                self.clear(p);
            }
        }
    }

    /// The layer's turn. Wherever the view or the resident set has changed —
    /// a page arrived, was folded or went — the cover is made again, what
    /// the camera left let go and the folds woken, since each is a function
    /// of both; and the wishlist every turn from that cover.
    pub fn step(&mut self) {
        let Some(e) = eye() else { return };
        let anchor = e.foot;
        let w = window_of(&e);
        let wish = (e.at, (w.start, w.end), crate::sched::turn::interactive(), self.revision, self.pinned);
        if self.last != Some(wish) {
            self.drop_unrecorded(anchor);
            let d = domain(&e);
            let mut cover = L::present(self.cover_of(&d), &d, self);
            cover.drawn.sort_unstable();
            cover.gaps.sort_unstable();
            self.domain = Some(d);
            // The eye moves every frame the camera does, and most of those
            // moves cross no chunk's split distance: whoever reads the cover
            // is told only when what it draws has changed.
            if cover != self.cover {
                self.cover = cover;
                self.covers = self.covers.wrapping_add(1);
            }
            // Letting a page go moves the revision; the wish is the resident
            // set the cover was made from.
            self.last = Some((wish.0, wish.1, wish.2, self.revision, wish.4));
            self.folds.notify();
        }
        declare(L::CATEGORY, &self.wants(anchor));
    }

    /// Asks for nothing: a layer the view does not show holds what it has
    /// and wants no more.
    pub fn idle(&mut self) {
        declare(L::CATEGORY, &[]);
        self.last = None;
    }
}

fn folds<L: Layer>() -> &'static mut Notify {
    &mut L::pyramid().folds
}

/// A layer's folds: one page a step, on what the turn's quota leaves after
/// the tick, for as long as there is one to make. A fold is work — a page
/// voted or summed cell by cell — and work goes on the budgeted path, not in
/// the turn where the wishlist is decided.
pub async fn fold_task<L: Layer>() {
    loop {
        notified(folds::<L>).await;
        loop {
            yield_if_spent(world_edge).await;
            let Some(anchor) = eye().map(|e| e.foot) else { break };
            if !L::pyramid().fold_one(anchor) {
                break;
            }
        }
    }
}

#[cfg(test)]
#[path = "pyramid.test.rs"]
mod test;
