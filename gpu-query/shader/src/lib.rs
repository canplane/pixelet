// gpu-query-shader/lib.rs

//! The cast's compute entry (`cast_cs`): each drawn pixel's ray down the
//! pyramid by pxl-portable's descent, the one the CPU's cast runs, over
//! the world as the GPU holds it; and the ground of each square the eye's
//! windows hold, filled once as it is handed: its columns' tops off its
//! tile's samples (`heights_cs`), its ways' runs off its lines (`ways_cs`)
//! and its levels (`bound_cs`), which the CPU reads back; and the decor the
//! ground grows in the eye's windows (`decor_cs`). Each entry is a
//! module of its own.

#![cfg_attr(target_arch = "spirv", no_std)]

use pxl_portable::area::{self, Area, Shard};
use pxl_portable::cast::{self, Detail, Ends, In, Kind, Met, Query, Until};
use pxl_portable::caves::Cave;
use pxl_portable::cube::object;
use pxl_portable::decor::{Decor, COUNTS};
use pxl_portable::eye;
use pxl_portable::fill::{self, column_of, level_of, put};
use pxl_portable::ground;
use pxl_portable::lattice::GROUND;
use pxl_portable::placed::{self, Cut, Held, Lanes, BLOCK, PLACED, SPLIT};
use pxl_portable::scatter::{self, drawn, Grounds, PLACED_BLOCK, SQUARE_BLOCKS, SQUARE_GROWN};
use pxl_portable::window::{self, Squares, FIRST_CELL, SLOTS, SQUARE, SQUARE_SHIFT, SQUARE_WORDS};
use pxl_portable::window::{Found, Ran, FRESH_WORDS, RUNS_AT};
use pxl_portable::{at, mask, ray, wrap_index, LEAF_CELL, SHARD_ZOOM, UNIT_ZOOM};
use spirv_std::glam::{IVec4, UVec2, UVec3, UVec4, Vec2, Vec4};
use spirv_std::memory::{Scope, Semantics};
use spirv_std::{spirv, Image};

/// The eye a cast is from (pxl-kernel view/lens.rs `gpu_cast`), its words
/// where pxl-portable `eye` lays them (asserted below).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Eye {
    /// The Q3 position, and whether a ray reports the nodes its descent
    /// asked (`word`), not what it met.
    pub origin: IVec4,
    /// The unit forward, and the reach.
    pub forward_far: Vec4,
    /// The unit right, and τ.
    pub right_tau: Vec4,
    /// The unit up, and a pad.
    pub up: Vec4,
    /// The tangents of the screen's half-angles across and up.
    pub tan: Vec2,
    /// The zooms a block is asked at. [naga] A vec2 of u32s, which naga's
    /// SPIR-V reader looks for, and never makes, where it splits the layer
    /// off an arrayed image's coordinate (`Rays`).
    pub range: UVec2,
    /// A preview's cut to air and its left-out leaf: X, Y, Z and the zoom,
    /// `eye::NO_CUT` for none.
    pub carve: IVec4,
    pub hide: IVec4,
}

/// Word `words`'s byte.
const fn at_word(words: usize) -> usize {
    words * core::mem::size_of::<u32>()
}

const _: () = {
    use at_word as at;
    use core::mem::{offset_of, size_of};
    assert!(size_of::<Eye>() == at(eye::WORDS));
    assert!(offset_of!(Eye, origin) == at(eye::ORIGIN) && offset_of!(Eye, forward_far) == at(eye::FORWARD) && offset_of!(Eye, right_tau) == at(eye::RIGHT));
    assert!(offset_of!(Eye, up) == at(eye::UP) && offset_of!(Eye, tan) == at(eye::TAN) && offset_of!(Eye, range) == at(eye::RANGE));
    assert!(offset_of!(Eye, carve) == at(eye::CARVE) && offset_of!(Eye, hide) == at(eye::HIDE));
};

/// A preview's entry as pxl-portable `placed` takes it.
fn cut(e: IVec4) -> Cut {
    Cut {
        zoom: e.w as u32,
        at: [e.x, e.y, e.z],
    }
}

use pxl_portable::ground::VOID;
const PLACED_AIR: u32 = PLACED_BLOCK | object::air as u32;

/// Square `square`'s first word in the ground's buffer (pxl-portable
/// `window::SQUARE_WORDS`).
fn square_at(square: u32) -> u32 {
    square * SQUARE_WORDS
}

/// The world as the GPU holds it (pxl-kernel scene/cast.rs fills it): the
/// squares the eye's windows hold, their columns, levels and runs, the
/// service area, and what is placed with a preview's cuts; the caves are a
/// hash, held nowhere. Decor is not here (pxl-portable `cast::Query`).
#[derive(Clone, Copy)]
pub struct World<'a> {
    /// Per window of the eye's, from the top's, a row a slot: the square it
    /// holds, and whether it is present (pxl-portable `window`).
    window: &'a [IVec4],
    /// Per square of every window, its looks as handed, its columns
    /// (`heights_cs`, `ways_cs`), their levels (`bound_cs`), their runs
    /// (`ways_cs`), and its tile's samples and lines as handed
    /// (`window::SQUARE_WORDS`); the eye's windows first.
    ground: &'a [u32],
    /// The served shards: zoom, x, y.
    shards: &'a [IVec4],
    /// The overwrites' tree (pxl-kernel layer/object/store.rs): its heap
    /// lane, then its data lane from half the buffer.
    overwrites: &'a [u32],
    /// The eye, whose preview cuts are read where something placed or cut
    /// is asked about, not held through the descent.
    eye: &'a Eye,
    /// The eye's windows folded over their squares (`joined_cs`,
    /// pxl-portable `window::fold`), each window's `JOINED_ENTRIES` a run.
    joined: &'a [u32],
}

impl Lanes for World<'_> {
    fn heap(&self, slot: u32) -> u32 {
        at(self.overwrites, slot)
    }
    fn data(&self, slot: u32) -> u32 {
        at(self.overwrites, self.overwrites.len() as u32 / 2 + slot)
    }
}

impl Area for World<'_> {
    fn ends(&self) -> bool {
        true
    }
    /// Its keys four a row (pxl-kernel scene/cast.rs `shards`).
    fn keys(&self) -> u32 {
        4 * self.shards.len() as u32
    }
    fn key(&self, i: u32) -> u32 {
        let r = at(self.shards, i / 4);
        (match i % 4 {
            0 => r.x,
            1 => r.y,
            2 => r.z,
            _ => r.w,
        }) as u32
    }
}

impl World<'_> {
    /// Which of the blocks about the block `c` at `zoom` fill, as pxl-portable
    /// `mask::filled` names them: a column at a time, each read once (its
    /// cell, or the Earth past the area), its three blocks decided on values;
    /// what is placed asked of each block, the caves only where one may open
    /// it (`Cell::opens`).
    ///
    /// `shard` and `place` are the cast's at its stop (`ray::place_of`):
    /// what is known of the area there, and where the block's own column
    /// was held, which every column in its square is read off and none
    /// searched for anew (`window::ground`).
    fn about(&mut self, zoom: u32, c: [i32; 3], shard: Shard, place: Found) -> u32 {
        let (carve, hide) = (cut(self.eye.carve), cut(self.eye.hide));
        let own = (zoom, [c[0], c[1]]);
        let mut filled = 0;
        let mut k = 0;
        while k < 9 {
            let n = [wrap_index(zoom, c[0] + (k % 3) as i32 - 1), c[1] + (k / 3) as i32 - 1];
            let past = zoom >= SHARD_ZOOM && shard.past_leaf(own, area::tile_at(zoom, n));
            let cell = window::ground(self, (shard, own), place, zoom, n).0;
            let mut dz = 0;
            while dz < 3 {
                let (bit, b) = (1 << (dz * 9 + k), [n[0], n[1], c[2] + dz as i32 - 1]);
                if mask::READS & bit != 0 {
                    let held = placed::cursor(self, PLACED, zoom, b);
                    let over = if held.holds() || self.eye.carve.w as u32 != eye::NO_CUT {
                        placed::over(self, held, carve, hide, zoom, b, true)
                    } else {
                        placed::Over { says: placed::UNDER, block: VOID }
                    };
                    let fills = if over.says == BLOCK {
                        self.fills(&[PLACED_BLOCK | over.block, 0])
                    } else if past {
                        area::earth(b[2]) != VOID
                    } else {
                        cell.block(zoom, b[2], false).0 != VOID && !(cell.opens(zoom, b[2]) && Cave::of(zoom, b).hollow())
                    };
                    filled |= (fills as u32) * bit;
                }
                dz += 1;
            }
            k += 1;
        }
        filled
    }
}

/// The eye's windows, as the GPU holds them.
impl Squares for World<'_> {
    /// The slot square `q` is held in, while it holds it present.
    fn present(&self, cell: u32, q: [i32; 2]) -> (u32, bool) {
        let s = window::slot(q);
        let h = at(self.window, (cell - FIRST_CELL) * SLOTS + s);
        (s, h.z != 0 && h.x == q[0] && h.y == q[1])
    }
    fn square(&self, cell: u32, slot: u32) -> ([i32; 2], bool) {
        let h = at(self.window, (cell - FIRST_CELL) * SLOTS + slot);
        ([h.x, h.y], h.z != 0)
    }
    /// Off the fold's lane (`joined_cs`).
    fn joined(&self, cell: u32, e: u32) -> [u32; window::JOINED_WORDS as usize] {
        let k = ((cell - FIRST_CELL) * window::JOINED_ENTRIES + e) * window::JOINED_WORDS;
        [at(self.joined, k), at(self.joined, k + 1), at(self.joined, k + 2)]
    }
    fn look(&self, cell: u32, slot: u32, i: u32) -> u32 {
        at(self.ground, square_at((cell - FIRST_CELL) * SLOTS + slot) + i)
    }
    fn column(&self, cell: u32, slot: u32, i: u32) -> u32 {
        column_of(self.ground, square_at((cell - FIRST_CELL) * SLOTS + slot), i)
    }
    fn level(&self, cell: u32, slot: u32, j: u32, at: [u32; 2]) -> i32 {
        level_of(self.ground, square_at((cell - FIRST_CELL) * SLOTS + slot), j, at)
    }
}

/// The ways' runs `ways_cs` wrote in the eye's windows.
impl Ran for World<'_> {
    fn ran(&self, cell: u32, slot: u32, r: u32) -> u32 {
        at(self.ground, square_at((cell - FIRST_CELL) * SLOTS + slot) + RUNS_AT + r)
    }
}

/// The ground as the GPU holds it, for pxl-portable's one answer for a node.
impl ground::Held for World<'_> {
    /// The bound over the node of the window `cell` reads its cells from,
    /// the finest holding them present (`window::bounded`), as the CPU reads
    /// it (pxl-portable `Held::relief`).
    fn relief(&mut self, place: &mut Found, cell: u32, (zoom, c): (u32, [i32; 3])) -> ([i32; 2], bool) {
        let (r, _, here) = window::bounded(self, place, window::held_to(cell), (zoom, [c[0], c[1]]));
        (r, here)
    }

    /// The block's column off the windows, the area's edge laid on it
    /// (pxl-portable `window::ground`), as the CPU reads it (pxl-kernel
    /// query/node.rs `Reading::cell`).
    fn cell(&mut self, shard: Shard, place: Found, zoom: u32, c: [i32; 3]) -> ground::Cell {
        window::ground(self, (shard, (zoom, [c[0], c[1]])), place, zoom, [c[0], c[1]]).0
    }
}

/// What a node is handed down on the GPU: its caves, what is placed there,
/// what is known of the service area at its shard (`area::Shard`), and
/// where the ground's cells of the nodes over it were held (`window::step`),
/// which its own read steps from.
#[derive(Clone, Copy)]
pub struct At {
    cave: Cave,
    placed: Held,
    shard: Shard,
    ground: Found,
}

/// What a node is on the GPU: its value, and for the ground the four looks
/// a coarse block folds, a byte each, `dx + 2·dy` (`ground::Cell`), what
/// the screen paints a hit with.
impl Query for World<'_> {
    type At = At;
    type Block = [u32; 2];
    const AIR: [u32; 2] = [VOID, 0];

    fn root(&mut self, node: [i32; 3]) -> At {
        At {
            cave: Cave::of(0, node),
            placed: placed::cursor(self, PLACED, 0, node),
            shard: Shard::UNKNOWN,
            ground: Found::NONE,
        }
    }

    fn child(&mut self, at: &At, zoom: u32, node: [i32; 3]) -> At {
        At {
            cave: at.cave.child(zoom, node),
            placed: placed::child(self, at.placed, zoom, node),
            shard: at.shard,
            ground: at.ground,
        }
    }

    /// What is in the node `c` at `zoom`, the layers top down: what is
    /// placed, then the ground's one answer for a node (pxl-portable
    /// `ground::node`); asked as a block at `near` or finer, or where the
    /// world holds nothing finer — past the ground's cells, with nothing
    /// placed or carved finer in it.
    fn node(&mut self, at: &mut At, zoom: u32, c: [i32; 3], asked: [u32; 2], _t0: f32) -> In<[u32; 2]> {
        // What is known of the service area at the node, before any layer
        // answers it: a stop's shard is its own, whichever layer stops it.
        at.shard = at.shard.at(self, (zoom, [c[0], c[1]]));
        let mut block = zoom >= asked[1] || zoom >= LEAF_CELL;
        // Asked of what is placed only where something is, or a preview cuts.
        if at.placed.holds() || self.eye.carve.w as u32 != eye::NO_CUT {
            let carve = cut(self.eye.carve);
            block = zoom >= asked[1] || zoom >= LEAF_CELL && placed::whole(at.placed, carve, zoom, c);
            let over = placed::over(self, at.placed, carve, cut(self.eye.hide), zoom, c, block);
            if over.says == BLOCK {
                return In {
                    kind: Kind::Block,
                    block: [PLACED_BLOCK | over.block, 0],
                };
            }
            if over.says == SPLIT {
                return In { kind: Kind::Split, block: [VOID, 0] };
            }
        }
        let (i, cell, found) = ground::node(self, at.cave, at.shard, &mut at.ground, (zoom, c), asked, block);
        // What the ways write over the ground's block, read as the CPU reads
        // it (pxl-portable `window::ways_over`).
        let (value, f) = if block && i.kind == Kind::Block {
            window::ways_over(self, found, cell, zoom, c, i.block)
        } else {
            i.block
        };
        In {
            kind: i.kind,
            block: [value, ground::word(f)],
        }
    }

    fn fills(&self, block: &[u32; 2]) -> bool {
        block[0] != VOID && block[0] != PLACED_AIR
    }
}

/// The world as decor's growth asks it on the GPU (pxl-portable `scatter`):
/// the cast's, and the catalog's rows as handed.
struct Growing<'a> {
    world: World<'a>,
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

/// A cell off the window drawing it, and a block asked by the cast's
/// descent, from a node started as the CPU's world starts one (pxl-kernel
/// query/node.rs `World::start`): its caves and what is placed found down
/// its line from the top.
impl Grounds for Growing<'_> {
    type At = At;

    fn rows(&self) -> &[Decor] {
        self.rows
    }

    fn face(&mut self, cell: u32, g: [i32; 2]) -> (u32, i32, bool) {
        let shard = Shard::UNKNOWN.at(&self.world, (cell, g));
        let c = ground::Held::cell(&mut self.world, shard, Found::NONE, cell, [g[0], g[1], 0]);
        (c.look, c.top, c.grounded())
    }

    fn start(&mut self, zoom: u32, c: [i32; 3]) -> At {
        At {
            cave: Cave::of(zoom, c),
            placed: placed::cursor(&self.world, PLACED, zoom, c),
            shard: Shard::UNKNOWN,
            ground: Found::NONE,
        }
    }

    fn block(&mut self, at: &At, from: u32, zoom: u32, c: [i32; 3]) -> u32 {
        let mut at = *at;
        let mut z = from;
        while z < zoom {
            z += 1;
            let s = zoom - z;
            at = self.world.child(&at, z, [c[0] >> s, c[1] >> s, c[2] >> s]);
        }
        self.world.node(&mut at, zoom, c, [zoom, zoom], 0.0).block[0]
    }
}

/// A ray's words, a layer each of `Rays`: the value it stopped on, its kind (ground or
/// placed) and the face it entered through, then how far as f32 bits, then
/// the block's index's low bits, for `face_cs` to find the block by and
/// write its face bits over, then the four looks a coarse ground block
/// folds or, in the steps view, the nodes its descent asked about, then
/// where the stop's column is held and the shards about it served
/// (`ray::place_word`); where it met nothing, the first three zero.
fn word(o: &cast::Origin, ray: &cast::Ray, met: Met<At, [u32; 2]>, steps: bool) -> [u32; ray::WORDS as usize] {
    if met.ends != Ends::Stop {
        return [0, 0, 0, met.visits, 0];
    }
    let s = met.stop;
    let down = pxl_portable::axis(ray.direction, s.axis) > 0.0;
    let face = 2 * s.axis + down as u32;
    let [value, folds] = s.block;
    let kind = if value & PLACED_BLOCK != 0 { ray::KIND_PLACED } else { ray::KIND_GROUND };
    let (zoom, c) = (s.place.zoom, s.place.node(o));
    let w3 = if steps { met.visits } else { folds };
    [ray::first_word(value, kind, face, zoom), s.t.to_bits(), low_bits(c), w3, ray::place_word(s.at.ground, s.at.shard)]
}

/// Bits of a block's index a ray keeps on each axis (`word`), enough to tell it
/// from the block its ray's hit point rounds to, which f32 holds to within
/// a few blocks.
const LOW_BITS: u32 = 10;

/// The greatest integer at most `v`.
fn floor(v: f32) -> i32 {
    let i = v as i32;
    if (i as f32) > v {
        i - 1
    } else {
        i
    }
}

fn low_bits(c: [i32; 3]) -> u32 {
    let m = (1 << LOW_BITS) - 1;
    (c[0] & m) as u32 | ((c[1] & m) as u32) << LOW_BITS | ((c[2] & m) as u32) << (2 * LOW_BITS)
}

/// The block at `zoom` a ray stopped on, entering across `axis`, `t` along
/// it, its index's low bits `low` (`low_bits`): the block its hit point
/// rounds to, moved half a block into it across the face it entered,
/// corrected to the one whose low bits the cast kept.
fn hit_block(o: &cast::Origin, ray: &cast::Ray, t: f32, axis: u32, zoom: u32, low: u32) -> [i32; 3] {
    let side = cast::Place { zoom, corner: [0; 3] }.side();
    let half = (1 << LOW_BITS) / 2;
    let mut c = [0; 3];
    let mut a = 0;
    while a < 3 {
        let d = pxl_portable::axis(ray.direction, a as u32);
        let into = if a as u32 == axis {
            if d > 0.0 {
                0.5 * side
            } else {
                -0.5 * side
            }
        } else {
            0.0
        };
        let rel = o.frac[a] + d * t + into;
        let near = if zoom <= UNIT_ZOOM {
            (o.unit[a] + floor(rel)) >> (UNIT_ZOOM - zoom)
        } else {
            (o.unit[a] << (zoom - UNIT_ZOOM)) + floor(rel * (1 << (zoom - UNIT_ZOOM)) as f32)
        };
        let kept = ((low >> (LOW_BITS * a as u32)) & ((1 << LOW_BITS) - 1)) as i32;
        c[a] = near + ((kept - near + half) & ((1 << LOW_BITS) - 1)) - half;
        a += 1;
    }
    [wrap_index(zoom, c[0]), c[1], c[2]]
}

/// The face bits of each ray's block the cast found (`word`): the blocks
/// about it asked at its zoom, each once (pxl-portable `mask`), as a
/// chunk's box takes them, over the ray's index bits. A pass of its own, so
/// the cast's kernel holds the descent alone.
#[spirv(compute(threads(8, 8, 1)))]
pub fn face_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] joined: &mut [u32],
) {
    let joined: &[u32] = joined;
    let size: UVec3 = rays.query_size();
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let ray_word = |k: u32| rays.read(UVec3::new(id.x, id.y, k));
    let w = UVec4::new(ray_word(0), ray_word(1), ray_word(2), ray_word(3));
    if ray::kind_of(w.x) == ray::KIND_NONE {
        return;
    }
    let (origin, ray) = (eye_origin(eye).0, pixel_ray(eye, id, size));
    let mut world = World {
        window,
        ground,
        shards,
        overwrites,
        eye,
        joined,
    };
    let (axis, zoom) = (ray::face_of(w.x) / 2, ray::zoom_of(w.x));
    let c = hit_block(&origin, &ray, f32::from_bits(w.y), axis, zoom, w.z);
    let (place, shard) = ray::place_of(ray_word(ray::PLACE), zoom);
    let filled = world.about(zoom, c, shard, place);
    // SAFETY: within the rays, just bounded by their size.
    unsafe { rays.write(UVec3::new(id.x, id.y, 2), mask::mask(filled)) };
}

/// The start's stack as `start_cs` hands it (pxl-portable `cast::Over`).
struct Starts<'a>(&'a [u32]);

impl cast::Over<At> for Starts<'_> {
    fn over(&self, zoom: u32) -> At {
        at_of(self.0, cast::start_stack(zoom))
    }
}

/// An `At`'s words as `start_cs` hands it to `cast_cs` (pxl-portable
/// `cast::START_WORDS`), each its place among them.
const AT_CAVE: usize = 0;
const AT_PLACED: usize = 1;
const AT_SHARD: usize = 2;
const AT_GROUND: usize = 3;
const _: () = assert!(AT_GROUND + 1 == cast::AT_WORDS as usize);

fn at_words(a: At) -> [u32; cast::AT_WORDS as usize] {
    let mut w = [0; cast::AT_WORDS as usize];
    (w[AT_CAVE], w[AT_PLACED], w[AT_SHARD], w[AT_GROUND]) = (a.cave.word(), a.placed.word(), a.shard.word(), a.ground.word());
    w
}

fn put_at(w: &mut [u32], k: u32, a: At) {
    let words = at_words(a);
    let mut j = 0;
    while j < cast::AT_WORDS {
        put(w, k + j, words[j as usize]);
        j += 1;
    }
}

fn at_of(w: &[u32], k: u32) -> At {
    At {
        cave: Cave::of_word(at(w, k + AT_CAVE as u32)),
        placed: Held::of_word(at(w, k + AT_PLACED as u32)),
        shard: Shard::of_word(at(w, k + AT_SHARD as u32)),
        ground: Found::of_word(at(w, k + AT_GROUND as u32)),
    }
}

/// Where the eye's rays start, and the zooms each distance asks.
fn eye_origin(eye: &Eye) -> (cast::Origin, Detail) {
    (
        cast::Origin::of([eye.origin.x, eye.origin.y, eye.origin.z]),
        Detail {
            tau: eye.right_tau.w,
            range: [eye.range.x, eye.range.y],
        },
    )
}

/// The ray through the middle of drawn pixel `p` of the `size` the rays
/// are, row 0 at the top, as the screen pass forms it from its own pixel:
/// its tangents the screen's half-angles' scaled by where it lies.
fn pixel_ray(eye: &Eye, p: UVec3, size: UVec3) -> cast::Ray {
    let ndc_x = (p.x as f32 + 0.5) / size.x as f32 * 2.0 - 1.0;
    let ndc_y = 1.0 - (p.y as f32 + 0.5) / size.y as f32 * 2.0;
    let v3 = |v: Vec4| [v.x, v.y, v.z];
    cast::Ray {
        direction: cast::through(v3(eye.forward_far), v3(eye.right_tau), v3(eye.up), ndc_x * eye.tan.x, ndc_y * eye.tan.y),
        reach: eye.forward_far.w,
    }
}

/// The eye's rays, a drawn pixel each, a layer a word of the ray's (`word`;
/// pxl-portable `ray`): the cast writes them, the face bits
/// write the third over.
type Rays = Image!(2D, format = r32ui, sampled = false, arrayed = true);

/// The nodes over where every ray from the eye starts alike, asked once a
/// frame (pxl-portable `cast::start`) for every ray of `cast_cs` to start
/// from, one thread.
#[spirv(compute(threads(1, 1, 1)))]
pub fn start_cs(
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] starts: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] joined: &mut [u32],
) {
    let joined: &[u32] = joined;
    let (origin, detail) = eye_origin(eye);
    let mut world = World {
        window,
        ground,
        shards,
        overwrites,
        eye,
        joined,
    };
    let s = cast::start(&mut world, &origin, detail);
    put(starts, cast::START_PLACE, s.place.zoom);
    put(starts, cast::START_PLACE + 1, s.place.corner[0] as u32);
    put(starts, cast::START_PLACE + 2, s.place.corner[1] as u32);
    put(starts, cast::START_PLACE + 3, s.place.corner[2] as u32);
    put_at(starts, cast::START_AT, s.at);
    let mut k = 0;
    while k < cast::DEPTH {
        put_at(starts, cast::start_stack(k as u32), s.stack[k]);
        k += 1;
    }
}

#[spirv(compute(threads(8, 8, 1)))]
pub fn cast_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] starts: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] joined: &mut [u32],
) {
    let joined: &[u32] = joined;
    let size: UVec3 = rays.query_size();
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let ((origin, detail), ray) = (eye_origin(eye), pixel_ray(eye, id, size));
    let mut world = World {
        window,
        ground,
        shards,
        overwrites,
        eye,
        joined,
    };
    // Started where every ray from the eye starts alike (`start_cs`), what
    // was handed the nodes over it read only where a ray climbs past it.
    let read: &[u32] = starts;
    let place = cast::Place {
        zoom: at(read, cast::START_PLACE),
        corner: [at(read, cast::START_PLACE + 1) as i32, at(read, cast::START_PLACE + 2) as i32, at(read, cast::START_PLACE + 3) as i32],
    };
    let met = cast::first_from(&mut world, &origin, &ray, detail, Until::Fills, (place, at_of(read, cast::START_AT)), &Starts(read));
    let w = word(&origin, &ray, met, eye.origin.w != 0);
    let mut k = 0;
    while k < ray::WORDS {
        // SAFETY: within the rays, just bounded by their size.
        unsafe { rays.write(UVec3::new(id.x, id.y, k), w[k as usize]) };
        k += 1;
    }
}

/// The eye's windows folded over their squares, a thread an entry
/// (pxl-portable `window::fold`): each window's levels over a square, the
/// join of its aligned blocks of squares, which a node wider than a square
/// reads as one entry (`window::bounded`). Run where the windows' slots
/// moved; a present square's levels stand while it does.
#[spirv(compute(threads(64, 1, 1)))]
#[allow(clippy::too_many_arguments)]
pub fn joined_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] joined: &mut [u32],
) {
    if id.x >= window::JOINED_ENTRIES * window::EYE_WINDOWS {
        return;
    }
    let (cell, e) = (FIRST_CELL + id.x / window::JOINED_ENTRIES, id.x % window::JOINED_ENTRIES);
    let entry = {
        let read: &[u32] = joined;
        let world = World {
            window,
            ground,
            shards,
            overwrites,
            eye,
            joined: read,
        };
        window::fold(&world, cell, e)
    };
    let mut k = 0;
    while k < window::JOINED_WORDS {
        put(joined, id.x * window::JOINED_WORDS + k, entry[k as usize]);
        k += 1;
    }
}

/// The columns' ground tops of the squares `fresh` names, the squares
/// handed their tiles' samples this turn, one a `z`: the square, and where
/// it lies in its tile, in squares, X low; a thread a column
/// (`ground::height`) off the samples handed there.
#[spirv(compute(threads(8, 8, 1)))]
pub fn heights_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] fresh: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] ground: &mut [u32],
) {
    if id.x >= SQUARE || id.y >= SQUARE {
        return;
    }
    let base = square_at(at(fresh, FRESH_WORDS * id.z + window::FRESH_SQUARE));
    let place = window::place_of(at(fresh, FRESH_WORDS * id.z + window::FRESH_PLACE));
    fill::heights(ground, base, place, [id.x, id.y]);
}

/// Level `level.x` of the squares `fresh` names, one a `z`: level 0 each
/// column's own, its top and its runs (`fill::level`), a higher one over the
/// level under it, written before (`window::join`): a dispatch a level, from
/// 0 up.
#[spirv(compute(threads(8, 8, 1)))]
pub fn bound_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] level: &UVec4,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] fresh: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &mut [u32],
) {
    let j = level.x;
    let side = SQUARE >> j;
    if id.x >= side || id.y >= side {
        return;
    }
    let base = square_at(at(fresh, FRESH_WORDS * id.z + window::FRESH_SQUARE));
    fill::level(ground, base, at(fresh, FRESH_WORDS * id.z + window::FRESH_CELL), j, [id.x, id.y]);
}

/// The ways of the squares `fresh` names, one a `z`, a thread a column
/// (`x`, cells row by row): each column's spans off the square's lines
/// (pxl-portable `ways::column`) read along z as runs (`ways::runs`), from
/// its column word's first (`window::COLUMN_WORDS`). Three dispatches,
/// `pass.x` 0 to 2: each column counts its runs into its word; one thread a
/// square sums them in order into each column's first; each column writes
/// its runs from its first. Over `heights_cs`'s tops: a way is told from
/// their faces. Each square's entry as handed whole: where it lies and its
/// shard lay the area's edge (pxl-portable `ways::Columns::walled`).
#[spirv(compute(threads(64, 1, 1)))]
pub fn ways_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] pass: &UVec4,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] fresh: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &mut [u32],
) {
    let mut entry = [0; FRESH_WORDS as usize];
    let mut k = 0;
    while k < FRESH_WORDS {
        entry[k as usize] = at(fresh, FRESH_WORDS * id.z + k);
        k += 1;
    }
    fill::ways(ground, entry, pass.x, id.x);
}

/// The eye's slots whose decor grows again this frame, a thread a slot,
/// listed with each kind for `decor_cs` (pxl-portable `scatter::GROWS_AT`),
/// its count the dispatch's: every one where `marks` asks it, else one whose
/// square its records were not grown for (`scatter::grown_for`), beside a
/// square filled this frame (`fresh`), or met by a box (`scatter::meets`):
/// an edit, or a preview's cut as it was or is. A listed slot's head is
/// laid anew, its count none.
#[spirv(compute(threads(64, 1, 1)))]
pub fn decor_mark_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] marks: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] fresh: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] records: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] grows: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] rows: &[Decor],
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
    let mut k = 0;
    while k < at(marks, 1) {
        let f = at(fresh, FRESH_WORDS * k + window::FRESH_SQUARE);
        if f < scatter::EYE_SLOTS && f / SLOTS == s / SLOTS {
            let other = at(window, f);
            marked = marked || scatter::beside(cell, q, [other.x, other.y]);
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
    let kinds = rows.len() as u32;
    // SAFETY: an atomic add on the list's count, the dispatch's z.
    let n = unsafe { spirv_std::arch::atomic_i_add::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(&mut grows[2], kinds) };
    let mut j = 0;
    while j < kinds {
        put(grows, scatter::GROWS_AT + n + j, s | j << 16);
        j += 1;
    }
}

/// Decor the ground grows in a slot `decor_mark_cs` listed, one candidate a
/// thread: `x` its index in its block, `y` the block among those meeting the
/// slot's square (pxl-portable `scatter::block_of`), `z` the slot and kind
/// listed. Each one that stands, stands on its cell and grows there
/// (`scatter::stands`, `stand`, `grows`) is added to the slot's records
/// (`scatter::record_at`), naming the slot's window zoom, the one it is
/// drawn at where the rays ask it (`decor_count_cs`). A kind is grown at a
/// zoom where bit `zoom − FIRST_CELL` of its `shown` word is set; the kinds
/// are the catalog's rows as handed (`rows`). What it grows is the windows'
/// squares' and what is placed alone, never the eye's.
#[spirv(compute(threads(8, 8, 1)))]
pub fn decor_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shown: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] out: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] rows: &[Decor],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] grows: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] joined: &[u32],
) {
    let listed = at(grows, scatter::GROWS_AT + id.z);
    let (square, kind) = (listed & 0xffff, listed >> 16);
    let (cell, slot) = (window::eye_cell(square / SLOTS), square % SLOTS);
    if id.x >= COUNTS as u32 || id.y >= SQUARE_BLOCKS || cell > LEAF_CELL || at(shown, kind) >> (cell - FIRST_CELL) & 1 == 0 {
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
            shards,
            overwrites,
            eye,
            joined,
        },
        rows,
    };
    let decor = &rows[kind as usize];
    let zoom = GROUND.read_zoom(cell);
    let (c, here) = scatter::candidate_of(decor, zoom, region, id.y, id.x);
    if !here || !scatter::stands(decor, zoom, &c) {
        return;
    }
    let (g, here) = scatter::stand(&mut world, decor, cell, &c);
    if !here || !scatter::grows(&mut world, decor, cell, &g) {
        return;
    }
    // SAFETY: an atomic add on the slot's head, its count.
    let n = unsafe { spirv_std::arch::atomic_i_add::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(&mut out[scatter::head_at(square) as usize], 1) };
    let i = scatter::record_at(square * SQUARE_GROWN + n).0 as usize;
    if n < SQUARE_GROWN && i + 3 < out.len() {
        out[i] = scatter::record_word(kind, g.variant, g.turn, cell);
        out[i + 1] = g.at[0] as u32;
        out[i + 2] = g.at[1] as u32;
        out[i + 3] = g.at[2] as u32;
    }
}

/// Record `n` of `grown` (pxl-portable `scatter::record_at`): its first
/// word, and whether its head holds it, of a slot whose square the windows
/// still hold there (`scatter::grown_for`) or of placed decor.
fn record(grown: &[u32], window: &[IVec4], n: u32) -> (usize, bool) {
    let (w, h, i) = scatter::record_at(n);
    let count = at(grown, h);
    let stands = n >= scatter::SLOT_RECORDS || {
        let q = at(window, h / scatter::HEAD_WORDS);
        q.z != 0 && scatter::grown_for([count, at(grown, h + 1), at(grown, h + 2), at(grown, h + 3)], [q.x, q.y])
    };
    (w as usize, i < count && stands && w + 3 < grown.len() as u32)
}

/// Each record for the eye this frame, its region written into its first
/// word and counted: nothing where the rays ask another zoom at its
/// distance than the one it was grown at (`scatter::drawn_at`), or where it
/// is the decor left out; else sized for the screen (pxl-portable
/// `scatter::drawn`), a model where its group's extent spans `MODEL_PX` at
/// its distance, a sprite down to `SPRITE_PX`, else nothing.
#[spirv(compute(threads(64, 1, 1)))]
pub fn decor_count_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] params: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] rows: &[Decor],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] grown: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] regions: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] window: &[IVec4],
) {
    let (i, held) = record(grown, window, id.x);
    if !held {
        return;
    }
    let w = grown[i];
    let (kind, variant, _, cell) = scatter::record_of(w);
    let corner = [grown[i + 1] as i32, grown[i + 2] as i32, grown[i + 3] as i32];
    let origin = [eye.origin.x, eye.origin.y, eye.origin.z];
    let detail = Detail {
        tau: eye.right_tau.w,
        range: [eye.range.x, eye.range.y],
    };
    let here = cell == scatter::PLACED_AT || scatter::drawn_at(detail, origin, corner, cell);
    let group = scatter::first_group(rows, kind) + variant;
    let hide = drawn::HIDE as usize;
    let hidden = params[hide + 3] != 0 && corner[0] == params[hide] as i32 && corner[1] == params[hide + 1] as i32 && corner[2] == params[hide + 2] as i32;
    let d = scatter::distance(origin, corner);
    let d = if d > 1e-3 { d } else { 1e-3 };
    let px = f32::from_bits(params[(drawn::EXTENTS + group) as usize]) / (d * f32::from_bits(params[drawn::UPP as usize]));
    let region = if hidden || !here {
        drawn::NONE
    } else if px >= f32::from_bits(params[drawn::MODEL_PX as usize]) {
        2 * group
    } else if px >= f32::from_bits(params[drawn::SPRITE_PX as usize]) {
        2 * group + 1
    } else {
        drawn::NONE
    };
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
#[spirv(compute(threads(1, 1, 1)))]
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

/// Each record its region drew a slot of: its box's base middle about the
/// anchor, its heading and scale, in the lanes the decor and impostor
/// shaders read, and the slot its own in the index list.
#[spirv(compute(threads(64, 1, 1)))]
pub fn decor_place_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] params: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] rows: &[Decor],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] grown: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] regions: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] block: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] window: &[IVec4],
) {
    let (i, held) = record(grown, window, id.x);
    if !held {
        return;
    }
    let w = grown[i];
    let region = w >> drawn::REGION_SHIFT;
    if region == drawn::NONE {
        return;
    }
    // SAFETY: an atomic add on the region's next slot, a word of the buffer.
    let slot = unsafe { spirv_std::arch::atomic_i_add::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(&mut regions[(drawn::REGIONS + region) as usize], 1) };
    let (kind, variant, turn, _) = scatter::record_of(w);
    let [across, deep, _] = rows[kind as usize].cells(variant, turn);
    let room = block.len() as u32 / (drawn::LANES + 1);
    if slot >= room {
        return;
    }
    let anchor = drawn::ANCHOR as usize;
    // X about the anchor the short way round: leaf cells wrap at 2^24.
    let x = (((grown[i + 1] as i32).wrapping_sub(params[anchor] as i32)) << 8 >> 8) as f32 + across as f32 / 2.0;
    let y = (grown[i + 2] as i32).wrapping_sub(params[anchor + 1] as i32) as f32 + deep as f32 / 2.0;
    let z = (grown[i + 3] as i32).wrapping_sub(params[anchor + 2] as i32) as f32;
    let (room, slot) = (room as usize, slot as usize);
    block[slot] = x.to_bits();
    block[room + slot] = y.to_bits();
    block[2 * room + slot] = z.to_bits();
    block[3 * room + slot] = ((turn % 4) as f32 * core::f32::consts::FRAC_PI_2).to_bits();
    block[4 * room + slot] = 1.0f32.to_bits();
    block[drawn::LANES as usize * room + slot] = slot as u32;
}
