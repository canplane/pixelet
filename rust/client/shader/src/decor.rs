// pxl-shader/decor.rs

//! The decor the ground grows in the eye's windows (pxl-spec-core
//! `scatter`): the entries that grow again listed (`decor_mark_cs`), which
//! candidates stand, a patch at a time (`decor_stand_cs`), each that does
//! grown (`decor_cs`), and the records counted, laid out and
//! placed for their draws (`decor_count_cs`, `decor_args_cs`,
//! `decor_place_cs`).

use pxl_spec_core::answer::{self, In};
use pxl_spec_core::cast::{End, Query};
use pxl_spec_core::decor::{Decor, COUNTS};
use pxl_spec_core::fill::put;
use pxl_spec_core::lattice::GROUND;
use pxl_spec_core::overwrite::Cut;
use pxl_spec_core::scatter::{self, drawn, is_marked, mark, Grounds, BRICK_PATCHES, MARKS};
use pxl_spec_core::support;
use pxl_spec_core::window::{self, ENTRIES, FIRST_CELL};
use pxl_spec_core::world::{self, Handed};
use pxl_spec_core::{at, way, LEAF_CELL};
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

/// The cast's world unplaced, nothing placed and no preview's cuts: the
/// ground's alone, as decor grows on it (pxl-spec-core `world::Handed::unplaced`).
struct Unplaced<'w, 'a>(&'w mut World<'a, Unlooked>);

impl Query for Unplaced<'_, '_> {
    type At = Handed;

    fn root(&mut self, node: [i32; 3]) -> Handed {
        Handed::unplaced((0, node))
    }

    fn child(&mut self, at: &Handed, zoom: u32, node: [i32; 3]) -> Handed {
        at.child(&*self.0, &self.0.pool, (zoom, node))
    }

    fn node(
        &mut self,
        at: &mut Handed,
        zoom: u32,
        c: [i32; 3],
        asked: [u32; 2],
        _t0: f32,
    ) -> In<[u32; 2]> {
        let w = *self.0;
        world::node(
            self.0,
            &w,
            &w,
            &w.pool,
            at,
            (zoom, c),
            asked,
            [Cut::NONE; 2],
        )
    }
}

/// The query's support on the cast's world (pxl-spec-core `support`): a
/// column's footing of the world unplaced (`Unplaced`), its line
/// kept, each asset about it asked off that line; a cell's edit asked of
/// the world as edited down its own line; and what is placed in a box
/// (`world::placed_in`).
impl Grounds for Growing<'_> {
    type Footing = support::Footing<Handed>;

    fn rows(&self) -> &[Decor] {
        self.rows
    }

    fn footing(&mut self, cell: u32, g: [i32; 2]) -> (Self::Footing, (i32, f32), bool) {
        let f = support::footing(&mut Unplaced(&mut self.world), cell, g);
        let (top, met) = (f.top, f.ends == End::Stop);
        (f, top, met)
    }

    fn asset(&mut self, f: &Self::Footing, zoom: u32, c: [i32; 3]) -> u32 {
        support::asset(&mut Unplaced(&mut self.world), &f.line, zoom, c)
    }

    fn is_edited(&mut self, zoom: u32, c: [i32; 3]) -> bool {
        let line = support::line(&mut self.world, zoom, c);
        answer::is_placed(support::asset(&mut self.world, &line, zoom, c))
    }

    fn next_placed(&self, p: &mut pxl_spec_core::world::Placed) -> ([i32; 3], u32, bool) {
        p.next(&self.world)
    }
}

/// Whether decor `j` is shown at the window zoom `cell`: bit `cell −
/// FIRST_CELL` of its `shown` word, at a leaf's cells or coarser.
fn shown_at(shown: &[u32], j: u32, cell: u32) -> bool {
    cell <= LEAF_CELL && at(shown, j) >> (cell - FIRST_CELL) & 1 != 0
}

/// Whether a box of the frame's (pxl-spec-core `scatter::BOXES_AT`) meets
/// the leaf cells `reads` of the window at `cell` (`scatter::meets`): one
/// at every zoom, or at `cell`'s.
fn is_changed(grows: &[u32], cell: u32, reads: [i32; 4]) -> bool {
    let mut k = 0;
    while k < at(grows, scatter::GROWS_BOXES) {
        let b = scatter::BOXES_AT + scatter::BOX_WORDS * k;
        let zoom = at(grows, b + 4);
        let edit = [
            at(grows, b) as i32,
            at(grows, b + 1) as i32,
            at(grows, b + 2) as i32,
            at(grows, b + 3) as i32,
        ];
        if (zoom == scatter::EVERY || zoom == cell) && scatter::meets(reads, edit) {
            return true;
        }
        k += 1;
    }
    false
}

/// The eye's entries whose decor grows again this frame, a thread an entry,
/// listed with each decor shown at the entry's window zoom (`shown_at`) for
/// `decor_cs` (pxl-spec-core `scatter::GROWS_AT`), the decors the catalog's
/// rows as handed (`rows`), its count the dispatch's. An entry grows whole
/// where every one does, decor having been off, or its records were not
/// grown for its brick (`scatter::grown_for`), its head then laid anew and
/// no patch bearing; else, where a box of the frame's meets what it reads
/// (`is_changed`), only its candidates that read what the box holds.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "decor_mark_cs"))]
pub fn decor_mark_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] window: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] records: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] grows: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] rows: &[Decor],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] shown: &[u32],
) {
    let s = id.x;
    if s >= scatter::EYE_ENTRIES {
        return;
    }
    let held = at(window, s);
    if held.z == 0 {
        return;
    }
    let (cell, q, h) = (
        window::eye_cell(s / ENTRIES),
        [held.x, held.y],
        scatter::head_at(s),
    );
    let head = [at(records, h), at(records, h + 1), at(records, h + 2)];
    let whole = at(grows, scatter::GROWS_EVERY) != 0 || !scatter::grown_for(head, q);
    if !whole && !is_changed(grows, cell, scatter::brick_reads(cell, q)) {
        return;
    }
    if whole {
        put(records, h, q[0] as u32);
        put(records, h + 1, q[1] as u32);
        put(records, h + 2, 1);
        let mut k = 0;
        while k < scatter::RUNS {
            put(records, h + scatter::BEARS_AT + k, 0);
            k += 1;
        }
    }
    let decors = rows.len() as u32;
    let mut j = 0;
    while j < decors {
        if shown_at(shown, j, cell) {
            // SAFETY: atomic adds on the list's count, each dispatch's z.
            let n = unsafe {
                spirv_std::arch::atomic_i_add::<
                    u32,
                    { Scope::QueueFamily as u32 },
                    { Semantics::NONE.bits() },
                >(&mut grows[(scatter::STAND_ARGS + 2) as usize], 1)
            };
            // SAFETY: as above.
            unsafe {
                spirv_std::arch::atomic_i_add::<
                    u32,
                    { Scope::QueueFamily as u32 },
                    { Semantics::NONE.bits() },
                >(&mut grows[(scatter::GROW_ARGS + 2) as usize], 1)
            };
            let whole = if whole { scatter::WHOLE } else { 0 };
            put(grows, scatter::GROWS_AT + n, s | j << 16 | whole);
        }
        j += 1;
    }
}

/// Lanes `decor_stand_cs`'s workgroup has: a brick's patches, a lane each.
const PATCH_LANES: u32 = 32;
const _: () = assert!(BRICK_PATCHES <= PATCH_LANES);

/// Which candidates of an entry `decor_mark_cs` listed stand, a patch a
/// thread: `x` the patch among those meeting the entry's brick
/// (pxl-spec-core `scatter::patch_of`), `z` the entry and decor listed. Its
/// stream is drawn once (`scatter::each_in_patch`): where the entry grows
/// whole, every candidate it bears is asked and how many is written
/// (`scatter::bears_at`); else only one that reads what a box of the
/// frame's holds (`scatter::reads`, `is_changed`). Their contest is drawn once
/// for them all (`scatter::beaten`, each `stands`'s answer). Each asked
/// writes its own record (`scatter::entry_record`): `STOOD`, its cell, seed
/// and born zoom, where it lies in the brick and stands, for `decor_cs`
/// to grow; else `UNGROWN`. What it reads is the decor's stream alone,
/// never the world.
#[spirv(compute(threads(32, 1, 1), entry_point_name = "decor_stand_cs"))]
pub fn decor_stand_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] window: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] out: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] rows: &[Decor],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] grows: &[u32],
) {
    let (brick, j, whole) = listed(grows, id.z);
    let (p, cell) = (id.x, window::eye_cell(brick / ENTRIES));
    if p >= BRICK_PATCHES {
        return;
    }
    let held = at(window, brick);
    if held.z == 0 {
        return;
    }
    let [x0, y0, x1, y1] = scatter::brick_box(cell, [held.x, held.y]);
    let decor = &rows[j as usize];
    let zoom = GROUND.read_zoom(cell);
    let (born, b, real) = scatter::patch_of(zoom, [[x0, y0], [x1, y1]], p);
    let mut candidates = [scatter::Candidate::NONE; COUNTS];
    let bears = if real {
        scatter::each_in_patch(decor, born, b, |i, c| candidates[i as usize] = c)
    } else {
        0
    };
    if whole {
        put(out, scatter::bears_at(brick, j, p), bears);
    }
    // Those grown again that lie in the brick; one outside it is none.
    let mut asked = [0u32; MARKS];
    let mut i = 0;
    while i < bears as usize {
        let [x, y] = candidates[i].cell;
        if whole || is_changed(grows, cell, scatter::reads(cell, [x, y])) {
            if x0 <= x && x < x1 && y0 <= y && y < y1 {
                mark(&mut asked, i);
            } else {
                put(
                    out,
                    entry_word(rows, brick, j, p, i as u32),
                    scatter::UNGROWN,
                );
            }
        }
        i += 1;
    }
    let beaten = scatter::beaten(decor, zoom, &candidates, bears, &asked);
    i = 0;
    while i < bears as usize {
        if is_marked(&asked, i) {
            let (r, c) = (entry_word(rows, brick, j, p, i as u32), candidates[i]);
            if is_marked(&beaten, i) {
                put(out, r, scatter::UNGROWN);
            } else {
                put(out, r, scatter::STOOD);
                put(out, r + 1, c.cell[0] as u32);
                put(out, r + 2, c.cell[1] as u32);
                put(out, r + 3, c.seed);
                put(out, r + 4, c.born);
            }
        }
        i += 1;
    }
}

/// The decor an entry's stood candidates grow (`decor_stand_cs`), a thread a
/// record: `x` its candidate's index in its patch, `y` the patch, `z` the
/// entry and decor listed, as `decor_stand_cs` laid them. Each record
/// `STOOD` grows on its cell (`scatter::grown`), naming the entry's window
/// zoom; else it is `UNGROWN`. Only a decor shown at that zoom is listed
/// (`decor_mark_cs`). What it grows on is the windows' bricks and the ways
/// over them alone, what is placed left out (`Unplaced`); what is placed
/// and the eye's cuts only take it away where they hold its box
/// (`Grounds::is_edited`).
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
    let (brick, j, _) = listed(grows, id.z);
    // A patch's records are its decor's room, the next patch's past it.
    if id.x >= rows[j as usize].room() || id.y >= BRICK_PATCHES {
        return;
    }
    let r = entry_word(rows, brick, j, id.y, id.x) as usize;
    if r + 4 >= out.len() || out[r] != scatter::STOOD {
        return;
    }
    let c = scatter::Candidate {
        cell: [out[r + 1] as i32, out[r + 2] as i32],
        seed: out[r + 3],
        born: out[r + 4],
    };
    let cell = window::eye_cell(brick / ENTRIES);
    let mut world = Growing {
        world: World {
            window,
            ground,
            looks: Unlooked(0),
            shards,
            overwrites,
            eye,
            pool: way::tree::Read(pool),
        },
        rows,
    };
    let (g, stood, grew) = scatter::grown(&mut world, &rows[j as usize], cell, &c, true);
    if stood && grew {
        out[r] = scatter::record_word(j, g.variant, g.turn, cell);
        out[r + 1] = g.at[0] as u32;
        out[r + 2] = g.at[1] as u32;
        out[r + 3] = g.at[2] as u32;
        out[r + 4] = g.sink;
    } else {
        out[r] = scatter::UNGROWN;
    }
}

/// The `z`-th entry and decor `decor_mark_cs` listed, and whether the entry
/// grows whole (pxl-spec-core `scatter::GROWS_AT`).
fn listed(grows: &[u32], z: u32) -> (u32, u32, bool) {
    let w = at(grows, scatter::GROWS_AT + z);
    (w & 0xffff, w >> 16 & 0xff, w & scatter::WHOLE != 0)
}

/// The first word of entry `s`'s record of candidate `i` of the `p`-th patch
/// of decor `k` (pxl-spec-core `scatter::entry_record`).
fn entry_word(rows: &[Decor], s: u32, k: u32, p: u32, i: u32) -> u32 {
    scatter::record_at(scatter::entry_record(rows, s, k, p, i))
}

/// Lanes a decor pass's workgroup has (`decor_count_cs`, `decor_place_cs`).
const RECORD_LANES: u32 = 64;

/// Whether workgroup `g` of a decor pass reads records: past the entries,
/// placed decor's always; an entry's where the windows still hold the brick
/// they were grown for (`scatter::grown_for`).
fn is_held(grown: &[u32], window: &[IVec4], g: u32) -> bool {
    if g >= scatter::EYE_ENTRIES {
        return true;
    }
    let (h, q) = (scatter::head_at(g), at(window, g));
    let head = [at(grown, h), at(grown, h + 1), at(grown, h + 2)];
    q.z != 0 && scatter::grown_for(head, [q.x, q.y])
}

/// The `k`-th run of records workgroup `g` of a decor pass reads, as record
/// numbers (pxl-spec-core `scatter::record_at`): the first and how many,
/// where it reads any (`is_held`). A workgroup an entry, each run the records a
/// decor's patch bears (`scatter::bears_at`); past the entries, a workgroup
/// `scatter::RUNS` of placed decor's, a record a run. A pass reads the
/// records the patches bear, not the room.
fn records(grown: &[u32], rows: &[Decor], g: u32, k: u32) -> (u32, u32) {
    if g < scatter::EYE_ENTRIES {
        let (j, p) = (k / BRICK_PATCHES, k % BRICK_PATCHES);
        let (bears, room) = (
            at(grown, scatter::bears_at(g, j, p)),
            rows[j as usize].room(),
        );
        (
            scatter::entry_record(rows, g, j, p, 0),
            if bears < room { bears } else { room },
        )
    } else {
        let n = (g - scatter::EYE_ENTRIES) * scatter::RUNS + k;
        (
            scatter::SLOT_RECORDS + n,
            u32::from(n < at(grown, scatter::PLACED_HEAD)),
        )
    }
}

/// Each record for the eye this frame, a workgroup an entry's or a run of
/// placed decor's (`records`), its region written into its first word and
/// counted: nothing where the rays ask a coarser zoom at its distance than
/// the leaves' cells (`scatter::drawn_at`), grown or placed, or where it
/// is the decor left out; else sized for the screen (pxl-spec-core
/// `scatter::drawn`), a model where its group's extent spans `MODEL_ANGLE`
/// at its distance, a sprite down to `SPRITE_ANGLE`, else nothing.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "decor_count_cs"))]
pub fn decor_count_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] drawing: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] rows: &[Decor],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] grown: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] regions: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] window: &[IVec4],
) {
    if !is_held(grown, window, group.x) {
        return;
    }
    let mut k = lane.x;
    while k < scatter::RUNS {
        let (first, count) = records(grown, rows, group.x, k);
        let mut n = 0;
        while n < count {
            count_one(
                eye,
                drawing,
                rows,
                grown,
                regions,
                scatter::record_at(first + n) as usize,
            );
            n += 1;
        }
        k += RECORD_LANES;
    }
}

/// `decor_count_cs` for the record at word `i`.
fn count_one(
    eye: &Eye,
    drawing: &[u32],
    rows: &[Decor],
    grown: &mut [u32],
    regions: &mut [u32],
    i: usize,
) {
    if i + 3 >= grown.len() || grown[i] == scatter::UNGROWN {
        return;
    }
    let w = grown[i];
    let (decor, variant, _, _) = scatter::record_of(w);
    let corner = [
        grown[i + 1] as i32,
        grown[i + 2] as i32,
        grown[i + 3] as i32,
    ];
    let (origin, detail) = eye_origin(eye);
    // Its distance once, for the zoom it is drawn at and its size.
    let d2 = scatter::distance2(origin, corner);
    // Grown or placed alike, drawn only where the rays ask the leaves' cells
    // or finer at its distance (`shown`'s one zoom).
    let here = scatter::drawn_at(detail, d2);
    let group = scatter::first_group(rows, decor) + variant;
    let hide = drawn::HIDE as usize;
    let hidden = drawing[hide + 3] != 0
        && corner[0] == drawing[hide] as i32
        && corner[1] == drawing[hide + 1] as i32
        && corner[2] == drawing[hide + 2] as i32;
    let d = spirv_std::num_traits::Float::sqrt(d2);
    let d = if d > 1e-3 { d } else { 1e-3 };
    let across = f32::from_bits(drawing[(drawn::EXTENTS + group) as usize]) / d;
    let tier = drawn::tier(
        across,
        f32::from_bits(drawing[drawn::MODEL_ANGLE as usize]),
        f32::from_bits(drawing[drawn::SPRITE_ANGLE as usize]),
    );
    let region = if hidden || !here || tier == drawn::GONE {
        drawn::NONE
    } else {
        2 * group + tier
    };
    grown[i] = w & ((1 << drawn::REGION_SHIFT) - 1) | region << drawn::REGION_SHIFT;
    if region != drawn::NONE {
        // SAFETY: an atomic add on the region's count, a word of the buffer.
        unsafe {
            spirv_std::arch::atomic_i_add::<
                u32,
                { Scope::QueueFamily as u32 },
                { Semantics::NONE.bits() },
            >(&mut regions[region as usize], 1)
        };
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
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] drawing: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] rows: &[Decor],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] grown: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] regions: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] instances: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] window: &[IVec4],
) {
    if !is_held(grown, window, group.x) {
        return;
    }
    let mut k = lane.x;
    while k < scatter::RUNS {
        let (first, count) = records(grown, rows, group.x, k);
        let mut n = 0;
        while n < count {
            place_one(
                drawing,
                rows,
                grown,
                regions,
                instances,
                scatter::record_at(first + n) as usize,
            );
            n += 1;
        }
        k += RECORD_LANES;
    }
}

/// `decor_place_cs` for the record at word `i`.
fn place_one(
    drawing: &[u32],
    rows: &[Decor],
    grown: &[u32],
    regions: &mut [u32],
    instances: &mut [u32],
    i: usize,
) {
    if i + 4 >= grown.len() {
        return;
    }
    let w = grown[i];
    let region = w >> drawn::REGION_SHIFT;
    if region == drawn::NONE {
        return;
    }
    // SAFETY: an atomic add on the region's next slot, a word of the buffer.
    let slot = unsafe {
        spirv_std::arch::atomic_i_add::<
            u32,
            { Scope::QueueFamily as u32 },
            { Semantics::NONE.bits() },
        >(&mut regions[(drawn::REGIONS + region) as usize], 1)
    };
    let (decor, variant, turn, _) = scatter::record_of(w);
    let [across, deep, _] = rows[decor as usize].cells(variant, turn);
    let room = instances.len() as u32 / (drawn::LANES + 1);
    if slot >= room {
        return;
    }
    // [rust-gpu] Each by a call, not a closure over a binding's slice.
    let x = from_anchor(drawing, grown, i, 0) + across as f32 / 2.0;
    let y = from_anchor(drawing, grown, i, 1) + deep as f32 / 2.0;
    // Its base as drawn: under its box's corner by its footing's kept top.
    let sink = grown[i + 4] as f32 / (1 << pxl_spec_core::FRAC_W) as f32;
    let z = from_anchor(drawing, grown, i, 2) - sink;
    let (room, slot) = (room as usize, slot as usize);
    instances[slot] = x.to_bits();
    instances[room + slot] = y.to_bits();
    instances[2 * room + slot] = z.to_bits();
    instances[3 * room + slot] = ((turn % 4) as f32 * core::f32::consts::FRAC_PI_2).to_bits();
    instances[4 * room + slot] = 1.0f32.to_bits();
    instances[drawn::LANES as usize * room + slot] = slot as u32;
}

/// The corner of the record at word `i` on `axis` about the anchor, in
/// unit cells (`place_one`).
fn from_anchor(drawing: &[u32], grown: &[u32], i: usize, axis: usize) -> f32 {
    let anchor = drawn::ANCHOR as usize + axis;
    about(
        (grown[i + 1 + axis] as i32).wrapping_sub(drawing[anchor] as i32),
        axis as u32,
    ) as f32
}

/// A difference of unit cells on `axis`, X the short way round, as unit
/// cells wrap at 2^24 on X: the one statement of the wrap, the placed
/// decor's about the anchor (`place_one`) and the anchor's about the eye
/// (`Instances::stands`) alike.
fn about(d: i32, axis: u32) -> i32 {
    if axis == 0 {
        d << 8 >> 8
    } else {
        d
    }
}

const _: () = assert!(
    pxl_spec_core::FRAC_W == 8,
    "a Q3's unit cells wrap at 2^24 as `about` takes them"
);

/// The decor placed this frame as its draws read it (`decor_place_cs`):
/// its drawing words, its regions and its instances. What a reader beside
/// the draws asks of it, how many and where each stands, decor answers
/// here, so none names its lanes (pxl-shader sun.rs `sun_mark_drawn_cs`).
pub(crate) struct Instances<'a> {
    pub(crate) drawing: &'a [u32],
    pub(crate) regions: &'a [u32],
    pub(crate) instances: &'a [u32],
}

impl Instances<'_> {
    /// The slots the instances hold.
    fn room(&self) -> u32 {
        self.instances.len() as u32 / (drawn::LANES + 1)
    }

    /// How many are placed: the last region's end once placed, the regions
    /// laid in order from the first slot (`decor_args_cs`), no more than the
    /// slots.
    pub(crate) fn count(&self) -> u32 {
        at(self.regions, 2 * drawn::REGIONS - 1).min(self.room())
    }

    /// Where placed slot `k` stands, its base middle as drawn (`place_one`),
    /// units from the eye at Q3 `origin`.
    pub(crate) fn stands(&self, k: u32, origin: [i32; 3]) -> [f32; 3] {
        [
            self.anchored(0, origin[0]) + self.lane(0, k),
            self.anchored(1, origin[1]) + self.lane(1, k),
            self.anchored(2, origin[2]) + self.lane(2, k),
        ]
    }

    /// The anchor on `axis` from a Q3 `o`, in units.
    fn anchored(&self, axis: u32, o: i32) -> f32 {
        let w = pxl_spec_core::FRAC_W;
        let a = at(self.drawing, drawn::ANCHOR + axis) as i32;
        about(a.wrapping_sub(o >> w), axis) as f32 - (o & ((1 << w) - 1)) as f32 / (1 << w) as f32
    }

    /// Lane `l` of slot `k`, f32.
    fn lane(&self, l: u32, k: u32) -> f32 {
        f32::from_bits(at(self.instances, l * self.room() + k))
    }
}
