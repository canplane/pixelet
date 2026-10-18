// pxl-kernel/layer/residency.rs

//! Residency: which of a layer's tiles are held, in which frame, and what
//! answers a tile that is not. Any tile fits any frame. Tiles the fallback
//! asks for are held hard; the rest is cache. A tile is readable while it is
//! held and asked for, on its own, so a missing ancestor never hides a
//! resident descendant; a tile held only as cache is bytes kept for when it
//! is asked again, and answers nothing, so what is read is a function of
//! what this observation asks and never of where the eye has been. A miss
//! falls back to the nearest resident tile over it (`drawn_from`).
//!
//! Capacity is asserted, not observed, since thrash here would be silent:
//! frames cover everything held hard plus everything demand can name at once
//! (the reach), so a view at rest converges to exact.

pub mod floor;

use fnv::FnvHashMap;
use pxl_spec::bake::tile::Occupancy;

use pxl_spec::space::layer::demand::reach_tiles;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};

use crate::io::demand::{Ask, Why};
use crate::layer::{Asks, Base};

/// Only the eye. Another point observer would add its own misses.
const OBSERVERS: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Hold {
    Hard,
    Demand,
    Cache,
}

#[derive(Clone, Copy, Debug)]
pub struct Candidate {
    pub tile: Tile,
}

/// Which frame goes when one must, chosen only among those that may, so a
/// policy changes what is held next and never what is correct.
pub trait Victim {
    fn victim(&self, candidates: &[Candidate]) -> Option<usize>;
}

/// No frame for an empty tile, whose body is its own. One asked for again
/// (`again`) is read meanwhile and fetched once more.
struct Held<L> {
    body: L,
    frame: Option<u32>,
    again: bool,
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

pub struct Residency<L: Base> {
    tiles: FnvHashMap<Tile, Entry<L>>,
    /// The tile each frame holds: the frames' side of `Held::frame`, one
    /// association read both ways — a tile's frame by every reader of a
    /// tile (`frame`, `framed`; layer/earth.rs `earth_window`,
    /// layer/terrain/tile.rs `kept_for`, layer/terrain/window.rs `voids`),
    /// a frame's tile by the GPU's hand-offs, which walk the frames
    /// (scene/cast.rs `upload`, layer/earth.rs `earth_upload`). Written only
    /// where a frame is taken or given up (`put`, `remove`).
    in_frame: Vec<Option<Tile>>,
    free: Vec<u32>,
    /// Kept so `ask` allocates nothing.
    going: Vec<Tile>,
    /// Tiles whose answer moved since `take_moved`: held or let go, answered
    /// from above or no longer. A tile's content is fixed by its identity, so
    /// this is all that changes.
    moved: Vec<Tile>,
    /// How many times what it holds moved: a tile held or let go, asked for
    /// again or no longer, answered from above. Not what is read over a
    /// tile (`moved_at`), which holds nothing.
    revision: u32,
    refused: u32,
    admitted: u32,
    evicted: u32,
}

/// What a layer's residency holds at once, from what it asks for. A layer
/// holds only what it asks for (`Base::ASKS`).
impl<L: Base> Residency<L> {
    /// The zooms the layer's pyramid holds, its leaves to its lattice's top.
    pub const ZOOMS: u32 = (L::LEAF - L::LATTICE.top() + 1);

    /// A destination's line from its leaf to the top.
    pub const DESTINATION_TILES: u32 = Self::ZOOMS;

    /// The most of its tiles a zoom's demand names, by its lattice.
    pub const ZOOM_DEMAND_TILES: u32 = reach_tiles(L::LATTICE);

    const DEMAND_TILES: u32 = if matches!(L::ASKS, Asks::View) { OBSERVERS * Self::ZOOM_DEMAND_TILES * Self::ZOOMS } else { 0 };

    /// The lines over the misses, every zoom's but the finest.
    const ANCESTOR_TILES: u32 = if matches!(L::ASKS, Asks::View) {
        OBSERVERS * Self::ZOOM_DEMAND_TILES * (Self::ZOOMS - 1)
    } else {
        0
    };

    /// Held hard apart from the view: the floor, a destination's line, and
    /// what a reader that cannot take a coarser answer declares (`Base::PINNED`).
    const PINNED_TILES: u32 = match L::ASKS {
        Asks::Floor => floor::tiles(L::LATTICE),
        Asks::Line => Self::DESTINATION_TILES + L::PINNED,
        Asks::View => floor::tiles(L::LATTICE) + Self::DESTINATION_TILES + L::PINNED,
    };

    /// What is held hard at most.
    const HARD_TILES: u32 = Self::PINNED_TILES + Self::ANCESTOR_TILES;

    /// A frame for a tile nothing holds hard to land in. The floor alone holds
    /// every tile it asks for hard, and asks for at most `floor::tiles`, so a
    /// tile it asks for finds a frame free or held only as cache.
    const WORKSPACE: u32 = matches!(L::ASKS, Asks::View) as u32;

    /// Convergence: the misses and their lines share each zoom's ring, so a tile
    /// the view reads never gives up another the view reads.
    pub const FRAMES: u32 = Self::PINNED_TILES + Self::DEMAND_TILES;

    /// Tiles residency knows of at once: every framed tile and every asked
    /// one, and what is asked is at most the frames.
    const ENTRIES: u32 = 2 * Self::FRAMES;

    const SAFE: () = assert!(Self::FRAMES >= Self::HARD_TILES + Self::WORKSPACE, "safety: no frame for a tile nothing holds hard to land in");
}

impl<L: Base> Default for Residency<L> {
    fn default() -> Self {
        let () = Self::SAFE;
        Residency {
            tiles: FnvHashMap::with_capacity_and_hasher(Self::ENTRIES as usize, Default::default()),
            in_frame: vec![None; Self::FRAMES as usize],
            // Lowest first, as the frames are taken.
            free: (0..Self::FRAMES).rev().collect(),
            going: Vec::with_capacity(Self::ENTRIES as usize),
            moved: Vec::new(),
            revision: 0,
            refused: 0,
            admitted: 0,
            evicted: 0,
        }
    }
}

/// What a tile's line, it and the tiles over it to the layer's top, says of
/// it; a child's is its parent's and its own entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line {
    pub tile: Tile,
    /// The finest resident tile on the line, which the tile is drawn from.
    pub drawn: Option<Tile>,
    /// The resident tile that says the tile's ground exactly.
    pub source: Option<Tile>,
    /// Some tile on the line is said.
    pub settled: bool,
    /// Answered for itself, or its parent holds no child there.
    said: bool,
    /// What says it if it is said: the resident tile over it, past said ones.
    over: Option<Tile>,
    /// What says it through the first said tile on its line.
    says: Option<Tile>,
    /// Its children as it holds them, where it is resident.
    children: Option<Occupancy>,
}

/// The lines of a walk's last tile and the tiles over it, by zoom, so a walk
/// to a tile beside it steps down only from where their lines part. Good
/// while the residency it was walked over is unchanged.
pub struct Lines {
    at: [Option<Line>; LEAF_ZOOM as usize + 1],
}

impl Default for Lines {
    fn default() -> Self {
        Lines { at: [None; LEAF_ZOOM as usize + 1] }
    }
}

impl Line {
    /// Over the top: nothing.
    const ABOVE: Line = Line {
        tile: Tile { zoom: 0, at: [0, 0] },
        drawn: None,
        source: None,
        settled: false,
        said: false,
        over: None,
        says: None,
        children: None,
    };

    pub fn resident(&self) -> bool {
        self.children.is_some()
    }
}

impl<L: Base> Residency<L> {
    fn entry(&self, p: Tile) -> Option<&Entry<L>> {
        self.tiles.get(&p)
    }

    /// Past `ENTRIES` is a tile kept for no reason the bound counts.
    fn entry_mut(&mut self, p: Tile) -> &mut Entry<L> {
        let full = self.tiles.len() >= Self::ENTRIES as usize;
        self.tiles.entry(p).or_insert_with(|| {
            xpute_kit::ensure!(!full, ENOTRECOVERABLE, p.zoom);
            Entry::NONE
        })
    }

    fn held_as(&self, p: Tile) -> Option<&Held<L>> {
        self.entry(p)?.held.as_ref()
    }

    /// Held and asked for: what a reader may read.
    fn readable(&self, p: Tile) -> Option<&Held<L>> {
        self.entry(p).filter(|e| e.ask.is_some())?.held.as_ref()
    }

    /// Held and asked for, and not asked for again: nothing more is fetched for it.
    pub fn resident(&self, p: Tile) -> bool {
        self.readable(p).is_some_and(|h| !h.again)
    }

    /// Held in a frame of its own, not as an empty tile.
    fn framed(&self, p: Tile) -> bool {
        self.held_as(p).is_some_and(|h| h.frame.is_some())
    }

    /// The frame a held tile is in; none for an empty one.
    pub fn frame(&self, p: Tile) -> Option<u32> {
        self.readable(p)?.frame
    }

    pub fn get(&self, p: Tile) -> Option<&L> {
        self.readable(p).map(|h| &h.body)
    }

    /// Every readable tile.
    pub fn held(&self) -> impl Iterator<Item = (Tile, &L)> {
        self.tiles.iter().filter(|(_, e)| e.ask.is_some()).filter_map(|(&p, e)| Some((p, &e.held.as_ref()?.body)))
    }

    pub fn frames_used(&self) -> u32 {
        Self::FRAMES - self.free.len() as u32
    }

    /// What `p`'s line says of it, stepped down from the top a tile at a time.
    pub fn line(&self, p: Tile) -> Line {
        self.seek(&mut Lines::default(), p)
    }

    /// `p`'s line, stepped down from the deepest tile over it `lines` still
    /// holds, which then holds `p`'s.
    pub fn seek(&self, lines: &mut Lines, p: Tile) -> Line {
        let held = |q: Tile| lines.at[q.zoom as usize].filter(|l| l.tile == q);
        let line = match held(p) {
            Some(l) => l,
            None if p.zoom > L::LATTICE.top() => {
                let over = self.seek(lines, p.parent());
                self.step(&over, p)
            }
            None => self.step(&Line::ABOVE, p),
        };
        lines.at[p.zoom as usize] = Some(line);
        line
    }

    /// `child`'s line from its parent's, reading only `child`'s own entry: a
    /// descent pays one lookup a step and never walks back up.
    pub fn step(&self, parent: &Line, child: Tile) -> Line {
        let e = self.entry(child);
        let held = e.filter(|e| e.ask.is_some()).and_then(|e| e.held.as_ref());
        // Answered for itself, or its parent holds no child there.
        let said = e.is_some_and(|e| e.answered) || parent.children.is_some_and(|o| !o.has_child(child.slot()));
        let over = match parent.resident() {
            true => Some(parent.tile),
            false if parent.said => parent.over,
            false => None,
        };
        let says = if said { over } else { parent.says };
        Line {
            tile: child,
            drawn: if held.is_some() { Some(child) } else { parent.drawn },
            source: if held.is_some() { Some(child) } else { says },
            settled: said || parent.settled,
            said,
            over,
            says,
            children: held.map(|h| h.body.occupancy()),
        }
    }

    pub fn settled(&self, p: Tile) -> bool {
        self.line(p).settled
    }

    /// The finest resident tile on `p`'s line: the miss's answer.
    pub fn drawn_from(&self, p: Tile) -> Option<Tile> {
        self.line(p).drawn
    }

    /// The resident tile that says `p`'s ground exactly; none where `p` is a
    /// miss. A resident tile above that does not say `p` only draws it coarser.
    pub fn source(&self, p: Tile) -> Option<Tile> {
        self.line(p).source
    }

    /// Empty tiles and tiles answered above are kept only while asked:
    /// they cost no frame, so nothing would ever evict them. Framed tiles are
    /// left to normal turnover.
    pub fn ask(&mut self, asks: &FnvHashMap<Tile, Ask>) {
        xpute_kit::ensure!(asks.len() <= Self::FRAMES as usize, ENOTRECOVERABLE, asks.len());
        for (&p, e) in &mut self.tiles {
            let was = e.ask.is_some();
            e.ask = asks.get(&p).copied();
            // A held tile asked for again, or no longer, is read or not: its
            // answer moved as surely as one arriving or going.
            let read = e.held.is_some() && was != e.ask.is_some();
            if read | (e.ask.is_none() && core::mem::take(&mut e.answered)) {
                self.moved.push(p);
                self.revision = self.revision.wrapping_add(1);
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
        }
        self.going = going;
        self.tiles.retain(|_, e| !e.idle());
        for (&p, &a) in asks {
            self.entry_mut(p).ask = Some(a);
        }
    }

    pub fn asked(&self, p: Tile) -> Option<Ask> {
        self.entry(p)?.ask
    }

    fn hold(&self, p: Tile) -> Hold {
        let Some(e) = self.entry(p) else { return Hold::Cache };
        match e.ask.map(|a| a.why) {
            Some(Why::Floor | Why::Destination | Why::Ancestor) => Hold::Hard,
            Some(Why::Demand) => Hold::Demand,
            None => Hold::Cache,
        }
    }

    /// A frame for arriving `p`: its own, a free one, or a victim among frames
    /// held only as cache. None only for a tile no longer asked for, counted;
    /// for one asked for, none means the capacity bound is broken.
    fn frame_for(&mut self, p: Tile, victim: &impl Victim) -> Option<u32> {
        if self.framed(p) {
            return self.remove(p);
        }
        if let Some(free) = self.free.pop() {
            return Some(free);
        }
        let unasked = self.hold(p) == Hold::Cache;
        let mut candidates: Vec<Candidate> = self
            .tiles
            .iter()
            .filter(|&(&q, _)| self.framed(q) && self.hold(q) == Hold::Cache)
            .map(|(&q, _)| Candidate { tile: q })
            .collect();
        // Sorted, so the policy sees the same list for the same state.
        candidates.sort_unstable_by_key(|c| c.tile);
        let Some(at) = victim.victim(&candidates) else {
            xpute_kit::ensure!(unasked, ENOTRECOVERABLE, p.zoom);
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
        if !core::mem::replace(&mut self.entry_mut(p).answered, true) {
            self.moved.push(p);
            self.revision = self.revision.wrapping_add(1);
        }
    }

    /// What it holds as of now (`revision`): equal where nothing it holds
    /// moved between.
    pub fn revision(&self) -> u32 {
        self.revision
    }

    /// The tiles whose answer moved since last taken, into `out`.
    pub fn take_moved(&mut self, out: &mut Vec<Tile>) {
        out.clear();
        core::mem::swap(out, &mut self.moved);
    }

    fn put(&mut self, p: Tile, body: L, frame: Option<u32>) {
        if let Some(was) = self.entry_mut(p).held.replace(Held { body, frame, again: false }).and_then(|h| h.frame) {
            self.in_frame[was as usize] = None;
        }
        if let Some(f) = frame {
            self.in_frame[f as usize] = Some(p);
        }
        self.moved.push(p);
        self.revision = self.revision.wrapping_add(1);
    }

    /// A framed tile held is fetched once more, read meanwhile: what a
    /// layer that keeps nothing of a packet asks when it needs it again.
    pub fn again(&mut self, p: Tile) {
        if let Some(h) = self.tiles.get_mut(&p).and_then(|e| e.held.as_mut()).filter(|h| h.frame.is_some()) {
            h.again = true;
        }
    }

    /// Held and asked for again (`again`).
    pub fn again_held(&self, p: Tile) -> bool {
        self.readable(p).is_some_and(|h| h.again)
    }

    /// What is read over `p` moved, as a tile's arriving would.
    pub fn moved_at(&mut self, p: Tile) {
        self.moved.push(p);
    }

    /// The tile held in frame `record`.
    pub fn in_frame(&self, record: u32) -> Option<Tile> {
        self.in_frame[record as usize]
    }

    /// Returns the frame `p` was in, if any, for the caller to fill or free.
    fn remove(&mut self, p: Tile) -> Option<u32> {
        let std::collections::hash_map::Entry::Occupied(mut e) = self.tiles.entry(p) else { return None };
        let held = e.get_mut().held.take()?;
        if e.get().idle() {
            e.remove();
        }
        if let Some(f) = held.frame {
            self.in_frame[f as usize] = None;
        }
        self.moved.push(p);
        self.revision = self.revision.wrapping_add(1);
        held.frame
    }
}
