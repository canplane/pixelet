// pxl-shader/decor.rs

//! The decor the ground grows in the eye's windows (pxl-spec-core
//! `scatter`): the slots that grow again listed (`decor_mark_cs`), each
//! candidate grown (`decor_cs`), and the records counted, laid out and
//! placed for their draws (`decor_count_cs`, `decor_args_cs`,
//! `decor_place_cs`).

use pxl_spec_core::decor::{Decor, COUNTS};
use pxl_spec_core::fill::put;
use pxl_spec_core::lattice::GROUND;
use pxl_spec_core::overwrite::Lanes;
use pxl_spec_core::scatter::{self, drawn, Grounds, SQUARE_GROWN, SQUARE_PATCHES};
use pxl_spec_core::window::{self, FIRST_CELL, SLOTS, SQUARE, SQUARE_SHIFT};
use pxl_spec_core::{at, ways, LEAF_CELL};
use spirv_std::glam::{IVec4, UVec3};
use spirv_std::memory::{Scope, Semantics};
use spirv_std::spirv;

use crate::eye::{eye_origin, Eye};
use crate::world::{Unlooked, World};

/// The world as decor's growth asks it on the GPU (pxl-spec-core `scatter`):
/// the cast's, and the catalog's rows as handed.
struct Growing<'a> {
    world: World<'a, Unlooked>,
    rows: &'a [Decor],
}

impl Lanes for Growing<'_> {
    fn heap(&self, slot: u32) -> u32 {
        self.world.heap(slot)
    }
    fn data(&self, slot: u32) -> u32 {
        self.world.data(slot)
    }
}

/// The world's ray down a column and a block, each the cast's world's one
/// query.
impl Grounds for Growing<'_> {
    fn rows(&self) -> &[Decor] {
        self.rows
    }

    /// The world's ray down the column, the cast's own descent
    /// (pxl-spec-core `scatter::footing`).
    fn footing(&mut self, cell: u32, g: [i32; 2]) -> ((i32, f32), bool) {
        scatter::footing(&mut self.world, cell, g)
    }

    fn block(&mut self, zoom: u32, c: [i32; 3]) -> u32 {
        self.world.block(zoom, c)
    }
}

/// Whether decor `j` is shown at the window zoom `cell`: bit `cell −
/// FIRST_CELL` of its `shown` word, at a leaf's cells or coarser.
fn shown_at(shown: &[u32], j: u32, cell: u32) -> bool {
    cell <= LEAF_CELL && at(shown, j) >> (cell - FIRST_CELL) & 1 != 0
}

/// The eye's slots whose decor grows again this frame, a thread a slot,
/// listed with each decor shown at the slot's window zoom (`shown_at`) for
/// `decor_cs` (pxl-spec-core `scatter::GROWS_AT`), the decors the catalog's
/// rows as handed (`rows`), its count the dispatch's. A slot grows again
/// where `marks` asks every one, decor having been off; where its records
/// were not grown for its square (`scatter::grown_for`); beside a square of
/// its window filled this frame in any berth (`fresh`); or where a box meets
/// it, an edit or a preview's cut as it was or is (`scatter::meets`). A
/// listed slot's head is laid anew, its count none.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "decor_mark_cs"))]
pub fn decor_mark_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] marks: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] fresh: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] records: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] grows: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] rows: &[Decor],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] shown: &[u32],
) {
    let s = id.x;
    if s >= scatter::EYE_SLOTS {
        return;
    }
    let held = at(window, s);
    if held.z == 0 {
        return;
    }
    let (cell, q, h) = (window::eye_cell(s / SLOTS), [held.x, held.y], scatter::head_at(s));
    let head = [at(records, h), at(records, h + 1), at(records, h + 2), at(records, h + 3)];
    let mut marked = at(marks, 0) != 0 || !scatter::grown_for(head, q);
    // A square handed at this window's zoom that it wants, whichever
    // window's berth it is filled into.
    let mut k = 0;
    while k < at(marks, 1) {
        let r = window::handed_at(k);
        let other = window::fresh_square([at(fresh, r + window::FRESH_FIRST), at(fresh, r + window::FRESH_FIRST + 1)]);
        if at(fresh, r + window::FRESH_CELL) == cell {
            let wants = at(window, (s / SLOTS) * SLOTS + window::slot(other));
            marked = marked || wants.x == other[0] && wants.y == other[1] && scatter::beside(cell, q, other);
        }
        k += 1;
    }
    k = 0;
    while k < at(marks, 2) {
        let b = scatter::MARKS_AT + 4 * k;
        let edit = [at(marks, b) as i32, at(marks, b + 1) as i32, at(marks, b + 2) as i32, at(marks, b + 3) as i32];
        marked = marked || scatter::meets(cell, q, edit);
        k += 1;
    }
    if !marked {
        return;
    }
    put(records, h, 0);
    put(records, h + 1, q[0] as u32);
    put(records, h + 2, q[1] as u32);
    put(records, h + 3, 1);
    let decors = rows.len() as u32;
    let mut j = 0;
    while j < decors {
        if shown_at(shown, j, cell) {
            // SAFETY: an atomic add on the list's count, the dispatch's z.
            let n = unsafe { spirv_std::arch::atomic_i_add::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(&mut grows[2], 1) };
            put(grows, scatter::GROWS_AT + n, s | j << 16);
        }
        j += 1;
    }
}

/// Decor the ground grows in a slot `decor_mark_cs` listed, one candidate a
/// thread: `x` its index in its patch, `y` the patch among those meeting the
/// slot's square (pxl-spec-core `scatter::patch_of`), `z` the slot and decor
/// listed. Each one that stands, stands on its cell and grows there
/// (`scatter::stands`, `grown`) is added to the slot's records
/// (`scatter::record_at`), naming the slot's window zoom, the one it is
/// drawn at where the rays ask it (`decor_count_cs`); only a decor shown at
/// that zoom is listed (`decor_mark_cs`). What it grows is the windows'
/// squares', the ways over them and what is placed alone, never the eye's.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "decor_cs"))]
pub fn decor_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] out: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] rows: &[Decor],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] grows: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] pool: &[u32],
) {
    let listed = at(grows, scatter::GROWS_AT + id.z);
    let (square, j) = (listed & 0xffff, listed >> 16);
    let cell = window::eye_cell(square / SLOTS);
    if id.x >= COUNTS as u32 || id.y >= SQUARE_PATCHES {
        return;
    }
    let held = at(window, square);
    if held.z == 0 {
        return;
    }
    // The square's leaf cells.
    let s = LEAF_CELL - cell;
    let first = [(held.x << SQUARE_SHIFT) << s, (held.y << SQUARE_SHIFT) << s];
    let region = [first, [first[0] + (SQUARE << s) as i32, first[1] + (SQUARE << s) as i32]];
    let mut world = Growing {
        world: World {
            window,
            ground,
            looks: Unlooked(0),
            shards,
            overwrites,
            eye,
            pool: ways::tree::Read(pool),
        },
        rows,
    };
    let decor = &rows[j as usize];
    let zoom = GROUND.read_zoom(cell);
    let (c, here) = scatter::candidate_of(decor, zoom, region, id.y, id.x);
    if !here || !scatter::stands(decor, zoom, &c) {
        return;
    }
    let (g, here) = scatter::grown(&mut world, decor, cell, &c, true);
    if !here {
        return;
    }
    // SAFETY: an atomic add on the slot's head, its count.
    let n = unsafe { spirv_std::arch::atomic_i_add::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(&mut out[scatter::head_at(square) as usize], 1) };
    let i = scatter::record_at(square * SQUARE_GROWN + n).0 as usize;
    if n < SQUARE_GROWN && i + 4 < out.len() {
        out[i] = scatter::record_word(j, g.variant, g.turn, cell);
        out[i + 1] = g.at[0] as u32;
        out[i + 2] = g.at[1] as u32;
        out[i + 3] = g.at[2] as u32;
        out[i + 4] = g.sink;
    }
}

/// Lanes a decor pass's workgroup has (`decor_count_cs`, `decor_place_cs`).
const RECORD_LANES: u32 = 64;

/// The records workgroup `g` of a decor pass reads, as record numbers
/// (pxl-spec-core `scatter::record_at`): the first and how many. A workgroup
/// a slot, its records to its head's count where the windows still hold the
/// square they were grown for (`scatter::grown_for`), asked once for them
/// all, none where they do not; past the slots, a workgroup a
/// `RECORD_LANES` of placed decor's. Its lanes take them `RECORD_LANES`
/// apart, so a pass reads the records the heads hold, not the room.
fn records(grown: &[u32], window: &[IVec4], g: u32) -> (u32, u32) {
    if g < scatter::EYE_SLOTS {
        let (h, q) = (scatter::head_at(g), at(window, g));
        let head = [at(grown, h), at(grown, h + 1), at(grown, h + 2), at(grown, h + 3)];
        let held = q.z != 0 && scatter::grown_for(head, [q.x, q.y]);
        let count = if head[0] < SQUARE_GROWN { head[0] } else { SQUARE_GROWN };
        (g * SQUARE_GROWN, if held { count } else { 0 })
    } else {
        let first = (g - scatter::EYE_SLOTS) * RECORD_LANES;
        let count = at(grown, scatter::PLACED_HEAD);
        let left = if count > first { count - first } else { 0 };
        (scatter::SLOT_RECORDS + first, if left < RECORD_LANES { left } else { RECORD_LANES })
    }
}

/// Each record for the eye this frame, a workgroup a slot's or a run of
/// placed decor's (`records`), its region written into its first word and
/// counted: nothing where the rays ask another zoom at its
/// distance than the one it was grown at (`scatter::drawn_at`), or where it
/// is the decor left out; else sized for the screen (pxl-spec-core
/// `scatter::drawn`), a model where its group's extent spans `MODEL_PX` at
/// its distance, a sprite down to `SPRITE_PX`, else nothing.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "decor_count_cs"))]
pub fn decor_count_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] params: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] rows: &[Decor],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] grown: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] regions: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] window: &[IVec4],
) {
    let (first, count) = records(grown, window, group.x);
    let mut k = lane.x;
    while k < count {
        count_one(eye, params, rows, grown, regions, scatter::record_at(first + k).0 as usize);
        k += RECORD_LANES;
    }
}

/// `decor_count_cs` for the record at word `i`.
fn count_one(eye: &Eye, params: &[u32], rows: &[Decor], grown: &mut [u32], regions: &mut [u32], i: usize) {
    if i + 3 >= grown.len() {
        return;
    }
    let w = grown[i];
    let (decor, variant, _, cell) = scatter::record_of(w);
    let corner = [grown[i + 1] as i32, grown[i + 2] as i32, grown[i + 3] as i32];
    let (origin, detail) = eye_origin(eye);
    // Its distance once, for the zoom it is drawn at and its size.
    let d2 = scatter::distance2(origin, corner);
    let here = cell == scatter::PLACED_AT || scatter::drawn_at(detail, d2, cell);
    let group = scatter::first_group(rows, decor) + variant;
    let hide = drawn::HIDE as usize;
    let hidden = params[hide + 3] != 0 && corner[0] == params[hide] as i32 && corner[1] == params[hide + 1] as i32 && corner[2] == params[hide + 2] as i32;
    let d = spirv_std::num_traits::Float::sqrt(d2);
    let d = if d > 1e-3 { d } else { 1e-3 };
    let px = f32::from_bits(params[(drawn::EXTENTS + group) as usize]) / (d * f32::from_bits(params[drawn::UPP as usize]));
    let tier = drawn::tier(px, f32::from_bits(params[drawn::MODEL_PX as usize]), f32::from_bits(params[drawn::SPRITE_PX as usize]));
    let region = if hidden || !here || tier == drawn::GONE { drawn::NONE } else { 2 * group + tier };
    grown[i] = w & ((1 << drawn::REGION_SHIFT) - 1) | region << drawn::REGION_SHIFT;
    if region != drawn::NONE {
        // SAFETY: an atomic add on the region's count, a word of the buffer.
        unsafe { spirv_std::arch::atomic_i_add::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(&mut regions[region as usize], 1) };
    }
}

/// Each region laid after the one before (`decor_count_cs`'s counts), its
/// first slot kept for `decor_place_cs` and added to its draw uniform's
/// index, and its indirect args: a model region's indexed by its group's
/// mesh (`meshes`, its index count, none while it loads), a sprite region's
/// a strip of four corners an instance.
#[spirv(compute(threads(1, 1, 1), entry_point_name = "decor_args_cs"))]
pub fn decor_args_cs(
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] meshes: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] regions: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] args: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] uniforms: &mut [u32],
) {
    let (mut first, mut r) = (0, 0);
    while r < drawn::REGIONS {
        let n = regions[r as usize];
        regions[(drawn::REGIONS + r) as usize] = first;
        uniforms[(r * drawn::UNIFORM_WORDS + drawn::UNIFORM_INDEX) as usize] += first;
        let a = (drawn::ARGS_WORDS * r) as usize;
        if r % 2 == 0 {
            args[a] = at(meshes, r / 2);
            args[a + 1] = n;
            args[a + 2] = 0;
            args[a + 3] = 0;
            args[a + 4] = 0;
        } else {
            args[a] = 4;
            args[a + 1] = n;
            args[a + 2] = 0;
            args[a + 3] = 0;
            args[a + 4] = 0;
        }
        first += n;
        r += 1;
    }
}

/// Each record its region drew a slot of, read as `decor_count_cs` reads
/// them (`records`): its box's base middle about the
/// anchor, its heading and scale, in the lanes the decor and impostor
/// shaders read, and the slot its own in the index list.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "decor_place_cs"))]
pub fn decor_place_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] params: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] rows: &[Decor],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] grown: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] regions: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] instances: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] window: &[IVec4],
) {
    let (first, count) = records(grown, window, group.x);
    let mut k = lane.x;
    while k < count {
        place_one(params, rows, grown, regions, instances, scatter::record_at(first + k).0 as usize);
        k += RECORD_LANES;
    }
}

/// `decor_place_cs` for the record at word `i`.
fn place_one(params: &[u32], rows: &[Decor], grown: &[u32], regions: &mut [u32], instances: &mut [u32], i: usize) {
    if i + 4 >= grown.len() {
        return;
    }
    let w = grown[i];
    let region = w >> drawn::REGION_SHIFT;
    if region == drawn::NONE {
        return;
    }
    // SAFETY: an atomic add on the region's next slot, a word of the buffer.
    let slot = unsafe { spirv_std::arch::atomic_i_add::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(&mut regions[(drawn::REGIONS + region) as usize], 1) };
    let (decor, variant, turn, _) = scatter::record_of(w);
    let [across, deep, _] = rows[decor as usize].cells(variant, turn);
    let room = instances.len() as u32 / (drawn::LANES + 1);
    if slot >= room {
        return;
    }
    let anchor = drawn::ANCHOR as usize;
    // X about the anchor the short way round: leaf cells wrap at 2^24.
    let x = (((grown[i + 1] as i32).wrapping_sub(params[anchor] as i32)) << 8 >> 8) as f32 + across as f32 / 2.0;
    let y = (grown[i + 2] as i32).wrapping_sub(params[anchor + 1] as i32) as f32 + deep as f32 / 2.0;
    // Its base as drawn: under its box's corner by its footing's kept top.
    let sink = grown[i + 4] as f32 / (1 << pxl_spec_core::FRAC_W) as f32;
    let z = (grown[i + 3] as i32).wrapping_sub(params[anchor + 2] as i32) as f32 - sink;
    let (room, slot) = (room as usize, slot as usize);
    instances[slot] = x.to_bits();
    instances[room + slot] = y.to_bits();
    instances[2 * room + slot] = z.to_bits();
    instances[3 * room + slot] = ((turn % 4) as f32 * core::f32::consts::FRAC_PI_2).to_bits();
    instances[4 * room + slot] = 1.0f32.to_bits();
    instances[drawn::LANES as usize * room + slot] = slot as u32;
}
