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
//! **A rebuild allocates nothing once the grid has grown to its screen.**
//! Cells hold an intrusive singly-linked list over one node pool, and a
//! generation stamp makes a stale cell read as empty instead of being cleared
//! — the same two devices `spatial_hash.rs` uses, for the same reason. The
//! pool and the cells grow to what a generation needs and are kept, so there
//! is no capacity to run out of: what bounds the placed rectangles is the
//! screen, since they do not overlap, and a grid that could refuse one would
//! answer "collides" for a rectangle nothing stands on.

/// Mapbox's, on their reference viewport. Small enough that a label spans a
/// handful of cells, large enough that the grid stays short.
pub const DEFAULT_CELL_PX: f64 = 30.0;

/// What `place` answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    /// The rectangle owns its space; nothing blocked it.
    Placed,
    /// The id of the placed rectangle standing where this one wanted to.
    Blocked(u32),
}

pub struct GridIndex {
    cell_px: f64,

    cols: u32,
    rows: u32,

    /// x0, y0, x1, y1 interleaved, so one rectangle is one cache line's worth
    /// of contiguous reads rather than four strided ones.
    rects: Vec<[f32; 4]>,

    /// Head node index per cell, valid only while the stamp is current.
    heads: Vec<i32>,
    stamps: Vec<u32>,
    generation: u32,

    /// Intrusive list: `nodes[n].0` is in this cell, `nodes[n].1` is the next
    /// one, -1 ending the chain.
    nodes: Vec<(u32, i32)>,
}

impl GridIndex {
    /// `cell_px` is the cell edge; a label should span only a few.
    pub fn new(cell_px: f64) -> GridIndex {
        GridIndex {
            cell_px,
            cols: 0,
            rows: 0,
            rects: Vec::new(),
            heads: Vec::new(),
            stamps: Vec::new(),
            generation: 0,
            nodes: Vec::new(),
        }
    }

    /// Opens a generation for a viewport of this size. Nothing is cleared: a
    /// cell whose stamp is stale reads as empty on its next touch, and a cell
    /// the grid grows by starts stale.
    pub fn begin(&mut self, width_px: f64, height_px: f64) {
        self.cols = ((width_px / self.cell_px).ceil() as u32).max(1);
        self.rows = ((height_px / self.cell_px).ceil() as u32).max(1);
        let cells = (self.cols * self.rows) as usize;
        if self.stamps.len() < cells {
            self.heads.resize(cells, -1);
            self.stamps.resize(cells, 0);
        }

        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.stamps.fill(0);
            self.generation = 1;
        }
        self.rects.clear();
        self.nodes.clear();
    }

    fn col_of(&self, x: f64) -> i32 {
        ((x / self.cell_px).floor().max(0.0) as i32).min(self.cols as i32 - 1)
    }
    fn row_of(&self, y: f64) -> i32 {
        ((y / self.cell_px).floor().max(0.0) as i32).min(self.rows as i32 - 1)
    }

    /// Which placed rectangle this one runs into first, or `Placed` for none.
    ///
    /// A rectangle entirely off-screen collides with nothing, which is correct
    /// rather than convenient: this structure only knows about pixels, and
    /// deciding that something out of view should not be drawn belongs to
    /// whatever culls before this runs.
    ///
    /// "First" is the first found, not the nearest — with the caller placing in
    /// priority order every occupant of a contested space is one this candidate
    /// already lost to, so any of them is an honest answer.
    fn first_hit(&self, x0: f64, y0: f64, x1: f64, y1: f64) -> Placement {
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
                    let (id, next) = self.nodes[n as usize];
                    let o = self.rects[id as usize];
                    // A rectangle in more than one shared cell is reached more than
                    // once. Re-testing it is cheaper than the bookkeeping that would
                    // avoid it, since the first overlap returns immediately anyway.
                    if x0 <= o[2] as f64 && x1 >= o[0] as f64 && y0 <= o[3] as f64 && y1 >= o[1] as f64 {
                        return Placement::Blocked(id);
                    }
                    n = next;
                }
            }
        }
        Placement::Placed
    }

    fn insert(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) {
        let (c0, c1) = (self.col_of(x0), self.col_of(x1));
        let (r0, r1) = (self.row_of(y0), self.row_of(y1));

        let id = self.rects.len() as u32;
        self.rects.push([x0 as f32, y0 as f32, x1 as f32, y1 as f32]);

        for r in r0..=r1 {
            for c in c0..=c1 {
                let cell = (r * self.cols as i32 + c) as usize;
                if self.stamps[cell] != self.generation {
                    self.stamps[cell] = self.generation;
                    self.heads[cell] = -1;
                }
                self.nodes.push((id, self.heads[cell]));
                self.heads[cell] = self.nodes.len() as i32 - 1;
            }
        }
    }

    /// The greedy step: take the space if it is free, and otherwise say who
    /// has it. The id is the order of placement, so a caller placing
    /// candidates one at a time reads it back as "the candidate that took this
    /// space".
    ///
    /// Test and insert are one operation so the two halves cannot drift apart
    /// — a caller that tested one rectangle and inserted a slightly different
    /// one would corrupt every later answer.
    pub fn place(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) -> Placement {
        let hit = self.first_hit(x0, y0, x1, y1);
        if hit == Placement::Placed {
            self.insert(x0, y0, x1, y1);
        }
        hit
    }
}

#[cfg(test)]
#[path = "grid_index.test.rs"]
mod test;
