// pxl-kernel/world/pyramid.rs

//! The pyramid as the client holds it, for every layer the ground carries
//! (pxl-spec space/pyramid.rs is the hierarchy itself). What a view wants of
//! it is one window of levels and a ring of pages at each (window.rs). What
//! the resident pages can answer before the network is asked is `resolve`; a
//! page the four under it are held for is stood up from them (`fold_one`),
//! and anything else is fetched. A page lives in one of a fixed number of
//! records, and only another page needing the room takes a record from it.
//!
//! None of that is a layer's. A layer says what its page is and how it folds,
//! how a fetched body becomes one in a record, what a page with no object is,
//! and who reads its pages; the rest is here, once.

pub mod window;
#[path = "pyramid/window.spec.rs"]
pub mod window_spec;

use core::fmt;

use pxl_spec::bake::quadkey::quadkey_of;
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pyramid::{neighbourhood, zoom_of_level, Folds, LevelPage, TOP_LEVEL};
use xpute_runtime::abi::fetch::FetchFail;
use xpute_runtime::sched::edge::{notified, yield_if_spent, Notify};

use crate::io::demand::{Category, Phases};
use crate::io::policy;
use crate::io::sched::{declare, Rank};
use crate::world::change::{world_edge, WorldTopic};
use crate::world::frame::range_anchor::range_anchor;
use crate::world::page_table::PageTable;
use window::{centre_dist, centre_page, held, ring_of, window};
use window_spec::{HELD_AXIS, RENDER_AXIS, WINDOW_LEVELS};

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
/// Which of the four it is decides what a fetch is *for*: a page with nothing
/// over it is coverage and a page with something over it is refinement, so
/// latency shows up as a layer getting sharper rather than as it appearing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolved {
    /// The page itself is resident.
    Exact,
    /// Not resident, but every page it folds from is: the client stands it
    /// up itself, and no request is made.
    Foldable,
    /// Not resident and not foldable, but a page over it is, so it is drawn
    /// coarse. The fetch refines what is already on the screen.
    Covered,
    /// Nothing draws this. The fetch is what puts it on the screen.
    Missing,
}

/// Records a layer holds its pages in: a ring for every level of the widest
/// window, so every page a view can want at once has room, and what the
/// window leaves over is room for pages the camera has left.
pub const RECORDS: u32 = WINDOW_LEVELS * HELD_AXIS * HELD_AXIS;

/// What the wishlist is a function of. Two turns agreeing on all of it ask
/// for the same pages in the same order, so the second is skipped.
type Wish = (LevelPage, Option<LevelPage>, (u32, u32), bool);

pub struct Pyramid<L: Layer> {
    tables: Vec<PageTable<L>>,
    /// Which page each record carries. A record is a page's room, not its
    /// address: any page can sit in any record, and which pages the range
    /// holds is `worth`'s policy rather than a coordinate's residue.
    records: Vec<Option<LevelPage>>,
    refused: u32,
    last: Option<Wish>,
    folds: Notify,
}

impl<L: Layer> Default for Pyramid<L> {
    fn default() -> Self {
        Pyramid {
            tables: (0..=TOP_LEVEL).map(|level| PageTable::new(if level == 0 { L::LEAF_TOPIC } else { None })).collect(),
            records: vec![None; RECORDS as usize],
            refused: 0,
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
        L::changed(p, true);
    }

    pub fn clear(&mut self, p: LevelPage) -> Option<L> {
        let held = self.tables[p.level as usize].clear(p.page)?;
        L::changed(p, false);
        Some(held)
    }

    // ---- what is resident can answer ----

    /// What the resident pages can do for `p`, the view drawing `window`.
    pub fn resolve(&self, p: LevelPage, window: &std::ops::Range<u32>) -> Resolved {
        if self.get(p).is_some() {
            Resolved::Exact
        } else if self.children_resident(p) {
            Resolved::Foldable
        } else if self.ancestor(p, window).is_some() {
            Resolved::Covered
        } else {
            Resolved::Missing
        }
    }

    /// A page's nearest resident ancestor inside `window`: what draws it
    /// while the page itself is not there. A page resident above the window's
    /// top is one nothing draws — the range keeps it against a zoom back.
    pub fn ancestor(&self, p: LevelPage, window: &std::ops::Range<u32>) -> Option<LevelPage> {
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

    /// Whether letting `p` go leaves it drawn: something coarser over it is
    /// resident and drawn, or the pages it folds from are.
    fn covered(&self, p: LevelPage, window: &std::ops::Range<u32>) -> bool {
        self.ancestor(p, window).is_some() || self.children_resident(p)
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
        let wanted = view.contains(&p.level) && ring_of(anchor, p) <= HELD_AXIS / 2;
        (!wanted, self.covered(p, view), ring_of(anchor, p))
    }

    /// The record `p` goes into: its own, a free one, or the one whose page
    /// is worth least. None where every record carries a page worth more,
    /// which is an arrival the camera has moved past: dropped rather than
    /// turning out a page that is wanted, and counted, since whether the
    /// range is the wrong size is a reading and not an argument.
    fn record_for(&mut self, p: LevelPage) -> Option<u32> {
        if let Some(record) = self.record_of(p) {
            return Some(record);
        }
        if let Some(free) = self.records.iter().position(Option::is_none) {
            return Some(free as u32);
        }
        let anchor = range_anchor()?;
        let view = window();
        let mine = self.worth(p, anchor, &view);
        let worst = (0..RECORDS)
            .filter_map(|r| Some((self.worth(self.records[r as usize]?, anchor, &view), r)))
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
        let Some(record) = self.record_for(p) else { return Ok(()) };
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
                let Some(folded) = L::fold_held(neighbourhood(p).map(|q| self.get(q)), level) else { continue };
                let Some(record) = self.record_for(p) else { return false };
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
    /// ranked by what its absence costs; `pinned` is a leaf wanted wherever
    /// the view is.
    fn wants(&self, anchor: IVec, pinned: Option<LevelPage>) -> Vec<(u64, Rank)> {
        let w = window();
        let mut wants = Vec::new();
        for level in w.clone().rev() {
            for p in held(level, anchor) {
                let resolved = self.resolve(p, &w);
                if matches!(resolved, Resolved::Exact | Resolved::Foldable) {
                    continue;
                }
                // Outside the ring the view draws: margin against a move that
                // has not happened.
                let speculative = ring_of(anchor, p) > RENDER_AXIS / 2;
                wants.push((p.subject(), policy::page(resolved, speculative, centre_dist(anchor, p) * p.span())));
            }
        }
        if let Some(p) = pinned.filter(|p| self.get(*p).is_none() && !wants.iter().any(|(s, _)| *s == p.subject())) {
            wants.push((p.subject(), policy::pinned()));
        }
        wants
    }

    /// Lets go of the pages the camera has left that carry no record: a page
    /// the bake has no object for shares what it reads as with every other,
    /// so nothing will ever want its room and it would sit in the table for
    /// the session. A page with a record is left to `record_for`: the camera
    /// leaving a page is not a reason to throw it away.
    fn drop_unrecorded(&mut self, anchor: IVec, pinned: Option<LevelPage>) {
        let w = window();
        let wanted: Vec<LevelPage> = w.clone().flat_map(|level| held(level, anchor)).collect();
        for level in 0..=TOP_LEVEL {
            // Collected first: clearing moves the table's last page into the hole.
            let drop: Vec<LevelPage> = self.tables[level as usize]
                .keys()
                .map(|page| LevelPage::new(level, page))
                .filter(|p| Some(*p) != pinned && !wanted.contains(p) && self.record_of(*p).is_none())
                .collect();
            for p in drop {
                self.clear(p);
            }
        }
    }

    /// The layer's turn: the wishlist every turn, since a page arriving
    /// changes what its parent and children need; and, where the camera has
    /// moved, what it left let go and the folds woken.
    pub fn step(&mut self, pinned: Option<LevelPage>) {
        let Some(anchor) = range_anchor() else { return };
        declare(L::CATEGORY, &self.wants(anchor, pinned));
        let w = window();
        let wish = (centre_page(w.start, anchor), pinned, (w.start, w.end), crate::sched::turn::interactive());
        if self.last == Some(wish) {
            return;
        }
        self.last = Some(wish);
        self.drop_unrecorded(anchor, pinned);
        self.folds.notify();
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
            let Some(anchor) = range_anchor() else { break };
            if !L::pyramid().fold_one(anchor) {
                break;
            }
        }
    }
}
