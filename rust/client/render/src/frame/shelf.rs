// pxl-render/frame/shelf.rs

//! The marker atlas: one texture that holds every marker's bitmap, so that
//! all markers draw in one instanced call. Each sprite takes a slot, a
//! rectangle on a shelf with its sides rounded up to a size class, so that a
//! freed slot can be given to another sprite of the same class. A full atlas
//! doubles its side, and every live sprite is then painted again from its
//! `Paint`. A slot keeps its index for as long as it is live, but its
//! rectangle moves when the atlas changes size, so a caller keeps the index
//! and reads the rectangle again on each use. The rectangle is in canvas
//! orientation: `v0`, its top, is less than `v1`, its bottom.

/// The slot `add` returns when even the largest atlas has no room for the
/// sprite. Such a sprite is not shown until a slot is released.
pub const NO_SLOT: i32 = -1;

/// What a sprite paints. Its widths are logical pixels, which the painter
/// scales to device pixels by `ATLAS_SCALE` (look/marker.rs).
#[derive(Clone, Debug, PartialEq)]
pub enum Paint {
    /// A pill in color `fill` with `text` on it in color `ink`.
    Pill {
        text: String,
        fill: u32,
        ink: u32,
        logical_w: f64,
    },
    /// A place drawn as Carto lays it out: a dot, or a disc when `icon` is set,
    /// in color `fill`, and under it the place's name in color `ink`.
    Point {
        /// The place's name as laid out, one entry a line. It is empty when
        /// the place shows no name.
        lines: Vec<String>,
        icon: bool,
        /// The index in `SYMBOL_MASKS` of the place's symbol, drawn
        /// on the disc. With none, the disc is drawn bare.
        symbol: Option<u16>,
        fill: u32,
        ink: u32,
        logical_w: f64,
    },
    /// A speech bubble with a tail below it, holding `lines` of text in a
    /// box `box_w` wide.
    Bubble { lines: Vec<String>, box_w: f64 },
}

/// The steps a slot's width and height are rounded up to, in device pixels.
/// A freed slot is given only to a sprite whose rounded size is the same.
/// 16 device pixels are 8 logical pixels at `ATLAS_SCALE`.
const WIDTH_BUCKET: u32 = 16;
const HEIGHT_BUCKET: u32 = 16;

pub const INIT_SIZE: u32 = 1024;

/// The largest side the atlas grows to. An RGBA texture this size takes 16 MiB
/// before its mips.
const SIZE_CAP: u32 = 2048;

/// The transparent margin, in device pixels, round each sprite inside its
/// slot. A linear sample at the sprite's edge reads the margin, not a
/// neighbor.
pub const PAD: u32 = 1;

/// The most slots an atlas can have. A live or free slot holds a rectangle
/// at least `WIDTH_BUCKET` by `HEIGHT_BUCKET`, no two of them overlap, and
/// the atlas is at most `SIZE_CAP` a side. An unused slot is taken again
/// before a new one is made, so the slots never outnumber this bound.
pub const SLOTS_CAP: usize = ((SIZE_CAP / WIDTH_BUCKET) * (SIZE_CAP / HEIGHT_BUCKET)) as usize;

/// A set of slots, one bit a slot.
#[derive(Clone, Copy)]
pub struct SlotSet([u64; SLOTS_CAP / 64]);

impl Default for SlotSet {
    fn default() -> SlotSet {
        SlotSet([0; SLOTS_CAP / 64])
    }
}

impl SlotSet {
    pub fn insert(&mut self, slot: u32) {
        let s = slot as usize;
        xpute_kit::ensure!(s < SLOTS_CAP, ENOSPC, s);
        self.0[s / 64] |= 1 << (s % 64);
    }

    pub fn contains(&self, slot: u32) -> bool {
        let s = slot as usize;
        s < SLOTS_CAP && self.0[s / 64] >> (s % 64) & 1 == 1
    }

    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|&w| w == 0)
    }

    pub fn len(&self) -> u32 {
        self.0.iter().map(|w| w.count_ones()).sum()
    }

    /// The slots in the set, lowest first.
    pub fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        self.0.iter().enumerate().flat_map(|(k, &w)| {
            (0..64)
                .filter(move |b| w >> b & 1 == 1)
                .map(move |b| (k * 64 + b) as u32)
        })
    }
}

/// A slot's states. An unused slot has no rectangle on the atlas. A free slot
/// keeps its rectangle until a sprite of its size class takes it. A live slot
/// holds a sprite.
const UNUSED: u8 = 0;
const FREE: u8 = 1;
const LIVE: u8 = 2;

fn bucket(v: f64, to: u32) -> u32 {
    (v / to as f64).ceil() as u32 * to
}

/// The atlas's shelves. A shelf is a row of slots filled from the left, and
/// every slot on it has the shelf's height. `next_y` is the top of the next
/// shelf to be made.
#[derive(Default)]
struct ShelfTable {
    y: Vec<u32>,
    h: Vec<u32>,
    cursor: Vec<u32>,
    next_y: u32,
}

impl ShelfTable {
    /// The top-left corner of a `w` by `h` rectangle, placed on the first
    /// shelf of height `h` that has room, or on a new shelf below the others.
    /// It is None when the rectangle does not fit on an atlas `size` a side.
    fn fit(&mut self, w: u32, h: u32, size: u32) -> Option<(u32, u32)> {
        let shelf = match (0..self.h.len()).find(|&s| self.h[s] == h && self.cursor[s] + w <= size)
        {
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

/// Where each live sprite would lie on an atlas of another size. It is laid
/// out on shelves of its own, so a layout that fails leaves the atlas as it
/// was.
struct Layout {
    at: Vec<(u32, u32)>,
    shelves: ShelfTable,
}

fn pack(size: u32, ws: &[u32], hs: &[u32]) -> Option<Layout> {
    let mut shelves = ShelfTable::default();
    let at = (0..ws.len())
        .map(|i| shelves.fit(ws[i], hs[i], size))
        .collect::<Option<_>>()?;
    Some(Layout { at, shelves })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flush {
    pub whole: bool,
    pub size: u32,
    pub generation: u32,
}

#[derive(Default)]
pub struct Atlas {
    size: u32,

    shelves: ShelfTable,

    /// Each of these vectors holds one entry a slot. `x`, `y`, `w` and `h`
    /// are the slot's rectangle, its sides rounded up to the size class.
    /// `cw` and `ch` are the size of the drawing inside it, which is the only
    /// part a texture coordinate may cover.
    state: Vec<u8>,
    x: Vec<u32>,
    y: Vec<u32>,
    w: Vec<u32>,
    h: Vec<u32>,
    cw: Vec<u32>,
    ch: Vec<u32>,
    /// What each sprite paints, kept so that the sprite can be painted again
    /// when the atlas changes size.
    paints: Vec<Option<Paint>>,

    generation: u32,
    used_px: u32,

    dirty: SlotSet,
}

impl Atlas {
    pub fn new() -> Atlas {
        Atlas {
            size: INIT_SIZE,
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

    /// The slot's rectangle in device pixels, the margin included, as x, y,
    /// width and height.
    pub fn slot_rect(&self, slot: u32) -> [u32; 4] {
        let s = slot as usize;
        [self.x[s], self.y[s], self.w[s], self.h[s]]
    }

    /// Whether the slot is live and its sprite has been painted. A slot that
    /// was added or marked dirty counts as unpainted until a flush hands it
    /// to the painter.
    pub fn is_painted(&self, slot: u32) -> bool {
        let s = slot as usize;
        self.state.get(s) == Some(&LIVE) && !self.dirty.contains(slot)
    }

    pub fn slot_paint(&self, slot: u32) -> Option<&Paint> {
        (self.state.get(slot as usize) == Some(&LIVE))
            .then(|| self.paints[slot as usize].as_ref())
            .flatten()
    }

    /// The slot given to a sprite `w` by `h` device pixels that `paint` draws,
    /// or `NO_SLOT` when even the largest atlas has no room for it. The next
    /// flush hands the slot to the painter.
    pub fn add(&mut self, w: f64, h: f64, paint: Paint) -> i32 {
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
        self.paints[s] = Some(paint);
        self.state[s] = LIVE;
        self.mark_dirty(slot as u32);
        slot
    }

    pub fn mark_dirty(&mut self, slot: u32) {
        self.dirty.insert(slot);
    }

    /// Puts the slots to paint into `paint`: every live slot when the atlas
    /// has been made again since `generation`, or else the dirty ones. No slot
    /// is dirty afterward.
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
        self.paints.push(None);
        self.state.len() - 1
    }

    fn place(&mut self, w: u32, h: u32) -> i32 {
        let Some((x, y)) = self.shelves.fit(w, h, self.size) else {
            return NO_SLOT;
        };
        let slot = self.unused_slot();
        (self.x[slot], self.y[slot], self.w[slot], self.h[slot]) = (x, y, w, h);
        self.used_px += w * h;
        slot as i32
    }

    fn live_slots(&self) -> (Vec<usize>, Vec<u32>, Vec<u32>) {
        let slots: Vec<usize> = (0..self.state.len())
            .filter(|&s| self.state[s] == LIVE)
            .collect();
        let ws = slots.iter().map(|&s| self.w[s]).collect();
        let hs = slots.iter().map(|&s| self.h[s]).collect();
        (slots, ws, hs)
    }

    /// Halves the atlas's side for as long as the live sprites cover at most
    /// half of the smaller atlas and fit on it. It returns whether the atlas
    /// changed.
    pub fn compact(&mut self) -> bool {
        let mut changed = false;
        while self.size > INIT_SIZE {
            let target = self.size / 2;
            if self.used_px > (target * target) / 2 {
                break;
            }
            let (slots, ws, hs) = self.live_slots();
            let Some(layout) = pack(target, &ws, &hs) else {
                break;
            };
            self.rebuild(target, &slots, layout);
            changed = true;
        }
        changed
    }

    /// Doubles the atlas's side, or returns false when the atlas is already
    /// at `SIZE_CAP`.
    fn grow(&mut self) -> bool {
        if self.size >= SIZE_CAP {
            return false;
        }
        let (slots, ws, hs) = self.live_slots();
        // The live sprites fit on the atlas at its present side, so they fit
        // on one twice as wide.
        let Some(layout) = pack(self.size * 2, &ws, &hs) else {
            return false;
        };
        self.rebuild(self.size * 2, &slots, layout);
        true
    }

    fn rebuild(&mut self, size: u32, slots: &[usize], layout: Layout) {
        self.size = size;
        self.shelves = layout.shelves;
        for st in self.state.iter_mut() {
            if *st == FREE {
                *st = UNUSED;
            }
        }
        self.used_px = 0;
        self.generation += 1;
        for (i, &s) in slots.iter().enumerate() {
            (self.x[s], self.y[s]) = layout.at[i];
            self.used_px += self.w[s] * self.h[s];
        }
        self.dirty = SlotSet::default();
    }
}
