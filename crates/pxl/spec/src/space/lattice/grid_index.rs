// pxl-spec/space/lattice/grid_index.rs

//! Screen-space collision grid for greedy label placement.
//!
//! The question a spatial hash cannot answer. `spatial_hash.rs` indexes where
//! things are in the world; this indexes where they have already been *drawn*,
//! so the next candidate can find out whether the pixels it wants are free.
//! A renderer with more labels than room needs both, and they are different
//! structures because they are different spaces — one unbounded and
//! three-dimensional, one exactly the size of the viewport and flat.
//!
//! Mapbox GL's `GridIndex` is the shape, and the reason to follow it rather
//! than invent is that the failure modes here are not obvious: labels that
//! flicker as the camera moves, labels that vanish when a more important one
//! appears two cells away, ties that resolve differently frame to frame.
//! Theirs divides a 600x600 viewport into 30 px cells — 400 of them — and
//! reports near-constant query cost, since "doubling the dimensions of the
//! index is not expected to have a significant impact on the cost of a
//! query".
//!
//! **Greedy, in the caller's order, and that order is the design.** For each
//! candidate: test, and place if the space is free. There is no global
//! optimisation and no backtracking — first fit wins, so whatever sequence
//! the caller supplies *is* the priority rule. A candidate that loses is not
//! retried at a different position; a renderer that wants alternate anchors
//! offers them as separate candidates.
//!
//! **Placement is not a per-frame job.** Mapbox runs this synchronously but
//! explicitly not every frame, on the grounds that "a latency of a couple
//! hundred milliseconds in actually detecting a new collision is perceptually
//! acceptable". The structure is cheap enough either way; the caller decides.
//!
//! **Everything is preallocated and a rebuild allocates nothing.** Cells hold
//! an intrusive singly-linked list over one node pool, and a generation stamp
//! makes a stale cell read as empty instead of being cleared — the same two
//! devices `spatial_hash.rs` uses, for the same reason. Running out of nodes
//! is reported rather than absorbed: a full grid means every later candidate
//! collides, which is the honest answer and also the one that keeps the live
//! object count fixed.

/// Mapbox's, on their reference viewport. Small enough that a label spans a
/// handful of cells, large enough that the grid stays short.
pub const DEFAULT_CELL_PX: f64 = 30.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridIndexStats {
    /// Rectangles placed in the current generation.
    pub placed: u32,
    /// Placements refused for want of a rectangle slot or a node.
    pub dropped: u32,
    /// Node-pool entries consumed.
    pub nodes: u32,
}

/// What `place` and `first_hit` answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    /// The rectangle owns its space; nothing blocked it.
    Placed,
    /// Refused for want of a slot or a node, so it is neither drawn nor
    /// attributable to anything. Reported through `stats().dropped` too.
    /// `first_hit` never answers this.
    NoRoom,
    /// The id of the placed rectangle standing where this one wanted to.
    Blocked(u32),
}

pub struct GridIndex {
    cell_px: f64,
    capacity: u32,
    node_capacity: u32,

    cols: u32,
    rows: u32,

    /// x0, y0, x1, y1 interleaved, so one rectangle is one cache line's worth
    /// of contiguous reads rather than four strided ones.
    rects: Box<[f32]>,
    placed: u32,
    dropped: u32,

    /// Head node index per cell, valid only while the stamp is current.
    heads: Box<[i32]>,
    stamps: Box<[u32]>,
    generation: u32,

    /// Intrusive list: `node_rect[n]` is in this cell, `node_next[n]` is the
    /// next one, -1 ending the chain.
    node_rect: Box<[u32]>,
    node_next: Box<[i32]>,
    nodes: u32,
}

impl GridIndex {
    /// - `max_width_px`: largest viewport this will be asked about
    /// - `max_height_px`: same
    /// - `capacity`: most rectangles placed in one generation
    /// - `cell_px`: cell edge; a label should span only a few
    /// - `nodes_per_rect`: budget for how many cells one rectangle covers —
    ///   exceeding it stops placement rather than growing
    pub fn new(max_width_px: u32, max_height_px: u32, capacity: u32, cell_px: f64, nodes_per_rect: u32) -> GridIndex {
        let node_capacity = capacity * nodes_per_rect;

        let max_cells = ((max_width_px as f64 / cell_px + 1.0).ceil() * (max_height_px as f64 / cell_px + 1.0).ceil()) as usize;
        GridIndex {
            cell_px,
            capacity,
            node_capacity,
            cols: 0,
            rows: 0,
            rects: vec![0.0; capacity as usize * 4].into_boxed_slice(),
            placed: 0,
            dropped: 0,
            heads: vec![0; max_cells].into_boxed_slice(),
            stamps: vec![0; max_cells].into_boxed_slice(),
            generation: 0,
            node_rect: vec![0; node_capacity as usize].into_boxed_slice(),
            node_next: vec![0; node_capacity as usize].into_boxed_slice(),
            nodes: 0,
        }
    }

    /// Opens a generation for a viewport of this size. Nothing is cleared: a
    /// cell whose stamp is stale reads as empty on its next touch.
    ///
    /// The viewport is taken per generation rather than per construction
    /// because a window resize must not reallocate — the grid was sized for the
    /// largest case at construction and simply uses fewer cells when smaller.
    pub fn begin(&mut self, width_px: f64, height_px: f64) {
        self.cols = ((width_px / self.cell_px).ceil() as u32).max(1);
        self.rows = ((height_px / self.cell_px).ceil() as u32).max(1);

        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.stamps.fill(0);
            self.generation = 1;
        }
        self.placed = 0;
        self.dropped = 0;
        self.nodes = 0;
    }

    fn col_of(&self, x: f64) -> i32 {
        ((x / self.cell_px).floor().max(0.0) as i32).min(self.cols as i32 - 1)
    }
    fn row_of(&self, y: f64) -> i32 {
        ((y / self.cell_px).floor().max(0.0) as i32).min(self.rows as i32 - 1)
    }

    /// Whether this rectangle overlaps anything already placed.
    ///
    /// A rectangle entirely off-screen collides with nothing, which is correct
    /// rather than convenient: this structure only knows about pixels, and
    /// deciding that something out of view should not be drawn belongs to
    /// whatever culls before this runs.
    pub fn hit_test(&self, x0: f64, y0: f64, x1: f64, y1: f64) -> bool {
        self.first_hit(x0, y0, x1, y1) != Placement::Placed
    }

    /// Which placed rectangle this one runs into first, or `Placed` for none.
    ///
    /// The id is the caller's, in the sense that `insert` hands them out in
    /// order — so a caller placing candidates one at a time reads it back as
    /// "the candidate that took this space". That is what turns a rejection
    /// into an attribution: a label that cannot be drawn is not nothing, it is
    /// one more thing standing where the winner stands, and only the structure
    /// that refused it knows which winner that was.
    ///
    /// "First" is the first found, not the nearest — with the caller placing in
    /// priority order every occupant of a contested space is one this candidate
    /// already lost to, so any of them is an honest answer.
    pub fn first_hit(&self, x0: f64, y0: f64, x1: f64, y1: f64) -> Placement {
        if x1 < 0.0 || y1 < 0.0 || x0 > self.cols as f64 * self.cell_px || y0 > self.rows as f64 * self.cell_px {
            return Placement::Placed;
        }

        let (c0, c1) = (self.col_of(x0), self.col_of(x1));
        let (r0, r1) = (self.row_of(y0), self.row_of(y1));

        for r in r0..=r1 {
            for c in c0..=c1 {
                let cell = (r * self.cols as i32 + c) as usize;
                if self.stamps[cell] != self.generation {
                    continue;
                }
                let mut n = self.heads[cell];
                while n != -1 {
                    let o = self.node_rect[n as usize] as usize * 4;
                    // A rectangle in more than one shared cell is reached more than
                    // once. Re-testing it is cheaper than the bookkeeping that would
                    // avoid it, since the first overlap returns immediately anyway.
                    if x0 <= self.rects[o + 2] as f64 && x1 >= self.rects[o] as f64 && y0 <= self.rects[o + 3] as f64 && y1 >= self.rects[o + 1] as f64 {
                        return Placement::Blocked(self.node_rect[n as usize]);
                    }
                    n = self.node_next[n as usize];
                }
            }
        }
        Placement::Placed
    }

    /// Records a rectangle as occupied without testing it — for the things that
    /// are drawn whatever else wants the space, and which later candidates must
    /// still avoid.
    ///
    /// Returns false when a slot or a node is unavailable, which leaves the
    /// rectangle unregistered: from then on the grid under-reports occupancy,
    /// so a caller that cares reads `stats().dropped` rather than assuming.
    pub fn insert(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) -> bool {
        if self.placed >= self.capacity {
            self.dropped += 1;
            return false;
        }
        let (c0, c1) = (self.col_of(x0), self.col_of(x1));
        let (r0, r1) = (self.row_of(y0), self.row_of(y1));

        let needed = ((c1 - c0 + 1) * (r1 - r0 + 1)) as u32;
        if self.nodes + needed > self.node_capacity {
            self.dropped += 1;
            return false;
        }

        let id = self.placed;
        self.placed += 1;
        let o = id as usize * 4;
        self.rects[o] = x0 as f32;
        self.rects[o + 1] = y0 as f32;
        self.rects[o + 2] = x1 as f32;
        self.rects[o + 3] = y1 as f32;

        for r in r0..=r1 {
            for c in c0..=c1 {
                let cell = (r * self.cols as i32 + c) as usize;
                if self.stamps[cell] != self.generation {
                    self.stamps[cell] = self.generation;
                    self.heads[cell] = -1;
                }
                let n = self.nodes as usize;
                self.nodes += 1;
                self.node_rect[n] = id;
                self.node_next[n] = self.heads[cell];
                self.heads[cell] = n as i32;
            }
        }
        true
    }

    /// The greedy step: take the space if it is free, and otherwise say who has
    /// it — `Placed` when this rectangle now occupies it, `NoRoom` when the
    /// structure is full, and `Blocked` with the id that stands there.
    ///
    /// Test and insert are separately available because some callers need one
    /// without the other, but this pairing is the operation the structure
    /// exists for, and keeping it here means the two halves cannot drift apart
    /// — a caller that tested one rectangle and inserted a slightly different
    /// one would corrupt every later answer.
    pub fn place(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) -> Placement {
        match self.first_hit(x0, y0, x1, y1) {
            Placement::Placed if self.insert(x0, y0, x1, y1) => Placement::Placed,
            Placement::Placed => Placement::NoRoom,
            blocked => blocked,
        }
    }

    pub fn stats(&self) -> GridIndexStats {
        GridIndexStats {
            placed: self.placed,
            dropped: self.dropped,
            nodes: self.nodes,
        }
    }
}

#[cfg(test)]
#[path = "grid_index.test.rs"]
mod test;
