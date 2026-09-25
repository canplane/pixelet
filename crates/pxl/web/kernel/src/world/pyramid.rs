// pxl-kernel/world/pyramid.rs

//! The pyramid as the client holds it, for every layer the ground carries
//! (pxl-spec space/pyramid.rs is the hierarchy itself). What a view wants of
//! it is the ground in view once at the level the rule wants it at, and the
//! pages that takes with every ancestor over them (domain.rs `Domain`). What the
//! resident pages draw of that ground is the layer's cover (`cover`); a page
//! the four under it are held for is stood up from them (`fold_one`), and
//! what nothing covers is fetched. A page lives in one of a fixed number of
//! records, and only another page needing the room takes a record from it.
//!
//! The tree is as deep as a layer's data needs and no deeper: where a page
//! says all of a child's ground (`Folds::settles`), the child is no page, is
//! never asked for, and its ground is drawn from the page over it
//! (`source`). Only the finest level is a bound.
//!
//! None of that is a layer's. A layer says what its page is and how it folds,
//! how a fetched body becomes one in a record, what a page with no object is,
//! and who reads its pages; the rest is here, once.

pub mod chunk;
pub mod domain;

use pxl_spec::space::hierarchy::share;
use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::space::pyramid::{Folds, LevelPage, TOP_LEVEL};
use std::collections::{BTreeMap, HashSet};
use xpute_runtime::abi::fetch::FetchFail;
use xpute_runtime::sched::edge::{notified, yield_if_spent, Notify};

use crate::io::demand::{Category, Phases};
use crate::io::policy;
use crate::io::sched::{declare, Rank};
use crate::layer::environment::fog::eye_height;
use crate::world::change::world_edge;
use chunk::Chunk;
pub use domain::Domain;
use domain::{domain_for, eye, heights_revision, ring_of, Frustum, VIEW_PAGES};

/// A layer the pyramid holds: its page is itself, folded by `Folds`.
pub trait Layer: Folds + Clone + 'static {
    /// The stream a page of this layer is asked for under.
    const CATEGORY: Category;

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

/// Records a layer holds its pages in: every page a view can want at once
/// (domain.rs `VIEW_PAGES`, which the view's criterion is loosened to fit)
/// and the pinned page, so every page a layer can want has room — which is
/// what lets a page wanted never be turned out for another (`record_for`) —
/// and what the view leaves over is room for pages the camera has left.
pub const RECORDS: u32 = VIEW_PAGES + PINNED;

/// What the resident pages draw of a view's `Domain`: pieces of its ground,
/// none over another, and the gaps between them, the two together being
/// the domain's ground exactly.
///
/// Chosen a sibling group at a time. Where the view wants a chunk finer and
/// the four under it are not all drawn, the chunk itself is drawn if it is
/// resident and its finer pieces are not: a coarse page with one fine page
/// under it shows the coarse one, never both, so whatever is counted over
/// the cover is counted once. Where a wanted chunk is not resident, the
/// nearest resident chunk over it draws its ground; where
/// none is, the resident pages under it stand in, as far down as there are
/// any (Cesium's `skipLevelOfDetail`, which stands a node in by its loaded
/// descendants where no ancestor is loaded), with gaps where there are none.
#[derive(Default, PartialEq)]
pub struct Cover {
    pub drawn: Vec<Chunk>,
    pub gaps: Vec<Chunk>,
}

/// What the cover and the wishlist are a function of: where the eye is, what
/// it sees, whether the camera is moving, the
/// resident set, the pinned page, and the heights the domain measures the
/// ground by (`heights_revision`), which neither the eye nor this layer's
/// resident set moves. Two turns agreeing on all of it want the same pages
/// of the same ground, so the second decides nothing again.
type Wish = ([f64; 3], Frustum, bool, u32, Option<LevelPage>, (u32, u32));

/// A resident page, and the record it lives in: none for a page every such
/// page shares (a layer's `absent`), which takes no room.
struct Held<L> {
    page: L,
    record: Option<u32>,
}

pub struct Pyramid<L: Layer> {
    /// Every resident page, in level order. A record is a page's room, not
    /// its address: any page can sit in any record, and which pages the
    /// range holds is `worth`'s policy rather than a coordinate's residue.
    pages: BTreeMap<LevelPage, Held<L>>,
    refused: u32,
    /// Moves whenever a page is set or cleared. The resident set is what the
    /// cover and every fold are functions of, so this one number is what
    /// makes an arrival, a fold or an eviction reach both.
    revision: u32,
    pinned: Option<LevelPage>,
    /// The view the layer last drew, what it draws of it, and which making
    /// of it this is: whoever draws it re-reads it when the number moves.
    domain: Option<Domain>,
    /// Which making of the domain this is: it moves whenever the domain is
    /// made again, whether or not the cover changed with it.
    domains: u32,
    cover: Cover,
    covers: u32,
    /// What the layer asks the fetcher for, made with the cover: a function
    /// of the same wish, and said again every turn as it stands.
    wishlist: Vec<(u64, Rank)>,
    last: Option<Wish>,
    folds: Notify,
    /// Pages a resident page settles, so none of their own is asked for.
    settled: HashSet<LevelPage>,
    /// Pages under the top the bucket has no object for: a page over them
    /// says their ground, whether or not it is held yet.
    answered: HashSet<LevelPage>,
}

impl<L: Layer> Default for Pyramid<L> {
    fn default() -> Self {
        Pyramid {
            pages: BTreeMap::new(),
            refused: 0,
            revision: 0,
            pinned: None,
            domain: None,
            domains: 0,
            cover: Cover::default(),
            covers: 0,
            wishlist: Vec::new(),
            last: None,
            folds: Notify::new(),
            settled: HashSet::new(),
            answered: HashSet::new(),
        }
    }
}

impl<L: Layer> Pyramid<L> {
    /// A resident page. None past the pyramid's top too, which is what a top
    /// page's parent is.
    pub fn get(&self, p: LevelPage) -> Option<&L> {
        self.pages.get(&p).map(|held| &held.page)
    }

    /// Every resident page of `level`.
    pub fn level(&self, level: u32) -> impl Iterator<Item = (LevelPage, &L)> {
        self.pages
            .range(LevelPage { level, page: 0 }..LevelPage { level: level + 1, page: 0 })
            .map(|(&p, held)| (p, &held.page))
    }

    /// Sets a page with no record: one every such page shares, which nothing
    /// will want the room of, so it is held while the view wants it
    /// (`drop_unrecorded`).
    pub fn set(&mut self, p: LevelPage, page: L) {
        self.put(p, page, None);
    }

    fn put(&mut self, p: LevelPage, page: L, record: Option<u32>) {
        if p.level > 0 {
            self.settled.extend(p.children().into_iter().filter(|&c| page.settles(p, c)));
        }
        self.pages.insert(p, Held { page, record });
        self.revision = self.revision.wrapping_add(1);
        L::changed(p, true);
    }

    pub fn clear(&mut self, p: LevelPage) -> Option<L> {
        let held = self.pages.remove(&p)?.page;
        if p.level > 0 {
            for c in p.children() {
                self.settled.remove(&c);
            }
        }
        self.revision = self.revision.wrapping_add(1);
        L::changed(p, false);
        Some(held)
    }

    /// Which resident set this is: equal between two reads only if no page
    /// came or went between them.
    pub fn revision(&self) -> u32 {
        self.revision
    }

    /// The bucket has no object for `p`, under the top: a page over it says
    /// its ground.
    pub fn answer_above(&mut self, p: LevelPage) {
        if self.answered.insert(p) {
            self.revision = self.revision.wrapping_add(1);
        }
    }

    /// Whether `p` is no page because one over it says its ground: it or a
    /// page between it and the top is settled by the page over it, or has no
    /// object in the bucket.
    fn settled(&self, p: LevelPage) -> bool {
        std::iter::successors(Some(p), |q| (q.level < TOP_LEVEL).then(|| q.parent()))
            .take_while(|q| q.level < TOP_LEVEL)
            .any(|q| self.settled.contains(&q) || self.answered.contains(&q))
    }

    /// The resident page that draws `p`'s ground: `p` itself, or where it is
    /// settled, the nearest resident page over it.
    pub fn source(&self, p: LevelPage) -> Option<LevelPage> {
        if self.get(p).is_some() {
            return Some(p);
        }
        if !self.settled(p) {
            return None;
        }
        std::iter::successors(Some(p), |q| (q.level < TOP_LEVEL).then(|| q.parent())).find(|&q| self.get(q).is_some())
    }

    /// Keeps `p` wanted wherever the view is, and its record with it, until
    /// another page is pinned in its place.
    pub fn pin(&mut self, p: Option<LevelPage>) {
        self.pinned = p;
    }

    // ---- what is resident can answer ----

    /// A page's nearest resident ancestor.
    fn ancestor(&self, p: LevelPage) -> Option<LevelPage> {
        std::iter::successors(Some(p), |q| (q.level < TOP_LEVEL).then(|| q.parent())).skip(1).find(|&q| self.get(q).is_some())
    }

    fn children_resident(&self, p: LevelPage) -> bool {
        p.level > 0 && p.children().iter().all(|&c| self.get(c).is_some())
    }

    /// Whether letting `p` go leaves its ground drawn: something coarser
    /// over it is resident, which the cover falls back to once `p`'s sibling
    /// group is short of it, or the pages it folds from are.
    fn covered(&self, p: LevelPage) -> bool {
        self.ancestor(p).is_some() || self.children_resident(p)
    }

    /// What the resident pages draw of `d` (`Cover`).
    pub fn cover_of(&self, d: &Domain) -> Cover {
        // The pages that are resident or have one under them, which is how
        // far down a stand-in is looked for without walking ground that has
        // nothing.
        let mut below: HashSet<LevelPage> = HashSet::new();
        for &page in self.pages.keys() {
            let mut p = page;
            while below.insert(p) && p.level < TOP_LEVEL {
                p = p.parent();
            }
        }
        // The roots are the pyramid's top, so every page over the ground a
        // view draws is one the view wants held (domain.rs `pages`): a root's
        // own page, where it is resident, stands in for whatever under it has
        // not come.
        let mut out = Cover::default();
        for &c in d.roots() {
            self.wanted(d, &below, c, false, &mut out);
        }
        out
    }

    /// `c` as the domain wants it, into `out`, `above` saying whether a
    /// resident chunk over it draws its ground should `c` not be drawn whole.
    /// Whether it is.
    fn wanted(&self, d: &Domain, below: &HashSet<LevelPage>, c: Chunk, above: bool, out: &mut Cover) -> bool {
        let here = self.source(c.page()).is_some();
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
        if self.source(c.page()).is_some() {
            out.drawn.push(c);
            return true;
        }
        if c.level() == 0 || !below.contains(&c.children()[0].page()) {
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

    /// Whether the view keeps `p` now: the pinned page, or one the domain
    /// wants. What a page with no record lives by, and what anything held
    /// for the view is held by.
    pub fn keeps(&self, p: LevelPage) -> bool {
        keeps(self.pinned, self.domain.as_ref(), p)
    }

    /// Which making of the domain this is.
    pub fn domains(&self) -> u32 {
        self.domains
    }

    /// What the resident pages can do for `p`, by what the cover made of its
    /// ground.
    fn resolve(&self, p: LevelPage) -> Resolved {
        if self.source(p).is_some() {
            Resolved::Exact
        } else if self.children_resident(p) {
            Resolved::Foldable
        } else if self.cover.gaps.iter().any(|g| share(g.node(), p.node())) {
            Resolved::Missing
        } else {
            Resolved::Covered
        }
    }

    // ---- records ----

    fn record_of(&self, p: LevelPage) -> Option<u32> {
        self.pages.get(&p)?.record
    }

    /// A record no page is in.
    fn free_record(&self) -> Option<u32> {
        let mut used = [false; RECORDS as usize];
        for record in self.pages.values().filter_map(|held| held.record) {
            used[record as usize] = true;
        }
        used.iter().position(|&u| !u).map(|at| at as u32)
    }

    /// How readily a resident page gives up its record, highest first. A page
    /// the camera does not want goes before one it does; among those, one
    /// something else already draws goes before one nothing covers; and among
    /// equals the one furthest out, the rim being margin. Ordered by
    /// comparison rather than a score, so a page the camera wants is never
    /// given up for one it does not, whatever the distance.
    fn worth(&self, p: LevelPage, anchor: IVec) -> (bool, bool, u32) {
        let wanted = self.keeps(p);
        (!wanted, self.covered(p), ring_of(anchor, p))
    }

    /// The record `p` goes into: its own, a free one, or the one whose page
    /// is worth least. None where every record carries a page worth more,
    /// which is an arrival the camera has moved past: dropped rather than
    /// turning out a page that is wanted, and counted, since whether the
    /// range is the wrong size is a reading and not an argument.
    fn record_for(&mut self, p: LevelPage, anchor: Option<IVec>) -> Option<u32> {
        if let Some(record) = self.record_of(p) {
            return Some(record);
        }
        if let Some(free) = self.free_record() {
            return Some(free);
        }
        let anchor = anchor?;
        let mine = self.worth(p, anchor);
        // The pinned page is never the one to go, whatever it is weighed at.
        let worst = self
            .pages
            .iter()
            .filter(|(&held, _)| Some(held) != self.pinned)
            .filter_map(|(&held, h)| Some((self.worth(held, anchor), h.record?)))
            .max_by(|a, b| a.0.cmp(&b.0));
        let taken = worst.filter(|(w, _)| *w > mine).map(|(_, r)| r);
        if taken.is_none() {
            self.refused += 1;
        }
        taken
    }

    /// The record is `p`'s from here, and whatever it carried is gone.
    fn take(&mut self, p: LevelPage, record: u32, page: L) {
        let gone = self.pages.iter().find(|(&held, h)| held != p && h.record == Some(record)).map(|(&held, _)| held);
        if let Some(gone) = gone {
            self.clear(gone);
        }
        self.put(p, page, Some(record));
    }

    /// Arrivals dropped for want of a record since the last take, for the
    /// ledger.
    pub fn take_refusals(&mut self) -> u32 {
        core::mem::take(&mut self.refused)
    }

    // ---- arriving ----

    /// A page's fetched body, made resident.
    pub fn adopt(&mut self, p: LevelPage, pkt: &[u8], phases: &mut Phases) -> Result<(), FetchFail> {
        let Some(record) = self.record_for(p, eye().map(|e| e.foot)) else { return Ok(()) };
        let page = L::receive(record, pkt, phases)?;
        self.take(p, record, page);
        Ok(())
    }

    /// Stands one page up from the four under it, if any of the view's wants
    /// can be. The finest first — the domain's pages are in level order — so
    /// what one fold makes feeds the next. Whether it found one.
    ///
    /// The ring around those four is read where it is held and stood in for
    /// where it is not: requiring it would put this at one page a level, and
    /// a zoom out would fetch pages whose ground is already held.
    fn fold_one(&mut self, anchor: IVec) -> bool {
        let Some(d) = self.domain.as_ref() else { return false };
        let wanted: Vec<LevelPage> = d.pages().map(|(p, _)| p).filter(|p| p.level > 0).collect();
        for p in wanted {
            if self.get(p).is_some() {
                continue;
            }
            let Some(folded) = L::fold_page(p, |q| self.get(q)) else { continue };
            let Some(record) = self.record_for(p, Some(anchor)) else { return false };
            return match L::keep(record, &folded) {
                Ok(page) => {
                    self.take(p, record, page);
                    true
                }
                Err(_) => false,
            };
        }
        false
    }

    // ---- wanting ----

    /// Every page the view wants that the resident pages cannot answer, each
    /// ranked by what its absence costs, and the pinned page wherever the
    /// view is.
    fn wants(&self) -> Vec<(u64, Rank)> {
        let mut wants = Vec::new();
        if let Some(d) = self.domain.as_ref() {
            for (p, drawn) in d.pages() {
                let resolved = self.resolve(p);
                // A page the tree stops above is none to ask for, whether or
                // not the page over it has come yet: that page is its own want.
                if matches!(resolved, Resolved::Exact | Resolved::Foldable) || self.settled(p) {
                    continue;
                }
                // A page held over ground drawn finer is coverage by what it
                // is for, whatever the cover now makes of its ground: it is
                // what that ground falls back to.
                let resolved = if drawn { resolved } else { Resolved::Missing };
                wants.push((p.subject(), policy::page(resolved, d.page_nearest_to(p))));
            }
        }
        if let Some(p) = self.pinned.filter(|p| self.get(*p).is_none() && !self.settled(*p) && !wants.iter().any(|(s, _)| *s == p.subject())) {
            wants.push((p.subject(), policy::pinned()));
        }
        wants
    }

    /// Lets go of the pages the camera has left that carry no record: a page
    /// the bake has no object for shares what it reads as with every other,
    /// so nothing will ever want its room and it would sit in the table for
    /// the session. A page with a record is left to `record_for`: the camera
    /// leaving a page is not a reason to throw it away.
    fn drop_unrecorded(&mut self, d: &Domain) {
        // An answer the camera has left is asked again if it comes back.
        let pinned = self.pinned;
        self.answered.retain(|&p| keeps(pinned, Some(d), p));
        let drop: Vec<LevelPage> = self.pages.iter().filter(|(&p, held)| held.record.is_none() && !keeps(pinned, Some(d), p)).map(|(&p, _)| p).collect();
        for p in drop {
            self.clear(p);
        }
    }

    /// The layer's turn. Wherever the view or the resident set has changed —
    /// a page arrived, was folded or went — the cover and the wishlist are
    /// made again, what the camera left let go and the folds woken, since each
    /// is a function of both. A turn where neither changed decides nothing
    /// and says the wishlist it has.
    pub fn step(&mut self) {
        let Some(e) = eye() else { return };
        let frustum = Frustum::of_view();
        let wish = (e.at, frustum, crate::sched::turn::interactive(), self.revision, self.pinned, heights_revision());
        if self.last != Some(wish) {
            let d = domain_for(&e, eye_height(), Some(frustum));
            self.drop_unrecorded(&d);
            let mut cover = L::present(self.cover_of(&d), &d, self);
            cover.drawn.sort_unstable();
            cover.gaps.sort_unstable();
            self.domain = Some(d);
            self.domains = self.domains.wrapping_add(1);
            // The eye moves every frame the camera does, and most of those
            // moves cross no chunk's split distance: whoever reads the cover
            // is told only when what it draws has changed.
            if cover != self.cover {
                self.cover = cover;
                self.covers = self.covers.wrapping_add(1);
            }
            // Letting a page go moves the revision; the wish is the resident
            // set the cover was made from.
            self.last = Some((wish.0, wish.1, wish.2, self.revision, wish.4, heights_revision()));
            self.wishlist = self.wants();
            self.folds.notify();
        }
        declare(L::CATEGORY, &self.wishlist);
    }

    /// Asks for nothing: a layer the view does not show holds what it has
    /// and wants no more.
    pub fn idle(&mut self) {
        declare(L::CATEGORY, &[]);
        self.last = None;
    }
}

/// The one rule a view keeps a page by: pinned, or wanted by its domain.
fn keeps(pinned: Option<LevelPage>, d: Option<&Domain>, p: LevelPage) -> bool {
    Some(p) == pinned || d.is_some_and(|d| d.wants(p))
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
