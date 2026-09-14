// pxl_core/module/world/billboard/cell_node.rs

//! One frame's lattice nodes, and how many things fell into each.
//!
//! The tree, such as it is. A node is a cell of `1 << shamt` canonical units
//! and its address is the candidate's own cell shifted right by that shamt —
//! so every tier exists at once, none of them is stored, and moving between
//! them is an integer shift. That is 3D Tiles' implicit tiling reduced to the
//! part this needs: address by coordinate, build no nodes.
//!
//! It is a hierarchy of aggregates, which puts it in the same family as a
//! segment tree, but not the same shape: every question asked here is one whole
//! cell at one level, because that is what a badge is. The closer relative is a
//! mipmap: same grid, each level half the resolution, each texel the sum of its
//! children.
//!
//! Shamt is per node rather than per frame, and that is the whole reason for
//! the shape. A perspective camera holds near and far in one view, so one tier
//! for the whole frame is wrong wherever the other end of the view is — the
//! same argument that gives chunks their own LodStep instead of one for the
//! scene.
//!
//! Open addressing over flat integer arrays, generation-stamped rather than
//! cleared, for the same reason spatial_hash is: a per-frame structure that
//! allocates is a per-frame collection pause.

use xpute_core::ds::lattice_hash::lattice_hash4;

/// Marks a slot as never written in this generation.
pub const EMPTY: i32 = -1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CellNodeStats {
    pub nodes: u32,
    pub dropped: u32,
    pub probes: u32,
}

pub struct CellNodes {
    pub cap: u32,
    pub count: u32,

    /// Cell address at this node's own tier, on each axis.
    pub key_x: Vec<i32>,
    pub key_y: Vec<i32>,
    pub key_z: Vec<i32>,
    /// The tier, as a shift of the canonical unit.
    pub shamt: Vec<u8>,

    /// How many candidates fell in. One is a place; more is a count.
    pub members: Vec<u32>,
    /// The caller's index for the nearest member, which is the one whose
    /// identity a single-member node borrows and whose tiebreak orders it.
    pub first: Vec<u32>,
    /// That member's squared distance, kept so `first` stays the nearest rather
    /// than whichever arrived first — arrival order is the query's, and the
    /// query's order is the hash's, which is not a fact about the world.
    pub first_d2: Vec<f64>,

    slots: Vec<i32>,
    stamps: Vec<u32>,
    mask: u32,
    generation: u32,

    dropped: u32,
    probes: u32,
}

impl CellNodes {
    /// `cap` is nodes; the table is the next power of two above twice that, so
    /// the load factor stays under a half and probes stay short.
    pub fn new(cap: u32) -> CellNodes {
        let mut size = 1u32;
        while size < cap * 2 {
            size <<= 1;
        }
        let n = cap as usize;
        CellNodes {
            cap,
            count: 0,
            key_x: vec![0; n],
            key_y: vec![0; n],
            key_z: vec![0; n],
            shamt: vec![0; n],
            members: vec![0; n],
            first: vec![0; n],
            first_d2: vec![0.0; n],
            slots: vec![0; size as usize],
            stamps: vec![0; size as usize],
            mask: size - 1,
            generation: 0,
            dropped: 0,
            probes: 0,
        }
    }

    /// Opens a generation. Nothing is cleared — a slot whose stamp is stale
    /// reads as empty and is overwritten on its next touch.
    pub fn begin(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.stamps.fill(0);
            self.generation = 1;
        }
        self.count = 0;
        self.dropped = 0;
        self.probes = 0;
    }

    /// Files one candidate under its cell, and returns the node it landed in —
    /// or EMPTY when there is no room for a new one.
    ///
    /// `cell_*` is the candidate's address at the *finest* tier; the shift to
    /// `shamt` happens here, so a caller never has to hold more than one set of
    /// coordinates however many tiers are in play.
    #[allow(clippy::too_many_arguments)]
    pub fn add(&mut self, cell_x: i32, cell_y: i32, cell_z: i32, shamt: u32, src: u32, d2: f64) -> i32 {
        let (kx, ky, kz) = (cell_x >> shamt, cell_y >> shamt, cell_z >> shamt);
        // The shamt joins the key: two tiers can be live at once, and a parent's
        // address collides with its child's on a coarse axis.
        let h = lattice_hash4(kx, ky, kz, shamt);

        for probe in 0..=self.mask {
            let slot = (h.wrapping_add(probe) & self.mask) as usize;
            self.probes += 1;

            if self.stamps[slot] != self.generation {
                if self.count >= self.cap {
                    self.dropped += 1;
                    return EMPTY;
                }
                let id = self.count as usize;
                self.count += 1;
                self.stamps[slot] = self.generation;
                self.slots[slot] = id as i32;
                self.key_x[id] = kx;
                self.key_y[id] = ky;
                self.key_z[id] = kz;
                self.shamt[id] = shamt as u8;
                self.members[id] = 1;
                self.first[id] = src;
                self.first_d2[id] = d2;
                return id as i32;
            }

            let id = self.slots[slot] as usize;
            if self.key_x[id] != kx || self.key_y[id] != ky || self.key_z[id] != kz || self.shamt[id] as u32 != shamt {
                continue;
            }

            self.members[id] += 1;
            if d2 < self.first_d2[id] {
                self.first[id] = src;
                self.first_d2[id] = d2;
            }
            return id as i32;
        }
        self.dropped += 1;
        EMPTY
    }

    /// The node's centre, on one axis, in the same units its cells are in — the
    /// half being `1 << shamt` over two, so it follows the tier instead of being
    /// a constant that is right at one of them.
    pub fn centre(&self, key: i32, shamt: u32) -> f64 {
        let span = (1i64 << shamt) as f64;
        key as f64 * span + span / 2.0
    }

    pub fn stats(&self) -> CellNodeStats {
        CellNodeStats {
            nodes: self.count,
            dropped: self.dropped,
            probes: self.probes,
        }
    }
}
