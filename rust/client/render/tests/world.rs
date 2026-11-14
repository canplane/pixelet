//! The world's one query by hand (frame/shader/world.wgsl) must answer word for word as
//! pxl-engine's `world::node` does. The test builds real world buffers on the CPU, as the fill
//! and the store lay them, and asks nodes along whole descents of the Rust. Each step is asked
//! again of the query by hand on a device, and each node's kind and asset are compared. Before
//! that, on the CPU, each node of a descent is asked again with nothing handed to it
//! (`Inheritance::of`). It must answer as it did from what its parent handed it, because the
//! world's answer belongs to the node, whichever way a walk came to it.

mod gpu;
mod real_world;

use pxl_spec::brick;
use pxl_spec::ground::asset_ids;
use pxl_spec::share::FULL;
use pxl_spec::space::coord::world::{wrap_idx, SEA_LEVEL};
use pxl_spec::space::coord::zoom::{Zoom, LEAF_CELL_ZOOM, MIN_ZOOM, SHARD_ZOOM};
use pxl_spec::way::NOTHING;
use std::collections::BTreeMap;

use pxl_content::cave;
use pxl_engine::answer::{self, In, Kind};
use pxl_engine::clipmap as clip;
use pxl_engine::clipmap::{Lookup, ACROSS, FIRST_CELL_ZOOM, ORIGIN_AT};
use pxl_engine::ground;
use pxl_engine::object::{self, Cut, Table};
use pxl_engine::way;
use pxl_engine::world::{self, Inheritance};
use pxl_spec::asset::cube::{Ground, Sentinel};
use pxl_spec::brick::BRICK_SHIFT;
use pxl_spec::space::layer::area;
use pxl_spec::value;

use pxl_render::frame::shader::record::WORLD_ASKED;
use real_world::{level_first, Draw, World, CORNERS, EYE, FINEST, UNLAID};

/// The words of one ask and of the answer told back (world_asked.wgsl `world_asked_cs`).
const ASKED_WORDS: usize = 19;
const TOLD_WORDS: usize = 8;
const OP_NODE: u32 = 0;
const OP_HOLLOW: u32 = 3;

/// The asks made so far and what the Rust answered of each.
#[derive(Default)]
struct AskLog {
    asked: Vec<u32>,
    told: Vec<u32>,
}

impl AskLog {
    fn push(
        &mut self,
        (op, zoom, c): (u32, Zoom, [i32; 3]),
        range: [Zoom; 2],
        at: [u32; 3],
        cuts: [Cut; 2],
        kind: u32,
        asset: [u32; 2],
    ) {
        let word = |cut: Cut| {
            [
                cut.at[0] as u32,
                cut.at[1] as u32,
                cut.at[2] as u32,
                cut.zoom,
            ]
        };
        self.asked
            .extend([op, zoom, c[0] as u32, c[1] as u32, c[2] as u32]);
        self.asked.extend(range);
        self.asked.extend(at);
        self.asked.extend(word(cuts[0]));
        self.asked.extend(word(cuts[1]));
        self.asked.push(0);
        self.told.extend([kind, asset[0], asset[1]]);
        self.told.extend([0; TOLD_WORDS - 3]);
    }
}

/// How many answers fell into each kind, by the kind's name.
#[derive(Default)]
struct Tally(BTreeMap<&'static str, u32>);

impl Tally {
    fn add(&mut self, name: &'static str, yes: bool) {
        *self.0.entry(name).or_default() += yes as u32;
    }
}

/// One descent toward the node at `point`, given in `FINEST` cells. It steps one zoom at a time
/// from `start`, which is handed nothing, to `stop`, and records the Rust's answer at each step
/// and what the step hands on. Where `moving` is set, each node takes zooms of its own, as a
/// ray's interval gives them (`cast::walk`). The zooms mostly narrow, with the near zoom
/// falling and the far rising, and some jump finer.
#[expect(clippy::too_many_arguments, reason = "a descent's own, each its own")]
fn descend(
    w: World,
    point: [i32; 3],
    [start, stop]: [Zoom; 2],
    [mut far, mut near]: [Zoom; 2],
    moving: bool,
    cuts: [Cut; 2],
    ask_log: &mut AskLog,
    tally: &mut Tally,
    draw: &mut Draw,
    seen: &mut Lookup,
) {
    let idx = |z: Zoom| {
        let a = object::ancestor_at(FINEST, point, FINEST - z);
        [wrap_idx(z, a[0]), a[1], a[2]]
    };
    let root = idx(start);
    let mut at = Inheritance::of(&w, (start, root));
    tally.add("root", true);
    // Whether a node above this one was handed another descent's place.
    let mut foreign = false;
    for z in start..=stop {
        let c = idx(z);
        if z > start {
            at = at.child(&w, (z, c));
            tally.add("child", true);
            if moving {
                match draw.next() % 8 {
                    0..=2 => {
                        near = near
                            .saturating_sub(1 + draw.next() % 2)
                            .max(FIRST_CELL_ZOOM);
                        far = (far + 1).min(near);
                    }
                    3 | 4 => near = (near + 2 + draw.next() % 3).min(FINEST),
                    _ => {}
                }
                far = far.min(near);
            }
        }
        let zooms = [far, near];
        // Some nodes are handed the place where another descent's cells were held, as a word
        // kept in the node cache may name a brick that has since gone.
        if moving && *seen != Lookup::NONE && draw.next().is_multiple_of(16) {
            at.ground = *seen;
            foreign = true;
            tally.add("handed another descent's place", true);
        }
        let before = at;
        let mut resident = w;
        let got = world::node(&mut resident, &w, &w, w.pool, &mut at, (z, c), zooms, cuts);
        // Only a node handed what its parent gave it is asked again from the top. Another
        // descent's place is no state the node derives itself, so a node handed one, and every
        // node below it, is skipped.
        if z > start && !foreign {
            let mut top = Inheritance::of(&w, (z, c));
            let mut resident = w;
            let again = world::node(&mut resident, &w, &w, w.pool, &mut top, (z, c), zooms, cuts);
            tally.add("asked again from the top", true);
            if again != got {
                tally.add(OTHERWISE, true);
                eprintln!(
                    "answered otherwise from the top: z {z} {c:?} {zooms:?}: {again:?} against {got:?}, handed {:x?}",
                    before.words()
                );
            }
        }
        ask_log.push(
            (OP_NODE, z, c),
            zooms,
            before.words(),
            cuts,
            got.kind as u32,
            got.asset,
        );
        *seen = at.ground;
        tell(w, before, (z, c), zooms, cuts, got, at, tally);
    }
}

/// What the Rust reads of the ways at the cell `c` at `z` (`clipmap::way_at`), counted in
/// `t` by what it is; and in `seen` by the column, which records every way kind a cell of it held.
fn tell_way(w: World, (z, c): (Zoom, [i32; 3]), seen: &mut BTreeMap<[i32; 2], u32>, t: &mut Tally) {
    let mut place = Lookup::NONE;
    let (b, _, here) = clip::bound_over(&w, &mut place, [z, z], (z, [c[0], c[1]]));
    if !here {
        return;
    }
    let v = clip::way_at(&w, w.pool, place, b[1], (z, c));
    if v == NOTHING {
        return;
    }
    let s = 1 << (LEAF_CELL_ZOOM - z);
    let column = clip::cell(&w, Lookup::NONE, z, [c[0], c[1]]).1;
    let under = (c[2] + 1) * s <= clip::column_top(column);
    let kind = if v == Sentinel::Void as u32 {
        1
    } else if v == Ground::Building as u32 {
        2
    } else if under {
        3
    } else {
        4
    };
    t.add("way cell, a bore's air", kind == 1);
    t.add("way cell, a pier's", kind == 2);
    t.add("way cell, a bore's lining", kind == 3);
    t.add("way cell, a slab's or embankment's", kind == 4);
    *seen.entry([c[0], c[1]]).or_default() |= 1 << kind;
}

/// The tally's name for a node of a descent that answered otherwise when asked from the top
/// than from what its parent handed it (`descend`).
const OTHERWISE: &str = "answered otherwise from the top";

/// Counts the kinds of answer that the answer `got` falls among.
#[expect(clippy::too_many_arguments, reason = "an answer and what it was asked")]
fn tell(
    w: World,
    before: Inheritance,
    (z, c): (Zoom, [i32; 3]),
    range: [Zoom; 2],
    cuts: [Cut; 2],
    got: In<[u32; 2]>,
    at: Inheritance,
    t: &mut Tally,
) {
    let shard = at.shard;
    let v = answer::val(got.asset[0]);
    let is_cell = got.kind == Kind::Cell;
    // Compares the clip level this node asks with what the node above it found.
    let asked = clip::clip_zoom(if z >= range[1] { z } else { range[1] });
    let above = before.ground;
    if above != Lookup::NONE && asked <= z + clip::BRICK_MIP_LAST {
        t.add("asked coarser than above's cell", asked < above.cell_zoom());
        t.add("asked coarser than above's clip", asked < above.from());
        t.add("asked finer by 2 or more", asked >= above.cell_zoom() + 2);
    }
    if asked > z + clip::BRICK_MIP_LAST && !UNLAID.contains(&asked) {
        let up = asked - z;
        let g = [
            wrap_idx(asked, c[0] << up) >> BRICK_SHIFT,
            (c[1] << up) >> BRICK_SHIFT,
        ];
        let (first, side) = (level_first(EYE, asked), 1 << (up - clip::BRICK_MIP_LAST));
        let d = [
            wrap_idx(brick::bricks_zoom(asked), g[0] - first[0]),
            g[1] - first[1],
        ];
        t.add("a block ending at its level's first, x", d[0] + side == 0);
        t.add("a block ending at its level's first, y", d[1] + side == 0);
        t.add("a block starting on its level's first, x", d[0] == 0);
        t.add("a block starting on its level's first, y", d[1] == 0);
    }
    t.add(
        "asked of a level not laid",
        asked > z + clip::BRICK_MIP_LAST && UNLAID.contains(&asked),
    );
    if (MIN_ZOOM..SHARD_ZOOM).contains(&z) && got.kind == Kind::Split {
        let mut place = before.ground;
        let coarsest = ground::coarsest(z, range[0]).min(range[1]);
        let cells = [coarsest, range[1]];
        let (b, _, here) = clip::bound_over(&w, &mut place, cells, (z, [c[0], c[1]]));
        t.add(
            "wider than a shard, all under the sea",
            here && b[0][1] < SEA_LEVEL,
        );
    }
    if z >= LEAF_CELL_ZOOM && is_cell && !shard.past() {
        let leaf = [c[0] >> (z - LEAF_CELL_ZOOM), c[1] >> (z - LEAF_CELL_ZOOM)];
        for (name, corner) in CORNERS {
            let on = wrap_idx(LEAF_CELL_ZOOM, corner[0]) == leaf[0] && corner[1] == leaf[1];
            t.add(name, on && area::is_wall(shard, z, [c[0], c[1]]));
        }
    }
    t.add(
        "past the leaves, the node 0, 0, 0, coarser than asked",
        z >= LEAF_CELL_ZOOM && c == [0, 0, 0] && z < range[1],
    );
    t.add("past the poles or the world's height", {
        let row = 1 << (z.max(1) - 1);
        let side = if z > SHARD_ZOOM {
            1 << (z - SHARD_ZOOM - 1)
        } else {
            1
        };
        !(-row..row).contains(&c[1]) || !(-side..side).contains(&c[2])
    });
    if got.kind != Kind::Cell && z >= MIN_ZOOM && !is_cell {
        let (mut resident, mut place) = (w, before.ground);
        let (i, _, bound) = ground::node(&mut resident, shard, &mut place, (z, c), range, false);
        let ways = way::slab(bound, [ground::coarsest(z, range[0]), range[1]]);
        t.add(
            "ways' span rounded to none",
            bound[0] < bound[1] && ways[0] == ways[1],
        );
        t.add(
            "ways' span rounded to none, strictly in the node",
            bound[0] < bound[1] && ways[0] == ways[1] && ground::meets(ways, (z, c[2]), range[1]),
        );
        let z_first = got.asset[1] >> 10 & 1023;
        t.add(
            "split, no ground, ways' and placed's firsts over 0",
            got.kind == Kind::Split
                && i.kind == Kind::Cell
                && at.object.is_folded()
                && ways[0] < ways[1]
                && z_first > 0
                && z_first < answer::BOX_STEPS / 2,
        );
    }
    if is_cell && at.object.is_folded() {
        let word = w.data(at.object.slot());
        t.add(
            "a fold of none, of a kind",
            object::high(word) == 0 && object::low(word) != Sentinel::Void as u32,
        );
    }
    t.add("split", got.kind == Kind::Split);
    t.add(
        "split, a box short of whole",
        got.kind == Kind::Split && got.asset != answer::BOX_WHOLE,
    );
    t.add("lack", got.kind == Kind::Lack);
    t.add("cell", is_cell);
    t.add(
        "cell, air",
        is_cell && v == Sentinel::Void as u32 && got.asset[0] == 0,
    );
    if is_cell && answer::is_placed(got.asset[0]) && z >= FIRST_CELL_ZOOM && !shard.past() {
        let (mut resident, mut place) = (w, before.ground);
        let (i, _, _) = ground::node(&mut resident, shard, &mut place, (z, c), range, true);
        t.add("cell over a lack, placed", i.kind == Kind::Lack);
    }
    t.add("cell, placed", is_cell && answer::is_placed(got.asset[0]));
    t.add(
        "cell, placed air",
        is_cell && answer::is_placed(got.asset[0]) && v == Sentinel::Air as u32,
    );
    t.add("cell, an object's", is_cell && value::is_object(v));
    let placed = is_cell && answer::is_placed(got.asset[0]);
    t.add(
        "cell, placed, the first object's",
        placed && v == value::OBJECT[0],
    );
    t.add(
        "cell, placed, the first ground's",
        placed && v == value::GROUND[0],
    );
    t.add(
        "cell, kept short of its cube",
        is_cell && answer::fill_eighths(got.asset[0]) < 8 && v != Sentinel::Void as u32,
    );
    t.add("cell, a way's", is_cell && value::is_transport(v));
    t.add("cell, stone", is_cell && v == Ground::Stone as u32);
    t.add("cell, soil", is_cell && v == Ground::Soil as u32);
    t.add("cell, sandstone", is_cell && v == Ground::Sandstone as u32);
    t.add("cell, unknown", is_cell && v == Sentinel::Unknown as u32);
    t.add(
        "cell, coarser than asked, past the leaves",
        is_cell && z >= LEAF_CELL_ZOOM && z < range[1] && v != Sentinel::Void as u32,
    );
    t.add(
        "cell, four differ",
        is_cell && {
            let [a, b, c, d] = asset_ids(got.asset[1]);
            a != b || b != c || c != d
        },
    );
    t.add("past the area", shard.past());
    if z >= SHARD_ZOOM && is_cell {
        let wall = !shard.past() && area::is_wall(shard, z, [c[0], c[1]]);
        t.add("cell, on the wall", wall);
        t.add("cell, the wall's", wall && v == Sentinel::Unknown as u32);
    }
    // How much of the cell the caves took, from the ground's own answer.
    if is_cell && z >= FIRST_CELL_ZOOM && !shard.past() {
        let mut resident = w;
        let mut place = before.ground;
        let (i, _, _) = ground::node(&mut resident, shard, &mut place, (z, c), range, true);
        if i.kind == Kind::Cell {
            t.add("cell, some of it cave", i.asset[1] > 0 && i.asset[1] < FULL);
            t.add("cell, all of it cave", i.asset[1] >= FULL);
        }
    }
    if cuts != [Cut::NONE; 2] {
        let (mut resident, mut plain) = (w, before);
        let without = world::node(
            &mut resident,
            &w,
            &w,
            w.pool,
            &mut plain,
            (z, c),
            range,
            [Cut::NONE; 2],
        );
        t.add("a cut changed it", without != got);
    }
}

fn read(device: &wgpu::Device, queue: &wgpu::Queue, from: &wgpu::Buffer) -> Vec<u32> {
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: from.size(),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut e = device.create_command_encoder(&Default::default());
    e.copy_buffer_to_buffer(from, 0, &staging, 0, from.size());
    queue.submit([e.finish()]);
    staging
        .slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let v = staging.slice(..).get_mapped_range().unwrap();
    v.as_chunks::<4>()
        .0
        .iter()
        .map(|b| u32::from_le_bytes(*b))
        .collect()
}

/// The module of `source`. Its compiler messages are printed with their lines, and a module the
/// device refuses fails the test.
fn module_of(device: &wgpu::Device, label: &str, source: &str) -> wgpu::ShaderModule {
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let info = pollster::block_on(module.get_compilation_info());
    let refused = pollster::block_on(scope.pop()).is_some();
    let lines: Vec<&str> = source.lines().collect();
    for m in &info.messages {
        let line = m.location.map_or(0, |l| l.line_number as usize);
        eprintln!(
            "{label}: {:?} at {line}: {}\n  {}",
            m.message_type,
            m.message,
            lines.get(line.wrapping_sub(1)).copied().unwrap_or("")
        );
    }
    assert!(!refused, "{label} refused");
    module
}

#[test]
fn hand_world_answers_as_the_rust() {
    let built = real_world::build(EYE);
    let w = built.world();
    let (clipmap, ground_words, folds, shards, objects, pool) = (
        &built.clipmap,
        &built.ground,
        &built.folds,
        &built.shards,
        &built.objects,
        &built.pool,
    );
    let (way_points, placed, folded, lone, over_bridges, groundless) = (
        built.way_points.clone(),
        built.placed.clone(),
        built.folded.clone(),
        built.lone.clone(),
        built.over_bridges.clone(),
        built.groundless.clone(),
    );
    let mut draw = Draw(built.draw.0);
    let top_at = |g: [i32; 2]| built.top_at(g);

    // The points the descents are sent to, in leaf cells. Each group of points reaches some of
    // the kinds of answer a descent must meet, and some groups carry the zooms that meet them.
    let mut points: Vec<[i32; 3]> = Vec::new();
    let mut asked_at: Vec<([i32; 3], [Zoom; 2])> = Vec::new();
    // Points about the surface, at every scale the clip levels hold.
    for i in 0..3000 {
        let reach = 128 << (i % 12);
        let g = [
            EYE[0] + draw.within(-reach, reach),
            EYE[1] + draw.within(-reach, reach),
        ];
        points.push([g[0], g[1], top_at(g) + draw.within(-4, 4)]);
    }
    // Points in the caves: leaves the caves take, found by asking under the ground about the
    // eye.
    let mut caves = 0;
    for _ in 0..200_000 {
        let g = [
            EYE[0] + draw.within(-200, 200),
            EYE[1] + draw.within(0, 300),
        ];
        let z = top_at(g) - draw.within(4, 60);
        let c = [wrap_idx(LEAF_CELL_ZOOM, g[0]), g[1], z];
        if cave::hollow((LEAF_CELL_ZOOM, c)) >= FULL {
            points.push([g[0], g[1], z]);
            caves += 1;
            if caves == 800 {
                break;
            }
        }
    }
    // Points on the ways, about each line's middle. Each is asked at the zoom of the line's cells
    // alone, which is one clip level's.
    for &(cell_zoom, _, p) in &way_points {
        let s = 1 << (LEAF_CELL_ZOOM - cell_zoom);
        for _ in 0..12 {
            let d = [
                draw.within(-3, 3) * s,
                draw.within(-3, 3) * s,
                draw.within(-3, 2) * s,
            ];
            asked_at.push((
                [p[0] + d[0], p[1] + d[1], p[2] + d[2]],
                [cell_zoom, cell_zoom],
            ));
        }
    }
    for &p in &over_bridges {
        for d in -2..3 {
            asked_at.push(([p[0], p[1], p[2] + d], [LEAF_CELL_ZOOM - 1, LEAF_CELL_ZOOM]));
        }
    }
    for &p in &lone {
        asked_at.push((p, [20, 21]));
        asked_at.push((p, [18, 18 + draw.next() % 4]));
    }
    // Points at each corner of the notch.
    for (_, corner) in CORNERS {
        for _ in 0..150 {
            let leaves = LEAF_CELL_ZOOM + draw.next() % 4;
            let z = top_at(corner) + draw.within(-6, 3);
            asked_at.push(([corner[0], corner[1], z], [leaves - 1, leaves]));
        }
    }
    // Points past the poles, on rows whose low bits name a served row's key, and points past
    // the world's height.
    for _ in 0..1500 {
        let x = (-127 << 16) + draw.within(-20_000, 20_000);
        let row = 40 + draw.within(0, 4);
        let wide = if draw.next().is_multiple_of(2) {
            256
        } else {
            -256
        };
        let inside = (row << 16) + draw.within(0, 1 << 16);
        points.push([x, inside + (wide << 16), draw.within(-400, 400)]);
        let sign = if draw.next().is_multiple_of(2) { 1 } else { -1 };
        let z = sign * ((1 << 15) + draw.within(1, 8000));
        points.push([x, inside, z]);
    }
    // Points past the pole on the row whose low bits name row 0, beside the shard served there.
    for _ in 0..300 {
        let g = [
            draw.within(-1 << 16, 2 << 16),
            (256 << 16) + draw.within(0, 1 << 16),
        ];
        points.push([g[0], g[1], draw.within(-200, 200)]);
    }
    // Points under the sea, in nodes wider than a shard.
    for _ in 0..800 {
        let g = [
            [-128, -126, 126][draw.next() as usize % 3] << 16,
            [42, 43][draw.next() as usize % 2] << 16,
        ];
        let g = [
            g[0] + draw.within(0, 1 << 17),
            g[1] + draw.within(0, 1 << 16),
        ];
        let near = FIRST_CELL_ZOOM + draw.next() % 3;
        asked_at.push(([g[0], g[1], draw.within(-200, 200)], [near - 1, near]));
    }
    // Points in the world's first node along X and Y, asked at the finer zooms.
    for z in -2..3 {
        for near in [LEAF_CELL_ZOOM + 1, LEAF_CELL_ZOOM + 2, FINEST] {
            for _ in 0..12 {
                asked_at.push(([0, 0, z], [near - 1, near]));
            }
        }
    }
    // Points on the cells partly placed where no clip level holds the ground under them, and
    // on the cells that what is placed folds into, each asked as that cell.
    for &p in &groundless {
        asked_at.push((p, [LEAF_CELL_ZOOM - 1, LEAF_CELL_ZOOM]));
    }
    for &p in &folded {
        asked_at.push((p, [LEAF_CELL_ZOOM - 1, LEAF_CELL_ZOOM]));
    }
    // Points about the ways and what is placed, far over and under them.
    for _ in 0..1500 {
        let g = [
            EYE[0] + draw.within(-400, 400),
            EYE[1] + draw.within(0, 400),
        ];
        points.push([g[0], g[1], top_at(g) + draw.within(-2000, 2000)]);
    }
    // Points at the edges of every clip level, asked from nodes wider than a brick, which the
    // level's blocks reach or do not.
    for cell_zoom in FIRST_CELL_ZOOM..=LEAF_CELL_ZOOM {
        let s = LEAF_CELL_ZOOM - cell_zoom;
        let row = clipmap[(ORIGIN_AT + cell_zoom - FIRST_CELL_ZOOM) as usize];
        let (lo, side) = ([row[0], row[1]], ACROSS as i32);
        for _ in 0..500 {
            let along = |d: &mut Draw, v: i32| ((v + d.within(-2, side + 2)) << BRICK_SHIFT) << s;
            let edge = |d: &mut Draw, v: i32| {
                let k = d.within(0, 2) * side + d.within(-2, 2);
                ((v + k) << BRICK_SHIFT) << s
            };
            for g in [
                [along(&mut draw, lo[0]), edge(&mut draw, lo[1])],
                [edge(&mut draw, lo[0]), along(&mut draw, lo[1])],
            ] {
                let far = cell_zoom.saturating_sub(draw.next() % 3);
                let g = [wrap_idx(LEAF_CELL_ZOOM, g[0]), g[1]];
                let z = top_at(g) + 40 + draw.within(0, 1000);
                asked_at.push(([g[0], g[1], z], [far, cell_zoom]));
            }
        }
    }
    // Points under the ground about the eye: its strata at the leaves, and the caves' share of
    // coarser cells.
    for _ in 0..3000 {
        let g = [
            EYE[0] + draw.within(-250, 250),
            EYE[1] + draw.within(0, 250),
        ];
        let leaves = LEAF_CELL_ZOOM + draw.next() % 4;
        asked_at.push((
            [g[0], g[1], top_at(g) - draw.within(1, 70)],
            [leaves - 1, leaves],
        ));
        let g = [
            EYE[0] + draw.within(-4000, 4000),
            EYE[1] + draw.within(0, 4000),
        ];
        let near = 17 + draw.next() % 7;
        asked_at.push((
            [g[0], g[1], top_at(g) - draw.within(16, 300)],
            [near - 2, near],
        ));
    }
    // Points on what is placed.
    for &(zoom, c) in placed.iter().step_by(2) {
        let leaf = |v: i32| {
            if zoom >= LEAF_CELL_ZOOM {
                v >> (zoom - LEAF_CELL_ZOOM)
            } else {
                v << (LEAF_CELL_ZOOM - zoom)
            }
        };
        points.push([leaf(c[0]), leaf(c[1]), leaf(c[2]) + draw.within(-1, 2)]);
    }
    // Points on the wall along the area's south edge, and past it.
    for _ in 0..4000 {
        let g = [
            EYE[0] + draw.within(-250, 250),
            (40 << 16) + draw.within(-1, 2),
        ];
        let leaves = LEAF_CELL_ZOOM + draw.next() % 4;
        asked_at.push((
            [g[0], g[1], top_at(g) + draw.within(-6, 3)],
            [leaves - 1, leaves],
        ));
    }
    // Points on the wall along its east and north edges, far enough from the eye that a coarser
    // clip level's bricks hold them.
    for _ in 0..2000 {
        for g in [
            [
                (-124 << 16) + draw.within(-2, 2),
                EYE[1] + draw.within(-8000, 8000),
            ],
            [
                EYE[0] + draw.within(-8000, 8000),
                (44 << 16) + draw.within(-2, 2),
            ],
        ] {
            let leaves = LEAF_CELL_ZOOM + draw.next() % 4;
            asked_at.push((
                [g[0], g[1], top_at(g) + draw.within(-6, 3)],
                [leaves - 1, leaves],
            ));
        }
    }
    // Points past the area on every side, and inside it where no clip level holds it.
    for _ in 0..1000 {
        let row = if draw.next().is_multiple_of(2) {
            draw.within(-1 << 23, 40 << 16)
        } else {
            draw.within(-1 << 17, 1 << 17)
        };
        points.push([draw.next() as i32 >> 4, row, draw.within(-400, 400)]);
    }
    for _ in 0..800 {
        let g = [
            (110 << 16) + draw.within(-1 << 16, 1 << 16),
            (42 << 16) + draw.within(-1 << 16, 1 << 16),
        ];
        points.push([g[0], g[1], draw.within(-300, 300)]);
    }

    // Each point gets one descent, its zooms set by how far the point lies from the eye.
    let mut ask_log = AskLog::default();
    let mut tally = Tally::default();
    let mut seen = Lookup::NONE;
    let by_its_distance = points.iter().map(|p| {
        let off = (p[0] - EYE[0]).abs().max((p[1] - EYE[1]).abs()).max(1);
        let band = (32 - off.leading_zeros()).saturating_sub(8);
        (
            *p,
            (LEAF_CELL_ZOOM - band.min(LEAF_CELL_ZOOM - FIRST_CELL_ZOOM)).max(FIRST_CELL_ZOOM),
        )
    });
    let all: Vec<_> = by_its_distance
        .map(|(p, near)| (p, None, near))
        .chain(
            asked_at
                .iter()
                .map(|&(p, [far, near])| (p, Some(far), near)),
        )
        .collect();
    eprintln!("{} points", all.len());
    for (i, &(p, far, near)) in all.iter().enumerate() {
        let targeted = far.is_some();
        let near = if far.is_some() {
            near
        } else {
            match draw.next() % 4 {
                0 => FIRST_CELL_ZOOM + draw.next() % (FINEST - FIRST_CELL_ZOOM + 1),
                1 => LEAF_CELL_ZOOM + draw.next() % 4,
                _ => near,
            }
        };
        let far = far.unwrap_or(match draw.next() % 5 {
            0 => near,
            1 => near.saturating_sub(1),
            2 => near.saturating_sub(3),
            3 => near.saturating_sub(7),
            _ => 4,
        });
        let stop = (near + draw.next() % 2).min(FINEST);
        let point = [
            (p[0] << 3) + draw.within(0, 8),
            (p[1] << 3) + draw.within(0, 8),
            (p[2] << 3) + draw.within(0, 8),
        ];
        let ancestor = |z: Zoom| {
            let a = object::ancestor_at(FINEST, point, FINEST - z);
            [wrap_idx(z, a[0]), a[1], a[2]]
        };
        let carve = match i % 6 {
            1 | 2 => {
                let z = stop.saturating_sub(draw.next() % 3).max(1);
                Cut {
                    zoom: z,
                    at: ancestor(z),
                }
            }
            3 => {
                let z = (stop + 1 + draw.next() % 2).min(FINEST);
                Cut {
                    zoom: z,
                    at: ancestor(z),
                }
            }
            _ => Cut::NONE,
        };
        let hide = match i % 6 {
            2 => {
                let (zoom, at) = placed[draw.next() as usize % placed.len()];
                Cut { zoom, at }
            }
            4 => {
                // Hides the node over the point at a zoom a leaf is put at, which is a leaf
                // where one lies there.
                let z = [22, 24, 25, 26, FINEST][draw.next() as usize % 5];
                Cut {
                    zoom: z,
                    at: ancestor(z),
                }
            }
            5 => {
                let z = stop;
                Cut {
                    zoom: z,
                    at: ancestor(z),
                }
            }
            _ => Cut::NONE,
        };
        // Half the points sent to a kind of answer at its zooms keep those zooms at every step.
        let moving = !targeted || i % 2 == 0;
        // Most descents start from the world's root, and some from a node with nothing known
        // above it.
        let start = if i % 3 == 0 {
            draw.next() % (stop + 1)
        } else {
            0
        };
        descend(
            w,
            point,
            [start, stop],
            [far, near],
            moving,
            [carve, hide],
            &mut ask_log,
            &mut tally,
            &mut draw,
            &mut seen,
        );
    }
    // The caves' share alone (`cave::hollow`), asked of nodes at every zoom.
    let mut hollows = 0u32;
    for k in 0..6000u32 {
        let z = 14 + k % 14;
        let g = [
            EYE[0] + draw.within(-600, 600),
            EYE[1] + draw.within(-600, 600),
        ];
        let up = 24i32 - z as i32;
        let v = |x: i32| if up >= 0 { x >> up } else { x << -up };
        let c = [wrap_idx(z, v(g[0])), v(g[1]), v(draw.within(-300, 300))];
        let share = cave::hollow((z, c));
        hollows += (share > 0) as u32;
        ask_log.push(
            (OP_HOLLOW, z, c),
            [0, 0],
            [0; 3],
            [Cut::NONE; 2],
            0,
            [share, 0],
        );
    }
    tally.0.insert("a share of cave, asked apart", hollows);
    // For each pipe cell that a piece holds part of, every one of its 64 blocks two zooms finer
    // is asked apart.
    let mut partial = 0u32;
    for _ in 0..200_000 {
        if partial == 500 {
            break;
        }
        let c = [
            draw.within(-1 << 20, 1 << 20),
            draw.within(-1 << 20, 1 << 20),
            draw.within(-1 << 8, 1 << 8),
        ];
        let share = cave::hollow((cave::PIPE, c));
        if share == 0 || share >= FULL {
            continue;
        }
        partial += 1;
        for b in 0..64 {
            let d = [b & 3, b >> 2 & 3, b >> 4];
            let at = [4 * c[0] + d[0], 4 * c[1] + d[1], 4 * c[2] + d[2]];
            let z = cave::PIPE + 2;
            ask_log.push(
                (OP_HOLLOW, z, at),
                [0, 0],
                [0; 3],
                [Cut::NONE; 2],
                0,
                [cave::hollow((z, at)), 0],
            );
        }
    }
    tally
        .0
        .insert("pipe cells of a partial piece, every block", partial);

    // Every cell of the columns along the ways of a few bricks at the leaves and the zoom over
    // them, from under a bore's air to over a high deck, each asked of the world alone from
    // the top as a walk asks a cell, so a pier's station and stacked ways are met.
    let mut seen = BTreeMap::new();
    let sweep: Vec<_> = way_points
        .iter()
        .filter(|p| {
            let mut place = Lookup::NONE;
            let g = [
                wrap_idx(p.0, p.2[0] >> (LEAF_CELL_ZOOM - p.0)),
                p.2[1] >> (LEAF_CELL_ZOOM - p.0),
            ];
            let (b, _, here) = clip::bound_over(&w, &mut place, [p.0, p.0], (p.0, g));
            p.0 >= LEAF_CELL_ZOOM - 1 && here && place.from() == p.0 && b[1][0] < b[1][1]
        })
        .take(30)
        .collect();
    for &&(cell_zoom, _, p) in &sweep {
        let s = 1 << (LEAF_CELL_ZOOM - cell_zoom);
        for y in -6..=6 {
            for x in -40..40 {
                for k in (-34i32..75).step_by(s as usize) {
                    let g = [p[0] + x * s, p[1] + y * s];
                    let c = [
                        wrap_idx(cell_zoom, g[0] >> (LEAF_CELL_ZOOM - cell_zoom)),
                        g[1] >> (LEAF_CELL_ZOOM - cell_zoom),
                        (p[2] + k) >> (LEAF_CELL_ZOOM - cell_zoom),
                    ];
                    let mut at = Inheritance::of(&w, (cell_zoom, c));
                    let before = at;
                    let mut resident = w;
                    let zooms = [cell_zoom, cell_zoom];
                    let got = world::node(
                        &mut resident,
                        &w,
                        &w,
                        w.pool,
                        &mut at,
                        (cell_zoom, c),
                        zooms,
                        [Cut::NONE; 2],
                    );
                    ask_log.push(
                        (OP_NODE, cell_zoom, c),
                        zooms,
                        before.words(),
                        [Cut::NONE; 2],
                        got.kind as u32,
                        got.asset,
                    );
                    tell_way(w, (cell_zoom, c), &mut seen, &mut tally);
                }
            }
        }
    }
    let stacked = seen.values().filter(|&&b| b.count_ones() >= 2).count();
    tally
        .0
        .insert("way column, two kinds stacked", stacked as u32);

    let asked_n = ask_log.asked.len() / ASKED_WORDS;
    eprintln!("{asked_n} asks");
    for (name, n) in &tally.0 {
        eprintln!("  {n:>8}  {name}");
    }
    assert_eq!(
        tally.0.get(OTHERWISE).copied().unwrap_or(0),
        0,
        "a node answers as it does from the top off what its parent handed it"
    );

    let Some(gpu) = gpu::Gpu::new() else {
        eprintln!("no adapter: the hand query skipped");
        return;
    };
    let (device, queue) = (gpu.device, gpu.queue);

    // The same asks go to the query by hand, a chunk at a time.
    use wgpu::util::DeviceExt as _;
    let buf = |words: &[u32], usage: wgpu::BufferUsages| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            // SAFETY: every u32's bytes are u8s, in the slice's own lifetime.
            contents: unsafe { std::slice::from_raw_parts(words.as_ptr().cast(), words.len() * 4) },
            usage: usage | wgpu::BufferUsages::COPY_SRC,
        })
    };
    let flat = |rows: &[[u32; 4]]| rows.iter().flatten().copied().collect::<Vec<u32>>();
    let clip_flat: Vec<u32> = clipmap.iter().flatten().map(|&v| v as u32).collect();
    let store = wgpu::BufferUsages::STORAGE;
    let (clip_buf, ground_buf, folds_buf, shard_buf, placed_buf, pool_buf) = (
        buf(&clip_flat, store),
        buf(ground_words, store),
        buf(folds, store),
        buf(&flat(shards), store),
        buf(objects, store),
        buf(pool, store),
    );
    let source = WORLD_ASKED;
    let module = module_of(&device, "world", source);
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("world"),
        layout: None,
        module: &module,
        entry_point: Some("world_asked_cs"),
        compilation_options: Default::default(),
        cache: None,
    });
    const CHUNK: usize = 1 << 14;
    let (mut same, mut differ, mut first) = (0u64, 0u64, None);
    let mut differ_by_op = [0u32; 4];
    for (n, (a, t)) in ask_log
        .asked
        .chunks(CHUNK * ASKED_WORDS)
        .zip(ask_log.told.chunks(CHUNK * TOLD_WORDS))
        .enumerate()
    {
        let asked_buf = buf(a, store);
        let told_buf = buf(&vec![0u32; t.len()], store);
        let entries = [
            (1, &clip_buf),
            (2, &ground_buf),
            (4, &folds_buf),
            (5, &shard_buf),
            (6, &placed_buf),
            (7, &pool_buf),
            (12, &asked_buf),
            (13, &told_buf),
        ]
        .map(|(b, buf): (u32, &wgpu::Buffer)| wgpu::BindGroupEntry {
            binding: b,
            resource: buf.as_entire_binding(),
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &entries,
        });
        let mut e = device.create_command_encoder(&Default::default());
        {
            let mut pass = e.begin_compute_pass(&Default::default());
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(((a.len() / ASKED_WORDS) as u32).div_ceil(64), 1, 1);
        }
        queue.submit([e.finish()]);
        let got = read(&device, &queue, &told_buf);
        for (k, (want, have)) in t.chunks(TOLD_WORDS).zip(got.chunks(TOLD_WORDS)).enumerate() {
            // Only the answer's kind and asset are compared. What a node hands its children
            // belongs to the query and is no part of the answer.
            if want[..3] == have[..3] {
                same += 1;
            } else {
                differ += 1;
                let ask = &a[k * ASKED_WORDS..(k + 1) * ASKED_WORDS];
                differ_by_op[ask[0] as usize] += 1;
                first.get_or_insert((n * CHUNK + k, ask.to_vec(), want.to_vec(), have.to_vec()));
            }
        }
    }
    eprintln!(
        "{same} answered alike, {differ} differ, of nodes {}, of hollows {}",
        differ_by_op[OP_NODE as usize], differ_by_op[OP_HOLLOW as usize]
    );
    if let Some((k, ask, want, have)) = &first {
        eprintln!("first differing, ask {k}:\n  asked {ask:?}\n  rust  {want:?}\n  hand  {have:?}");
    }
    assert_eq!(differ, 0, "the hand query differs");
    assert_eq!(same as usize, asked_n);

    // Each kind of answer must be met at least half as often as this seeded run meets it.
    let least = [
        ("a block ending at its level's first, x", 462),
        ("a block ending at its level's first, y", 3040),
        ("a block starting on its level's first, x", 3298),
        ("a block starting on its level's first, y", 16412),
        ("a cut changed it", 17392),
        ("a fold of none, of a kind", 514),
        ("a share of cave, asked apart", 1172),
        ("asked coarser than above's cell", 14194),
        ("asked coarser than above's clip", 8071),
        ("asked finer by 2 or more", 12882),
        ("asked of a level not laid", 41692),
        ("cell", 149937),
        ("cell over a lack, placed", 403),
        ("cell, a way's", 905),
        ("cell, air", 98194),
        ("cell, all of it cave", 773),
        ("cell, an object's", 7380),
        ("way cell, a bore's air", 1),
        ("way cell, a pier's", 1),
        ("way cell, a bore's lining", 1),
        ("way cell, a slab's or embankment's", 1),
        ("way column, two kinds stacked", 1),
        ("cell, coarser than asked, past the leaves", 5599),
        ("cell, four differ", 5351),
        ("cell, kept short of its cube", 3708),
        ("cell, on the wall", 3463),
        ("cell, placed", 22888),
        ("cell, placed air", 15209),
        ("cell, placed, the first ground's", 9),
        ("cell, placed, the first object's", 1),
        ("cell, sandstone", 56),
        ("cell, soil", 351),
        ("cell, some of it cave", 1602),
        ("cell, stone", 2457),
        ("cell, the wall's", 906),
        ("cell, unknown", 10461),
        ("child", 629146),
        ("handed another descent's place", 7843),
        ("lack", 1226),
        ("past the area", 139883),
        ("past the leaves, the node 0, 0, 0, coarser than asked", 11),
        ("past the poles or the world's height", 22339),
        ("pipe cells of a partial piece, every block", 250),
        ("root", 36391),
        ("split", 514374),
        ("split, a box short of whole", 170538),
        ("split, no ground, ways' and placed's firsts over 0", 4),
        ("wall by a diagonal, north-east", 191),
        ("wall by a diagonal, north-west", 191),
        ("wall by a diagonal, south-east", 189),
        ("wall by a diagonal, south-west", 194),
        ("ways' span rounded to none", 437),
        ("ways' span rounded to none, strictly in the node", 182),
        ("wider than a shard, all under the sea", 113),
    ];
    for (name, n) in least {
        let got = tally.0.get(name).copied().unwrap_or(0);
        assert!(got >= n, "too few of {name} to say: {got}, wanted {n}");
    }
}
