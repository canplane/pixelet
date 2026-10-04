// pxl-render/asset/atlas.rs

//! Every billboard's bitmap in one texture, so all badges draw in one instanced
//! call. Shelf-packed with bucketed sizes so a freed slot is reusable; growth
//! doubles the sheet and replays each entry's `DrawSpec`. Only dirty rectangles
//! are uploaded: re-uploading the whole 4096 sheet cost a phone ~100 ms per frame.

/// Slot rectangles are in canvas orientation (v0 top < v1 bottom) and move on a
/// repack, so read them on every use; the slot index is what stays.
pub const NO_SLOT: i32 = -1;

/// What an entry paints, in logical pixels before the render scale.
#[derive(Clone, Debug, PartialEq)]
pub enum DrawSpec {
    Cluster { label: String, logical_w: f64 },
    Place { name: String, icon: &'static str, color: u32, logical_w: f64 },
    Bubble { lines: Vec<String>, box_w: f64, box_h: f64 },
    Text { text: String, logical_w: f64 },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AtlasStats {
    /// Device pixels; the sheet is square.
    pub size: u32,
    pub entries: u32,
    pub shelves: u32,
    pub used_px: u32,
    pub repacks: u32,
    pub uploads: u32,
}

/// Freed slots are matched by size class, not exact fit. 16 device pixels is 8
/// logical at the badge render scale.
const WIDTH_BUCKET: u32 = 16;
const HEIGHT_BUCKET: u32 = 16;

const INITIAL_SIZE: u32 = 1024;

/// 2048 px RGBA is 16 MiB plus mips. A screen holds a few dozen badges (one per
/// POI cell); past the limit an entry waits unshown (`NO_SLOT`).
const MAX_SIZE: u32 = 2048;

/// Transparent margin so a linear or mipmapped sample cannot reach a neighbor.
pub const PAD: u32 = 1;

/// UNUSED holds nothing, FREE is reusable by its size class, LIVE is drawn.
const UNUSED: u8 = 0;
const FREE: u8 = 1;
const LIVE: u8 = 2;

fn bucket(v: f64, to: u32) -> u32 {
    (v / to as f64).ceil() as u32 * to
}

/// Every entry's place on a sheet of `size`, planned on scratch shelves so a
/// failed plan changes nothing.
struct Plan {
    xs: Vec<u32>,
    ys: Vec<u32>,
    shelf_y: Vec<u32>,
    shelf_h: Vec<u32>,
    shelf_cursor: Vec<u32>,
    next_y: u32,
}

fn pack(size: u32, ws: &[u32], hs: &[u32]) -> Option<Plan> {
    let mut plan = Plan {
        xs: vec![0; ws.len()],
        ys: vec![0; ws.len()],
        shelf_y: Vec::new(),
        shelf_h: Vec::new(),
        shelf_cursor: Vec::new(),
        next_y: 0,
    };
    for i in 0..ws.len() {
        let (w, h) = (ws[i], hs[i]);
        let shelf = match (0..plan.shelf_h.len()).find(|&s| plan.shelf_h[s] == h && plan.shelf_cursor[s] + w <= size) {
            Some(s) => s,
            None => {
                if plan.next_y + h > size || w > size {
                    return None;
                }
                plan.shelf_y.push(plan.next_y);
                plan.shelf_h.push(h);
                plan.shelf_cursor.push(0);
                plan.next_y += h;
                plan.shelf_h.len() - 1
            }
        };
        plan.xs[i] = plan.shelf_cursor[shelf];
        plan.ys[i] = plan.shelf_y[shelf];
        plan.shelf_cursor[shelf] += w;
    }
    Some(plan)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flush {
    pub whole: bool,
    pub size: u32,
    pub generation: u32,
    pub dirty: Vec<u32>,
}

#[derive(Default)]
pub struct BillboardAtlas {
    size: u32,
    max_size: u32,

    /// Every entry in a shelf takes the shelf's height.
    shelf_y: Vec<u32>,
    shelf_h: Vec<u32>,
    shelf_cursor: Vec<u32>,
    next_y: u32,

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
    live: u32,
    used_px: u32,
    repacks: u32,
    uploads: u32,

    dirty: Vec<u32>,
    is_dirty: Vec<bool>,
}

impl BillboardAtlas {
    pub fn new() -> BillboardAtlas {
        BillboardAtlas {
            size: INITIAL_SIZE,
            max_size: max_size(handheld),
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
        self.live += 1;
        self.mark_dirty(slot as u32);
        slot
    }

    pub fn mark_dirty(&mut self, slot: u32) {
        let s = slot as usize;
        if self.is_dirty.len() <= s {
            self.is_dirty.resize(s + 1, false);
        }
        if self.is_dirty[s] {
            return;
        }
        self.is_dirty[s] = true;
        self.dirty.push(slot);
    }

    /// The whole sheet if it was replaced since `generation`, else the dirty slots,
    /// swapped into `dirty` rather than allocated.
    pub fn flush(&mut self, generation: u32, dirty: &mut Vec<u32>) -> Flush {
        let whole = generation != self.generation;
        let dirty = std::mem::take(&mut self.dirty);
        for &slot in &dirty {
            self.is_dirty[slot as usize] = false;
        }
        if !whole {
            self.uploads += dirty.len() as u32;
        }
        Flush {
            whole,
            size: self.size,
            generation: self.generation,
            dirty: if whole { Vec::new() } else { dirty },
        }
    }

    pub fn release(&mut self, slot: u32) {
        let s = slot as usize;
        if self.state.get(s) != Some(&LIVE) {
            return;
        }
        self.state[s] = FREE;
        self.live -= 1;
        self.used_px -= self.w[s] * self.h[s];
    }

    pub fn sheet_generation(&self) -> u32 {
        self.generation
    }

    pub fn stats(&self) -> AtlasStats {
        AtlasStats {
            size: self.size,
            entries: self.live,
            shelves: self.shelf_h.len() as u32,
            used_px: self.used_px,
            repacks: self.repacks,
            uploads: self.uploads,
        }
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
        let shelf = match (0..self.shelf_h.len()).find(|&s| self.shelf_h[s] == h && self.shelf_cursor[s] + w <= self.size) {
            Some(s) => s,
            None => {
                if self.next_y + h > self.size {
                    return NO_SLOT;
                }
                self.shelf_y.push(self.next_y);
                self.shelf_h.push(h);
                self.shelf_cursor.push(0);
                self.next_y += h;
                self.shelf_h.len() - 1
            }
        };
        let slot = self.unused_slot();
        self.x[slot] = self.shelf_cursor[shelf];
        self.y[slot] = self.shelf_y[shelf];
        self.w[slot] = w;
        self.h[slot] = h;
        self.shelf_cursor[shelf] += w;
        self.used_px += w * h;
        slot as i32
    }

    fn live_slots(&self) -> (Vec<usize>, Vec<u32>, Vec<u32>) {
        let slots: Vec<usize> = (0..self.state.len()).filter(|&s| self.state[s] == LIVE).collect();
        let ws = slots.iter().map(|&s| self.w[s]).collect();
        let hs = slots.iter().map(|&s| self.h[s]).collect();
        (slots, ws, hs)
    }

    pub fn live(&self) -> Vec<u32> {
        (0..self.state.len() as u32).filter(|&s| self.state[s as usize] == LIVE).collect()
    }

    /// Halves the sheet while every live entry still fits. Without it a sheet grown
    /// to 4096 stayed there, 128 MB of canvas and texture holding 12 entries.
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
        if self.size >= self.max_size {
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
        self.shelf_y = plan.shelf_y;
        self.shelf_h = plan.shelf_h;
        self.shelf_cursor = plan.shelf_cursor;
        self.next_y = plan.next_y;
        for st in self.state.iter_mut() {
            if *st == FREE {
                *st = UNUSED;
            }
        }
        self.used_px = 0;
        self.repacks += 1;
        self.generation += 1;
        for (i, &s) in slots.iter().enumerate() {
            self.x[s] = plan.xs[i];
            self.y[s] = plan.ys[i];
            self.used_px += self.w[s] * self.h[s];
        }
        for &slot in &self.dirty {
            self.is_dirty[slot as usize] = false;
        }
        self.dirty.clear();
    }
}
