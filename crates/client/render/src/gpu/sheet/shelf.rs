// pxl-render/gpu/sheet/shelf.rs

//! Where each billboard's bitmap lies in the sheet, one texture, so all badges draw in one instanced
//! call. Shelf-packed with bucketed sizes so a freed slot is reusable; growth
//! doubles the sheet and replays each entry's `DrawSpec`. Only dirty rectangles
//! are uploaded: re-uploading the whole 4096 sheet cost a phone ~100 ms per frame.

/// Slot rectangles are in canvas orientation (v0 top < v1 bottom) and move on a
/// repack, so read them on every use; the slot index is what stays.
pub const NO_SLOT: i32 = -1;

/// What an entry paints, in logical pixels before the render scale.
#[derive(Clone, Debug, PartialEq)]
pub enum DrawSpec {
    /// A label alone on a pill in the fill, in the text color: a placed
    /// billboard's words, or the count of the markers one cell holds.
    Pill { label: String, fill: u32, text: u32, logical_w: f64 },
    /// A place as Carto draws it: a dot or its icon in the fill, and under
    /// it the name, if shown, in the text color.
    Point {
        /// Its name as laid out, a line each; none where it is not named.
        lines: Vec<String>,
        icon: bool,
        /// Carto's symbol for it, by its place in `SYMBOLS`; a disc where none.
        symbol: Option<u16>,
        fill: u32,
        text: u32,
        logical_w: f64,
    },
    /// A speech bubble over its tail, its words a line each.
    Bubble { lines: Vec<String>, box_w: f64 },
}

/// Freed slots are matched by size class, not exact fit. 16 device pixels is 8
/// logical at the badge render scale.
const WIDTH_BUCKET: u32 = 16;
const HEIGHT_BUCKET: u32 = 16;

pub const INITIAL_SIZE: u32 = 1024;

/// 2048 px RGBA is 16 MiB plus mips. A screen holds a few dozen badges (one per
/// places cell); past the limit an entry waits unshown (`NO_SLOT`).
const MAX_SIZE: u32 = 2048;

/// Transparent margin so a linear or mipmapped sample cannot reach a neighbor.
pub const PAD: u32 = 1;

/// The most slots a sheet holds: a live or free one holds a rectangle of a
/// size class at least (`WIDTH_BUCKET` × `HEIGHT_BUCKET`), none overlapping,
/// in a sheet `MAX_SIZE` a side at most; an unused one is reused before
/// another is made.
pub const SLOTS_MAX: usize = ((MAX_SIZE / WIDTH_BUCKET) * (MAX_SIZE / HEIGHT_BUCKET)) as usize;

/// A set of a sheet's slots, a bit each.
#[derive(Clone, Copy)]
pub struct SlotSet([u64; SLOTS_MAX / 64]);

impl Default for SlotSet {
    fn default() -> SlotSet {
        SlotSet([0; SLOTS_MAX / 64])
    }
}

impl SlotSet {
    pub fn insert(&mut self, slot: u32) {
        let s = slot as usize;
        xpute_kit::ensure!(s < SLOTS_MAX, ENOSPC, s);
        self.0[s / 64] |= 1 << (s % 64);
    }

    pub fn contains(&self, slot: u32) -> bool {
        let s = slot as usize;
        s < SLOTS_MAX && self.0[s / 64] >> (s % 64) & 1 == 1
    }

    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|&w| w == 0)
    }

    pub fn len(&self) -> u32 {
        self.0.iter().map(|w| w.count_ones()).sum()
    }

    /// Its slots, lowest first.
    pub fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        self.0.iter().enumerate().flat_map(|(k, &w)| (0..64).filter(move |b| w >> b & 1 == 1).map(move |b| (k * 64 + b) as u32))
    }
}

/// UNUSED holds nothing, FREE is reusable by its size class, LIVE is drawn.
const UNUSED: u8 = 0;
const FREE: u8 = 1;
const LIVE: u8 = 2;

fn bucket(v: f64, to: u32) -> u32 {
    (v / to as f64).ceil() as u32 * to
}

/// Every entry in a shelf takes the shelf's height.
#[derive(Default)]
struct Shelves {
    y: Vec<u32>,
    h: Vec<u32>,
    cursor: Vec<u32>,
    next_y: u32,
}

impl Shelves {
    /// The first shelf of height `h` with room, else a new one; none past `size`.
    fn fit(&mut self, w: u32, h: u32, size: u32) -> Option<(u32, u32)> {
        let shelf = match (0..self.h.len()).find(|&s| self.h[s] == h && self.cursor[s] + w <= size) {
            Some(s) => s,
            None => {
                if self.next_y + h > size || w > size {
                    return None;
                }
                self.y.push(self.next_y);
                self.h.push(h);
                self.cursor.push(0);
                self.next_y += h;
                self.h.len() - 1
            }
        };
        let at = (self.cursor[shelf], self.y[shelf]);
        self.cursor[shelf] += w;
        Some(at)
    }
}

/// Every entry's place on a sheet of `size`, planned on scratch shelves so a
/// failed plan changes nothing.
struct Plan {
    at: Vec<(u32, u32)>,
    shelves: Shelves,
}

fn pack(size: u32, ws: &[u32], hs: &[u32]) -> Option<Plan> {
    let mut shelves = Shelves::default();
    let at = (0..ws.len()).map(|i| shelves.fit(ws[i], hs[i], size)).collect::<Option<_>>()?;
    Some(Plan { at, shelves })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flush {
    pub whole: bool,
    pub size: u32,
    pub generation: u32,
}

#[derive(Default)]
pub struct Sheet {
    size: u32,

    shelves: Shelves,

    /// `w`/`h` is the bucketed slot; `cw`/`ch` is what was drawn, the only part a
    /// UV may cover.
    state: Vec<u8>,
    x: Vec<u32>,
    y: Vec<u32>,
    w: Vec<u32>,
    h: Vec<u32>,
    cw: Vec<u32>,
    ch: Vec<u32>,
    /// What drew each entry, kept so a repack can replay it.
    draws: Vec<Option<DrawSpec>>,

    generation: u32,
    used_px: u32,

    dirty: SlotSet,
}

impl Sheet {
    pub fn new() -> Sheet {
        Sheet {
            size: INITIAL_SIZE,
            ..Default::default()
        }
    }

    pub fn u0(&self, slot: u32) -> f64 {
        (self.x[slot as usize] + PAD) as f64 / self.size as f64
    }

    pub fn v0(&self, slot: u32) -> f64 {
        (self.y[slot as usize] + PAD) as f64 / self.size as f64
    }

    pub fn u1(&self, slot: u32) -> f64 {
        (self.x[slot as usize] + PAD + self.cw[slot as usize]) as f64 / self.size as f64
    }

    pub fn v1(&self, slot: u32) -> f64 {
        (self.y[slot as usize] + PAD + self.ch[slot as usize]) as f64 / self.size as f64
    }

    /// A slot's rectangle in device pixels, the padding included: x, y, w, h.
    pub fn slot_rect(&self, slot: u32) -> [u32; 4] {
        let s = slot as usize;
        [self.x[s], self.y[s], self.w[s], self.h[s]]
    }

    /// Whether a live slot holds its drawing, which one added or marked
    /// dirty lacks until a sync paints it.
    pub fn painted(&self, slot: u32) -> bool {
        let s = slot as usize;
        self.state.get(s) == Some(&LIVE) && !self.dirty.contains(slot)
    }

    pub fn slot_draw(&self, slot: u32) -> Option<&DrawSpec> {
        (self.state.get(slot as usize) == Some(&LIVE)).then(|| self.draws[slot as usize].as_ref()).flatten()
    }

    /// The slot drawn into, or NO_SLOT when the sheet is full. `w`/`h` are device pixels.
    pub fn add(&mut self, w: f64, h: f64, draw: DrawSpec) -> i32 {
        let bw = bucket(w + (PAD * 2) as f64, WIDTH_BUCKET);
        let bh = bucket(h + (PAD * 2) as f64, HEIGHT_BUCKET);

        let mut slot = self.take_free(bw, bh);
        if slot < 0 {
            slot = self.place(bw, bh);
        }
        while slot < 0 {
            if !self.grow() {
                return NO_SLOT;
            }
            slot = self.place(bw, bh);
        }

        let s = slot as usize;
        self.cw[s] = w.ceil() as u32;
        self.ch[s] = h.ceil() as u32;
        self.draws[s] = Some(draw);
        self.state[s] = LIVE;
        self.mark_dirty(slot as u32);
        slot
    }

    pub fn mark_dirty(&mut self, slot: u32) {
        self.dirty.insert(slot);
    }

    /// The slots to paint into `paint`: every live one if the sheet was
    /// replaced since `generation`, else the dirty ones, which are then clean.
    pub fn flush(&mut self, generation: u32, paint: &mut SlotSet) -> Flush {
        let whole = generation != self.generation;
        if whole {
            *paint = SlotSet::default();
            for s in 0..self.state.len() as u32 {
                if self.state[s as usize] == LIVE {
                    paint.insert(s);
                }
            }
        } else {
            *paint = self.dirty;
        }
        self.dirty = SlotSet::default();
        Flush {
            whole,
            size: self.size,
            generation: self.generation,
        }
    }

    pub fn release(&mut self, slot: u32) {
        let s = slot as usize;
        if self.state.get(s) != Some(&LIVE) {
            return;
        }
        self.state[s] = FREE;
        self.used_px -= self.w[s] * self.h[s];
    }

    fn take_free(&mut self, w: u32, h: u32) -> i32 {
        for s in 0..self.state.len() {
            if self.state[s] != FREE || self.w[s] != w || self.h[s] != h {
                continue;
            }
            self.used_px += w * h;
            return s as i32;
        }
        NO_SLOT
    }

    fn unused_slot(&mut self) -> usize {
        if let Some(s) = self.state.iter().position(|&st| st == UNUSED) {
            return s;
        }
        self.state.push(UNUSED);
        self.x.push(0);
        self.y.push(0);
        self.w.push(0);
        self.h.push(0);
        self.cw.push(0);
        self.ch.push(0);
        self.draws.push(None);
        self.state.len() - 1
    }

    fn place(&mut self, w: u32, h: u32) -> i32 {
        let Some((x, y)) = self.shelves.fit(w, h, self.size) else { return NO_SLOT };
        let slot = self.unused_slot();
        (self.x[slot], self.y[slot], self.w[slot], self.h[slot]) = (x, y, w, h);
        self.used_px += w * h;
        slot as i32
    }

    fn live_slots(&self) -> (Vec<usize>, Vec<u32>, Vec<u32>) {
        let slots: Vec<usize> = (0..self.state.len()).filter(|&s| self.state[s] == LIVE).collect();
        let ws = slots.iter().map(|&s| self.w[s]).collect();
        let hs = slots.iter().map(|&s| self.h[s]).collect();
        (slots, ws, hs)
    }

    /// Halves the sheet while every live entry still fits. Without it a sheet one
    /// dense view grew stays that large, canvas and texture, for a dozen entries.
    pub fn compact(&mut self) -> bool {
        let mut changed = false;
        while self.size > INITIAL_SIZE {
            let target = self.size / 2;
            if self.used_px > (target * target) / 2 {
                break;
            }
            let (slots, ws, hs) = self.live_slots();
            let Some(plan) = pack(target, &ws, &hs) else { break };
            self.rebuild(target, &slots, plan);
            changed = true;
        }
        changed
    }

    /// Slots keep their indices; only their rectangles move.
    fn grow(&mut self) -> bool {
        if self.size >= MAX_SIZE {
            return false;
        }
        let (slots, ws, hs) = self.live_slots();
        // Everything fit at half the size, so this fits by construction.
        let Some(plan) = pack(self.size * 2, &ws, &hs) else { return false };
        self.rebuild(self.size * 2, &slots, plan);
        true
    }

    fn rebuild(&mut self, size: u32, slots: &[usize], plan: Plan) {
        self.size = size;
        self.shelves = plan.shelves;
        for st in self.state.iter_mut() {
            if *st == FREE {
                *st = UNUSED;
            }
        }
        self.used_px = 0;
        self.generation += 1;
        for (i, &s) in slots.iter().enumerate() {
            (self.x[s], self.y[s]) = plan.at[i];
            self.used_px += self.w[s] * self.h[s];
        }
        self.dirty = SlotSet::default();
    }
}
