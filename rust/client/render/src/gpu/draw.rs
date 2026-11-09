// pxl-render/gpu/draw.rs

//! The draw lists the core fills each turn and the renderer carries out without deciding
//! anything: uploads of heap blocks, rewrites of them, frees, and the draws. A free cancels its
//! block's pending upload and rewrites, because the block may be reused before the list runs.
//! The draws are kept from turn to turn, each at its slot, and each turn hands what it changed
//! of them along with them.

use core::cell::Cell;

use crate::frame::shader::record::{
    reads_shadow_map, PIPELINE_DEBUG, PIPELINE_DECOR, PIPELINE_GHOST_BOX, PIPELINE_GHOST_DECOR,
    PIPELINE_IMPOSTOR, PIPELINE_LOCATION_BEACON, PIPELINE_LOCATION_ZONE, PIPELINE_MARKER,
    PIPELINE_SKINNED,
};
use crate::frame::shader::word::{draw, ghost_box, instance, location, skinned, DRAW_FLOATS};
use crate::gpu::api::in_flight;
use pxl_content::generation::{TALLEST, WIDEST};
use pxl_engine::light;
use pxl_spec::space::coord::q3::{from_real, ONE, Q3};
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world::between;
use xpute_core::global::Global;
use xpute_core::mem;
use xpute_core::mem::heap::{alloc, free};
use xpute_kit::status::errno::Errno;

/// An upload record's words: handle, address, byte length, vertex count and index count.
pub const UPLOAD_WORDS: usize = 5;
/// A draw record's words: handle, two words the pipeline gives a meaning, the slot where the
/// draw's uniform lies, and pipeline.
pub const DRAW_WORDS: usize = 5;

pub fn uniform_of<'a>(uniforms: &'a [f32], d: &[u32; DRAW_WORDS]) -> &'a [f32] {
    &uniforms[d[3] as usize * DRAW_FLOATS..][..DRAW_FLOATS]
}

/// Whether a draw's uniform says it drifts with the wind (`DrawList::draw_instances`).
pub fn drifts(u: &[f32]) -> bool {
    u[draw::DRIFTS] != 0.0
}

/// Takes the wind's drift `drift` into the position in a draw's uniform `u`, if the draw
/// drifts, as instance.wgsl `draw_translation` does: Q3 from Q3 and remainder from remainder,
/// in X and Y. The uniform then says the draw does not drift, so a shader that reads it moves
/// it by no frame's drift.
pub fn drifted(u: &mut [f32], drift: Position) {
    if drifts(u) {
        for k in 0..2 {
            let q = draw::Q + k;
            u[q] = f32::from_bits((u[q].to_bits() as i32).wrapping_sub(drift.q[k]) as u32);
            u[draw::R + k] -= drift.r[k];
        }
        u[draw::DRIFTS] = 0.0;
    }
}

/// Where a draw stands in the world, independent of the eye. Q3 is the world's fixed-point
/// position, 1/256 of a unit a step (pxl-spec `q3`). `q` is the draw's position in Q3, and
/// `r` is the f32 remainder of a position finer than Q3, zero for a position on Q3. A draw's
/// uniform holds the position and the frame holds the eye's Q3 (word.rs `frame::EYE`). A
/// shader takes the one against the other (frame.wgsl `from_eye`), as `from_eye` here does:
/// first the wrapping i32 difference, then `f32(q - eye) / 256 + r`.
///
/// - The split is a floor: `q = floor(v * 256)` (pxl-spec `q3::from_real`) and
///   `r = v - q / 256`, so `r` lies in [0, 1/256) for negative coordinates too. The i64 floor
///   is narrowed to an i32 with `as i32`, which truncates mod 2^32. The position passes
///   through none of the world's clamps (pxl-spec `world::at`, `world::origin`), so it can
///   hold a point 100,000 units under the eye.
/// - X goes round the world, whose round is 2^32 Q3. The wrapping difference takes the short
///   way round, so a record on any lap draws where its copy nearest the eye does.
/// - Y and Z do not go round. Their wrapping difference is the true one while it is under
///   2^31 Q3, and a drawn record lies within the draw distance of the eye (pxl-core
///   domain/control/param.rs `MAX_DIST`, 2^17 units or 2^25 Q3).
/// - On Q3 (`r` zero), the result equals `f32((at - eye) as f64)`. The f64 difference is
///   exact and is narrowed once. `f32(q - eye)` rounds the same exact difference once, and the
///   power-of-two scale after it is exact. A device that rounds an i32 to f32 other than to
///   nearest can differ by an ulp past 2^24 Q3, where WGSL lets it pick either neighbour.
/// - With a remainder, the result is within an ulp of that f64 difference, or within an ulp
///   and 2^-33 units. Below 2^24 Q3, `r` has been rounded to f32 once and is added in a second
///   rounding, which can land on the other side of a tie. Past 2^24 Q3, the difference is
///   rounded before `r` is added. Where the difference is a step or two and `r` cancels it,
///   the result is smaller than `r` and keeps `r`'s own rounding, half an ulp of 1/256, which
///   is 2^-33 units. Only drawing uses this result. Whatever decides anything (selection,
///   collision, visibility, culling, LOD, the shadow's receivers) reads Q3 or f64.
/// - No decision on the GPU reads a remainder. Decor's gate (decor_cull.wgsl) reads decor's
///   anchor, which is on Q3. The light's receivers read each `ReceiverBox` rounded outward
///   from Q3 (`ReceiverBox::from_eye`). The screen tiles a draw marks (tile_mark.wgsl) are
///   where the draw is drawn, because they are placed the same way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position {
    pub q: Q3,
    pub r: [f32; 3],
}

/// Units as a position's Q3, X, Y and Z alike. The i64 floor is truncated to an i32, which
/// wraps; an f64's `as i32` would saturate instead.
fn q3_of(v: V3) -> Q3 {
    v.map(|x| from_real(x) as i32)
}

/// Units as a position's Q3 rounded up, as `q3_of` rounds down.
fn q3_up(v: V3) -> Q3 {
    v.map(|x| from_real(-x).wrapping_neg() as i32)
}

impl Position {
    /// The position of a draw that has none of its own, because each of its instances holds
    /// its own, as a marker set's instances do (word.rs `MARKER_INSTANCE_WORDS`).
    pub const NONE: Position = Position {
        q: [0; 3],
        r: [0.0; 3],
    };

    pub fn of(v: V3) -> Position {
        let q = q3_of(v);
        let wide = v.map(from_real);
        Position {
            q,
            r: core::array::from_fn(|k| (v[k] - wide[k] as f64 / ONE as f64) as f32),
        }
    }

    /// Units from the eye whose Q3 is `eye`, as a shader takes them.
    pub fn from_eye(&self, eye: Q3) -> [f32; 3] {
        core::array::from_fn(|k| {
            self.q[k].wrapping_sub(eye[k]) as f32 * (1.0 / ONE as f32) + self.r[k]
        })
    }

    /// A draw's position, read back from its uniform (`uniform_of`).
    pub fn in_uniform(u: &[f32]) -> Position {
        Position {
            q: [0, 1, 2].map(|k| u[draw::Q + k].to_bits() as i32),
            r: [0, 1, 2].map(|k| u[draw::R + k]),
        }
    }

    /// Its Q3 as a uniform's floats, bit for bit.
    fn q_words(&self) -> [f32; 3] {
        self.q.map(|v| f32::from_bits(v as u32))
    }
}

/// A model vertex's words: position 3, normal 3, UV 2, joints 4 (u32) and weights 4. The
/// indices follow the vertex columns.
pub const MODEL_VERTEX_WORDS: u32 = 16;

/// Every pipeline a draw record may name, in the order a frame's draw stats count them
/// (renderer.rs `FrameStat::draws`). The list draws with these alone.
pub const DRAW_PIPELINES: [u32; 9] = [
    PIPELINE_DECOR,
    PIPELINE_GHOST_BOX,
    PIPELINE_GHOST_DECOR,
    PIPELINE_SKINNED,
    PIPELINE_LOCATION_ZONE,
    PIPELINE_LOCATION_BEACON,
    PIPELINE_MARKER,
    PIPELINE_DEBUG,
    PIPELINE_IMPOSTOR,
];

/// The draws and triangles counted for each pipeline, by its place in `DRAW_PIPELINES`.
pub type DrawStat = [[f64; 2]; DRAW_PIPELINES.len()];

/// `pipeline`'s place in `DRAW_PIPELINES`; a draw naming another is a bug.
fn stats_slot(pipeline: u32) -> usize {
    use xpute_kit::status::bug::OrBug;
    DRAW_PIPELINES
        .iter()
        .position(|&p| p == pipeline)
        .or_bug(Errno::EINVAL)
}
/// A rewrite record's words: handle, address, byte offset and length.
pub const REWRITE_WORDS: usize = 4;

/// The uploads' and the frees' arrays in `.fixed.uploads` (gpu/upload.rs `set_uploads`),
/// each twice: one being written and one the renderer took. Each has room for as many records
/// as there are live uploads, because a turn's uploads become live and its frees were live.
struct ListMemory {
    uploads: [&'static mut [u32]; 2],
    frees: [&'static mut [u32]; 2],
}

static LIST_ARRAYS: Global<ListMemory> = Global::new();

fn list_arrays() -> &'static mut ListMemory {
    LIST_ARRAYS.get(|| xpute_kit::bug!(ENOTRECOVERABLE))
}

/// The bytes the lists' arrays take for `live` live uploads.
pub const fn lists_bytes(live: usize) -> u32 {
    (2 * (UPLOAD_WORDS + 1) * live * 4) as u32
}

/// # Safety
/// `at` must address `lists_bytes(live)` bytes, 4-aligned, that only the lists use for as long
/// as the module runs.
pub unsafe fn set_lists(at: usize, live: usize) {
    // SAFETY: the caller's range, one array after another.
    let arr = |from: usize, words: usize| unsafe {
        core::slice::from_raw_parts_mut(mem::at::<u32>(at + from * 4), words)
    };
    let (u, f) = (UPLOAD_WORDS * live, live);
    LIST_ARRAYS.set(ListMemory {
        uploads: [arr(0, u), arr(u, u)],
        frees: [arr(2 * u, f), arr(2 * u + f, f)],
    });
}

/// A producer's name for its draw in the list, kept across turns. It holds the slot where the
/// draw's uniform lies and the slot's generation, which moves on when the slot is let go. It is
/// None until the draw is first set, and a name whose slot was let go is drawn anew in another
/// slot.
#[derive(Default, Debug)]
pub struct DrawId(Cell<Option<(u32, u32)>>);

/// A draw as the list holds it: its record and its uniform.
pub type Draw = ([u32; DRAW_WORDS], [f32; DRAW_FLOATS]);

/// A draw's uniform: `at`'s Q3 and remainder (`Position`), and each of `words` at the index
/// that its kind's table in word.rs names.
fn uniform(at: Position, words: &[(usize, f32)]) -> [f32; DRAW_FLOATS] {
    let mut u = [0.0; DRAW_FLOATS];
    u[draw::Q..draw::Q + 3].copy_from_slice(&at.q_words());
    u[draw::R..draw::R + 3].copy_from_slice(&at.r);
    for &(k, v) in words {
        u[k] = v;
    }
    u
}

/// The draw that `DrawList::draw_scaled` lists, made here for a reader outside any list. Its
/// record names uniform slot 0, as a reader that binds the uniform alone reads it
/// (pass/character.rs).
pub fn scaled_draw(
    pipeline: u32,
    handle: u32,
    a: u32,
    b: u32,
    at: Position,
    rot: f32,
    scale: f32,
) -> Draw {
    let words = [(skinned::ROT, rot), (skinned::SCALE, scale)];
    ([handle, a, b, 0, pipeline], uniform(at, &words))
}

/// A draw the turn changed: its slot, and the draw as the turn before left it, None where this
/// turn drew it first. What it is now is read from the list (`DrawTable::now`), which gives None
/// where this turn let it go.
#[derive(Clone, Copy, Debug)]
pub struct Change {
    pub slot: u32,
    pub was: Option<Draw>,
}

/// The pipeline of a slot no draw holds.
const FREE: u32 = u32::MAX;

#[derive(Clone, Copy)]
pub struct Slot {
    gen: u32,
    /// Its draw's pipeline, `FREE` where it holds none.
    pipeline: u32,
    /// Its record's place among its pipeline's.
    at: u32,
    /// The turns its draw was last set in and last changed in.
    set: u32,
    changed: u32,
    /// Whether the other copy of the uniforms lacks what this slot holds
    /// (`DrawList::uniforms`).
    behind: bool,
}

/// The draws a turn sets, kept from turn to turn. Each turn sets every draw it draws, by its
/// producer's name (`DrawId`), and a draw the turn did not set is let go when the lists are
/// taken. A draw set as it was is no change. The turn's changes (`Change`) hold each draw that
/// changed with what it was, and they are all the renderer reads to tell what moved.
#[derive(Default)]
pub struct DrawList {
    /// The side of the lists' arrays being written, and how far its uploads and frees are
    /// written.
    side: usize,
    uploads: usize,
    frees: usize,
    rewrites: Vec<u32>,
    next_handle: u32,
    taken_rewrites: Vec<u32>,
    /// Each pipeline's records. Those this turn has set come first, in the order it set them,
    /// which is the order they are drawn in; `set` counts them.
    records: [Vec<[u32; DRAW_WORDS]>; DRAW_PIPELINES.len()],
    set: [usize; DRAW_PIPELINES.len()],
    slots: Vec<Slot>,
    /// The slots let go, which a new draw takes first.
    free_slots: Vec<u32>,
    /// The uniforms by slot, `DRAW_FLOATS` a slot, in two copies. A turn writes into the copy
    /// the host does not hold (gpu/api.rs `in_flight`), because the renderer hands the
    /// uniforms to the host where they lie. The slots written since the other copy was last
    /// written are copied into it when the turn takes it (`behind`).
    uniforms: Vec<f32>,
    other_uniforms: Vec<f32>,
    behind: Vec<u32>,
    /// Whether this turn has chosen the copy its uniforms are written into.
    begun: bool,
    /// The turn's number, its changes so far, and the changes `take` last handed.
    turn: u32,
    changes: Vec<Change>,
    taken_changes: Vec<Change>,
    /// Whether a record moved in its pipeline's order this turn, and whether one did in the
    /// turn `take` last handed.
    reordered: bool,
    taken_reordered: bool,
    /// How many draws drift with the wind.
    drifting: u32,
    /// The box each drawn thing that reads the shadow map stands within (`ReceiverBox`), as
    /// this turn set them and as `take` last handed them.
    receivers: Vec<ReceiverBox>,
    taken_receivers: Vec<ReceiverBox>,
}

/// The box that a drawn thing reading the shadow map stands within, in the world and
/// independent of the eye. Its low corner is rounded down to Q3 and its high corner up, so it
/// holds the box asked for. Every receiver of the map stands by the same rule (pxl-engine
/// `light::RECEIVER_BOXES_AT`, `Light::box_tiles`). Which of the light's tiles a receiver
/// stands on is a decision, so the box stays exact: it is held in Q3, taken against the eye on
/// the CPU in integers, and rounded outward to the f32 the light's passes read (`from_eye`).
/// X goes round as a `Position`'s does. A box is no wider than the draw distance, so the
/// differences between its corners are the true ones.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReceiverBox(pub [Q3; 2]);

impl ReceiverBox {
    /// The box from `lo` to `hi`, in units, rounded outward so it holds them whole.
    pub fn of(lo: V3, hi: V3) -> ReceiverBox {
        ReceiverBox([q3_of(lo), q3_up(hi)])
    }

    /// The box around the ball that a thing standing at `at` stands within. The ball is
    /// centred on the thing's middle, and its radius is as large as decor is tall and wide.
    /// decor_ball.wgsl reads a placed decor's ball the same way.
    pub fn ball(at: V3) -> ReceiverBox {
        let r = f64::from(TALLEST + WIDEST);
        let middle = [at[0], at[1], at[2] + f64::from(TALLEST) * 0.5];
        ReceiverBox::of(middle.map(|v| v - r), middle.map(|v| v + r))
    }

    /// Its corners in units from the eye whose Q3 is `eye`, as the light's passes read them.
    /// The differences are taken in integers, X the short way round. Each is rounded once and
    /// outward, the low corner down and the high corner up, so the box read holds the box
    /// stood.
    pub fn from_eye(&self, eye: Q3) -> [[f32; 3]; 2] {
        let [lo, hi] = self.0;
        let from = between(&eye, &lo);
        let across = between(&lo, &hi);
        let step = |d: i64, up: bool| {
            let f = d as f32;
            let v = match (f as f64).partial_cmp(&(d as f64)) {
                Some(core::cmp::Ordering::Greater) if !up => f.next_down(),
                Some(core::cmp::Ordering::Less) if up => f.next_up(),
                _ => f,
            };
            v * (1.0 / ONE as f32)
        };
        [
            from.map(|d| step(d, false)),
            core::array::from_fn(|k| step(from[k] + across[k], true)),
        ]
    }

    /// Grows the box to hold `b` too. Each axis is measured from the box's own low corner, so
    /// X goes round as the corners do.
    fn hold(&mut self, b: &ReceiverBox) {
        let [base, top] = self.0;
        let [lo, hi]: [Q3; 2] = [0, 1].map(|end| {
            core::array::from_fn(|k| {
                let off = |v: i32| v.wrapping_sub(base[k]);
                let v = if end == 0 {
                    off(b.0[0][k]).min(0)
                } else {
                    off(top[k]).max(off(b.0[1][k]))
                };
                base[k].wrapping_add(v)
            })
        });
        self.0 = [lo, hi];
    }
}

pub struct DrawUpdate<'a> {
    pub uploads: &'a [u32],
    pub rewrites: &'a [u32],
    pub frees: &'a [u32],
    pub draws: DrawTable<'a>,
    /// What the turn changed of the draws (`Change`), and whether it drew any
    /// in another order.
    pub changes: &'a [Change],
    pub reordered: bool,
    /// The box each drawn thing that reads the shadow map stands within, as its draw was set
    /// (`ReceiverBox`).
    pub receivers: &'a [ReceiverBox],
}

/// The draws as a turn left them: each pipeline's records in the order they are drawn, and
/// each record's uniform at its slot (`uniform_of`), handed where they lie.
#[derive(Clone, Copy)]
pub struct DrawTable<'a> {
    records: &'a [Vec<[u32; DRAW_WORDS]>; DRAW_PIPELINES.len()],
    slots: &'a [Slot],
    pub uniforms: &'a [f32],
    turn: u32,
    drifting: u32,
}

impl<'a> DrawTable<'a> {
    pub fn of(&self, pipeline: u32) -> core::slice::Iter<'a, [u32; DRAW_WORDS]> {
        self.records
            .get(pipeline as usize)
            .map_or(&[][..], Vec::as_slice)
            .iter()
    }

    /// Counts `pipeline`'s draws into `stat`, each as many triangles as `triangles` says.
    pub fn count(
        &self,
        pipeline: u32,
        stat: &mut DrawStat,
        triangles: impl Fn(&[u32; DRAW_WORDS]) -> u32,
    ) {
        let c = &mut stat[stats_slot(pipeline)];
        for d in self.of(pipeline) {
            c[0] += 1.0;
            c[1] += f64::from(triangles(d));
        }
    }

    /// The draw at `slot`, or None where the slot holds none.
    pub fn now(&self, slot: u32) -> Option<Draw> {
        let s = self
            .slots
            .get(slot as usize)
            .filter(|s| s.pipeline != FREE)?;
        let d = self.records[s.pipeline as usize][s.at as usize];
        let mut u = [0.0; DRAW_FLOATS];
        u.copy_from_slice(uniform_of(self.uniforms, &d));
        Some((d, u))
    }

    /// Whether the turn changed the draw at `slot` (`DrawUpdate::changes`).
    pub fn changed(&self, slot: u32) -> bool {
        self.slots
            .get(slot as usize)
            .is_some_and(|s| s.changed == self.turn)
    }

    /// Whether any draw drifts with the wind.
    pub fn drifting(&self) -> bool {
        self.drifting > 0
    }
}

impl DrawList {
    pub fn upload(&mut self, at: u32, bytes: u32, vertex_cnt: u32, idx_cnt: u32) -> u32 {
        self.next_handle = self.next_handle.wrapping_add(1).max(1);
        let handle = self.next_handle;
        let arr = &mut list_arrays().uploads[self.side];
        xpute_kit::ensure!(self.uploads + UPLOAD_WORDS <= arr.len(), ENOSPC, arr.len());
        arr[self.uploads..self.uploads + UPLOAD_WORDS]
            .copy_from_slice(&[handle, at, bytes, vertex_cnt, idx_cnt]);
        self.uploads += UPLOAD_WORDS;
        handle
    }

    /// `handle` 0 names no upload.
    pub fn free(&mut self, handle: u32) {
        if handle == 0 {
            return;
        }
        let mut k = 0;
        while k < self.rewrites.len() {
            if self.rewrites[k] == handle {
                self.rewrites.drain(k..k + REWRITE_WORDS);
            } else {
                k += REWRITE_WORDS;
            }
        }
        let (uploads, frees) = (
            &mut list_arrays().uploads[self.side],
            &mut list_arrays().frees[self.side],
        );
        let mut k = 0;
        while k < self.uploads {
            if uploads[k] == handle {
                uploads.copy_within(k + UPLOAD_WORDS..self.uploads, k);
                self.uploads -= UPLOAD_WORDS;
                // An upload the renderer never took needs no free either.
                return;
            }
            k += UPLOAD_WORDS;
        }
        xpute_kit::ensure!(self.frees < frees.len(), ENOSPC, frees.len());
        frees[self.frees] = handle;
        self.frees += 1;
    }

    pub fn rewrite(&mut self, handle: u32, at: u32, bytes: u32) {
        self.rewrite_part(handle, at, 0, bytes);
    }

    /// Asks for one rewrite of the part of an upload from byte `from`, however often it is
    /// asked before the list is taken. The renderer reads the bytes when the list is taken, at
    /// the latest place asked, and as many as the most asked.
    pub fn rewrite_part(&mut self, handle: u32, at: u32, from: u32, bytes: u32) {
        if handle == 0 {
            return;
        }
        if let Some(r) = self
            .rewrites
            .as_chunks_mut::<REWRITE_WORDS>()
            .0
            .iter_mut()
            .find(|r| r[0] == handle && r[2] == from)
        {
            (r[1], r[3]) = (at, r[3].max(bytes));
            return;
        }
        self.rewrites.extend_from_slice(&[handle, at, from, bytes]);
    }

    /// Chooses the copy the turn writes its uniforms into. Where the host holds the current
    /// copy, the turn switches to the other and first copies in the slots it lacks. A hand-off
    /// carries one frame's uniforms, so the host holds at most one copy.
    fn begin(&mut self) {
        if self.begun {
            return;
        }
        self.begun = true;
        if in_flight(&self.uniforms) {
            core::mem::swap(&mut self.uniforms, &mut self.other_uniforms);
            if self.uniforms.len() < self.other_uniforms.len() {
                self.uniforms.resize(self.other_uniforms.len(), 0.0);
            }
            for &slot in &self.behind {
                let at = slot as usize * DRAW_FLOATS;
                self.uniforms[at..at + DRAW_FLOATS]
                    .copy_from_slice(&self.other_uniforms[at..at + DRAW_FLOATS]);
                self.slots[slot as usize].behind = false;
            }
            self.behind.clear();
        }
        xpute_kit::ensure!(!in_flight(&self.uniforms), EBUSY);
    }

    fn write(&mut self, slot: u32, u: &[f32; DRAW_FLOATS]) {
        self.begin();
        let at = slot as usize * DRAW_FLOATS;
        if self.uniforms.len() < at + DRAW_FLOATS {
            self.uniforms.resize(at + DRAW_FLOATS, 0.0);
        }
        self.uniforms[at..at + DRAW_FLOATS].copy_from_slice(u);
        let s = &mut self.slots[slot as usize];
        if !s.behind {
            s.behind = true;
            self.behind.push(slot);
        }
    }

    fn uniform(&self, slot: u32) -> [f32; DRAW_FLOATS] {
        let mut u = [0.0; DRAW_FLOATS];
        let at = slot as usize * DRAW_FLOATS;
        u.copy_from_slice(&self.uniforms[at..at + DRAW_FLOATS]);
        u
    }

    /// Records `slot` among the turn's changes, with `was`, the draw before the turn changed
    /// it.
    fn change(&mut self, slot: u32, was: Option<Draw>) {
        let s = &mut self.slots[slot as usize];
        if s.changed != self.turn {
            s.changed = self.turn;
            self.changes.push(Change { slot, was });
        }
    }

    /// The slot `id` names, or a slot taken for it where it names none live.
    fn slot_of(&mut self, id: &DrawId) -> u32 {
        let live = id.0.get().filter(|&(slot, gen)| {
            self.slots
                .get(slot as usize)
                .is_some_and(|s| s.gen == gen && s.pipeline != FREE)
        });
        if let Some((slot, _)) = live {
            return slot;
        }
        let slot = self.free_slots.pop().unwrap_or_else(|| {
            self.slots.push(Slot {
                gen: 0,
                pipeline: FREE,
                at: 0,
                set: u32::MAX,
                changed: u32::MAX,
                behind: false,
            });
            self.slots.len() as u32 - 1
        });
        id.0.set(Some((slot, self.slots[slot as usize].gen)));
        slot
    }

    /// Takes `slot`'s record out of its pipeline's records and puts the last record in its
    /// place. Both records lie past those the turn has set, so the turn's order is kept.
    fn unlist(&mut self, slot: u32) {
        let s = self.slots[slot as usize];
        let records = &mut self.records[s.pipeline as usize];
        records.swap_remove(s.at as usize);
        if let Some(moved) = records.get(s.at as usize) {
            self.slots[moved[3] as usize].at = s.at;
        }
    }

    /// Sets the draw `id` names to the record words `words` and the uniform `u`, in `pipeline`,
    /// next in that pipeline's order. It is a change where the record or the uniform differs
    /// from what it was, bit for bit.
    fn set(&mut self, id: &DrawId, pipeline: u32, words: [u32; 3], u: [f32; DRAW_FLOATS]) {
        let p = pipeline as usize;
        xpute_kit::ensure!(p < DRAW_PIPELINES.len(), EINVAL, pipeline);
        let slot = self.slot_of(id);
        let s = self.slots[slot as usize];
        // A draw set twice in a turn would be drawn once, as the second set says, so the
        // second set is refused.
        xpute_kit::ensure!(s.set != self.turn, EEXIST, slot);
        self.slots[slot as usize].set = self.turn;
        let record = [words[0], words[1], words[2], slot, pipeline];
        let was = (s.pipeline != FREE).then(|| {
            (
                self.records[s.pipeline as usize][s.at as usize],
                self.uniform(slot),
            )
        });
        let same =
            was.is_some_and(|(r, w)| r == record && w.map(f32::to_bits) == u.map(f32::to_bits));
        if !same {
            self.change(slot, was);
            let was_drifts = was.is_some_and(|(_, w)| drifts(&w));
            self.drifting = self.drifting + u32::from(drifts(&u)) - u32::from(was_drifts);
            self.write(slot, &u);
        }
        if s.pipeline == pipeline {
            self.records[p][s.at as usize] = record;
        } else {
            if s.pipeline != FREE {
                self.unlist(slot);
            }
            self.records[p].push(record);
            let s = &mut self.slots[slot as usize];
            (s.pipeline, s.at) = (pipeline, self.records[p].len() as u32 - 1);
        }
        // The draw goes next in its pipeline's order. The record there has not been set this
        // turn, and it takes the draw's old place.
        let (at, next) = (self.slots[slot as usize].at as usize, self.set[p]);
        if at != next {
            self.records[p].swap(at, next);
            self.slots[self.records[p][at][3] as usize].at = at as u32;
            self.slots[slot as usize].at = next as u32;
            self.reordered = true;
        }
        self.set[p] += 1;
    }

    /// Records where the draw of `pipeline` stands, the box `receiver`. Every draw whose shader
    /// reads the shadow map gives a box and no other draw does (record.rs `reads_shadow_map`),
    /// so whatever reads the map stands somewhere. Past `light::RECEIVER_BOXES` boxes, the box
    /// is taken into the last one.
    fn stand(&mut self, pipeline: u32, receiver: Option<ReceiverBox>) {
        xpute_kit::ensure!(
            reads_shadow_map(pipeline) == receiver.is_some(),
            EINVAL,
            pipeline
        );
        let Some(b) = receiver else { return };
        let full = self.receivers.len() == light::RECEIVER_BOXES;
        match self.receivers.last_mut() {
            Some(last) if full => last.hold(&b),
            _ => self.receivers.push(b),
        }
    }

    /// Sets a draw whose uniform is at `at` with `words` (`uniform`), and records where it
    /// stands.
    fn put(
        &mut self,
        id: &DrawId,
        pipeline: u32,
        record: [u32; 3],
        at: Position,
        words: &[(usize, f32)],
        receiver: Option<ReceiverBox>,
    ) {
        self.stand(pipeline, receiver);
        self.set(id, pipeline, record, uniform(at, words));
    }

    /// A character's draw (`scaled_draw`).
    #[expect(
        clippy::too_many_arguments,
        reason = "each argument is a word of the draw record it writes, as WebGPU's draw takes its counts"
    )]
    pub fn draw_scaled(
        &mut self,
        id: &DrawId,
        pipeline: u32,
        handle: u32,
        a: u32,
        b: u32,
        at: Position,
        rot: f32,
        scale: f32,
        receiver: Option<ReceiverBox>,
    ) {
        let (d, u) = scaled_draw(pipeline, handle, a, b, at, rot, scale);
        self.stand(pipeline, receiver);
        self.set(id, pipeline, [d[0], d[1], d[2]], u);
    }

    /// A draw whose shader reads no word of its uniform past its position.
    #[expect(
        clippy::too_many_arguments,
        reason = "each argument is a word of the draw record it writes, as WebGPU's draw takes its counts"
    )]
    pub fn draw_as(
        &mut self,
        id: &DrawId,
        pipeline: u32,
        handle: u32,
        a: u32,
        b: u32,
        at: Position,
        receiver: Option<ReceiverBox>,
    ) {
        self.put(id, pipeline, [handle, a, b], at, &[], receiver);
    }

    /// Draws the location indicator's beacon at `at`, spun `spin` about Z, with the night's
    /// look where `night` is set (location.slot.wgsl).
    pub fn draw_beacon(
        &mut self,
        id: &DrawId,
        at: Position,
        spin: f32,
        night: bool,
        receiver: ReceiverBox,
    ) {
        let night = if night { 1.0 } else { 0.0 };
        let words = [(location::SPIN, spin), (location::NIGHT, night)];
        self.put(
            id,
            PIPELINE_LOCATION_BEACON,
            [0; 3],
            at,
            &words,
            Some(receiver),
        );
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "each argument is a word of the ghost box's uniform (word.rs `ghost_box`)"
    )]
    pub fn draw_box(
        &mut self,
        id: &DrawId,
        at: Position,
        opacity: f32,
        side: f32,
        rgb: u32,
        layer: u32,
        receiver: Option<ReceiverBox>,
    ) {
        let words = [
            (ghost_box::OPACITY, opacity),
            (ghost_box::SIDE, side),
            (ghost_box::RGB, f32::from_bits(rgb)),
            (ghost_box::LAYER, f32::from_bits(layer)),
        ];
        self.put(id, PIPELINE_GHOST_BOX, [0, 1, 0], at, &words, receiver);
    }

    /// A draw that `drifts` is a cloud's. Its `at` is its place on the cloud field's map,
    /// which the wind carries, and the shader moves it back by how far the wind has carried
    /// the field (word.rs `frame::DRIFT_Q`, instance.wgsl `draw_translation`). Its record
    /// therefore stays the same in the list while the wind blows.
    #[expect(
        clippy::too_many_arguments,
        reason = "each argument is a word of the draw record it writes, as WebGPU's draw takes its counts"
    )]
    pub fn draw_instances(
        &mut self,
        id: &DrawId,
        pipeline: u32,
        handle: u32,
        group: u32,
        cnt: u32,
        columns: [u32; 3],
        at: Position,
        opacity: f32,
        drifts: bool,
        receiver: Option<ReceiverBox>,
    ) {
        let [off, stride, idx] = columns;
        let words = [
            (instance::OFF, f32::from_bits(off)),
            (instance::STRIDE, f32::from_bits(stride)),
            (instance::OPACITY, opacity),
            (instance::IDX, f32::from_bits(idx)),
            (instance::LAYER, f32::from_bits(group)),
            (instance::DRIFTS, if drifts { 1.0 } else { 0.0 }),
        ];
        self.put(id, pipeline, [handle, group, cnt], at, &words, receiver);
    }

    /// Hands the turn's lists to the renderer. The draws the turn did not set are let go, and
    /// the turn's changes are handed with the lists. The next turn's uploads, rewrites, frees
    /// and changes start empty, on the lists' other side.
    pub fn take(&mut self) -> DrawUpdate<'_> {
        for p in 0..DRAW_PIPELINES.len() {
            while self.records[p].len() > self.set[p] {
                let Some(d) = self.records[p].pop() else {
                    break;
                };
                let slot = d[3];
                let u = self.uniform(slot);
                self.change(slot, Some((d, u)));
                self.drifting -= u32::from(drifts(&u));
                let s = &mut self.slots[slot as usize];
                (s.pipeline, s.gen) = (FREE, s.gen.wrapping_add(1));
                self.free_slots.push(slot);
            }
            self.set[p] = 0;
        }
        core::mem::swap(&mut self.changes, &mut self.taken_changes);
        self.changes.clear();
        core::mem::swap(&mut self.receivers, &mut self.taken_receivers);
        self.receivers.clear();
        self.taken_reordered = core::mem::take(&mut self.reordered);
        let turn = self.turn;
        self.turn = self.turn.wrapping_add(1);
        self.begun = false;
        let (side, uploads, frees) = (self.side, self.uploads, self.frees);
        (self.side, self.uploads, self.frees) = (1 - side, 0, 0);
        core::mem::swap(&mut self.taken_rewrites, &mut self.rewrites);
        self.rewrites.clear();
        let l = list_arrays();
        DrawUpdate {
            uploads: &l.uploads[side][..uploads],
            rewrites: &self.taken_rewrites,
            frees: &l.frees[side][..frees],
            draws: DrawTable {
                records: &self.records,
                slots: &self.slots,
                uniforms: &self.uniforms,
                turn,
                drifting: self.drifting,
            },
            changes: &self.taken_changes,
            reordered: self.taken_reordered,
            receivers: &self.taken_receivers,
        }
    }
}

/// A heap block. It grows to the next power of two as its holder needs more, never shrinks,
/// and is let go with its holder.
#[derive(Default)]
pub struct Block {
    at: usize,
    bytes: u32,
}

impl Block {
    /// The block's first `bytes`. Where the block must grow, what it held is lost. None where
    /// the heap refused, and the block is then gone.
    pub fn take(&mut self, bytes: u32) -> Option<&'static mut [u8]> {
        if bytes > self.bytes {
            self.free();
            (self.at, self.bytes) = alloc(bytes.next_power_of_two())?;
        }
        // SAFETY: a heap block this holds alone, at least `bytes` long.
        Some(unsafe { core::slice::from_raw_parts_mut(mem::at::<u8>(self.at), bytes as usize) })
    }

    pub fn free(&mut self) {
        if self.bytes > 0 {
            // SAFETY: the block this holds alone.
            unsafe { free((self.at, self.bytes)) };
            self.bytes = 0;
        }
    }
}

impl Drop for Block {
    fn drop(&mut self) {
        self.free();
    }
}

/// A heap block that the renderer mirrors, written whole each time, with its upload's handle,
/// or 0 for none.
#[derive(Default)]
pub struct MirroredBlock {
    block: Block,
    pub handle: u32,
}

impl MirroredBlock {
    /// False when the heap refused; the block is then gone.
    pub fn write(&mut self, draw: &mut DrawList, data: &[u8]) -> bool {
        let need = (data.len() as u32).max(4);
        if need > self.block.bytes {
            self.free(draw);
        }
        let Some(bytes) = self.block.take(need) else {
            return false;
        };
        bytes[..data.len()].copy_from_slice(data);
        let (at, room) = (self.block.at as u32, self.block.bytes);
        if self.handle == 0 {
            self.handle = draw.upload(at, room, 0, 0);
        } else {
            draw.rewrite(self.handle, at, need);
        }
        true
    }

    pub fn free(&mut self, draw: &mut DrawList) {
        draw.free(self.handle);
        self.block.free();
        self.handle = 0;
    }
}

/// Sets the lists' arrays once for the tests and hands each test a guard that lets it use them
/// alone: the arrays are a `Global`, and tests run on several threads.
#[cfg(test)]
pub(crate) fn test_lists() -> std::sync::MutexGuard<'static, ()> {
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    static SET: std::sync::Once = std::sync::Once::new();
    let guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    SET.call_once(|| {
        const LIVE: usize = 64;
        let block = Box::leak(vec![0u32; lists_bytes(LIVE) as usize / 4].into_boxed_slice());
        // SAFETY: a block of its own for the process's life, 4-aligned.
        unsafe { set_lists(block.as_mut_ptr() as usize, LIVE) };
    });
    guard
}

#[cfg(test)]
#[path = "draw.test.rs"]
mod tests;
