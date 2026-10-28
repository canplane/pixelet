// pxl-spec-core/cave.rs

//! gpu-portable: a node's seed and the caves, u32 hashes over i32 cells, no
//! `Option<bool>`, and index loops where rust-gpu lowers no array iterator.

use xpute_kit::math::rng::derive_seed_u32;

use crate::{wrap_index, Zoom, LEAF_CELL};

/// A node's seed for one use: `tag` names the use, and the node's zoom and
/// each of its indices are folded in, in order, X named as the world names
/// it (`wrap_index`), so a node seeds alike on whichever lap its asker counts
/// it. Every procedural draw seeds by it, each use under a tag of its own,
/// so no two uses draw one stream.
pub fn node_seed<const D: usize>(tag: u32, (zoom, at): (Zoom, [i32; D])) -> u32 {
    const { assert!(D <= 3, "a node has at most three indices") };
    let mut h = derive_seed_u32(derive_seed_u32(crate::param::seed::MASTER, tag), zoom);
    // [naga] each index by itself, not a loop: Naga's Metal backend breaks a
    // `loop { … continuing { break if … } }` an iteration early, re-reading
    // its condition after the counter moved.
    if D > 0 {
        h = derive_seed_u32(h, wrap_index(zoom, at[0]) as u32);
    }
    if D > 1 {
        h = derive_seed_u32(h, at[1] as u32);
    }
    if D > 2 {
        h = derive_seed_u32(h, at[2] as u32);
    }
    h
}

/// The caves' tag (`node_seed`).
const CAVES: u32 = 0xca7e_0001;

// ---- caves: a sparse tree of hashes ----

/// The coarsest cube a cave takes, 32 units a side: nothing coarser is
/// hollow, so every coarser fold of the caves is solid.
pub const CAVE_COARSEST: Zoom = LEAF_CELL - crate::param::cave::COARSEST_STEPS;

/// A cube's draws, `c` at `z`, from one hash: whether it turns a child over
/// (its top two bits), which (its low three), and which of its eight draw
/// again (its second byte, a bit each). A child that does not draw is its
/// value all through, so the caves are a sparse tree whose nodes are the
/// cubes that draw, every cube at the root's zoom among them.
fn draws(z: Zoom, c: [i32; 3]) -> (bool, u32, u32) {
    let h = node_seed(CAVES, (z, c));
    (h < crate::param::cave::TURN, h & 7, (h >> 8) & 0xff)
}

/// A child's place among its parent's eight.
fn slot(c: [i32; 3]) -> u32 {
    (c[0] & 1 | (c[1] & 1) << 1 | (c[2] & 1) << 2) as u32
}

/// The caves at a cube as its parent hands them down, so a descent asks
/// each cube's draws once rather than again from the caves' first zoom:
/// its value, and whether anything under it differs (`live`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cave {
    hollow: bool,
    live: bool,
}

impl Cave {
    /// Any cube coarser than the caves' first: solid, and every cube of the
    /// caves' first zoom under it draws.
    pub const ROOT: Cave = Cave {
        hollow: false,
        live: true,
    };

    /// As a word a GPU's storage holds, `bool` being none of its: hollow,
    /// then live.
    pub const fn word(self) -> u32 {
        self.hollow as u32 | (self.live as u32) << 1
    }

    pub const fn of_word(w: u32) -> Cave {
        Cave {
            hollow: w & 1 != 0,
            live: w & 2 != 0,
        }
    }

    /// The child `c` at `zoom` of a cube whose caves are `self`, by its
    /// place among the eight (`slot`): its parent's draws, one hash, turn one
    /// of them or none, and say which draw again. Past the leaves a cube is
    /// its leaf's.
    pub fn child(self, zoom: Zoom, c: [i32; 3]) -> Cave {
        if zoom < CAVE_COARSEST {
            return Cave::ROOT;
        }
        if !self.live || zoom > LEAF_CELL {
            return Cave {
                live: false,
                ..self
            };
        }
        let (turns, turn, live) = draws(zoom - 1, [c[0] >> 1, c[1] >> 1, c[2] >> 1]);
        let k = slot(c);
        Cave {
            hollow: self.hollow ^ (turns && turn == k),
            live: live >> k & 1 == 1,
        }
    }

    /// The caves at the cube `c` at `zoom`, down its line from the caves'
    /// first, for an asker with nothing handed down.
    pub fn of(zoom: Zoom, c: [i32; 3]) -> Cave {
        let mut cave = Cave::ROOT;
        let mut z = CAVE_COARSEST;
        while z <= zoom && z <= LEAF_CELL {
            let s = zoom - z;
            cave = cave.child(z, [c[0] >> s, c[1] >> s, c[2] >> s]);
            if !cave.live {
                break;
            }
            z += 1;
        }
        cave
    }

    /// Whether the cube is a cave's air. A cube is its parent's value unless
    /// its parent turned it, and a parent turns at most one of its eight, so
    /// the majority of any cube's children is the cube: the caves fold to
    /// themselves at every zoom.
    pub fn hollow(self) -> bool {
        self.hollow
    }

    /// What the caves are all through a cube at `zoom`: whether that is known
    /// there — its value where nothing under it differs, a leaf's or finer
    /// always, never coarser than the caves' first — and the value.
    pub fn whole(self, zoom: Zoom) -> (bool, bool) {
        let known = zoom >= CAVE_COARSEST && (!self.live || zoom >= LEAF_CELL);
        (known, known && self.hollow)
    }
}
