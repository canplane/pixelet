//! What the tests of a walk that writes the observed pyramid share (tests/light_miss.rs,
//! tests/forget.rs): `Writing`'s rule over a table of words, and what the world answers of a
//! node.

#![allow(dead_code, reason = "each test reads its own of it")]

use std::collections::BTreeMap;

use pxl_engine::answer::In;
use pxl_engine::cast::Query;
use pxl_engine::overwrite::Cut;
use pxl_engine::pyramid::{self, BUCKET, CLAIMED, NONE};
use pxl_engine::way::tree;
use pxl_engine::world::{self, Inheritance};
use pxl_engine::{Zoom, FRAC_W};

use crate::real_world::World;

/// How many visits each node had, by its zoom, its index and the zooms it was asked at.
pub type Met = BTreeMap<(Zoom, [i32; 3], [Zoom; 2]), u32>;

/// Why each visit was answered as it was: off a slot, or of the world because no slot may
/// keep it, none holds it, one holds it at other zooms, or this pass keeps it.
#[derive(Default, Clone, Copy)]
pub struct Asks {
    pub visits: u64,
    pub kept: u64,
    pub past_roots: u64,
    pub absent: u64,
    pub other_zooms: u64,
    pub own_pass: u64,
}

impl Asks {
    pub fn of_world(&self) -> u64 {
        self.past_roots + self.absent + self.other_zooms + self.own_pass
    }
}

pub fn is_node(t: &[u32], w: usize, zoom: Zoom, c: [i32; 3]) -> bool {
    let n = w + pyramid::W_NODE as usize;
    t[n] == zoom && t[n + 1] == c[0] as u32 && t[n + 2] == c[1] as u32 && t[n + 3] == c[2] as u32
}

/// `Writing` over a table of words, one walk at a time.
pub struct Keeping<'a> {
    pub w: World<'a>,
    pub cuts: [Cut; 2],
    pub table: &'a mut [u32],
    pub tag: u32,
    pub asks: Asks,
    /// Every node visited at the zoom asked, how many times, where it is wanted.
    pub met: Option<&'a mut Met>,
}

impl Query for Keeping<'_> {
    type At = Inheritance;

    fn root(&mut self, node: [i32; 3]) -> Inheritance {
        Inheritance::of(&self.w, (0, node))
    }

    fn child(&mut self, at: &Inheritance, zoom: Zoom, node: [i32; 3]) -> Inheritance {
        at.child(&self.w, &tree::Read(self.w.pool), (zoom, node))
    }

    fn node(
        &mut self,
        n: &mut Inheritance,
        zoom: Zoom,
        c: [i32; 3],
        zooms: [Zoom; 2],
        _: f32,
    ) -> In<[u32; 2]> {
        self.asks.visits += 1;
        if let Some(met) = &mut self.met {
            *met.entry((zoom, c, zooms)).or_default() += 1;
        }
        let t = &mut *self.table;
        let (mut to, mut key) = (NONE, CLAIMED);
        let mut other_zooms = false;
        let mut own_pass = false;
        let mut unstamped = false;
        let keeps = pyramid::keeps(zoom, c);
        if keeps {
            let first = pyramid::entry_of(zoom, c);
            let mut free = NONE;
            let mut k = 0;
            while k < BUCKET {
                let w = pyramid::entry_at(first + k) as usize;
                let held = t[w];
                if held != CLAIMED && held != 0 && is_node(t, w, zoom, c) {
                    if pyramid::holds(held, zooms) {
                        if pyramid::tag_of(held) == self.tag {
                            // Kept by a walk of this pass: asked of the world, as its words
                            // may be half written, and left to that walk.
                            own_pass = true;
                            to = NONE;
                            free = NONE;
                            break;
                        }
                        let a = w + pyramid::W_INHERITANCE as usize;
                        let handed = [t[a], t[a + 1], t[a + 2], t[a + 3]];
                        let at = w + pyramid::W_ASSET as usize;
                        let asset = [t[at], t[at + 1]];
                        let word = t[w + pyramid::W_KIND as usize];
                        if pyramid::stamp_holds(word, held, asset, handed, zoom, c) {
                            *n = Inheritance::of_words(handed);
                            self.asks.kept += 1;
                            return In {
                                kind: pyramid::kind_of(word),
                                asset,
                            };
                        }
                        // Words of two writes: asked of the world, and left to the walk that
                        // writes.
                        unstamped = true;
                        to = NONE;
                        free = NONE;
                        break;
                    }
                    other_zooms = true;
                    if pyramid::tag_of(held) / pyramid::PASSES < self.tag / pyramid::PASSES {
                        (to, key) = (first + k, held);
                    }
                } else if held == 0 && free == NONE {
                    free = first + k;
                }
                k += 1;
            }
            if to == NONE && free != NONE {
                (to, key) = (free, 0);
            } else if to == NONE && k == BUCKET {
                let v = first + pyramid::victim(zoom, c);
                (to, key) = (v, t[pyramid::entry_at(v) as usize]);
            }
        }
        let which = if !keeps {
            &mut self.asks.past_roots
        } else if unstamped {
            &mut self.asks.other_zooms
        } else if own_pass {
            &mut self.asks.own_pass
        } else if other_zooms {
            &mut self.asks.other_zooms
        } else {
            &mut self.asks.absent
        };
        *which += 1;
        let pool = tree::Read(self.w.pool);
        let mut held = self.w;
        let answer = world::node(
            &mut held,
            &self.w,
            &self.w,
            &pool,
            n,
            (zoom, c),
            zooms,
            self.cuts,
        );
        if to != NONE && key != CLAIMED {
            put_entry(
                t,
                to,
                zoom,
                c,
                pyramid::key(self.tag, zooms),
                &answer,
                n.words(),
            );
        }
        answer
    }
}

pub fn put_entry(
    t: &mut [u32],
    slot: u32,
    zoom: Zoom,
    c: [i32; 3],
    key: u32,
    a: &In<[u32; 2]>,
    words: [u32; 4],
) {
    let w = pyramid::entry_at(slot) as usize;
    t[w + pyramid::W_KEY as usize] = key;
    t[w + pyramid::W_KIND as usize] = pyramid::stamped(a.kind, key, a.asset, words, zoom, c);
    t[w + pyramid::W_ASSET as usize] = a.asset[0];
    t[w + pyramid::W_ASSET as usize + 1] = a.asset[1];
    let h = w + pyramid::W_INHERITANCE as usize;
    t[h..h + 4].copy_from_slice(&words);
    let n = w + pyramid::W_NODE as usize;
    t[n] = zoom;
    t[n + 1] = c[0] as u32;
    t[n + 2] = c[1] as u32;
    t[n + 3] = c[2] as u32;
}

/// What the world answers of a node with nothing handed down, what it hands on included:
/// what a stackless walk asks.
pub fn truth(
    w: World,
    cuts: [Cut; 2],
    zoom: Zoom,
    c: [i32; 3],
    zooms: [Zoom; 2],
) -> (In<[u32; 2]>, [u32; 4]) {
    let pool = tree::Read(w.pool);
    let mut held = w;
    let mut at = Inheritance::of(&w, (zoom, c));
    let a = world::node(&mut held, &w, &w, &pool, &mut at, (zoom, c), zooms, cuts);
    (a, at.words())
}

pub fn q8(units: [i32; 3]) -> [i32; 3] {
    [
        units[0].wrapping_shl(FRAC_W) | 5,
        units[1].wrapping_shl(FRAC_W) | 77,
        units[2].wrapping_shl(FRAC_W) | 13,
    ]
}
