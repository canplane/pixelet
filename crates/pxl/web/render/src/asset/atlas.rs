// pxl-render/asset/atlas.rs

//! Every billboard's bitmap in one texture.
//!
//! What this replaces gave each badge its own texture. That is one GPU
//! allocation and one material binding per badge, which is one draw call per
//! badge, because two sprites with different textures cannot be batched.
//! Holding them in one image is what makes a single instanced draw possible at
//! all — the atlas is not the optimization, it is the precondition for it.
//!
//! It is worth being clear about what it does *not* buy: the same badges at the
//! same resolution occupy the same pixels whether they live in one image or
//! five hundred. Memory comes down by drawing smaller or keeping fewer, not by
//! packing.
//!
//! **Shelf packing, because the height is nearly fixed.** Badges are a line of
//! text: width varies with the name, height does not. Rows of a bucketed
//! height, filled left to right, put the general rectangle-packing problem out
//! of scope entirely — and bucketing the width as well makes a freed slot
//! reusable by the next badge of roughly that size, which is what keeps a long
//! session from fragmenting the sheet into unusable gaps.
//!
//! **Growth repacks rather than reserving.** Most views show a handful of
//! badges, so this starts small and doubles. Each entry keeps what drew it, so
//! a repack is a redraw.
//!
//! **What the sheet holds is bounded by the screen, not by the world.** An
//! entry is a badge on the screen, and the places are counted in cells no
//! narrower there than the widest badge (pxl-spec bake/poi_page.rs
//! `POI_LATTICE`), one badge a cell, however many places a city has under
//! them; `MAX_SIZE` is sized from that.
//!
//! ---
//!
//! **An entry is drawn from what drew it.** An entry is a rectangle and a
//! DrawSpec — what to draw, as data — and the renderer draws it on the device
//! (gpu/sheet.rs): the whole sheet when the generation moved (growth and
//! compaction replace the sheet), else only the dirty entries, each into a
//! texture its own size and copied into its rectangle. What is written is
//! rectangles, never the sheet: re-uploading the whole sheet whenever a badge
//! was drawn cost 64 MB at 4096 px once for every frame a badge was drawn in,
//! and a phone paid it as a hundred-millisecond task each time.

/// An entry is a slot, named by its index, and a consumer keeps that index for
/// as long as the badge is cached. Its rectangle is read through the `u0`..`v1`
/// accessors, in texture coordinates in the canvas's own orientation: v0 is the
/// top edge and v1 the bottom, so v0 < v1. The mirror must not flip the sheet
/// on upload. Read on every use rather than once: the rectangle moves when the
/// sheet repacks, and the slot index is what stays.
pub const NO_SLOT: i32 = -1;

/// What an entry paints, in logical pixels before the render scale: the shell
/// scales the context by it and draws the shape.
#[derive(Clone, Debug, PartialEq)]
pub enum DrawSpec {
    /// A count of places: the number on the pill, and its anchor dot.
    Cluster { label: String, logical_w: f64 },
    /// One place: its kind's icon on its color in the pill's left cap, and
    /// its name after it.
    Place { name: String, icon: &'static str, color: u32, logical_w: f64 },
    /// The notched speech bubble and its wrapped lines.
    Bubble { lines: Vec<String>, box_w: f64, box_h: f64 },
    /// A line of small text on nothing: the debug gizmos' labels.
    Text { text: String, logical_w: f64 },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AtlasStats {
    /// Sheet edge in device pixels; the sheet is square.
    pub size: u32,
    pub entries: u32,
    /// Rows opened, and how much of their length is spent.
    pub shelves: u32,
    pub used_px: u32,
    pub repacks: u32,
    /// Rectangles sent to the GPU by a flush, since load.
    pub uploads: u32,
}

/// Freed slots are reused by size class rather than by exact fit, so a name one
/// pixel narrower than the last one still lands in the hole it left. Sixteen
/// device pixels is eight logical ones at the render scale badges are drawn at.
const WIDTH_BUCKET: u32 = 16;
const HEIGHT_BUCKET: u32 = 16;

const INITIAL_SIZE: u32 = 1024;

/// Where growth gives up, on every device: 2048 px, RGBA, 16 MiB and a third
/// again for the mip chain on the GPU. What the sheet holds is what is on the
/// screen — the places' badges, one a POI cell (pxl-spec bake/poi_page.rs
/// `POI_LATTICE`), and a bubble or two — and a POI cell is no narrower on
/// the screen than the widest badge, so a screen holds a few dozen of them;
/// a 2048 sheet holds over a hundred of the widest. A sheet that is full is
/// a bounded flow: what does not fit waits unshown for a slot (`NO_SLOT`).
const MAX_SIZE: u32 = 2048;

/// One device pixel of transparent margin, so a linear sample at the edge of
/// one entry cannot reach into its neighbor. Mipmaps make the reach wider than
/// a texel at distance, which is exactly when badges are small.
pub const PAD: u32 = 1;

/// A slot's state. UNUSED never held anything or lost its rectangle to a
/// repack; FREE was released and its rectangle can be taken by an entry of the
/// same size class; LIVE is drawn.
const UNUSED: u8 = 0;
const FREE: u8 = 1;
const LIVE: u8 = 2;

fn bucket(v: f64, to: u32) -> u32 {
    (v / to as f64).ceil() as u32 * to
}

/// Where every entry would go on a sheet of `size`, or None if one does not
/// fit — the same first-fit shelf rule `place` uses, run on scratch shelves so
/// a failed plan changes nothing.
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

/// What a mirror has to upload: the whole sheet when `whole`, else the dirty
/// slots' rectangles, which `flush` hands over in the vector it is given.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flush {
    pub whole: bool,
    pub size: u32,
    pub generation: u32,
}

#[derive(Default)]
pub struct BillboardAtlas {
    size: u32,

    /// The rows: where each starts, its bucketed height, how far along it is
    /// spent. Every entry in a row is allocated at the row's height.
    shelf_y: Vec<u32>,
    shelf_h: Vec<u32>,
    shelf_cursor: Vec<u32>,
    next_y: u32,

    /// The entries, one slot each. `w`/`h` is the slot, bucketed — what the
    /// shelf spends and what a FREE one is matched on. `cw`/`ch` is what was
    /// actually drawn, which is smaller: bucketing rounds the slot up so a
    /// freed one is reusable, and the difference is transparent padding that
    /// no UV may include.
    state: Vec<u8>,
    x: Vec<u32>,
    y: Vec<u32>,
    w: Vec<u32>,
    h: Vec<u32>,
    cw: Vec<u32>,
    ch: Vec<u32>,
    /// What drew each entry, kept so a repack can replay it.
    draws: Vec<Option<DrawSpec>>,

    /// Counts the sheet's replacements, for the renderer's mirror of it.
    generation: u32,
    live: u32,
    used_px: u32,
    repacks: u32,
    uploads: u32,

    /// Slots drawn since the last flush, each listed once however many times
    /// it was drawn.
    dirty: Vec<u32>,
    is_dirty: Vec<bool>,
}

impl BillboardAtlas {
    /// An empty sheet.
    pub fn new() -> BillboardAtlas {
        BillboardAtlas {
            size: INITIAL_SIZE,
            ..Default::default()
        }
    }

    // ---- the rectangle of a slot ----

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

    /// What a slot paints, while it is live.
    pub fn slot_draw(&self, slot: u32) -> Option<&DrawSpec> {
        (self.state.get(slot as usize) == Some(&LIVE)).then(|| self.draws[slot as usize].as_ref()).flatten()
    }

    /// Draws into the sheet and returns the slot it landed in, or NO_SLOT when
    /// the sheet is at its limit and nothing that size is free. `w`/`h` are
    /// device pixels — the caller's logical size times whatever supersampling
    /// it draws at.
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

    /// Draws `slot` again at the next flush: what it is drawn from was not
    /// there when it was last taken.
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

    /// What a mirror at `generation` has to take since it last looked: the
    /// whole sheet when it was replaced, else the dirty entries' rectangles,
    /// left in `dirty`. The atlas keeps the vector it is handed in their
    /// place, so the two trade buffers rather than one being made a flush.
    pub fn flush(&mut self, generation: u32, dirty: &mut Vec<u32>) -> Flush {
        let whole = generation != self.generation;
        dirty.clear();
        std::mem::swap(&mut self.dirty, dirty);
        for &slot in dirty.iter() {
            self.is_dirty[slot as usize] = false;
        }
        if whole {
            dirty.clear();
        } else {
            self.uploads += dirty.len() as u32;
        }
        Flush {
            whole,
            size: self.size,
            generation: self.generation,
        }
    }

    /// Returns a slot's space to the pool. The pixels are left alone: the only
    /// thing that made them visible was a UV pointing at them.
    pub fn release(&mut self, slot: u32) {
        let s = slot as usize;
        if self.state.get(s) != Some(&LIVE) {
            return;
        }
        self.state[s] = FREE;
        self.live -= 1;
        self.used_px -= self.w[s] * self.h[s];
    }

    /// Changes when the sheet is replaced, which moves every rectangle.
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

    /// A released slot of this size class, taken back.
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

    /// A slot index nobody holds, reused before a new one is opened.
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

    /// The live slots, in index order, and their bucketed sizes — what a repack
    /// lays out.
    fn live_slots(&self) -> (Vec<usize>, Vec<u32>, Vec<u32>) {
        let slots: Vec<usize> = (0..self.state.len()).filter(|&s| self.state[s] == LIVE).collect();
        let ws = slots.iter().map(|&s| self.w[s]).collect();
        let hs = slots.iter().map(|&s| self.h[s]).collect();
        (slots, ws, hs)
    }

    /// The live slots, for a mirror repainting the whole sheet.
    pub fn live(&self) -> Vec<u32> {
        (0..self.state.len() as u32).filter(|&s| self.state[s as usize] == LIVE).collect()
    }

    /// Halves the sheet for as long as every live entry still fits in half.
    ///
    /// Growth only ever went one way, so a sheet that doubled to 4096 crossing
    /// the densest part of the map stayed at 4096 after the badges that needed
    /// it were gone — the profiler read 12 entries on a 4096 sheet, which is a
    /// 64 MB canvas and another 64 MB of texture holding almost nothing.
    ///
    /// Planned before anything moves: the entries are laid out on the smaller
    /// sheet first, and only if every one of them lands is the sheet rebuilt. A
    /// cheap area test goes first. Called when badges are evicted, which is the
    /// only time the live set shrinks.
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

    /// Doubles the sheet and replays every live entry onto it. The slots keep
    /// their indices; only their rectangles move, which is why a consumer reads
    /// the rectangle on every use.
    fn grow(&mut self) -> bool {
        if self.size >= MAX_SIZE {
            return false;
        }
        let (slots, ws, hs) = self.live_slots();
        // The sheet doubles and held all of these at half the size, so there
        // is room by construction; None here would mean the packer disagrees
        // with itself.
        let Some(plan) = pack(self.size * 2, &ws, &hs) else { return false };
        self.rebuild(self.size * 2, &slots, plan);
        true
    }

    /// Replaces the sheet with one of `size` and moves the live slots to the
    /// places `plan` gave them. The one path both growth and compaction take;
    /// the generation tells a mirror to take the new sheet whole.
    fn rebuild(&mut self, size: u32, slots: &[usize], plan: Plan) {
        self.size = size;
        self.shelf_y = plan.shelf_y;
        self.shelf_h = plan.shelf_h;
        self.shelf_cursor = plan.shelf_cursor;
        self.next_y = plan.next_y;
        // Freed slots belonged to the old layout; the plan packed only live
        // entries, so nothing is free on the new sheet but unshelved space.
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
