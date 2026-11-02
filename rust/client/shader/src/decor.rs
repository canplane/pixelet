// pxl-shader/decor.rs

//! The decor the ground generates in the eye's clip levels (pxl-engine
//! `scatter`, `generation`): the entries that generate again listed
//! (`decor_mark_cs`), the candidates the cells under a brick hold that stand at
//! its clip level and stand clear, a clip level cell at a time
//! (`decor_thin_cs`), each generated (`decor_cs`), and the records counted,
//! laid out and compacted into their draws' instances (`decor_cnt_cs`,
//! `decor_args_cs`, `decor_compact_cs`).

use pxl_engine::answer::In;
use pxl_engine::cast::{End, Query};
use pxl_engine::decor::Decor;
use pxl_engine::fill::put;
use pxl_engine::generation::{self, drawn, DecorQuery};
use pxl_engine::overwrite::Cut;
use pxl_engine::scatter;
use pxl_engine::support::{self, Ancestors};
use pxl_engine::clipmap::{self, BRICK, BRICK_CELLS, ENTRIES, FIRST_CELL};
use pxl_engine::world::{self, Inheritance};
use pxl_engine::{at, way, LEAF_CELL};
use pxl_shader_macro::compute;
use spirv_std::glam::{IVec4, UVec3};
use spirv_std::memory::{Scope, Semantics};
use spirv_std::spirv;

use crate::eye::{eye_origin, Eye};
use crate::workgroup::{DECOR_CELL_LANES, DECOR_RECORD_LANES};
use crate::world::{Clipmap, NoPalette, World};

/// The world as decor's generation asks it on the GPU (pxl-engine `scatter`):
/// the cast's, and the catalog's rows as handed.
struct Generating<'a> {
    world: World<'a, NoPalette>,
    rows: &'a [Decor],
}

/// The cast's world as decor's generation walks it: unplaced, nothing placed
/// and no preview's cuts, the ground's alone, as decor generates on it
/// (pxl-engine `world::Inheritance::UNPLACED`); or, asked from a root
/// as edited (`AssetLine`), with the eye's cuts.
struct Ground<'w, 'a> {
    world: &'w mut World<'a, NoPalette>,
    cuts: [Cut; 2],
}

impl Query for Ground<'_, '_> {
    type At = Inheritance;

    fn root(&mut self, _: [i32; 3]) -> Inheritance {
        Inheritance::UNPLACED
    }

    fn child(&mut self, at: &Inheritance, zoom: u32, node: [i32; 3]) -> Inheritance {
        at.child(&*self.world, &self.world.pool, (zoom, node))
    }

    fn node(
        &mut self,
        at: &mut Inheritance,
        zoom: u32,
        c: [i32; 3],
        zooms: [u32; 2],
        _t0: f32,
    ) -> In<[u32; 2]> {
        let w = *self.world;
        world::node(
            self.world,
            &w,
            &w,
            &w.pool,
            at,
            (zoom, c),
            zooms,
            self.cuts,
        )
    }
}

/// The line a cell is asked off (`DecorQuery::asset`): the column surface's,
/// or, as edited, the cell's own from its root, what is placed found there
/// (`world::Inheritance::of`), the line of its root alone.
struct AssetLine<'l> {
    line: &'l support::Line<Inheritance>,
    edited: bool,
    root: Inheritance,
    root_at: [i32; 3],
}

impl Ancestors<Inheritance> for AssetLine<'_> {
    fn at(&self) -> [i32; 3] {
        if self.edited {
            self.root_at
        } else {
            self.line.at()
        }
    }

    fn zoom(&self) -> u32 {
        if self.edited {
            0
        } else {
            self.line.zoom()
        }
    }

    fn ancestor(&self, z: u32) -> Inheritance {
        if self.edited {
            self.root
        } else {
            self.line.ancestor(z)
        }
    }
}

/// The query's support on the cast's world (pxl-engine `support`): a
/// column's surface of the world unplaced (`Ground`), its line
/// kept, each asset about it asked off that line; a cell's edit asked of
/// the world as edited down its own line, the one ask; and what is placed in
/// a box (`world::placed_in`).
impl DecorQuery for Generating<'_> {
    type ColumnSurface = support::ColumnSurface<Inheritance>;

    fn rows(&self) -> &[Decor] {
        self.rows
    }

    fn column_surface(&mut self, cell: u32, g: [i32; 2]) -> (Self::ColumnSurface, (i32, f32), bool) {
        let mut ground = Ground {
            world: &mut self.world,
            cuts: [Cut::NONE; 2],
        };
        let f = support::column_surface(&mut ground, cell, g);
        let (top, met) = (f.top, f.ends == End::Hit);
        (f, top, met)
    }

    /// The finest clip level holding the leaf column present (`clipmap::step`).
    fn finest(&mut self, cell: u32, g: [i32; 2]) -> u32 {
        let (f, _) = clipmap::step(&self.world, clipmap::Lookup::NONE, LEAF_CELL, g);
        if f.here() && f.from() > cell {
            f.from()
        } else {
            cell
        }
    }

    fn asset(&mut self, f: &Self::ColumnSurface, zoom: u32, c: [i32; 3], edited: bool) -> u32 {
        let root_at = [c[0] >> zoom, c[1] >> zoom, c[2] >> zoom];
        let (cuts, root) = if edited {
            (self.world.cuts(), Inheritance::of(&self.world, (0, root_at)))
        } else {
            ([Cut::NONE; 2], f.line.ancestor(0))
        };
        let line = AssetLine {
            line: &f.line,
            edited,
            root,
            root_at,
        };
        let mut ground = Ground {
            world: &mut self.world,
            cuts,
        };
        support::asset(&mut ground, &line, zoom, c)
    }

    fn next_placed(&self, p: &mut pxl_engine::world::PlacedCursor) -> ([i32; 3], u32, bool) {
        p.next(&self.world)
    }
}

/// Whether decor `j` is shown at the clip level zoom `cell`: bit `cell −
/// FIRST_CELL` of its `shown` word, at a leaf's cells or coarser.
fn shown_at(shown: &[u32], j: u32, cell: u32) -> bool {
    cell <= LEAF_CELL && at(shown, j) >> (cell - FIRST_CELL) & 1 != 0
}

/// Whether a box of the frame's (pxl-engine `generation::BOXES_AT`)
/// reaches what a candidate in `cells`, leaf cells of the clip level at `cell`,
/// reads (`generation::reaches`): a brick's, or one candidate's own cell.
fn is_changed(list: &[u32], cell: u32, cells: [i32; 4]) -> bool {
    let mut k = 0;
    while k < at(list, generation::LIST_BOXES) {
        let b = generation::BOXES_AT + generation::BOX_WORDS * k;
        let edit = [
            at(list, b) as i32,
            at(list, b + 1) as i32,
            at(list, b + 2) as i32,
            at(list, b + 3) as i32,
        ];
        if generation::reaches(edit, at(list, b + 4), cell, cells) {
            return true;
        }
        k += 1;
    }
    false
}

/// The eye's entries whose decor generates again this frame, a thread an entry,
/// listed with each decor shown at the entry's clip level zoom (`shown_at`) for
/// `decor_cs` (pxl-engine `generation::LIST_AT`), the decors the catalog's
/// rows as handed (`rows`), its count the dispatch's. An entry generates whole
/// where every one does, decor having been off, or its records were not
/// generated for its brick (`generation::generated_for`), its head then laid
/// anew, each decor listed laying its records anew, none laid yet
/// (`generation::cnt_at`), for `decor_thin_cs` and `decor_cs`. Else, where a
/// box of the frame's reaches what its brick reads (`is_changed`), it
/// generates in part: its candidates are its brick's hash's alone, so the
/// records each decor laid stand, listed from the list's end
/// (`generation::part_at`) for `decor_cs` alone.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "decor_mark_cs"))]
pub fn decor_mark_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] clipmap: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] records: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] list: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] rows: &[Decor],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] shown: &[u32],
) {
    let s = id.x;
    if s >= generation::EYE_ENTRIES {
        return;
    }
    let held = at(clipmap, s);
    if held.z == 0 {
        return;
    }
    let (cell, q, h) = (
        clipmap::eye_cell(s / ENTRIES),
        [held.x, held.y],
        generation::head_at(s),
    );
    let head = [at(records, h), at(records, h + 1), at(records, h + 2)];
    let whole = at(list, generation::LIST_EVERY) != 0 || !generation::generated_for(head, q);
    if !whole && !is_changed(list, cell, generation::brick_box(cell, q)) {
        return;
    }
    if whole {
        put(records, h, q[0] as u32);
        put(records, h + 1, q[1] as u32);
        put(records, h + 2, 1);
        let mut k = 0;
        while k < rows.len() as u32 {
            put(records, generation::cnt_at(s, k), 0);
            k += 1;
        }
    }
    let decors = rows.len() as u32;
    let mut j = 0;
    while j < decors {
        if shown_at(shown, j, cell) {
            // The pairs whole counted as `decor_thin_cs`'s z, those in part
            // apart, every one as `decor_cs`'s z.
            let counted = if whole {
                put(records, generation::cnt_at(s, j), 0);
                generation::THIN_ARGS + 2
            } else {
                generation::LIST_PARTS
            };
            // SAFETY: atomic adds on the list's counts.
            let n = unsafe {
                spirv_std::arch::atomic_i_add::<
                    u32,
                    { Scope::QueueFamily as u32 },
                    { Semantics::NONE.bits() },
                >(&mut list[counted as usize], 1)
            };
            // SAFETY: as above.
            unsafe {
                spirv_std::arch::atomic_i_add::<
                    u32,
                    { Scope::QueueFamily as u32 },
                    { Semantics::NONE.bits() },
                >(&mut list[(generation::GENERATE_ARGS + 2) as usize], 1)
            };
            let w = if whole {
                generation::LIST_AT + n
            } else {
                generation::part_at(n)
            };
            put(list, w, s | j << 16);
        }
        j += 1;
    }
}

const _: () = assert!(BRICK_CELLS.is_multiple_of(DECOR_CELL_LANES));

/// The candidates of an entry `decor_mark_cs` listed, a clip level cell of its
/// brick a thread: `x` the cell, `z` the entry and decor listed. Where its
/// column holds any (`scatter::count`), each leaf cell under it is read off
/// the decor's hashes (`scatter::candidate_at`); one holding a candidate
/// whose variant stands at the entry's clip level (`scatter::stands_at`) and
/// which stands clear (`scatter::retained`) is laid as the decor's next record
/// (`generation::cnt_at`), `RETAINED`, its cell and seed after it, for
/// `decor_cs` to generate; none past the decor's room (`Decor::cap`). What
/// it reads is the decor's hash alone, never the world.
#[compute(threads(DECOR_CELL_LANES), entry_point_name = "decor_thin_cs")]
pub fn decor_thin_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] clipmap: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] out: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] rows: &[Decor],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] list: &[u32],
) {
    let (brick, j, _) = list_entry(list, id.z);
    let (i, cell) = (id.x, clipmap::eye_cell(brick / ENTRIES));
    if i >= BRICK_CELLS {
        return;
    }
    let held = at(clipmap, brick);
    if held.z == 0 {
        return;
    }
    let [x0, y0, _, _] = generation::brick_box(cell, [held.x, held.y]);
    let (decor, s) = (&rows[j as usize], LEAF_CELL - cell);
    let corner = [
        x0 + ((i % BRICK) << s) as i32,
        y0 + ((i / BRICK) << s) as i32,
    ];
    // None under a clip level cell its column holds none of.
    if scatter::count(j as usize, cell, [corner[0] >> s, corner[1] >> s]) == 0 {
        return;
    }
    let side = 1u32 << s;
    let mut n = 0;
    while n < side * side {
        let leaf = [corner[0] + (n % side) as i32, corner[1] + (n / side) as i32];
        let (c, here) = scatter::candidate_at(decor, leaf);
        if here
            && scatter::stands_at(decor, scatter::variant(decor, &c), cell)
            && scatter::retained(decor, &c)
        {
            // SAFETY: an atomic add on the decor's count, a word of its head.
            let k = unsafe {
                spirv_std::arch::atomic_i_add::<
                    u32,
                    { Scope::QueueFamily as u32 },
                    { Semantics::NONE.bits() },
                >(&mut out[generation::cnt_at(brick, j) as usize], 1)
            };
            if k < decor.cap() {
                let r = entry_word(rows, brick, j, k);
                put(out, r, generation::RETAINED);
                put(out, r + 1, c.cell[0] as u32);
                put(out, r + 2, c.cell[1] as u32);
                put(out, r + 3, c.seed);
            }
        }
        n += 1;
    }
}

/// The decor an entry's retained candidates generate (`decor_thin_cs`), a
/// thread a record: `x` its place among the decor's records, `z` the entry and
/// decor listed, as `decor_thin_cs` laid them. Each record `RETAINED` is
/// generated on its cell (`generation::placement`), naming the entry's clip
/// level zoom; else it is `UNGENERATED`. Of a pair listed in part, each record
/// a box of the frame's reaches (`is_changed`) is generated again on the
/// candidate its cell holds (`scatter::candidate_at`), the cell it keeps
/// generated or not; the rest stand. Only a decor shown at that zoom is listed
/// (`decor_mark_cs`). What it generates on is the clip levels' bricks and the
/// ways over them alone, what is placed left out (`Ground`); what is placed
/// and the eye's cuts only take it away where they hold its box
/// (`DecorQuery::asset`, edited).
#[spirv(compute(threads(64, 1, 1), entry_point_name = "decor_cs"))]
pub fn decor_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] clipmap: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] out: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] rows: &[Decor],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] list: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] pool: &[u32],
) {
    let (brick, j, part) = list_entry(list, id.z);
    // As many as the decor laid, no more than its room.
    let (cnt, cap) = (at(out, generation::cnt_at(brick, j)), rows[j as usize].cap());
    if id.x >= cnt || id.x >= cap {
        return;
    }
    let r = entry_word(rows, brick, j, id.x) as usize;
    if r + 4 >= out.len() {
        return;
    }
    let cell = clipmap::eye_cell(brick / ENTRIES);
    let leaf = [out[r + 1] as i32, out[r + 2] as i32];
    let seed = if part {
        if !is_changed(list, cell, [leaf[0], leaf[1], leaf[0] + 1, leaf[1] + 1]) {
            return;
        }
        scatter::candidate_at(&rows[j as usize], leaf).0.seed
    } else if out[r] == generation::RETAINED {
        out[r + 3]
    } else {
        return;
    };
    let c = scatter::Candidate {
        cell: leaf,
        draw: 0,
        seed,
    };
    let mut world = Generating {
        world: World {
            clipmap,
            ground,
            palette_indices: NoPalette(0),
            shards,
            overwrites,
            eye,
            pool: way::tree::Read(pool),
        },
        rows,
    };
    let (g, has_footing, generates) =
        generation::placement(&mut world, &rows[j as usize], cell, &c, true);
    if has_footing && generates {
        out[r] = generation::record_word(j, g.variant, g.turn, cell);
        out[r + 1] = g.at[0] as u32;
        out[r + 2] = g.at[1] as u32;
        out[r + 3] = g.at[2] as u32;
        out[r + 4] = g.sink;
    } else {
        out[r] = generation::UNGENERATED;
    }
}

/// The `z`-th entry and decor `decor_mark_cs` listed (pxl-engine
/// `generation::LIST_AT`), and whether it generates in part: the pairs
/// generated whole first, as many as `decor_thin_cs` is dispatched over,
/// then those in part (`generation::part_at`).
fn list_entry(list: &[u32], z: u32) -> (u32, u32, bool) {
    let whole = at(list, generation::THIN_ARGS + 2);
    let part = z >= whole;
    let w = at(
        list,
        if part {
            generation::part_at(z - whole)
        } else {
            generation::LIST_AT + z
        },
    );
    (w & 0xffff, w >> 16 & 0xff, part)
}

/// The first word of entry `s`'s `i`-th record of decor `k` (pxl-engine
/// `generation::entry_record`).
fn entry_word(rows: &[Decor], s: u32, k: u32, i: u32) -> u32 {
    generation::record_at(generation::entry_record(rows, s, k, i))
}

/// Whether workgroup `g` of a decor pass reads records: past the entries,
/// placed decor's always; an entry's where the clip levels still hold the brick
/// they were generated for (`generation::generated_for`).
fn is_held(records: &[u32], clipmap: &[IVec4], g: u32) -> bool {
    if g >= generation::EYE_ENTRIES {
        return true;
    }
    let (h, q) = (generation::head_at(g), at(clipmap, g));
    let head = [at(records, h), at(records, h + 1), at(records, h + 2)];
    q.z != 0 && generation::generated_for(head, [q.x, q.y])
}

/// The `k`-th run of records workgroup `g` of a decor pass reads, as record
/// numbers (pxl-engine `generation::record_at`): the first and how many,
/// where it reads any (`is_held`). A workgroup an entry, each run
/// `generation::RUN` of its records, those of the decor whose room the run
/// lies in that it laid (`generation::cnt_at`); past the entries, a workgroup
/// `generation::RUNS` of placed decor's, a record a run. A pass reads the
/// records laid, not the room.
fn run_of(records: &[u32], rows: &[Decor], g: u32, k: u32) -> (u32, u32) {
    if g < generation::EYE_ENTRIES {
        let at_run = k * generation::RUN;
        let (mut j, mut first) = (0, 0);
        while j + 1 < rows.len() as u32 && at_run >= first + rows[j as usize].cap() {
            first += rows[j as usize].cap();
            j += 1;
        }
        let (laid, cap) = (at(records, generation::cnt_at(g, j)), rows[j as usize].cap());
        let laid = if laid < cap { laid } else { cap };
        let past = at_run - first;
        let cnt = if laid > past { laid - past } else { 0 };
        (
            generation::entry_record(rows, g, j, past),
            if cnt < generation::RUN {
                cnt
            } else {
                generation::RUN
            },
        )
    } else {
        let n = (g - generation::EYE_ENTRIES) * generation::RUNS + k;
        (
            generation::SLOT_RECORDS + n,
            u32::from(n < at(records, generation::PLACED_HEAD)),
        )
    }
}

/// Each record for the eye this frame, a workgroup an entry's or a run of
/// placed decor's (`run_of`), its region written into its first word and
/// counted: nothing where the rays ask another zoom at its distance, held to
/// the clip levels holding its column (`generation::held_at`), than the one
/// its record's clip level draws at (`generation::drawn_at`), or where it is
/// the decor left out; else sized for the screen (pxl-engine
/// `generation::drawn`), a model where its group's extent spans `MODEL_ANGLE`
/// at its distance, an impostor down to `IMPOSTOR_ANGLE`, else culled.
#[compute(threads(DECOR_RECORD_LANES), entry_point_name = "decor_cnt_cs")]
pub fn decor_cnt_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] drawing: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] rows: &[Decor],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] records: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] regions: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] clipmap: &[IVec4],
) {
    if !is_held(records, clipmap, group.x) {
        return;
    }
    let mut k = lane.x;
    while k < generation::RUNS {
        let (first, cnt) = run_of(records, rows, group.x, k);
        let mut n = 0;
        while n < cnt {
            cnt_one(
                eye,
                drawing,
                rows,
                records,
                regions,
                clipmap,
                generation::record_at(first + n) as usize,
            );
            n += 1;
        }
        k += DECOR_RECORD_LANES;
    }
}

/// `decor_cnt_cs` for the record at word `i`.
fn cnt_one(
    eye: &Eye,
    drawing: &[u32],
    rows: &[Decor],
    records: &mut [u32],
    regions: &mut [u32],
    clipmap: &[IVec4],
    i: usize,
) {
    if i + 3 >= records.len() || records[i] == generation::UNGENERATED {
        return;
    }
    let w = records[i];
    let (decor, variant, _, cell) = generation::record_of(w);
    let corner = [
        records[i + 1] as i32,
        records[i + 2] as i32,
        records[i + 3] as i32,
    ];
    let (origin, detail) = eye_origin(eye);
    // Its distance once, for the zoom it is drawn at and its size.
    let d2 = generation::distance2(origin, corner);
    // Drawn only where the rays ask its clip level's zoom at its distance,
    // held to the clip levels holding its column, or, placed, past the
    // entries' records, the coarsest it stands at or finer.
    let placed = i >= generation::record_at(generation::SLOT_RECORDS) as usize;
    let at = detail.at(d2);
    let at = if placed {
        at
    } else {
        generation::held_at(&Clipmap(clipmap), at, [corner[0], corner[1]])
    };
    let here = generation::drawn_at(at, cell, placed);
    let group = generation::first_group(rows, decor) + variant;
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
        f32::from_bits(drawing[drawn::IMPOSTOR_ANGLE as usize]),
    );
    let region = if hidden || !here || tier == drawn::CULLED {
        drawn::NONE
    } else {
        2 * group + tier
    };
    records[i] = w & ((1 << drawn::REGION_SHIFT) - 1) | region << drawn::REGION_SHIFT;
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

/// Each region laid after the one before (`decor_cnt_cs`'s counts), its
/// first slot kept for `decor_compact_cs` and added to its draw uniform's
/// index, and its indirect args: a model region's indexed by its group's
/// mesh (`meshes`, its index count, none while it loads), an impostor
/// region's a strip of four corners an instance.
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
        uniforms[(r * drawn::UNIFORM_WORDS + drawn::UNIFORM_IDX) as usize] += first;
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

/// Each record its region drew a slot of, read as `decor_cnt_cs` reads
/// them (`run_of`), compacted into the decor's instances at that slot: its
/// box's base middle about the
/// anchor, its heading and scale, in the lanes the decor and impostor
/// shaders read, and the slot its own in the index list.
#[compute(threads(DECOR_RECORD_LANES), entry_point_name = "decor_compact_cs")]
pub fn decor_compact_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] drawing: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] rows: &[Decor],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] records: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] regions: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] instances: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] clipmap: &[IVec4],
) {
    if !is_held(records, clipmap, group.x) {
        return;
    }
    let mut k = lane.x;
    while k < generation::RUNS {
        let (first, cnt) = run_of(records, rows, group.x, k);
        let mut n = 0;
        while n < cnt {
            compact_one(
                drawing,
                rows,
                records,
                regions,
                instances,
                generation::record_at(first + n) as usize,
            );
            n += 1;
        }
        k += DECOR_RECORD_LANES;
    }
}

/// `decor_compact_cs` for the record at word `i`.
fn compact_one(
    drawing: &[u32],
    rows: &[Decor],
    records: &[u32],
    regions: &mut [u32],
    instances: &mut [u32],
    i: usize,
) {
    if i + 4 >= records.len() {
        return;
    }
    let w = records[i];
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
    let (decor, variant, turn, _) = generation::record_of(w);
    let [across, deep, _] = rows[decor as usize].cells(variant, turn);
    let cap = instances.len() as u32 / (drawn::COLUMNS + 1);
    if slot >= cap {
        return;
    }
    // [rust-gpu] Each by a call, not a closure over a binding's slice.
    let x = from_anchor(drawing, records, i, 0) + across as f32 / 2.0;
    let y = from_anchor(drawing, records, i, 1) + deep as f32 / 2.0;
    // Its base as drawn: under its box's corner by its column surface's kept top.
    let sink = records[i + 4] as f32 / (1 << pxl_engine::FRAC_W) as f32;
    let z = from_anchor(drawing, records, i, 2) - sink;
    let (cap, slot) = (cap as usize, slot as usize);
    instances[slot] = x.to_bits();
    instances[cap + slot] = y.to_bits();
    instances[2 * cap + slot] = z.to_bits();
    instances[3 * cap + slot] = ((turn % 4) as f32 * core::f32::consts::FRAC_PI_2).to_bits();
    instances[4 * cap + slot] = 1.0f32.to_bits();
    instances[drawn::COLUMNS as usize * cap + slot] = slot as u32;
}

/// The corner of the record at word `i` on `axis` about the anchor, in
/// unit cells (`compact_one`).
fn from_anchor(drawing: &[u32], records: &[u32], i: usize, axis: usize) -> f32 {
    let anchor = drawn::ANCHOR as usize + axis;
    about(
        (records[i + 1 + axis] as i32).wrapping_sub(drawing[anchor] as i32),
        axis as u32,
    ) as f32
}

/// A difference of unit cells on `axis`, X the short way round, as unit
/// cells wrap at 2^24 on X: the one statement of the wrap, the placed
/// decor's about the anchor (`compact_one`) and the anchor's about the eye
/// (`Instances::pos`) alike.
fn about(d: i32, axis: u32) -> i32 {
    if axis == 0 {
        d << 8 >> 8
    } else {
        d
    }
}

const _: () = assert!(
    pxl_engine::FRAC_W == 8,
    "a Q3's unit cells wrap at 2^24 as `about` takes them"
);

/// The decor compacted this frame as its draws read it (`decor_compact_cs`):
/// its drawing words, its regions and its instances. What a reader beside
/// the draws asks of it, how many and where each stands, decor answers
/// here, so none names its lanes (pxl-shader light.rs `light_mark_draws_cs`).
pub(crate) struct Instances<'a> {
    pub(crate) drawing: &'a [u32],
    pub(crate) regions: &'a [u32],
    pub(crate) instances: &'a [u32],
}

impl Instances<'_> {
    /// The slots the instances hold.
    fn cap(&self) -> u32 {
        self.instances.len() as u32 / (drawn::COLUMNS + 1)
    }

    /// How many are placed: the last region's end once placed, the regions
    /// laid in order from the first slot (`decor_args_cs`), no more than the
    /// slots.
    pub(crate) fn cnt(&self) -> u32 {
        at(self.regions, 2 * drawn::REGIONS - 1).min(self.cap())
    }

    /// Where placed slot `k` stands, its base middle as drawn (`compact_one`),
    /// units from the eye at Q3 `origin`.
    pub(crate) fn pos(&self, k: u32, origin: [i32; 3]) -> [f32; 3] {
        [
            self.anchor(0, origin[0]) + self.lane(0, k),
            self.anchor(1, origin[1]) + self.lane(1, k),
            self.anchor(2, origin[2]) + self.lane(2, k),
        ]
    }

    /// The anchor on `axis` from a Q3 `o`, in units.
    fn anchor(&self, axis: u32, o: i32) -> f32 {
        let w = pxl_engine::FRAC_W;
        let a = at(self.drawing, drawn::ANCHOR + axis) as i32;
        about(a.wrapping_sub(o >> w), axis) as f32 - (o & ((1 << w) - 1)) as f32 / (1 << w) as f32
    }

    /// Lane `l` of slot `k`, f32.
    fn lane(&self, l: u32, k: u32) -> f32 {
        f32::from_bits(at(self.instances, l * self.cap() + k))
    }
}
