//! Decor's passes by hand (frame/shader/decor_*.wgsl) must answer as the
//! Rust does, over the real world of tests/real_world. Each record a thread leaves must equal,
//! word for word, the record `generation::placement` stands through the real walk
//! (`support::column_surface`). The pass reads a column's surface off the column and walks
//! only a column under ways, so these tests hold that read to the walk, which owns the rule.

mod real_world;

mod gpu;

use pxl_spec::brick;
use pxl_spec::share::EIGHTHS;
use std::collections::{BTreeMap, BTreeSet};

use pxl_content::generation::{self, drawing, DecorQuery, BRICK_REC_CAP, REC_WORDS};
use pxl_content::{halve, scatter};
use pxl_engine::answer::{self, In};
use pxl_engine::cast::Line;
use pxl_engine::cast::{Detail, End, Query, Spacing};
use pxl_engine::clipmap as clip;
use pxl_engine::clipmap::{Lookup, ENTRIES, FIRST_CELL_ZOOM};
use pxl_engine::ground::Resident;
use pxl_engine::light::{self, Light};
use pxl_engine::object::{self, Cut};
use pxl_engine::seed::node_seed;
use pxl_engine::support;
use pxl_engine::world::{self, Inheritance, PlacedCursor};
use pxl_engine::{beam, eye, ray};
use pxl_render::frame::shader::record::{
    src, SHADER_GENERATION, SHADER_GENERATION_ARGS, SHADER_GENERATION_CNT,
    SHADER_GENERATION_COMPACT, SHADER_GENERATION_MARK, SHADER_GENERATION_THIN,
    SHADER_LIGHT_MARK_DRAWS, SHADER_LIGHT_MARK_DRAWS_OPEN,
};
use pxl_render::frame::shader::word::{instance, ROW_LANES, SQUARE_SIDE};
use pxl_spec::asset::decor::{Decor, Orientation, DECORS, ROWS};
use pxl_spec::brick::{BRICK, BRICK_CELLS};
use pxl_spec::space::coord::q3::FRAC_W;
use pxl_spec::space::coord::world::wrap_idx;
use pxl_spec::space::coord::zoom::{Zoom, LEAF_CELL_ZOOM};
use pxl_spec::space::layer::area;
use pxl_spec::value;
use wgpu::util::DeviceExt;

use real_world::{mix, Built, World};

/// How many answers fell into each kind, by the kind's name.
#[derive(Default)]
struct Tally(BTreeMap<&'static str, u64>);

impl Tally {
    fn add(&mut self, name: &'static str, yes: bool) {
        *self.0.entry(name).or_default() += yes as u64;
    }
    fn merge(&mut self, other: Tally) {
        for (k, v) in other.0 {
            *self.0.entry(k).or_default() += v;
        }
    }
}

/// The world as decor's generation walks it, with nothing placed handed down from the root.
/// It is the test's reference.
struct Ground<'w> {
    w: World<'w>,
    cuts: [Cut; 2],
}

impl Query for Ground<'_> {
    type At = Inheritance;

    fn root(&mut self, _: [i32; 3]) -> Inheritance {
        Inheritance {
            object: object::NodeSlot::NOTHING,
            shard: area::Shard::UNKNOWN,
            ground: Lookup::NONE,
        }
    }

    fn child(&mut self, at: &Inheritance, zoom: Zoom, node: [i32; 3]) -> Inheritance {
        at.child(&self.w, (zoom, node))
    }

    fn node(
        &mut self,
        at: &mut Inheritance,
        zoom: Zoom,
        c: [i32; 3],
        zooms: [Zoom; 2],
        _: f32,
    ) -> In<[u32; 2]> {
        let mut resident = self.w;
        world::node(
            &mut resident,
            &self.w,
            &self.w,
            self.w.pool,
            at,
            (zoom, c),
            zooms,
            self.cuts,
        )
    }
}

/// `generation::DecorQuery` over the real world, the test's reference, which tallies what it
/// answers.
struct Generating<'w, 't> {
    w: World<'w>,
    cuts: [Cut; 2],
    tally: &'t mut Tally,
    /// Counts kept through `next_placed`, which takes `&self`: how often it was asked, how many
    /// decors and how many other things it met, and how many decors left out it met.
    /// `last_decor` is the last decor it met.
    placed: std::cell::Cell<[u64; 4]>,
    last_decor: std::cell::Cell<[i32; 3]>,
    /// The decors whose boxes took a record.
    claims: Vec<[i32; 3]>,
    /// Whether no clip level holds the column asked last.
    last_lacks: bool,
}

impl Generating<'_, '_> {
    /// What the base answers near `f`, or with none the world as edited, tallied.
    fn ask(
        &mut self,
        f: Option<&support::ColumnSurface<Inheritance>>,
        zoom: Zoom,
        c: [i32; 3],
    ) -> u32 {
        let edited = f.is_none();
        let root_at = object::ancestor_at(zoom, c, zoom);
        let (cuts, root) = if edited {
            (self.cuts, Inheritance::of(&self.w, (0, root_at)))
        } else {
            ([Cut::NONE; 2], f.unwrap().line.ancestor(0))
        };
        // Where the ask is edited, the cell's root alone; else the column surface's line.
        let line = if edited {
            Line::of_root(root_at, root)
        } else {
            f.unwrap().line
        };
        let mut ground = Ground { w: self.w, cuts };
        let v = support::asset(&mut ground, &line, zoom, c);
        let lacks = !clip::step(&self.w, Lookup::NONE, zoom, [c[0], c[1]])
            .0
            .here();
        self.last_lacks = lacks;
        self.tally.add("ask: a column not held", lacks);
        self.tally
            .add("edit: asked over a column not held", edited && lacks);
        self.tally.add("ask: of the world unplaced", !edited);
        self.tally.add("ask: as edited", edited);
        if edited && answer::is_placed(v) {
            self.tally
                .add("edit: air takes it", answer::val(v) == value::AIR);
            self.tally.add(
                "edit: something placed takes it",
                answer::val(v) != value::AIR,
            );
            self.tally
                .add("edit: part of its cube", answer::fill_eighths(v) < EIGHTHS);
            self.tally
                .add("edit: at a coarse zoom", zoom < LEAF_CELL_ZOOM);
            let under = Inheritance::of(&self.w, (zoom, c));
            self.tally
                .add("edit: a cut takes it", !under.object.holds());
        }
        if edited && !answer::is_placed(v) {
            let under = Inheritance::of(&self.w, (zoom, c));
            self.tally
                .add("edit: something placed takes nothing", under.object.holds());
            self.tally.add(
                "edit: under an eighth, or air on a lack",
                under.object.holds() && under.object.is_folded(),
            );
        }
        if edited {
            let under = Inheritance::of(&self.w, (zoom, c));
            let air_cut = cuts[0].zoom != Cut::NONE.zoom;
            self.tally.add(
                "edit: no tree there, no ask by hand",
                !under.object.holds() && !air_cut,
            );
        }
        v
    }
}

impl DecorQuery for Generating<'_, '_> {
    type ColumnSurface = support::ColumnSurface<Inheritance>;

    fn column_surface(
        &mut self,
        cell_zoom: Zoom,
        g: [i32; 2],
    ) -> (Self::ColumnSurface, (i32, f32), bool) {
        let mut ground = Ground {
            w: self.w,
            cuts: [Cut::NONE; 2],
        };
        let f = support::column_surface(&mut ground, cell_zoom, g);
        self.tally.add("surface: columns walked", true);
        assert!(
            f.end != End::Lacks,
            "a listed pair's brick holds its leaves' columns"
        );
        self.tally.add("surface: nothing fills", f.end == End::Miss);
        let met = f.end == End::Hit;
        let mut resident = self.w;
        let wrapped = wrap_idx(cell_zoom, g[0]);
        let shard = area::Shard::UNKNOWN.at(&self.w, (cell_zoom, [wrapped, g[1]]));
        self.tally.add("surface: past the area", shard.past());
        if met && !shard.past() {
            let col = resident.cell(shard, Lookup::NONE, cell_zoom, [wrapped, g[1], f.top.0]);
            self.tally
                .add("surface: the ground's top", f.top.0 == col.top - 1);
            self.tally
                .add("surface: over the ground, a way", f.top.0 >= col.top);
            self.tally.add(
                "surface: under the ground's top, a bore",
                f.top.0 < col.top - 1,
            );
            self.tally.add(
                "surface: a way on a void column",
                col.asset_id == value::VOID,
            );
            self.tally.add("surface: a part of its cube", f.top.1 < 1.0);
            self.tally
                .add("surface: a wall", col.asset_id == value::UNKNOWN);
            // What the pass by hand asks of the world: where ways bound the column, its cells
            // down to the surface, and otherwise nothing.
            let (b, _, _) = clip::bound_over(
                &resident,
                &mut Lookup::NONE.clone(),
                [cell_zoom, cell_zoom],
                (cell_zoom, [wrapped, g[1]]),
            );
            let [lo, hi] = b[1];
            if lo < hi {
                let start = ((hi - 1) >> (LEAF_CELL_ZOOM - cell_zoom)).max(col.top - 1);
                self.tally.add("hand: columns walked", true);
                *self.tally.0.entry("hand: cells walked").or_default() +=
                    (start - f.top.0 + 1) as u64;
                // The cell the walk met, asked as the walk asked it.
                let mut ground = Ground {
                    w: self.w,
                    cuts: [Cut::NONE; 2],
                };
                let top = [wrapped, g[1], f.top.0];
                let eighths =
                    answer::fill_eighths(support::asset(&mut ground, &f.line, cell_zoom, top));
                self.tally
                    .add("walk: met a part of its cube", eighths < EIGHTHS);
                self.tally
                    .add("walk: met an odd part of its cube", eighths % 2 == 1);
            }
        }
        if f.end == End::Miss && !shard.past() {
            let col = resident.cell(shard, Lookup::NONE, cell_zoom, [wrapped, g[1], 0]);
            self.tally
                .add("surface: void column, no way", col.asset_id == value::VOID);
        }
        (f, f.top, met)
    }

    fn finest(&mut self, cell_zoom: Zoom, g: [i32; 2]) -> Zoom {
        let (f, _) = clip::step(&self.w, Lookup::NONE, LEAF_CELL_ZOOM, g);
        assert!(
            f.here() && f.from() >= cell_zoom,
            "a listed pair's brick holds its leaf columns, at its clip level or finer"
        );
        self.tally.add(
            "finest: a finer clip level holds the leaf column",
            f.from() > cell_zoom,
        );
        self.tally.add(
            "finest: its own clip level is the finest",
            f.from() == cell_zoom,
        );
        f.from()
    }

    fn asset(&mut self, f: &Self::ColumnSurface, zoom: Zoom, c: [i32; 3]) -> u32 {
        self.ask(Some(f), zoom, c)
    }

    fn edited(&mut self, zoom: Zoom, c: [i32; 3]) -> u32 {
        self.ask(None, zoom, c)
    }
    fn next_placed(&self, p: &mut PlacedCursor) -> ([i32; 3], u32, bool) {
        let (c, word, more) = p.next(&self.w);
        let mut t = self.placed.get();
        t[0] += 1;
        if more {
            let decor = Decor::find(object::low(word)).is_some();
            t[if decor { 1 } else { 2 }] += 1;
            t[3] += (decor && c == self.cuts[1].at && self.cuts[1].zoom == LEAF_CELL_ZOOM) as u64;
            if decor {
                self.last_decor.set(c);
            }
        }
        self.placed.set(t);
        (c, word, more)
    }
}

/// What a thread leaves of the record of decor `j` at `leaf`: its first word, the Z of its
/// box's lowest corner, and its sink.
fn stand(q: &mut Generating, j: u32, cell_zoom: Zoom, leaf: [i32; 2]) -> (u32, u32, u32) {
    let decor = &ROWS[j as usize];
    let c = scatter::candidate_at(j as usize, leaf);
    let placed_at = generation::placement(q, decor, cell_zoom, &c, true);
    let has_footing = placed_at.is_some();
    let none = generation::Prop {
        variant: 0,
        turn: 0,
        at: [0; 3],
        sink: 0,
    };
    let (g, generates) = placed_at.unwrap_or((none, false));
    let word = if generates {
        generation::rec_word(j, g.variant, g.turn, cell_zoom)
    } else {
        generation::UNGENERATED
    };
    let placed = q.placed.replace([0; 4]);
    q.tally.add("placed: a decor in reach", placed[1] > 0);
    q.tally
        .add("placed: something else in reach", placed[2] > 0);
    q.tally.add(
        "placed: a decor left out still claims",
        placed[3] > 0 && !generates,
    );
    q.tally.add(
        "placed: a decor's box takes it",
        placed[0] > 0 && !generates,
    );
    if placed[0] > 0 && !generates {
        q.claims.push(q.last_decor.get());
    }
    q.tally.add(
        "box: a column not held takes it",
        !generates && q.last_lacks,
    );
    q.tally.add("record: no footing", !has_footing);
    q.tally.add(
        "record: footing, nothing generates",
        has_footing && !generates,
    );
    q.tally.add("record: generates", generates);
    match j {
        0 => q.tally.add("generates: a tree", generates),
        1 => q.tally.add("generates: a boat", generates),
        2 => q.tally.add("generates: a car", generates),
        _ => q.tally.add("generates: a train", generates),
    }
    if generates {
        q.tally
            .add("generates: at the leaves", cell_zoom == LEAF_CELL_ZOOM);
        q.tally.add(
            "generates: at a coarser clip level",
            cell_zoom < LEAF_CELL_ZOOM,
        );
        let along = decor.orient == Orientation::along_transport;
        q.tally.add("vote: across", along && g.turn & 1 == 1);
        q.tally.add("vote: along", along && g.turn & 1 == 0);
        q.tally.add("vote: flipped", along && g.turn >= 2);
        q.tally.add("generates: sunk under its corner", g.sink != 0);
    }
    (word, g.at[2] as u32, g.sink)
}

/// One dispatch: its pairs, each an entry, a decor and whether it runs in part, and the frame's
/// boxes and cuts.
struct Round {
    name: &'static str,
    pairs: Vec<(u32, u32, bool)>,
    boxes: Vec<([i32; 4], Zoom)>,
    cuts: [Cut; 2],
}

/// The leaves a pair laid.
#[derive(Default, Clone)]
struct LeafList {
    leaves: Vec<[i32; 2]>,
}

/// What each pair holds. It starts with what `decor_thin_cs` lays, the candidates the hashes
/// give. Leaves about the places that make a case are added, up to the decor's room: any leaf
/// whose variant stands at the clip level, as a thread asks only what stands.
fn pairs_about(built: &Built, edges: &[[i32; 2]]) -> BTreeMap<(u32, u32), LeafList> {
    let mut pairs: BTreeMap<(u32, u32), LeafList> = BTreeMap::new();
    let cells = [22u32, 23, 24];
    for cell_zoom in cells {
        for e in 0..ENTRIES {
            let s = (cell_zoom - FIRST_CELL_ZOOM) * ENTRIES + e;
            let row = built.clipmap[s as usize];
            if row[2] == 0 {
                continue;
            }
            for (j, decor) in ROWS.iter().enumerate() {
                if cell_zoom < scatter::first_cell_zoom(decor) {
                    continue;
                }
                let [x0, y0, _, _] = generation::brick_rect(cell_zoom, [row[0], row[1]]);
                let t = LEAF_CELL_ZOOM - cell_zoom;
                let mut leaves = Vec::new();
                for i in 0..BRICK_CELLS {
                    let corner = [
                        x0 + ((i % BRICK) << t) as i32,
                        y0 + ((i / BRICK) << t) as i32,
                    ];
                    if scatter::cnt(j, cell_zoom, [corner[0] >> t, corner[1] >> t]) == 0 {
                        continue;
                    }
                    let side = 1u32 << t;
                    for n in 0..side * side {
                        let leaf = [corner[0] + (n % side) as i32, corner[1] + (n / side) as i32];
                        let c = scatter::candidate_at(j, leaf);
                        let here = scatter::cnt(j, LEAF_CELL_ZOOM, leaf) > 0;
                        if here
                            && scatter::stands_at(
                                decor,
                                scatter::variant_of(decor, c.seed),
                                cell_zoom,
                            )
                            && scatter::retained(j, &c)
                        {
                            leaves.push(leaf);
                        }
                    }
                }
                pairs.insert((s, j as u32), LeafList { leaves });
            }
        }
    }
    // Adds a leaf and the leaves about it to whichever pairs may hold them.
    let mut near = |at: [i32; 2], reach: i32, step: i32| {
        for cell_zoom in cells {
            let k = brick::BRICK_SHIFT + LEAF_CELL_ZOOM - cell_zoom;
            let mut dy = -reach;
            while dy <= reach {
                let mut dx = -reach;
                while dx <= reach {
                    let leaf = [real_world::wrap(at[0] + dx), at[1] + dy];
                    let q = [
                        wrap_idx(brick::bricks_zoom(cell_zoom), leaf[0] >> k),
                        leaf[1] >> k,
                    ];
                    let s = (cell_zoom - FIRST_CELL_ZOOM) * ENTRIES + clip::entry(q);
                    let row = built.clipmap[s as usize];
                    if row[2] != 0 && [row[0], row[1]] == q {
                        for (j, decor) in ROWS.iter().enumerate() {
                            let c = scatter::candidate_at(j, leaf);
                            let stands = scatter::stands_at(
                                decor,
                                scatter::variant_of(decor, c.seed),
                                cell_zoom,
                            );
                            if let Some(p) = pairs.get_mut(&(s, j as u32)) {
                                if stands
                                    && (p.leaves.len() as u32) < generation::cap(decor)
                                    && !p.leaves.contains(&leaf)
                                {
                                    p.leaves.push(leaf);
                                }
                            }
                        }
                    }
                    dx += step;
                }
                dy += step;
            }
        }
    };
    let every = |n: usize, by: usize| n.is_multiple_of(by);
    for (n, &(_, _, p)) in built.way_points.iter().enumerate() {
        if every(n, 4) {
            near([p[0], p[1]], 8, 2);
        }
    }
    for (n, p) in built.decors.iter().enumerate() {
        if every(n, 2) {
            near([p[0], p[1]], 3, 1);
        }
    }
    for (n, p) in built.folded.iter().enumerate() {
        if every(n, 2) {
            near([p[0], p[1]], 2, 1);
        }
    }
    for (n, &(zoom, c)) in built.placed.iter().enumerate() {
        if zoom >= 24 && every(n, 6) {
            let t = zoom - 24;
            near([c[0] >> t, c[1] >> t], 1, 1);
        }
    }
    for (n, p) in built.over_bridges.iter().enumerate() {
        if every(n, 2) {
            near([p[0], p[1]], 2, 1);
        }
    }
    for p in real_world::CORNERS.map(|c| c.1) {
        near(p, 6, 3);
    }
    near(real_world::NOTCH.map(|v| v << 16), 12, 4);
    for &p in edges {
        near(p, 6, 1);
    }
    pairs
}

fn bytes(words: &[u32]) -> &[u8] {
    // SAFETY: every u32's bytes are u8s, in the slice's own lifetime.
    unsafe { std::slice::from_raw_parts(words.as_ptr().cast(), words.len() * 4) }
}

/// How long a dispatch may take. Past it the test fails, and the label names the dispatch that
/// never ended.
const DISPATCH_LIMIT: std::time::Duration = std::time::Duration::from_secs(60);

fn read(device: &wgpu::Device, queue: &wgpu::Queue, from: &wgpu::Buffer, label: &str) -> Vec<u32> {
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
    let poll = device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(DISPATCH_LIMIT),
    });
    assert!(
        poll.is_ok(),
        "{label}: not done in {DISPATCH_LIMIT:?}: {poll:?}"
    );
    let v = staging.slice(..).get_mapped_range().unwrap();
    v.as_chunks::<4>()
        .0
        .iter()
        .map(|b| u32::from_le_bytes(*b))
        .collect()
}

/// A compute pipeline of `entry` in `source`. A module the device refuses fails the test.
fn pipeline(
    device: &wgpu::Device,
    label: &str,
    source: &str,
    entry: &str,
) -> wgpu::ComputePipeline {
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    assert!(pollster::block_on(scope.pop()).is_none(), "{label} refused");
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(label),
        layout: None,
        module: &module,
        entry_point: Some(entry),
        compilation_options: Default::default(),
        cache: None,
    })
}

/// The pipeline of shader `kind` as the renderer makes it, over the group the renderer states
/// (gpu/generator.rs) and not one derived from the module.
fn hand_pipeline(device: &wgpu::Device, kind: u32) -> wgpu::ComputePipeline {
    use pxl_render::api_spec::record::{STORAGE, STORAGE_RW, UNIFORM};
    use pxl_render::frame::shader::group::{
        ARGS_GROUP, CNT_GROUP, COMPACT_GROUP, GENERATE_GROUP, MARK_GROUP, THIN_GROUP,
    };
    let (group, entry): (&[u32], &str) = match kind {
        SHADER_GENERATION => (&GENERATE_GROUP, "decor_pose_cs"),
        SHADER_GENERATION_MARK => (&MARK_GROUP, "decor_mark_cs"),
        SHADER_GENERATION_THIN => (&THIN_GROUP, "decor_thin_cs"),
        SHADER_GENERATION_CNT => (&CNT_GROUP, "decor_cnt_cs"),
        SHADER_GENERATION_COMPACT => (&COMPACT_GROUP, "decor_compact_cs"),
        _ => (&ARGS_GROUP, "decor_args_cs"),
    };
    let entries: Vec<_> = group
        .chunks(3)
        .map(|w| wgpu::BindGroupLayoutEntry {
            binding: w[0],
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: match w[2] {
                    UNIFORM => wgpu::BufferBindingType::Uniform,
                    STORAGE => wgpu::BufferBindingType::Storage { read_only: true },
                    STORAGE_RW => wgpu::BufferBindingType::Storage { read_only: false },
                    k => panic!("a decor pass binds no kind {k}"),
                },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        })
        .collect();
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(entry),
        entries: &entries,
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(entry),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let source = src(kind);
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(entry),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(entry),
        layout: Some(&layout),
        module: &module,
        entry_point: Some(entry),
        compilation_options: Default::default(),
        cache: None,
    });
    if let Some(e) = pollster::block_on(scope.pop()) {
        panic!("{entry} refused over its group: {e}");
    }
    pipeline
}

fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let Some(gpu) = gpu::Gpu::new() else {
        eprintln!("no adapter: skipped");
        return None;
    };
    Some((gpu.device, gpu.queue))
}

/// The first word of the `i`-th record of decor `j` in entry `s`.
fn word_of(s: u32, j: u32, i: u32) -> usize {
    generation::rec_at(generation::entry_rec(&ROWS, s, j, i)) as usize
}

/// The GPU's side of a world: its buffers, and `decor_pose_cs` over them.
struct Gpu<'d> {
    device: &'d wgpu::Device,
    queue: &'d wgpu::Queue,
    clipmap: wgpu::Buffer,
    ground: wgpu::Buffer,
    shards: wgpu::Buffer,
    objects: wgpu::Buffer,
    pool: wgpu::Buffer,
    rows: wgpu::Buffer,
    hand: &'d wgpu::ComputePipeline,
}

impl Gpu<'_> {
    /// Runs `decor_pose_cs` over the records `init` for the `pairs` pairs `list` names, and returns
    /// the records.
    fn run(
        &self,
        label: &str,
        init: &[u32],
        list: &[u32],
        eye_words: &[u32],
        pairs: u32,
    ) -> Vec<u32> {
        let d = self.device;
        let store = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC;
        let make = |w: &[u32], usage: wgpu::BufferUsages| {
            d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytes(w),
                usage,
            })
        };
        let recs = make(init, store);
        let list = make(list, store);
        let eye_buf = make(eye_words, wgpu::BufferUsages::UNIFORM);
        let x = generation::DECOR_REC_CAP.div_ceil(generation::LANES);
        let mut e = d.create_command_encoder(&Default::default());
        let bind = |p: &wgpu::ComputePipeline, at: &[(u32, &wgpu::Buffer)]| {
            let entries: Vec<_> = at
                .iter()
                .map(|&(binding, b)| wgpu::BindGroupEntry {
                    binding,
                    resource: b.as_entire_binding(),
                })
                .collect();
            d.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &p.get_bind_group_layout(0),
                entries: &entries,
            })
        };
        {
            let mut pass = e.begin_compute_pass(&Default::default());
            let g = bind(
                self.hand,
                &[
                    (0, &eye_buf),
                    (1, &self.clipmap),
                    (2, &self.ground),
                    (5, &self.shards),
                    (6, &self.objects),
                    (7, &self.pool),
                    (9, &recs),
                    (10, &self.rows),
                    (11, &list),
                ],
            );
            pass.set_pipeline(self.hand);
            pass.set_bind_group(0, &g, &[]);
            pass.dispatch_workgroups(x, 1, pairs);
        }
        self.queue.submit([e.finish()]);
        read(d, self.queue, &recs, label)
    }
}

/// Whether one of the round's boxes reaches `leaf`, as decor_rect.wgsl `is_changed` decides.
fn is_changed(boxes: &[([i32; 4], Zoom)], cell_zoom: Zoom, leaf: [i32; 2]) -> bool {
    let at = [leaf[0], leaf[1], leaf[0] + 1, leaf[1] + 1];
    boxes
        .iter()
        .any(|&(b, z)| generation::reaches(b, z, cell_zoom, at))
}

/// What a round expects: the words `want` it must leave, starting from `init`, and the words
/// `free` that the test does not hold a thread to.
struct Expected {
    init: Vec<u32>,
    want: Vec<u32>,
    free: Vec<usize>,
    asked: u64,
    tally: Tally,
    claims: Vec<[i32; 3]>,
}

/// The words a round starts from: each pair's count, and its records. A whole pair's records
/// are as `decor_thin_cs` leaves them, and a pair in part holds what a frame before left.
fn init_of(laid: &BTreeMap<(u32, u32), LeafList>, round: &Round) -> Vec<u32> {
    let words = generation::rec_at(generation::ENTRY_RECS) as usize;
    let mut init = vec![0u32; words];
    for (&(s, j), p) in laid {
        init[generation::cnt_at(s, j) as usize] = p.leaves.len() as u32;
    }
    for &(s, j, part) in &round.pairs {
        let cell_zoom = clip::eye_cell_zoom(s / ENTRIES);
        for (i, leaf) in laid[&(s, j)].leaves.iter().enumerate() {
            let r = word_of(s, j, i as u32);
            let h = mix(i as u32 + 7, *leaf);
            // A whole pair's first word is whatever a frame before left, which thin does not
            // write. A pair in part holds what a frame before generated, or nothing generated.
            init[r] = if !part {
                h
            } else if h.is_multiple_of(5) {
                generation::UNGENERATED
            } else {
                generation::rec_word(j, h % 3, h >> 8 & 3, cell_zoom)
                    | (h >> 12 & 0x7f) << drawing::REGION_SHIFT
            };
            init[r + 1] = leaf[0] as u32;
            init[r + 2] = leaf[1] as u32;
            init[r + 3] = h >> 3;
            init[r + 4] = h >> 6;
        }
    }
    init
}

fn expect(built: &Built, laid: &BTreeMap<(u32, u32), LeafList>, round: &Round) -> Expected {
    let init = init_of(laid, round);
    let mut order: Vec<(u32, u32, bool)> = round.pairs.clone();
    order.sort_by_key(|p| p.2);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = order.len().div_ceil(threads).max(1);
    let w = built.world();
    let results: Vec<_> = std::thread::scope(|sc| {
        let handles: Vec<_> = order
            .chunks(chunk)
            .map(|pairs| {
                sc.spawn(move || {
                    let mut tally = Tally::default();
                    let mut out = Vec::new();
                    let mut asked = 0;
                    let mut claims = Vec::new();
                    for &(s, j, part) in pairs {
                        let cell_zoom = clip::eye_cell_zoom(s / ENTRIES);
                        let p = &laid[&(s, j)];
                        for (i, leaf) in p.leaves.iter().enumerate() {
                            if part && !is_changed(&round.boxes, cell_zoom, *leaf) {
                                tally.add("part: not reached, it stands", true);
                                continue;
                            }
                            tally.add("part: reached", part);
                            let mut q = Generating {
                                w,
                                cuts: round.cuts,
                                tally: &mut tally,
                                placed: Default::default(),
                                last_decor: Default::default(),
                                claims: Vec::new(),
                                last_lacks: false,
                            };
                            let (word, z, sink) = stand(&mut q, j, cell_zoom, *leaf);
                            claims.append(&mut q.claims);
                            asked += 1;
                            out.push((word_of(s, j, i as u32), word, z, sink));
                        }
                    }
                    (out, tally, asked, claims)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut want = init.clone();
    let mut free = Vec::new();
    let mut tally = Tally::default();
    let mut asked = 0;
    let mut claims = Vec::new();
    for (out, t, a, mut by) in results {
        claims.append(&mut by);
        tally.merge(t);
        asked += a;
        for (r, word, z, sink) in out {
            want[r] = word;
            if word == generation::UNGENERATED {
                free.push(r + 3);
                free.push(r + 4);
            } else {
                want[r + 3] = z;
                want[r + 4] = sink;
            }
        }
    }
    Expected {
        init,
        want,
        free,
        asked,
        tally,
        claims,
    }
}

/// The list a dispatch reads: its boxes, the whole pairs from the list's start, and the pairs
/// in part from its end.
fn list_of(round: &Round, pairs: &[(u32, u32, bool)]) -> (Vec<u32>, u32) {
    let mut list = vec![0u32; generation::LIST_WORDS as usize];
    let whole: Vec<_> = pairs.iter().filter(|p| !p.2).collect();
    list[generation::THIN_ARGS as usize + 2] = whole.len() as u32;
    list[generation::LIST_RECTS as usize] = round.boxes.len() as u32;
    for (k, &(b, z)) in round.boxes.iter().enumerate() {
        let at = generation::RECTS_AT as usize + k * generation::RECT_WORDS as usize;
        for a in 0..4 {
            list[at + a] = b[a] as u32;
        }
        list[at + 4] = z;
    }
    for (k, p) in whole.iter().enumerate() {
        list[generation::LIST_AT as usize + k] = generation::pair_word(p.0, p.1);
    }
    for (k, p) in pairs.iter().filter(|p| p.2).enumerate() {
        list[generation::part_at(k as u32) as usize] = generation::pair_word(p.0, p.1);
    }
    (list, pairs.len() as u32)
}

fn eye_words(cuts: [Cut; 2]) -> Vec<u32> {
    let mut words = vec![0u32; eye::WORDS];
    for (k, c) in [(eye::CARVE, cuts[0]), (eye::HIDE, cuts[1])] {
        words[k] = c.at[0] as u32;
        words[k + 1] = c.at[1] as u32;
        words[k + 2] = c.at[2] as u32;
        words[k + 3] = c.zoom;
    }
    words
}

/// Compares each word with what is expected, prints the first few that differ, and returns how
/// many differ.
fn differ(name: &str, by: &str, got: &[u32], e: &Expected) -> u64 {
    let mut skip = vec![false; got.len()];
    for &f in &e.free {
        skip[f] = true;
    }
    let (mut n, mut told) = (0, 0);
    for (k, (&g, &w)) in got.iter().zip(&e.want).enumerate() {
        if g != w && !skip[k] {
            n += 1;
            if told < 12 {
                told += 1;
                let (s, at) = if (k as u32) < generation::ENTRY_RECS_AT {
                    (
                        k as u32 / generation::HDR_WORDS,
                        k as u32 % generation::HDR_WORDS,
                    )
                } else {
                    let rec = (k as u32 - generation::ENTRY_RECS_AT) / REC_WORDS;
                    (
                        rec / BRICK_REC_CAP,
                        (k as u32 - generation::ENTRY_RECS_AT) % REC_WORDS,
                    )
                };
                eprintln!(
                    "{name} {by}: word {k}, entry {s}, at {at}: got {g:#x} want {w:#x} (was {:#x}), leaf {:?}",
                    e.init[k],
                    if (k as u32) >= generation::ENTRY_RECS_AT {
                        let r = k - at as usize;
                        [e.init[r + 1] as i32, e.init[r + 2] as i32]
                    } else {
                        [0, 0]
                    },
                );
            }
        }
    }
    n
}

/// Clears, at every zoom, the rows of the bricks east of some leaf bricks, so that no clip
/// level holds them and a box from the west runs onto a column not held. Returns points by
/// those edges for records to stand about.
fn lack_columns(built: &mut Built) -> Vec<[i32; 2]> {
    let mut edges = Vec::new();
    let k = |cell_zoom: Zoom| brick::BRICK_SHIFT + LEAF_CELL_ZOOM - cell_zoom;
    for e in (5..ENTRIES).step_by(13) {
        let row = built.clipmap[((LEAF_CELL_ZOOM - FIRST_CELL_ZOOM) * ENTRIES + e) as usize];
        if row[2] == 0 {
            continue;
        }
        let east = [
            (row[0] + 1) << k(LEAF_CELL_ZOOM),
            row[1] << k(LEAF_CELL_ZOOM),
        ];
        for cell_zoom in FIRST_CELL_ZOOM..=LEAF_CELL_ZOOM {
            let q = [
                wrap_idx(brick::bricks_zoom(cell_zoom), east[0] >> k(cell_zoom)),
                east[1] >> k(cell_zoom),
            ];
            built.clipmap[((cell_zoom - FIRST_CELL_ZOOM) * ENTRIES + clip::entry(q)) as usize][2] =
                0;
        }
        edges.push([real_world::wrap(east[0] - 1), east[1] + 20]);
        if edges.len() == 4 {
            break;
        }
    }
    edges
}

/// The leaf cells once round the world along X, a lap.
const LAP: i32 = 1 << LEAF_CELL_ZOOM;

/// The pairs that hold leaves and whose bricks lie within `reach` of `p`, each run whole.
fn pairs_near(
    laid: &BTreeMap<(u32, u32), LeafList>,
    built: &Built,
    p: [i32; 2],
    reach: i32,
) -> Vec<(u32, u32, bool)> {
    laid.iter()
        .filter(|((s, _), l)| {
            let row = built.clipmap[*s as usize];
            let cell_zoom = clip::eye_cell_zoom(s / ENTRIES);
            let b = generation::brick_rect(cell_zoom, [row[0], row[1]]);
            !l.leaves.is_empty()
                && wrap_idx(LEAF_CELL_ZOOM, p[0] - b[0]) + reach >= 0
                && wrap_idx(LEAF_CELL_ZOOM, p[0] - b[2]) - reach < 0
                && p[1] + reach >= b[1]
                && p[1] - reach < b[3]
        })
        .map(|(&(s, j), _)| (s, j, false))
        .collect()
}

/// The rounds of a world: all of it whole, then cuts and boxes about what stood there.
fn rounds(
    built: &Built,
    laid: &BTreeMap<(u32, u32), LeafList>,
    stood: &[(u32, u32, [i32; 2], i32)],
    claims: &[[i32; 3]],
) -> Vec<Round> {
    let none = [Cut::NONE; 2];
    let mut out = Vec::new();
    let leaf_of = |n: usize| stood[n % stood.len()];
    // Rounds with a carve: a leaf cell cut in a record's box or at its base, or a coarser node
    // cut over it.
    for n in 0..12 {
        let (_, _, leaf, z) = leaf_of(n * 37 + 3);
        let at = |zoom: Zoom, dz: i32| Cut {
            zoom,
            at: {
                let t = LEAF_CELL_ZOOM - zoom;
                [wrap_idx(zoom, leaf[0] >> t), leaf[1] >> t, (z + dz) >> t]
            },
        };
        let cut = match n % 4 {
            0 => at(LEAF_CELL_ZOOM, 0),
            1 => at(LEAF_CELL_ZOOM, -1),
            2 => at(22, 0),
            _ => at(18, 0),
        };
        out.push(Round {
            name: "a cut over a record",
            pairs: pairs_near(laid, built, leaf, 8),
            boxes: Vec::new(),
            cuts: [cut, Cut::NONE],
        });
    }
    // Rounds with something placed left out. A placed decor left out still claims its box, and
    // another placed thing left out does not stand in the way.
    for n in 0..12 {
        let p = if claims.is_empty() {
            built.decors[n * 41 % built.decors.len()]
        } else {
            claims[n * 41 % claims.len()]
        };
        out.push(Round {
            name: "a placed decor left out",
            pairs: pairs_near(laid, built, [p[0], p[1]], 8),
            boxes: Vec::new(),
            cuts: [
                Cut::NONE,
                Cut {
                    zoom: LEAF_CELL_ZOOM,
                    at: p,
                },
            ],
        });
    }
    for n in 0..12 {
        let (zoom, c) = built.placed[n * 97 % built.placed.len()];
        let t = zoom.saturating_sub(LEAF_CELL_ZOOM);
        out.push(Round {
            name: "a placed leaf left out",
            pairs: pairs_near(laid, built, [c[0] >> t, c[1] >> t], 8),
            boxes: Vec::new(),
            cuts: [Cut::NONE, Cut { zoom, at: c }],
        });
    }
    // Rounds with pairs whole and in part in one dispatch, under boxes of every kind.
    for n in 0..8u32 {
        let (_, _, leaf, _) = leaf_of(n as usize * 113 + 11);
        let mut pairs = pairs_near(laid, built, leaf, 300);
        for (k, p) in pairs.iter_mut().enumerate() {
            p.2 = k % 3 != 0;
        }
        let around = |r: i32| [leaf[0] - r, leaf[1] - r, leaf[0] + r, leaf[1] + r];
        let boxes = match n {
            0 => vec![(around(4), generation::EVERY)],
            1 => vec![(around(30), 24), (around(3), 23), (around(1), 22)],
            2 => (0..21)
                .map(|k| {
                    let b = [
                        leaf[0] + k * 20,
                        leaf[1] - k * 9,
                        leaf[0] + k * 20 + 16,
                        leaf[1] - k * 9 + 8,
                    ];
                    (
                        b,
                        if k % 3 == 0 {
                            generation::EVERY
                        } else {
                            22 + k as u32 % 3
                        },
                    )
                })
                .collect(),
            // Laps of X: case 3 puts the box a lap west, and case 4 makes one the whole way
            // round.
            3 => vec![(
                [
                    leaf[0] - (1 << 24) - 2,
                    leaf[1] - 2,
                    leaf[0] - (1 << 24) + 2,
                    leaf[1] + 2,
                ],
                generation::EVERY,
            )],
            4 => vec![([0, leaf[1] - 3, 1 << 24, leaf[1] + 3], generation::EVERY)],
            // A box over a brick filled at clip level 22 alone, which no candidate at a finer
            // level reads.
            5 => {
                let k = brick::BRICK_SHIFT + LEAF_CELL_ZOOM - 22;
                let q = [leaf[0] >> k, leaf[1] >> k];
                vec![(generation::brick_rect(22, q), 22)]
            }
            // Case 6 puts the box a lap east, across the seam into the region. Case 7 puts one
            // box ending at the region's west edge a lap round, and one reaching a cell past it.
            6 => vec![(
                [
                    leaf[0] + LAP - 3,
                    leaf[1] - 3,
                    leaf[0] + LAP + 3,
                    leaf[1] + 3,
                ],
                generation::EVERY,
            )],
            7 => {
                let west = leaf[0] - generation::READ_REACH;
                vec![
                    (
                        [west + LAP - 6, leaf[1] - 1, west + LAP, leaf[1] + 1],
                        generation::EVERY,
                    ),
                    (
                        [west + LAP - 6, leaf[1] + 20, west + LAP + 1, leaf[1] + 22],
                        generation::EVERY,
                    ),
                ]
            }
            _ => vec![],
        };
        out.push(Round {
            name: "whole and in part",
            pairs,
            boxes,
            cuts: none,
        });
    }
    out
}

/// A world's buffers on the device, and `decor_pose_cs` over them.
fn world_gpu<'d>(
    device: &'d wgpu::Device,
    queue: &'d wgpu::Queue,
    hand: &'d wgpu::ComputePipeline,
    built: &Built,
) -> Gpu<'d> {
    let make = |w: &[u32]| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytes(w),
            usage: wgpu::BufferUsages::STORAGE,
        })
    };
    let clip_flat: Vec<u32> = built.clipmap.iter().flatten().map(|&v| v as u32).collect();
    let shards: Vec<u32> = built.shards.iter().flatten().copied().collect();
    Gpu {
        device,
        queue,
        clipmap: make(&clip_flat),
        ground: make(&built.ground),
        shards: make(&shards),
        objects: make(&built.objects),
        pool: make(&built.pool),
        rows: make(rows_words()),
        hand,
    }
}

/// A world as `run_world` asks it, built with `extra` placed and with some rows cleared so no
/// clip level holds them, and what its pairs lay.
fn world_of(eye: [i32; 2], extra: &[Leaf]) -> (Built, BTreeMap<(u32, u32), LeafList>) {
    let mut built = real_world::build_with(eye, 600, extra);
    let edges = lack_columns(&mut built);
    let laid = pairs_about(&built, &edges);
    (built, laid)
}

/// Runs a round through `decor_pose_cs`, a dispatch at a time, and compares the words it leaves
/// with the Rust's.
fn run_round(
    gpu: &Gpu,
    built: &Built,
    laid: &BTreeMap<(u32, u32), LeafList>,
    round: &Round,
    label: &str,
    tally: &mut Tally,
) -> (Expected, Vec<u32>) {
    let e = expect(built, laid, round);
    let mut sorted = round.pairs.clone();
    sorted.sort_by_key(|p| p.2);
    let eye_buf = eye_words(round.cuts);
    let mut hand = e.init.clone();
    for pairs in sorted.chunks(48) {
        let (list, n) = list_of(round, pairs);
        let at = format!(
            "{label}, {}, pairs {:?}",
            round.name,
            &pairs[..pairs.len().min(4)]
        );
        hand = gpu.run(&at, &hand, &list, &eye_buf, n);
    }
    let d = differ(round.name, "hand", &hand, &e);
    eprintln!("{label}, {}: {} asked; {d} differ", round.name, e.asked);
    *tally.0.entry("records asked").or_default() += e.asked;
    *tally.0.entry("words differing").or_default() += d;
    (e, hand)
}

/// A leaf placed: its zoom, its node and its word.
type Leaf = (Zoom, [i32; 3], u32);

/// A record that stood: its pair's entry and decor, its leaf, the Z of its box's lowest
/// corner, and its first word.
type StoodRecord = (u32, u32, [i32; 2], i32, u32);

/// What is placed in the boxes of records that stand at the leaves, and those records. Each
/// record of `cubed` gets a cube over its base. Each record of `folded` is a wide one, and gets
/// a cube a sixty-fourth of a cell in its first column's top cell and a cube at its second
/// column's base.
fn placed_in_boxes(stood: &[StoodRecord]) -> (Vec<Leaf>, Vec<StoodRecord>, Vec<StoodRecord>) {
    let object = object::leaf_word(value::OBJECTS[0], 0);
    let at_leaves: Vec<StoodRecord> = stood
        .iter()
        .copied()
        .filter(|r| clip::eye_cell_zoom(r.0 / ENTRIES) == LEAF_CELL_ZOOM)
        .collect();
    let mut extra = Vec::new();
    let cubed: Vec<StoodRecord> = at_leaves.iter().step_by(37).take(12).copied().collect();
    for &(_, _, leaf, z, _) in &cubed {
        extra.push((LEAF_CELL_ZOOM, [leaf[0], leaf[1], z], object));
    }
    let mut folded = Vec::new();
    for &r in at_leaves.iter().skip(5) {
        let (j, variant, turn, _) = generation::rec_of(r.4);
        let [w, d, h] = ROWS[j as usize].size(variant, turn);
        if w * d < 2 || folded.iter().any(|f: &StoodRecord| f.2 == r.2) {
            continue;
        }
        let (leaf, base) = (r.2, r.3 - 1);
        let top = base + h as i32;
        let fine = LEAF_CELL_ZOOM + 2;
        extra.push((fine, [leaf[0] << 2, leaf[1] << 2, top << 2], object));
        let next = if w >= 2 {
            [leaf[0] + 1, leaf[1]]
        } else {
            [leaf[0], leaf[1] + 1]
        };
        extra.push((LEAF_CELL_ZOOM, [next[0], next[1], base], object));
        folded.push(r);
        if folded.len() == 12 {
            break;
        }
    }
    (extra, cubed, folded)
}

/// The rounds over the records whose boxes hold something placed (`placed_in_boxes`): all of
/// them whole, then each of the first six of `cubed` with its cube left out.
fn placed_rounds(cubed: &[StoodRecord], folded: &[StoodRecord]) -> Vec<Round> {
    let pairs = |rs: &[StoodRecord]| -> Vec<(u32, u32, bool)> {
        let mut p: Vec<_> = rs.iter().map(|r| (r.0, r.1, false)).collect();
        p.sort_unstable();
        p.dedup();
        p
    };
    let mut all = cubed.to_vec();
    all.extend_from_slice(folded);
    let mut out = vec![Round {
        name: "placed in a record's box",
        pairs: pairs(&all),
        boxes: Vec::new(),
        cuts: [Cut::NONE; 2],
    }];
    for r in cubed.iter().take(6) {
        out.push(Round {
            name: "a cube in a record's box left out",
            pairs: pairs(&[*r]),
            boxes: Vec::new(),
            cuts: [
                Cut::NONE,
                Cut {
                    zoom: LEAF_CELL_ZOOM,
                    at: [r.2[0], r.2[1], r.3],
                },
            ],
        });
    }
    out
}

/// Runs the records whose boxes hold something placed (`placed_in_boxes`) on a world with it
/// placed. A cube over a record's base takes the record, and does not once it is left out. A
/// sixty-fourth of a cell in a box is no edit, and neither is a cube at a base, so those
/// records stand.
fn run_placed(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    hand: &wgpu::ComputePipeline,
    eye: [i32; 2],
    label: &str,
    tally: &mut Tally,
    stood: &[StoodRecord],
) {
    let (extra, cubed, folded) = placed_in_boxes(stood);
    let (built, laid) = world_of(eye, &extra);
    let gpu = world_gpu(device, queue, hand, &built);
    let word = |e: &Expected, r: &StoodRecord, k: usize| {
        let i = laid[&(r.0, r.1)]
            .leaves
            .iter()
            .position(|l| *l == r.2)
            .unwrap();
        e.want[word_of(r.0, r.1, i as u32) + k]
    };
    let rounds = placed_rounds(&cubed, &folded);
    let (e, _) = run_round(&gpu, &built, &laid, &rounds[0], label, tally);
    let taken = cubed
        .iter()
        .filter(|r| word(&e, r, 0) == generation::UNGENERATED)
        .count();
    let standing = folded
        .iter()
        .filter(|r| word(&e, r, 0) != generation::UNGENERATED)
        .count();
    *tally
        .0
        .entry("box: a cube over its base takes it")
        .or_default() += taken as u64;
    *tally
        .0
        .entry("box: a fold under an eighth and a cube at a base leave it")
        .or_default() += standing as u64;
    tally.merge(e.tally);
    for (r, round) in cubed.iter().zip(&rounds[1..]) {
        let (e, _) = run_round(&gpu, &built, &laid, round, label, tally);
        let back = word(&e, r, 0) != generation::UNGENERATED;
        *tally.0.entry("box: a cube left out leaves it").or_default() += back as u64;
        tally.merge(e.tally);
    }
}

fn run_world(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    hand: &wgpu::ComputePipeline,
    eye: [i32; 2],
    label: &str,
    tally: &mut Tally,
) -> Vec<StoodRecord> {
    let t0 = std::time::Instant::now();
    let (built, laid) = world_of(eye, &[]);
    let all_recs: usize = laid.values().map(|p| p.leaves.len()).sum();
    eprintln!(
        "{label}: {all_recs} records in {} pairs, built in {:?}",
        laid.len(),
        t0.elapsed()
    );
    let gpu = world_gpu(device, queue, hand, &built);
    let all: Vec<(u32, u32, bool)> = laid.iter().map(|(&(s, j), _)| (s, j, false)).collect();
    let first = Round {
        name: "every pair whole",
        pairs: all,
        boxes: Vec::new(),
        cuts: [Cut::NONE; 2],
    };
    let run = |round: &Round, tally: &mut Tally| -> (Vec<u32>, Vec<[i32; 3]>) {
        let (e, hand) = run_round(&gpu, &built, &laid, round, label, tally);
        tally.merge(e.tally);
        (hand, e.claims)
    };
    let (hand, claims) = run(&first, tally);
    // The records that stood, which the later rounds cut and box about.
    let mut stood = Vec::new();
    let mut words = Vec::new();
    for (&(s, j), p) in &laid {
        for i in 0..p
            .leaves
            .len()
            .min(generation::cap(&ROWS[j as usize]) as usize)
        {
            let r = word_of(s, j, i as u32);
            if hand[r] != generation::UNGENERATED {
                stood.push((s, j, p.leaves[i], hand[r + 3] as i32));
                words.push((s, j, p.leaves[i], hand[r + 3] as i32, hand[r]));
            }
        }
    }
    eprintln!("{label}: {} stood", stood.len());
    if !stood.is_empty() {
        for round in rounds(&built, &laid, &stood, &claims) {
            run(&round, tally);
        }
    }
    eprintln!("{label}: {:?}", t0.elapsed());
    words
}

/// `decor_pose_cs` by hand must leave the records `generation::placement` stands.
#[test]
fn hand_decor_answers_as_placement() {
    let Some((device, queue)) = device() else {
        return;
    };
    let hand = hand_pipeline(&device, SHADER_GENERATION);
    let mut tally = Tally::default();
    // Three eyes: east of the seam over the service area's south edge, west of the seam, and
    // about X = 0 south of the world's equator, where X and Y run negative, all past the area.
    for (eye, label) in [
        (real_world::EYE, "east of the seam"),
        ([(1 << 23) - 300, (40 << 16) + 30], "west of the seam"),
        ([300, -50_670], "past the area"),
    ] {
        let stood = run_world(&device, &queue, &hand, eye, label, &mut tally);
        if !stood.is_empty() {
            run_placed(&device, &queue, &hand, eye, label, &mut tally, &stood);
        }
    }
    for (k, v) in &tally.0 {
        eprintln!("{v:>12} {k}");
    }
    assert_eq!(tally.0["words differing"], 0);
    // A box's column is asked from its lowest cell up. On a column no clip level holds, that
    // cell reads void, and no decor stands on void, so the ask never reaches an edit there.
    assert!(ROWS.iter().all(|d| !d.allows(value::VOID)));
    assert_eq!(tally.0["edit: asked over a column not held"], 0);
    // A decor stands no coarser than two zooms over the leaves. There the ground's top, the
    // ways and the caves under a face keep an even share of a cube, so the walk meets no odd
    // share.
    assert!(ROWS
        .iter()
        .all(|d| scatter::first_cell_zoom(d) >= LEAF_CELL_ZOOM - 2));
    assert_eq!(tally.0["walk: met an odd part of its cube"], 0);
    let zero: Vec<_> = tally
        .0
        .iter()
        .filter(|(k, v)| {
            **v == 0
                && !k.starts_with("words differing")
                && **k != "edit: asked over a column not held"
                && **k != "walk: met an odd part of its cube"
        })
        .map(|(k, _)| *k)
        .collect();
    assert!(zero.is_empty(), "never reached: {zero:?}");
}

/// Dispatches `p` over the buffers `at`. The test waits on it, with a bound, when it reads a
/// buffer back (`read`).
fn dispatch(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    p: &wgpu::ComputePipeline,
    at: &[(u32, &wgpu::Buffer)],
    size: [u32; 3],
) {
    let entries: Vec<_> = at
        .iter()
        .map(|&(binding, b)| wgpu::BindGroupEntry {
            binding,
            resource: b.as_entire_binding(),
        })
        .collect();
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &p.get_bind_group_layout(0),
        entries: &entries,
    });
    let mut e = device.create_command_encoder(&Default::default());
    {
        let mut pass = e.begin_compute_pass(&Default::default());
        pass.set_pipeline(p);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(size[0], size[1], size[2]);
    }
    queue.submit([e.finish()]);
}

/// A storage buffer holding `w`, which a pass reads and writes and the test reads back.
fn buf(device: &wgpu::Device, w: &[u32]) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytes(w),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
    })
}

fn rows_words() -> &'static [u32] {
    // SAFETY: the rows are `repr(C)` words alone (pxl-spec asset/decor.spec.rs).
    unsafe { std::slice::from_raw_parts(ROWS.as_ptr().cast::<u32>(), size_of_val(&ROWS) / 4) }
}

/// The candidates of decor `j` in brick `q` at `cell_zoom` that stand clear there by
/// `scatter`'s rules. It is the test's reference for `decor_thin_cs`.
fn thin_leaves(j: usize, cell_zoom: Zoom, q: [i32; 2]) -> Vec<[i32; 2]> {
    let decor = &ROWS[j];
    let [x0, y0, _, _] = generation::brick_rect(cell_zoom, q);
    let t = LEAF_CELL_ZOOM - cell_zoom;
    let mut leaves = Vec::new();
    for i in 0..BRICK_CELLS {
        let corner = [
            x0 + ((i % BRICK) << t) as i32,
            y0 + ((i / BRICK) << t) as i32,
        ];
        if scatter::cnt(j, cell_zoom, [corner[0] >> t, corner[1] >> t]) == 0 {
            continue;
        }
        let side = 1u32 << t;
        for n in 0..side * side {
            let leaf = [corner[0] + (n % side) as i32, corner[1] + (n / side) as i32];
            let c = scatter::candidate_at(j, leaf);
            let here = scatter::cnt(j, LEAF_CELL_ZOOM, leaf) > 0;
            if here
                && scatter::stands_at(decor, scatter::variant_of(decor, c.seed), cell_zoom)
                && scatter::retained(j, &c)
            {
                leaves.push(leaf);
            }
        }
    }
    leaves
}

/// `decor_thin_cs` by hand must lay what the hashes hold, each pair's records in any order,
/// however its threads fall. The cases cover pairs at every clip level a decor stands at and at
/// one level past them, where none stand, in bricks either side of the seam and where X and Y
/// are negative.
#[test]
fn hand_thin_lays_as_scatter() {
    let Some((device, queue)) = device() else {
        return;
    };
    let hand = hand_pipeline(&device, SHADER_GENERATION_THIN);
    let words = generation::rec_at(generation::ENTRY_RECS) as usize;
    let size = |pairs: u32| [BRICK_CELLS.div_ceil(generation::LANES), 1, pairs];
    let (mut laid_total, mut pairs_total) = (0u64, 0u64);
    let (mut none_past, mut stale_hdrs) = (0u64, 0u64);
    for (name, eye) in [
        ("east of the seam", real_world::EYE),
        ("west of the seam", [(1 << 23) - 300, (40 << 16) + 30]),
        ("south", [300, -50_670]),
    ] {
        let built = real_world::build(eye);
        let rows = buf(&device, rows_words());
        let mut pairs = Vec::new();
        let mut init = vec![0u32; words];
        let mut want = BTreeMap::new();
        // Every decor at the clip levels about those it stands at.
        for cell_zoom in 21..=24u32 {
            for e in 0..ENTRIES {
                let s = (cell_zoom - FIRST_CELL_ZOOM) * ENTRIES + e;
                let row = built.clipmap[s as usize];
                if row[2] == 0 {
                    continue;
                }
                let h = generation::hdr_at(s) as usize;
                init[h] = row[0] as u32;
                init[h + 1] = row[1] as u32;
                init[h + 2] = 1;
                for j in 0..ROWS.len() {
                    let leaves = thin_leaves(j, cell_zoom, [row[0], row[1]]);
                    none_past += (cell_zoom == 21 && leaves.is_empty()) as u64;
                    want.insert((s, j as u32), leaves);
                    pairs.push((s, j as u32));
                }
            }
        }
        let read_laid = |got: &[u32], s: u32, j: u32| -> (u32, Vec<[i32; 2]>) {
            let cnt = got[generation::cnt_at(s, j) as usize];
            let mut leaves = Vec::new();
            for i in 0..cnt.min(generation::cap(&ROWS[j as usize])) {
                let r = word_of(s, j, i);
                leaves.push([got[r + 1] as i32, got[r + 2] as i32]);
            }
            leaves.sort();
            (cnt, leaves)
        };
        for chunk in pairs.chunks(64) {
            let mut list = vec![0u32; generation::LIST_WORDS as usize];
            list[generation::THIN_ARGS as usize + 2] = chunk.len() as u32;
            for (k, p) in chunk.iter().enumerate() {
                list[generation::LIST_AT as usize + k] = generation::pair_word(p.0, p.1);
            }
            let list = buf(&device, &list);
            {
                let recs = buf(&device, &init);
                {
                    let at = [(9, &recs), (10, &rows), (11, &list)];
                    dispatch(&device, &queue, &hand, &at, size(chunk.len() as u32));
                }
                let got = read(&device, &queue, &recs, name);
                for &(s, j) in chunk {
                    let mut leaves = want[&(s, j)].clone();
                    leaves.sort();
                    assert_eq!(
                        read_laid(&got, s, j),
                        (leaves.len() as u32, leaves),
                        "{name}, {s} {j}"
                    );
                    laid_total += want[&(s, j)].len() as u64;
                }
            }
            pairs_total += chunk.len() as u64;
        }
        // Heads that name the brick east of the one the entry's row holds, which no mark of
        // the frame writes. Thin must not read the row: it lays the candidates of the head's
        // brick.
        let stale: Vec<(u32, u32)> = pairs
            .iter()
            .copied()
            .filter(|&(s, j)| {
                let row = built.clipmap[s as usize];
                let cell_zoom = FIRST_CELL_ZOOM + s / ENTRIES;
                let east = thin_leaves(j as usize, cell_zoom, [row[0] + 1, row[1]]);
                !east.is_empty() && east != want[&(s, j)]
            })
            .step_by(53)
            .take(4)
            .collect();
        for &(s, j) in &stale {
            let row = built.clipmap[s as usize];
            let cell_zoom = FIRST_CELL_ZOOM + s / ENTRIES;
            let mut before = init.clone();
            before[generation::hdr_at(s) as usize] = (row[0] + 1) as u32;
            let mut list = vec![0u32; generation::LIST_WORDS as usize];
            list[generation::THIN_ARGS as usize + 2] = 1;
            list[generation::LIST_AT as usize] = generation::pair_word(s, j);
            let (list, recs) = (buf(&device, &list), buf(&device, &before));
            let at = [(9, &recs), (10, &rows), (11, &list)];
            dispatch(&device, &queue, &hand, &at, size(1));
            let got = read(&device, &queue, &recs, name);
            let mut east = thin_leaves(j as usize, cell_zoom, [row[0] + 1, row[1]]);
            east.sort();
            assert_eq!(
                read_laid(&got, s, j),
                (east.len() as u32, east),
                "{name}, {s} {j}: a head naming another brick"
            );
            stale_hdrs += 1;
        }
        eprintln!("{name}: {} pairs", pairs.len());
    }
    eprintln!(
        "thin: {pairs_total} pairs, {laid_total} laid, {none_past} past where they stand, {stale_hdrs} headers naming another brick"
    );
    assert!(laid_total > 0 && none_past > 0 && stale_hdrs > 0);
}

/// The test's reference for `decor_mark_cs`, which marks the words one entry after another by
/// `generation`'s rules. Its list follows the entries' order, and a pair's place in it carries
/// no meaning.
fn mark_rust(clipmap: &[[i32; 4]], recs: &mut [u32], list: &mut [u32], shown: &[u32]) {
    let boxes: Vec<([i32; 4], Zoom)> = (0..list[generation::LIST_RECTS as usize])
        .map(|k| {
            let b = (generation::RECTS_AT + generation::RECT_WORDS * k) as usize;
            (
                [
                    list[b] as i32,
                    list[b + 1] as i32,
                    list[b + 2] as i32,
                    list[b + 3] as i32,
                ],
                list[b + 4],
            )
        })
        .collect();
    for s in 0..generation::EYE_ENTRIES {
        let entry = clipmap[s as usize];
        if entry[2] == 0 {
            continue;
        }
        let (cell_zoom, q, h) = (
            clip::eye_cell_zoom(s / ENTRIES),
            [entry[0], entry[1]],
            generation::hdr_at(s) as usize,
        );
        let hdr = [recs[h], recs[h + 1], recs[h + 2]];
        let whole =
            list[generation::LIST_EVERY as usize] != 0 || !generation::generated_for(hdr, q);
        let at = generation::brick_rect(cell_zoom, q);
        if !whole
            && !boxes
                .iter()
                .any(|&(b, z)| generation::reaches(b, z, cell_zoom, at))
        {
            continue;
        }
        if whole {
            recs[h] = q[0] as u32;
            recs[h + 1] = q[1] as u32;
            recs[h + 2] = 1;
            for k in 0..ROWS.len() as u32 {
                recs[generation::cnt_at(s, k) as usize] = 0;
            }
        }
        for j in 0..ROWS.len() as u32 {
            if shown[j as usize] >> (cell_zoom - FIRST_CELL_ZOOM) & 1 != 0 {
                let cap = generation::LIST_WORDS - generation::LIST_AT;
                let all = list[generation::GENERATE_CNT as usize];
                list[generation::GENERATE_CNT as usize] += 1;
                if all >= cap {
                    continue;
                }
                list[generation::GENERATE_ARGS as usize + 2] = all + 1;
                let counted = if whole {
                    generation::THIN_CNT
                } else {
                    generation::LIST_PARTS
                } as usize;
                let n = list[counted];
                list[counted] += 1;
                if whole {
                    list[generation::THIN_ARGS as usize + 2] = n + 1;
                }
                let w = if whole {
                    generation::LIST_AT + n
                } else {
                    generation::part_at(n)
                };
                list[w as usize] = generation::pair_word(s, j);
            }
        }
    }
}

/// Whether workgroup `g` of a pass over the records reads them: the test's reference for
/// decor_run.wgsl `holds_recs`.
fn holds_recs(recs: &[u32], clipmap: &[[i32; 4]], g: u32) -> bool {
    if g >= generation::EYE_ENTRIES {
        return true;
    }
    let (h, q) = (generation::hdr_at(g) as usize, clipmap[g as usize]);
    q[2] != 0 && generation::generated_for([recs[h], recs[h + 1], recs[h + 2]], [q[0], q[1]])
}

/// The `k`-th run of records workgroup `g` reads: the test's reference for decor_run.wgsl
/// `run_of`.
fn run_of(recs: &[u32], g: u32, k: u32) -> (u32, u32) {
    if g < generation::EYE_ENTRIES {
        let at_run = k * generation::RUN;
        let (mut j, mut first) = (0, 0);
        while j + 1 < ROWS.len() as u32 && at_run >= first + generation::cap(&ROWS[j as usize]) {
            first += generation::cap(&ROWS[j as usize]);
            j += 1;
        }
        let laid = recs[generation::cnt_at(g, j) as usize];
        let past = at_run - first;
        let cnt = laid.saturating_sub(past);
        (
            generation::entry_rec(&ROWS, g, j, past),
            cnt.min(generation::RUN),
        )
    } else {
        let n = (g - generation::EYE_ENTRIES) * generation::RUNS + k;
        (
            generation::ENTRY_RECS + n,
            u32::from(n < recs[generation::PLACED_HDR as usize]),
        )
    }
}

/// The first word of each record a pass reads, a workgroup at a time, as `decor_cnt_cs` and
/// `decor_compact_cs` take them.
fn each_rec(recs: &[u32], clipmap: &[[i32; 4]], workgroups: u32) -> Vec<usize> {
    let mut out = Vec::new();
    for g in 0..workgroups {
        if holds_recs(recs, clipmap, g) {
            for k in 0..generation::RUNS {
                let (first, cnt) = run_of(recs, g, k);
                for n in 0..cnt {
                    out.push(generation::rec_at(first + n) as usize);
                }
            }
        }
    }
    out
}

/// What `decor_cnt_cs` is asked over: the eye and the drawing words, as a frame hands them.
#[derive(Clone)]
struct Sizing {
    name: &'static str,
    origin: [i32; 3],
    range: [Zoom; 2],
    drawing: Vec<u32>,
}

impl Sizing {
    fn eye(&self) -> Vec<u32> {
        let mut words = vec![0u32; eye::WORDS];
        for a in 0..3 {
            words[eye::ORIGIN + a] = self.origin[a] as u32;
        }
        words[eye::RANGE] = self.range[0];
        words[eye::RANGE + 1] = self.range[1];
        // What `light_mark_draws_cs` reads of the eye: where it looks, and the light's box.
        for (at, v) in [
            (eye::FORWARD, [0.0f32, 1.0, -0.2]),
            (eye::RIGHT, [1.0, 0.0, 0.0]),
            (eye::UP, [0.0, 0.2, 1.0]),
        ] {
            for a in 0..3 {
                words[at + a] = v[a].to_bits();
            }
        }
        words[eye::TAN] = 2.0f32.to_bits();
        words[eye::TAN + 1] = 2.0f32.to_bits();
        // A surface receives the map within 0.15 of the box's half-width, some 135 units, and
        // some of the decor drawn lies past that.
        let light = Light::of(
            [
                [900.0, 0.0, 0.0],
                [0.0, 900.0, 0.0],
                [0.0, 0.0, -900.0],
                [0.0, 0.0, 450.0],
            ],
            MAP_SIDE,
            [0, 0],
            0.15,
        );
        words[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS].copy_from_slice(&light.words());
        words
    }
}

/// The map's side `light_mark_draws_cs` is run at, and its cast's number.
const MAP_SIDE: u32 = 4096;
const CAST_NUMBER: u32 = 7;

/// Records as generation leaves them, with placed decor's records after the entries'. An
/// entry's head is held or stale. Each decor laid none, a few or its room. Each record
/// holds a place in its brick, a variant and a turn, or nothing generated. Returns the records
/// and the workgroups a pass over them takes.
fn recs_to_size(built: &Built, eye: [i32; 2], placed: u32) -> (Vec<u32>, u32) {
    let need = generation::ENTRY_RECS + placed.next_power_of_two().max(64);
    let mut recs = vec![0u32; generation::rec_at(need) as usize];
    let mut draw = real_world::Draw(0x0bad_cafe);
    for s in 0..generation::EYE_ENTRIES {
        let row = built.clipmap[s as usize];
        let cell_zoom = clip::eye_cell_zoom(s / ENTRIES);
        let h = generation::hdr_at(s) as usize;
        let stale = mix(s, [5, 5]).is_multiple_of(9);
        recs[h] = (row[0] + stale as i32) as u32;
        recs[h + 1] = row[1] as u32;
        recs[h + 2] = 1;
        let b = generation::brick_rect(cell_zoom, [row[0], row[1]]);
        for (j, d) in ROWS.iter().enumerate() {
            let cap = generation::cap(d);
            let laid = match draw.next() % 16 {
                0..=5 => 0,
                6 => cap,
                7 => cap + 5,
                8 => generation::RUN,
                9 => generation::RUN + 1,
                _ => draw.next() % 40,
            }
            .min(cap);
            recs[generation::cnt_at(s, j as u32) as usize] = laid;
            let z = built.top_at([b[0], b[1]]);
            for i in 0..laid.min(cap) {
                let r = word_of(s, j as u32, i);
                let h = draw.next();
                let leaf = [
                    b[0] + (h % (b[2] - b[0]) as u32) as i32,
                    b[1] + ((h >> 8) % (b[3] - b[1]) as u32) as i32,
                ];
                // Each record keeps the region the frame before gave it, which cnt sizes anew.
                recs[r] = if h.is_multiple_of(8) {
                    generation::UNGENERATED
                } else {
                    generation::rec_word(
                        j as u32,
                        draw.next() % d.variants,
                        draw.next() & 3,
                        cell_zoom,
                    ) | (draw.next() % (drawing::REGIONS + 1)) << drawing::REGION_SHIFT
                };
                recs[r + 1] = real_world::wrap(leaf[0]) as u32;
                recs[r + 2] = leaf[1] as u32;
                recs[r + 3] = (z + draw.within(-4, 5)) as u32;
                recs[r + 4] = draw.next() % 256;
            }
        }
    }
    recs[generation::PLACED_HDR as usize] = placed;
    for n in 0..placed {
        let r = generation::rec_at(generation::ENTRY_RECS + n) as usize;
        let j = n % ROWS.len() as u32;
        let d = &ROWS[j as usize];
        let v = draw.next() % d.variants;
        recs[r] = generation::rec_word(j, v, draw.next() & 3, scatter::coarsest_stand(d, v))
            | (draw.next() % (drawing::REGIONS + 1)) << drawing::REGION_SHIFT;
        // Most lie about the eye, where they are drawn. One in seven lies farther off, where it
        // is not drawn.
        let reach = if n % 7 == 3 { 900 } else { 40 };
        let at = [
            eye[0] + draw.within(-reach, reach),
            eye[1] + draw.within(-reach, reach),
        ];
        recs[r + 1] = real_world::wrap(at[0]) as u32;
        recs[r + 2] = at[1] as u32;
        recs[r + 3] = (built.top_at(at) + draw.within(0, 3)) as u32;
        // The second lies beside the first on its row, a few cells along X.
        if n == 1 {
            let first = generation::rec_at(generation::ENTRY_RECS) as usize;
            recs[r + 1] = real_world::wrap(recs[first + 1] as i32 + 3) as u32;
            recs[r + 2] = recs[first + 2];
            recs[r + 3] = recs[first + 3];
        }
    }
    (
        recs,
        generation::EYE_ENTRIES + placed.div_ceil(generation::RUNS),
    )
}

/// The drawing words a frame hands: the model and impostor floors as angles, and each group's
/// extent.
fn drawing_words(model: f32, impostor: f32) -> Vec<u32> {
    let mut d = vec![0u32; drawing::WORDS as usize];
    d[drawing::MODEL_ANGLE as usize] = model.to_bits();
    d[drawing::IMPOSTOR_ANGLE as usize] = impostor.to_bits();
    for g in 0..generation::GROUPS {
        d[(drawing::EXTENTS + g) as usize] = (1.0 + g as f32 * 0.37).to_bits();
    }
    d
}

/// The bits of a record's first word below its region, which hold what generation laid.
const UNDER_REGION: u32 = (1 << drawing::REGION_SHIFT) - 1;

/// Each region's count off the records `order` names, as their first words say.
fn regions_of(recs: &[u32], order: &[usize]) -> Vec<u32> {
    let mut counted = vec![0u32; 2 * drawing::REGIONS as usize];
    for &i in order {
        let region = recs[i] >> drawing::REGION_SHIFT;
        if recs[i] != generation::UNGENERATED && region != drawing::NONE {
            counted[region as usize] += 1;
        }
    }
    counted
}

/// The test's reference for `decor_cnt_cs`, which writes into each record the pass reads the
/// region `generation`'s rules size it into, for what the frame `sizing` asks over the clip
/// levels of `w`. A record is asked at the zoom the rays ask at its distance (`dist2`,
/// `Detail::at`); one the ground generated is asked at that zoom held to what the clip levels
/// hold where it stands (`ray_zoom`). It is drawn only where that zoom draws its record's zoom
/// (`drawn_at`), as a model, an impostor or culled by the angle its group spans
/// (`drawing::lod`), and the decor left out is drawn as nothing.
fn cnt_rust(w: &World, sizing: &Sizing, recs: &mut [u32], workgroups: u32) {
    let detail = Detail {
        range: sizing.range,
        spacing: Spacing::NONE,
    };
    let d = &sizing.drawing;
    let angle = |k: u32| f32::from_bits(d[k as usize]);
    let hide = &d[drawing::HIDE as usize..drawing::HIDE as usize + 4];
    for i in each_rec(recs, w.clipmap, workgroups) {
        let word = recs[i];
        if word == generation::UNGENERATED {
            continue;
        }
        let at = [recs[i + 1] as i32, recs[i + 2] as i32, recs[i + 3] as i32];
        let d2 = generation::dist2(sizing.origin, at);
        // Placed decor's records lie past the entries'.
        let placed = i >= generation::rec_at(generation::ENTRY_RECS) as usize;
        let ask = detail.at(d2);
        let zoom = if placed {
            ask
        } else {
            generation::ray_zoom(w, ask, [at[0], at[1]])
        };
        let (decor, variant, _, cell_zoom) = generation::rec_of(word);
        let group = DECORS[decor as usize].first_group() + variant;
        let hidden = hide[3] != 0 && hide[..3] == at.map(|v| v as u32);
        let across = angle(drawing::EXTENTS + group) / d2.sqrt().max(drawing::NEAREST_U);
        let lod = drawing::lod(
            across,
            angle(drawing::MODEL_ANGLE),
            angle(drawing::IMPOSTOR_ANGLE),
        );
        let region =
            if hidden || !generation::drawn_at(zoom, cell_zoom, placed) || lod == drawing::CULLED {
                drawing::NONE
            } else {
                2 * group + lod
            };
        recs[i] = word & UNDER_REGION | region << drawing::REGION_SHIFT;
    }
}

/// `decor_cnt_cs` by hand, however its threads fall, run `RUNS_OF_CNT` times a case. Each
/// record must be the Rust's word for word, with the region it sizes the record into
/// (`cnt_rust`), and each region's count must be the count of its records.
#[test]
fn hand_cnt_counts_its_regions() {
    const RUNS_OF_CNT: u32 = 6;
    let Some((device, queue)) = device() else {
        return;
    };
    let hand = hand_pipeline(&device, SHADER_GENERATION_CNT);
    let mut drawn_total = 0u64;
    for (name, at, placed) in [
        ("east of the seam", real_world::EYE, 0),
        ("east of the seam, placed", real_world::EYE, 80),
        ("west of the seam", [(1 << 23) - 300, (40 << 16) + 30], 500),
    ] {
        let built = real_world::build(at);
        let world = built.world();
        let (recs, workgroups) = recs_to_size(&built, at, placed);
        let order = each_rec(&recs, &built.clipmap, workgroups);
        let clip_flat: Vec<u32> = built.clipmap.iter().flatten().map(|&v| v as u32).collect();
        let clip_buf = buf(&device, &clip_flat);
        let origin = [at[0] << 8, at[1] << 8, (built.top_at(at) + 30) << 8];
        let base = Sizing {
            name: "as a frame asks",
            origin,
            range: [13, 32],
            drawing: drawing_words(0.02, 0.004),
        };
        let mut near = base.clone();
        near.name = "a far range";
        near.range = [24, 24];
        let mut coarse = base.clone();
        coarse.name = "a coarse range";
        coarse.range = [16, 22];
        for sizing in [base, near, coarse] {
            let mut want = recs.clone();
            cnt_rust(&world, &sizing, &mut want, workgroups);
            let (drawing, eye_buf) = (
                buf(&device, &sizing.drawing),
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytes(&sizing.eye()),
                    usage: wgpu::BufferUsages::UNIFORM,
                }),
            );
            for run in 0..RUNS_OF_CNT {
                let (r_buf, regions) = (
                    buf(&device, &recs),
                    buf(&device, &vec![0u32; 2 * drawing::REGIONS as usize]),
                );
                let at = [
                    (0, &eye_buf),
                    (1, &clip_buf),
                    (9, &r_buf),
                    (13, &drawing),
                    (14, &regions),
                ];
                dispatch(&device, &queue, &hand, &at, [workgroups, 1, 1]);
                let got = read(&device, &queue, &r_buf, sizing.name);
                let got_regions = read(&device, &queue, &regions, sizing.name);
                let apart: Vec<usize> = (0..want.len()).filter(|&k| got[k] != want[k]).collect();
                if let Some(&k) = apart.first() {
                    panic!(
                        "{name}, {}, run {run}: {} words apart, at {k} {:x} against {:x}",
                        sizing.name,
                        apart.len(),
                        got[k],
                        want[k]
                    );
                }
                assert_eq!(
                    got_regions[..drawing::REGIONS as usize],
                    regions_of(&want, &order)[..drawing::REGIONS as usize],
                    "{name}, {}: counts, run {run}",
                    sizing.name
                );
                if run == 0 {
                    drawn_total += got_regions[..drawing::REGIONS as usize]
                        .iter()
                        .map(|&n| n as u64)
                        .sum::<u64>();
                }
            }
        }
    }
    eprintln!("cnt: {drawn_total} drawn");
    assert!(drawn_total > 0);
}

/// The region and the lanes' bits that decor_compact.wgsl `compact_one` writes for the record
/// at word `i`, or `None` where the record has no region.
fn lanes_of(drawing: &[u32], recs: &[u32], i: usize) -> Option<(u32, [u32; 5])> {
    let w = recs[i];
    let region = w >> drawing::REGION_SHIFT;
    if region == drawing::NONE {
        return None;
    }
    let (decor, variant, turn, _) = generation::rec_of(w);
    let [across, deep, _] = ROWS[decor as usize].size(variant, turn);
    let anchor = |axis: usize| {
        let d = (recs[i + 1 + axis] as i32)
            .wrapping_sub(drawing[drawing::ANCHOR as usize + axis] as i32);
        (if axis == 0 {
            wrap_idx(LEAF_CELL_ZOOM, d)
        } else {
            d
        }) as f32
    };
    let sink = recs[i + 4] as f32 / (1 << FRAC_W) as f32;
    Some((
        region,
        [
            (anchor(0) + across as f32 / 2.0).to_bits(),
            (anchor(1) + deep as f32 / 2.0).to_bits(),
            (anchor(2) - sink).to_bits(),
            ((turn % 4) as f32 * core::f32::consts::FRAC_PI_2).to_bits(),
            1.0f32.to_bits(),
        ],
    ))
}

/// `decor_compact_cs` by hand, however its threads fall, run `RUNS_OF_COMPACT` times a case.
/// Each region's slots must hold its records' lanes, in any order. Where the records outnumber
/// the instances' slots, each region's count still runs on, and every slot holds a record's
/// lanes.
#[test]
fn hand_compact_lays_as_the_rust() {
    const RUNS_OF_COMPACT: u32 = 6;
    let Some((device, queue)) = device() else {
        return;
    };
    let hand = hand_pipeline(&device, SHADER_GENERATION_COMPACT);
    let mut tally = Tally::default();
    for (name, at, placed) in [
        (
            "east of the seam",
            real_world::EYE,
            2 * generation::RUNS + 37,
        ),
        ("west of the seam", [(1 << 23) - 300, (40 << 16) + 30], 0),
    ] {
        let built = real_world::build(at);
        let (mut sized, workgroups) = recs_to_size(&built, at, placed);
        let order = each_rec(&sized, &built.clipmap, workgroups);
        // Each record's region as a frame's cnt may leave it: a region past the last is none.
        for &i in &order {
            if sized[i] != generation::UNGENERATED
                && sized[i] >> drawing::REGION_SHIFT >= drawing::REGIONS
            {
                sized[i] = sized[i] & UNDER_REGION | drawing::NONE << drawing::REGION_SHIFT;
            }
        }
        let clip_flat: Vec<u32> = built.clipmap.iter().flatten().map(|&v| v as u32).collect();
        let (clip_buf, rows) = (buf(&device, &clip_flat), buf(&device, rows_words()));
        // Two anchors: one a little off the eye, and one across the seam.
        for anchor_at in [
            [at[0] + 40, at[1] - 7, 12],
            [real_world::wrap(at[0] + (1 << 23)), at[1], -300],
        ] {
            let mut drawing = drawing_words(0.02, 0.0001);
            for a in 0..3 {
                drawing[drawing::ANCHOR as usize + a] = anchor_at[a] as u32;
            }
            let mut regions = regions_of(&sized, &order);
            // The regions laid out end to end, as `decor_args_cs` may leave them.
            let mut first = 0;
            let mut firsts = vec![0u32; drawing::REGIONS as usize];
            for r in 0..drawing::REGIONS as usize {
                regions[drawing::REGIONS as usize + r] = first;
                firsts[r] = first;
                first += regions[r];
            }
            let total = first;
            let mut wanted: Vec<(u32, [u32; 5])> = order
                .iter()
                .filter(|&&i| sized[i] != generation::UNGENERATED)
                .filter_map(|&i| lanes_of(&drawing, &sized, i))
                .collect();
            wanted.sort_unstable();
            assert_eq!(wanted.len() as u32, total);
            *tally.0.entry("instances laid").or_default() += total as u64;
            // Room for all and to spare, and room for fewer than are drawn.
            for cap in [total + 64, total / 2 + 1] {
                let drawing_buf = buf(&device, &drawing);
                let r_buf = buf(&device, &sized);
                for run in 0..RUNS_OF_COMPACT {
                    {
                        let regions_buf = buf(&device, &regions);
                        let instances = buf(
                            &device,
                            &vec![0xffff_ffffu32; ((drawing::COLUMNS + 1) * cap) as usize],
                        );
                        {
                            let at = [
                                (1, &clip_buf),
                                (9, &r_buf),
                                (10, &rows),
                                (13, &drawing_buf),
                                (14, &regions_buf),
                                (15, &instances),
                            ];
                            dispatch(&device, &queue, &hand, &at, [workgroups, 1, 1]);
                        }
                        let got_regions = read(&device, &queue, &regions_buf, "compact's regions");
                        let got = read(&device, &queue, &instances, "compact's instances");
                        // Each region's end word must name its next slot past its end, which
                        // light_mark_draws.wgsl `instance_cnt` reads.
                        for r in 0..drawing::REGIONS as usize {
                            assert_eq!(
                                got_regions[drawing::REGIONS as usize + r],
                                firsts[r] + regions[r],
                                "{name}, region {r}"
                            );
                            assert_eq!(got_regions[r], regions[r], "{name}, region {r}'s count");
                        }
                        let mut have = Vec::new();
                        for r in 0..drawing::REGIONS as usize {
                            for slot in firsts[r]..(firsts[r] + regions[r]).min(cap) {
                                let (c, k) = (cap as usize, slot as usize);
                                let lanes = [
                                    got[k],
                                    got[c + k],
                                    got[2 * c + k],
                                    got[3 * c + k],
                                    got[4 * c + k],
                                ];
                                have.push((r as u32, lanes));
                            }
                        }
                        have.sort_unstable();
                        if cap > total {
                            assert!(have == wanted, "{name}: instances, run {run}");
                            let spare = &got[total as usize..cap as usize];
                            assert!(
                                spare.iter().all(|&w| w == 0xffff_ffff),
                                "{name}: past the slots laid"
                            );
                        } else {
                            // The threads decide which records take the slots, but each slot
                            // must hold some record's lanes.
                            let mut left = wanted.clone();
                            for h in &have {
                                let at = left.binary_search(h);
                                assert!(at.is_ok(), "{name}: a slot no record's");
                                left.remove(at.unwrap());
                            }
                        }
                        *tally.0.entry("runs").or_default() += 1;
                    }
                }
            }
        }
    }
    eprintln!("compact: {:?}", tally.0);
    assert!(tally.0["instances laid"] > 0);
}

/// Where the draws' words disagree with the regions they draw. Each region's args must draw its
/// count of its mesh's corners or an impostor's. Its uniform's columns must start at its first
/// slot, its count must be `cnts`'s, and no two regions' slots may overlap. `firsts` holds
/// each region's first slot, and `fresh` holds the uniforms before args added to them.
fn draws_differ(
    cnts: &[u32],
    regions: &[u32],
    firsts: &[u32],
    args: &[u32],
    uniforms: &[u32],
    fresh: &[u32],
    meshes: &[u32],
) -> Vec<String> {
    let n = drawing::REGIONS as usize;
    let u = drawing::UNIFORM_WORDS as usize;
    let mut out = Vec::new();
    for r in 0..n {
        let a = drawing::ARGS_WORDS as usize * r;
        let corners = if r % 2 == 0 {
            meshes[r / 2]
        } else {
            drawing::IMPOSTOR_CORNERS
        };
        if args[a..a + 5] != [corners, cnts[r], 0, 0, 0] {
            out.push(format!("region {r}'s args {:?}", &args[a..a + 5]));
        }
        if regions[r] != cnts[r] {
            out.push(format!("region {r}'s count {}", regions[r]));
        }
        let idx = r * u + instance::IDX;
        if uniforms[idx] != fresh[idx] + firsts[r] {
            out.push(format!(
                "region {r}'s uniform's columns at {}",
                uniforms[idx]
            ));
        }
        let rest = |w: &[u32]| {
            let mut w = w[r * u..(r + 1) * u].to_vec();
            w[instance::IDX] = 0;
            w
        };
        if rest(uniforms) != rest(fresh) {
            out.push(format!("region {r}'s uniform past its columns"));
        }
    }
    let mut spans: Vec<(u32, u32)> = (0..n).map(|r| (firsts[r], firsts[r] + cnts[r])).collect();
    spans.sort_unstable();
    if spans.windows(2).any(|w| w[0].1 > w[1].0) {
        out.push(format!("regions' slots overlap: {spans:?}"));
    }
    out
}

/// `decor_args_cs` by hand: what the draws read must agree with the regions it lays, over
/// uniforms as fresh as a frame writes them.
#[test]
fn hand_args_draws_its_regions() {
    let Some((device, queue)) = device() else {
        return;
    };
    let hand = hand_pipeline(&device, SHADER_GENERATION_ARGS);
    let (regions_n, groups) = (drawing::REGIONS as usize, generation::GROUPS as usize);
    let store = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC;
    let make = |w: &[u32]| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytes(w),
            usage: store,
        })
    };
    for round in 0..4u32 {
        // A mesh still loading has 0 corners.
        let meshes: Vec<u32> = (0..groups as u32)
            .map(|g| {
                if mix(round, [g as i32, 1]).is_multiple_of(3) {
                    0
                } else {
                    36 + g * 6
                }
            })
            .collect();
        let mut regions = vec![0u32; 2 * regions_n];
        for (r, c) in regions.iter_mut().take(regions_n).enumerate() {
            *c = if mix(round + 9, [r as i32, 2]).is_multiple_of(4) {
                0
            } else {
                mix(round, [r as i32, 5]) % 500
            };
        }
        // Each region's uniform as `Generator::prepare` writes it, with the columns' base at
        // `instance::IDX`.
        let u = drawing::UNIFORM_WORDS as usize;
        let mut uniforms = vec![0u32; u * regions_n];
        let base = drawing::COLUMNS * 2048;
        for r in 0..regions_n {
            uniforms[r * u] = 1.5f32.to_bits();
            uniforms[r * u + instance::IDX] = base;
        }
        let args0 = vec![0xdead_beefu32; drawing::ARGS_WORDS as usize * regions_n];
        let (mb, rb, ab, ub) = (make(&meshes), make(&regions), make(&args0), make(&uniforms));
        // Instances that hold every region's count, so none is cut short.
        let slots = regions[..regions_n].iter().sum::<u32>().max(1);
        let ib = make(&vec![0u32; ((drawing::COLUMNS + 1) * slots) as usize]);
        let entries = [(0, &mb), (1, &rb), (2, &ab), (3, &ub), (15, &ib)].map(|(binding, b)| {
            wgpu::BindGroupEntry {
                binding,
                resource: b.as_entire_binding(),
            }
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &hand.get_bind_group_layout(0),
            entries: &entries,
        });
        let mut e = device.create_command_encoder(&Default::default());
        {
            let mut pass = e.begin_compute_pass(&Default::default());
            pass.set_pipeline(&hand);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        queue.submit([e.finish()]);
        let got_regions = read(&device, &queue, &rb, "regions");
        let d = draws_differ(
            &regions[..regions_n],
            &got_regions,
            &got_regions[regions_n..],
            &read(&device, &queue, &ab, "args"),
            &read(&device, &queue, &ub, "uniforms"),
            &uniforms,
            &meshes,
        );
        assert!(d.is_empty(), "round {round}: {d:?}");
    }
}

/// A frame of decor's passes, as `Generator::prepare` and `dispatch` run them.
#[derive(Clone)]
struct Frame {
    name: &'static str,
    /// `marking` says whether the frame marks at all, and `every` makes every entry whole.
    /// `boxes` are the frame's boxes, and `stale` names the entries whose heads a brick left.
    marking: bool,
    every: bool,
    boxes: Vec<([i32; 4], Zoom)>,
    stale: Vec<u32>,
    /// Which of the world's two placings the frame reads: the first with the placed decor, the
    /// second without it.
    placed_decor: bool,
    cuts: [Cut; 2],
    sizing: Sizing,
}

/// A pair's count and, by leaf, each record's first word, Z and sink.
type RecordMap = (u32, BTreeMap<[i32; 2], [u32; 3]>);

/// Each pair's records keyed by their leaf, so their order does not matter: `(entry, decor)`
/// maps to what the pair laid.
fn pairs_of(recs: &[u32]) -> BTreeMap<(u32, u32), RecordMap> {
    let mut pairs = BTreeMap::new();
    for s in 0..generation::EYE_ENTRIES {
        for j in 0..ROWS.len() as u32 {
            let cnt = recs[generation::cnt_at(s, j) as usize];
            if cnt == 0 {
                continue;
            }
            let mut by_leaf = BTreeMap::new();
            for i in 0..cnt.min(generation::cap(&ROWS[j as usize])) {
                let r = word_of(s, j, i);
                by_leaf.insert(
                    [recs[r + 1] as i32, recs[r + 2] as i32],
                    [recs[r], recs[r + 3], recs[r + 4]],
                );
            }
            pairs.insert((s, j), (cnt, by_leaf));
        }
    }
    pairs
}

/// The Rust's frame over its own records, which a pass over them reads in `workgroups`.
struct Model {
    recs: Vec<u32>,
    shown: [u32; ROWS.len()],
    workgroups: u32,
}

/// What a frame's uniforms hold before its args add to them (`Generator::prepare`).
fn fresh_uniforms(cap: u32) -> Vec<u32> {
    let u = drawing::UNIFORM_WORDS as usize;
    let mut uniforms = vec![0u32; u * drawing::REGIONS as usize];
    for r in 0..drawing::REGIONS as usize {
        let layer = if r % 2 == 1 { r as u32 / 2 } else { 0 };
        uniforms[r * u..r * u + 8].copy_from_slice(&[
            1.5f32.to_bits(),
            2.5f32.to_bits(),
            3.5f32.to_bits(),
            0,
            cap,
            1.0f32.to_bits(),
            layer,
            drawing::COLUMNS * cap,
        ]);
    }
    uniforms
}

/// A list's head and boxes as `Generator::prepare` writes them, before a mark.
fn list_hdr(frame: &Frame) -> Vec<u32> {
    let mut list = vec![0u32; generation::LIST_WORDS as usize];
    list[generation::THIN_ARGS as usize] = BRICK_CELLS.div_ceil(generation::LANES);
    list[generation::THIN_ARGS as usize + 1] = 1;
    list[generation::GENERATE_ARGS as usize] =
        generation::DECOR_REC_CAP.div_ceil(generation::LANES);
    list[generation::GENERATE_ARGS as usize + 1] = 1;
    list[generation::LIST_EVERY as usize] = frame.every as u32;
    list[generation::LIST_RECTS as usize] = frame.boxes.len() as u32;
    for (k, &(b, z)) in frame.boxes.iter().enumerate() {
        let at = generation::RECTS_AT as usize + k * generation::RECT_WORDS as usize;
        for a in 0..4 {
            list[at + a] = b[a] as u32;
        }
        list[at + 4] = z;
    }
    list
}

impl Model {
    /// Runs the frame through the Rust's passes, laying a pair's records in `thin_leaves`'s
    /// order, and returns the records it leaves, each sized for the frame.
    fn step(
        &mut self,
        world: World,
        frame: &Frame,
        tally: &mut Tally,
        claims: &mut Vec<[i32; 3]>,
    ) -> Vec<u32> {
        for &s in &frame.stale {
            self.recs[generation::hdr_at(s) as usize + 2] = 0;
        }
        if frame.marking {
            let mut list = list_hdr(frame);
            mark_rust(world.clipmap, &mut self.recs, &mut list, &self.shown);
            // Thin, then what each listed record stands as.
            let whole = list[generation::THIN_ARGS as usize + 2] as usize;
            let parts = list[generation::LIST_PARTS as usize] as usize;
            let at_whole = generation::LIST_AT as usize;
            let at_part = generation::LIST_WORDS as usize;
            let mut asked: Vec<(u32, u32, u32, [i32; 2])> = Vec::new();
            for &w in &list[at_whole..at_whole + whole] {
                let (s, j) = generation::pair_of(w);
                let cell_zoom = clip::eye_cell_zoom(s / ENTRIES);
                let h = generation::hdr_at(s) as usize;
                let q = [self.recs[h] as i32, self.recs[h + 1] as i32];
                let leaves = thin_leaves(j as usize, cell_zoom, q);
                let cap = generation::cap(&ROWS[j as usize]);
                self.recs[generation::cnt_at(s, j) as usize] = leaves.len() as u32;
                for (i, leaf) in leaves.iter().take(cap as usize).enumerate() {
                    let r = word_of(s, j, i as u32);
                    self.recs[r + 1] = leaf[0] as u32;
                    self.recs[r + 2] = leaf[1] as u32;
                    asked.push((s, j, i as u32, *leaf));
                }
            }
            for &w in &list[at_part - parts..at_part] {
                let (s, j) = generation::pair_of(w);
                let cell_zoom = clip::eye_cell_zoom(s / ENTRIES);
                let n = self.recs[generation::cnt_at(s, j) as usize]
                    .min(generation::cap(&ROWS[j as usize]));
                for i in 0..n {
                    let r = word_of(s, j, i);
                    let leaf = [self.recs[r + 1] as i32, self.recs[r + 2] as i32];
                    if is_changed(&frame.boxes, cell_zoom, leaf) {
                        asked.push((s, j, i, leaf));
                    }
                }
            }
            let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
            let chunk = asked.len().div_ceil(threads).max(1);
            let results: Vec<_> = std::thread::scope(|sc| {
                let hs: Vec<_> = asked
                    .chunks(chunk)
                    .map(|part| {
                        sc.spawn(move || {
                            let mut tally = Tally::default();
                            let mut claims = Vec::new();
                            let mut out = Vec::new();
                            for &(s, j, i, leaf) in part {
                                let cell_zoom = clip::eye_cell_zoom(s / ENTRIES);
                                let mut q = Generating {
                                    w: world,
                                    cuts: frame.cuts,
                                    tally: &mut tally,
                                    placed: Default::default(),
                                    last_decor: Default::default(),
                                    claims: Vec::new(),
                                    last_lacks: false,
                                };
                                let stood = stand(&mut q, j, cell_zoom, leaf);
                                claims.append(&mut q.claims);
                                out.push((word_of(s, j, i), stood));
                            }
                            (out, tally, claims)
                        })
                    })
                    .collect();
                hs.into_iter().map(|h| h.join().unwrap()).collect()
            });
            for (out, t, mut by) in results {
                tally.merge(t);
                claims.append(&mut by);
                for (r, (word, z, sink)) in out {
                    let was = self.recs[r];
                    tally.add(
                        "frame: a record comes back",
                        was == generation::UNGENERATED && word != was,
                    );
                    tally.add(
                        "frame: a record taken away",
                        was != generation::UNGENERATED
                            && was != 0
                            && word == generation::UNGENERATED,
                    );
                    self.recs[r] = word;
                    if word != generation::UNGENERATED {
                        self.recs[r + 3] = z;
                        self.recs[r + 4] = sink;
                    }
                }
            }
        }
        cnt_rust(&world, &frame.sizing, &mut self.recs, self.workgroups);
        self.recs.clone()
    }
}

/// Where what a frame left differs from what it must leave. Each pair's records, and placed
/// decor's, must match the Rust's `want` word for word, with the regions it sized them into.
/// What the draws read must agree with the records the frame sized: each region's count is its
/// records', its slots hold their lanes, and the args, the uniforms and the count that
/// light_mark_draws.wgsl `instance_cnt` reads follow from them.
fn frame_differs(
    got: &FrameLeft,
    want: &[u32],
    clipmap: &[[i32; 4]],
    workgroups: u32,
    drawing: &[u32],
    cap: u32,
    meshes: &[u32],
) -> Vec<String> {
    let mut out = Vec::new();
    let pairs = pairs_of(&got.recs);
    let want_pairs = pairs_of(want);
    if pairs.len() != want_pairs.len() {
        out.push(format!(
            "pairs {} against {}",
            pairs.len(),
            want_pairs.len()
        ));
    }
    for (k, (cnt, leaves)) in &want_pairs {
        let Some((got_cnt, got_leaves)) = pairs.get(k) else {
            out.push(format!("{k:?}: no records"));
            continue;
        };
        if got_cnt != cnt || got_leaves.len() != leaves.len() {
            out.push(format!("{k:?}: laid {got_cnt} against {cnt}"));
            continue;
        }
        for (leaf, w) in leaves {
            let Some(g) = got_leaves.get(leaf) else {
                out.push(format!("{k:?}: no leaf {leaf:?}"));
                continue;
            };
            // A record that generates nothing keeps no Z or sink of its own.
            let same = if w[0] == generation::UNGENERATED {
                g[0] == w[0]
            } else {
                g == w
            };
            if !same {
                out.push(format!("{k:?} {leaf:?}: got {g:x?}, want {w:x?}"));
            }
        }
    }
    let r = REC_WORDS as usize;
    let first = generation::rec_at(generation::ENTRY_RECS) as usize;
    for p in 0..want[generation::PLACED_HDR as usize] as usize {
        let at = first + p * r;
        if got.recs[at..at + r] != want[at..at + r] {
            out.push(format!(
                "placed record {p}: got {:x?}, want {:x?}",
                &got.recs[at..at + r],
                &want[at..at + r]
            ));
        }
    }
    let n = drawing::REGIONS as usize;
    let order = each_rec(&got.recs, clipmap, workgroups);
    let cnts = regions_of(&got.recs, &order);
    // Compact leaves each region's next slot in its end word, so its first slot is that less
    // its count.
    let firsts: Vec<u32> = (0..n)
        .map(|r| got.regions[n + r].wrapping_sub(got.regions[r]))
        .collect();
    out.extend(draws_differ(
        &cnts[..n],
        &got.regions,
        &firsts,
        &got.args,
        &got.uniforms,
        &fresh_uniforms(cap),
        meshes,
    ));
    let mut want_lanes: Vec<(u32, [u32; 5])> = order
        .iter()
        .filter(|&&i| got.recs[i] != generation::UNGENERATED)
        .filter_map(|&i| lanes_of(drawing, &got.recs, i))
        .collect();
    want_lanes.sort_unstable();
    let c = cap as usize;
    let mut lanes = Vec::new();
    for r in 0..n {
        for slot in firsts[r]..(firsts[r] + cnts[r]).min(cap) {
            let k = slot as usize;
            let i = &got.instances;
            lanes.push((
                r as u32,
                [i[k], i[c + k], i[2 * c + k], i[3 * c + k], i[4 * c + k]],
            ));
        }
    }
    lanes.sort_unstable();
    if lanes != want_lanes {
        out.push(format!(
            "lanes, {} against its records' {}",
            lanes.len(),
            want_lanes.len()
        ));
    }
    if got.regions[2 * n - 1].min(cap) != lanes.len() as u32 {
        out.push(
            "the instances a reader takes, the last region's end at most `DecorBuffer::cap`"
                .to_string(),
        );
    }
    out
}

/// The light's head `light_mark_draws_cs` is run under, laid out for the map's side as the
/// renderer lays it (ray/cast.rs). The world's receivers stood in the cast before, no host's
/// draw stands within a box, and the marks take no footprint.
fn mark_hdr() -> Vec<u32> {
    let mut hdr = vec![0u32; light::HDR_WORDS];
    hdr[light::HDR_CAST] = CAST_NUMBER;
    hdr[light::HDR_WORLD] = CAST_NUMBER - 1;
    for (k, v) in light::hdr_layout(MAP_SIDE) {
        hdr[k] = v;
    }
    hdr
}

/// The tiles of the light's box the decor in the frame's instances stands on, as the Rust says
/// each placed slot stands (light_mark_draws.wgsl). A slot stands where its lanes lay it from
/// the drawing's anchor, in units from the eye. Its ball (decor_ball.wgsl) must lie in the
/// eye's view (`eye::sees`) and within the light's reach (`Light::reach`, `Light::within`), and
/// then it stands on the tiles of its footprint, `WIDEST` square at its base
/// (`Light::corner_tiles`).
fn stood_rust(got: &FrameLeft, sizing: &Sizing, cap: u32, tally: &mut Tally) -> BTreeSet<[i32; 2]> {
    let (eye_words, hdr) = (sizing.eye(), mark_hdr());
    let f = |k: usize| f32::from_bits(eye_words[k]);
    let v3 = |k: usize| [f(k), f(k + 1), f(k + 2)];
    let axes = [v3(eye::FORWARD), v3(eye::RIGHT), v3(eye::UP)];
    let tan = [f(eye::TAN), f(eye::TAN + 1)];
    let slope = [
        f(eye::GRID + beam::GRID_SLOPE),
        f(eye::GRID + beam::GRID_SLOPE + 1),
    ];
    let l = Light::of_words(
        eye_words[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS]
            .try_into()
            .unwrap(),
    );
    let footprint = hdr[light::HDR_FOOTPRINT] != 0;
    // How far the anchor lies from the eye on `axis`, in units: its whole cells less the eye's,
    // wrapping round the world on X, less the eye's fraction of a cell.
    let anchor = |axis: usize| {
        let o = eye_words[eye::ORIGIN + axis] as i32;
        let d = (sizing.drawing[drawing::ANCHOR as usize + axis] as i32).wrapping_sub(o >> FRAC_W);
        let d = if axis == 0 {
            wrap_idx(LEAF_CELL_ZOOM, d)
        } else {
            d
        };
        d as f32 - (o & ((1 << FRAC_W) - 1)) as f32 * (1.0 / (1 << FRAC_W) as f32)
    };
    let (tall, r) = (
        generation::TALLEST as f32,
        (generation::TALLEST + generation::WIDEST) as f32,
    );
    let half = 0.5 * generation::WIDEST as f32;
    let c = cap as usize;
    let placed = got.regions[2 * drawing::REGIONS as usize - 1].min(cap) as usize;
    let mut stood = BTreeSet::new();
    for k in 0..placed {
        let base = [0, 1, 2].map(|a| anchor(a) + f32::from_bits(got.instances[a * c + k]));
        let middle = [base[0], base[1], base[2] + tall * 0.5];
        let (lo, hi) = (middle.map(|v| v - r), middle.map(|v| v + r));
        // The view is asked of the ball about the box that holds the decor's ball.
        let ball = [0, 1, 2].map(|a| 0.5 * (lo[a] + hi[a]));
        let side = [0, 1, 2].map(|a| hi[a] - lo[a]);
        let ball_r = 0.5 * (side[0] * side[0] + side[1] * side[1] + side[2] * side[2]).sqrt();
        let seen = eye::sees(axes, tan, slope, ball, ball_r);
        let reached = Light::within(lo, hi, l.reach(MAP_SIDE));
        tally.add("frame: draws out of view", !seen);
        tally.add(
            "frame: draws in view past the light's reach",
            seen && !reached,
        );
        if !seen || !reached {
            continue;
        }
        let foot = |d: f32| [base[0] + d, base[1] + d, base[2]];
        if let Some([first, last]) = l.corner_tiles(MAP_SIDE, foot(-half), foot(half), footprint) {
            for j in first[1]..=last[1] {
                for i in first[0]..=last[0] {
                    stood.insert([i, j]);
                }
            }
        }
    }
    stood
}

/// Where what `light_mark_draws_cs` left differs from the tiles the Rust says the frame's
/// draws stand on (`stood_rust`), with `tally` counting the draws that stand on nothing. No
/// tile was marked before the pass, so each tile stood on must be listed once
/// (`light::LIST_RECEIVERS`), and its mark must name the cast the world's receivers stood in
/// (`light::HDR_WORLD`). The stood list's args must hold their count, the propagation's rows
/// for it and, once a tile is stood on, the slide's workgroups across
/// (`light::RECEIVER_TILES_WORDS`). No other word of the list or the args may change.
fn stood_differs(got: &FrameLeft, sizing: &Sizing, cap: u32, tally: &mut Tally) -> Vec<String> {
    let mut out = Vec::new();
    let want = stood_rust(got, sizing, cap, tally);
    let n = light::tiles(MAP_SIDE);
    let cnt = got.stood[light::RECEIVER_TILES_CNT as usize];
    let at = light::list_cnt_at(MAP_SIDE, light::LIST_RECEIVERS) as usize + 1;
    let listed = &got.list[at..at + cnt.min(n * n) as usize];
    let tiles: BTreeSet<[i32; 2]> = listed
        .iter()
        .map(|&w| [(w & 0xffff) as i32, (w >> 16) as i32])
        .collect();
    if tiles.len() != listed.len() {
        out.push(format!(
            "stood: {} listed, {} tiles",
            listed.len(),
            tiles.len()
        ));
    }
    if tiles != want {
        let extra: Vec<_> = tiles.difference(&want).take(4).collect();
        let lacking: Vec<_> = want.difference(&tiles).take(4).collect();
        out.push(format!(
            "stood: {} tiles against {}, past them {extra:?}, lacking {lacking:?}",
            tiles.len(),
            want.len()
        ));
    }
    let per = ray::MISS_LANES * ray::MISS_ACROSS;
    let mut args = vec![0u32; light::RECEIVER_TILES_WORDS as usize];
    args[..ray::LEFT_EMPTY.len()].copy_from_slice(&ray::LEFT_EMPTY);
    args[light::RECEIVER_TILES_LIST as usize + 1] = (want.len() as u32).div_ceil(per);
    args[light::RECEIVER_TILES_CNT as usize] = want.len() as u32;
    if !want.is_empty() {
        args[light::RECEIVER_TILES_SLIDE as usize] = n.div_ceil(SQUARE_SIDE);
    }
    if got.stood != args {
        out.push(format!(
            "stood list's args {:?} against {args:?}",
            got.stood
        ));
    }
    let mut marks = vec![0u32; light::args_words(MAP_SIDE) as usize];
    let marks_at =
        (light::ARGS_WORDS + light::list_marks_at(MAP_SIDE, light::LIST_RECEIVERS)) as usize;
    for t in &want {
        marks[marks_at + (t[1] as u32 * n + t[0] as u32) as usize] = CAST_NUMBER - 1;
    }
    let apart = (0..marks.len())
        .filter(|&k| got.light_args[k] != marks[k])
        .count();
    if apart != 0 {
        out.push(format!("stood: {apart} of the light's args apart"));
    }
    let untouched = got
        .list
        .iter()
        .enumerate()
        .all(|(k, &w)| w == 0 || (at..at + listed.len()).contains(&k));
    if !untouched {
        out.push("stood: the list written past its stood tiles".to_string());
    }
    out
}

/// The buffers one chain of passes runs over, and its pipelines.
struct Chain<'d> {
    device: &'d wgpu::Device,
    queue: &'d wgpu::Queue,
    pipelines: &'d [wgpu::ComputePipeline; 6],
    /// `light_mark_draws_cs`, which reads what compact lays beside the draws, and the one
    /// thread that opens its workgroups (`light_mark_draws_open_cs`).
    mark_draws: &'d wgpu::ComputePipeline,
    mark_draws_open: &'d wgpu::ComputePipeline,
    recs: wgpu::Buffer,
    list: wgpu::Buffer,
    regions: wgpu::Buffer,
    args: wgpu::Buffer,
    uniforms: wgpu::Buffer,
    instances: wgpu::Buffer,
    cap: u32,
    workgroups: u32,
}

/// What a frame binds of the world: the clip levels, the ground, the area, what is placed and
/// the ways, with decor's rows, what the core shows and the meshes.
struct WorldBinding<'b> {
    clipmap: &'b wgpu::Buffer,
    ground: &'b wgpu::Buffer,
    shards: &'b wgpu::Buffer,
    objects: &'b wgpu::Buffer,
    pool: &'b wgpu::Buffer,
    rows: &'b wgpu::Buffer,
    shown: &'b wgpu::Buffer,
    meshes: &'b wgpu::Buffer,
}

impl Chain<'_> {
    /// Runs one frame as `Generator::dispatch` records it, in one compute pass. Thin and
    /// `decor_pose_cs` are dispatched off the list the mark writes.
    fn frame(&self, frame: &Frame, b: &WorldBinding) -> FrameLeft {
        let (d, q) = (self.device, self.queue);
        let store = wgpu::BufferUsages::STORAGE;
        for &s in &frame.stale {
            q.write_buffer(
                &self.recs,
                (generation::hdr_at(s) as u64 + 2) * 4,
                bytes(&[0]),
            );
        }
        if frame.marking {
            q.write_buffer(
                &self.list,
                0,
                bytes(&list_hdr(frame)[..generation::LIST_AT as usize]),
            );
        }
        q.write_buffer(
            &self.regions,
            0,
            bytes(&vec![0u32; 2 * drawing::REGIONS as usize]),
        );
        q.write_buffer(&self.uniforms, 0, bytes(&fresh_uniforms(self.cap)));
        let drawing = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytes(&frame.sizing.drawing),
            usage: store,
        });
        let mut eye = frame.sizing.eye();
        for (k, c) in [(eye::CARVE, frame.cuts[0]), (eye::HIDE, frame.cuts[1])] {
            eye[k..k + 3].copy_from_slice(&c.at.map(|v| v as u32));
            eye[k + 3] = c.zoom;
        }
        let eye_buf = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytes(&eye),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind = |p: &wgpu::ComputePipeline, at: &[(u32, &wgpu::Buffer)]| {
            let entries: Vec<_> = at
                .iter()
                .map(|&(binding, buf)| wgpu::BindGroupEntry {
                    binding,
                    resource: buf.as_entire_binding(),
                })
                .collect();
            d.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &p.get_bind_group_layout(0),
                entries: &entries,
            })
        };
        let [mark, thin, generate, cnt, args, compact] = self.pipelines;
        let mut e = d.create_command_encoder(&Default::default());
        {
            let mut pass = e.begin_compute_pass(&Default::default());
            if frame.marking {
                let damage_group = {
                    bind(
                        mark,
                        &[
                            (1, b.clipmap),
                            (9, &self.recs),
                            (11, &self.list),
                            (12, b.shown),
                        ],
                    )
                };
                pass.set_pipeline(mark);
                pass.set_bind_group(0, &damage_group, &[]);
                pass.dispatch_workgroups(generation::EYE_ENTRIES.div_ceil(generation::LANES), 1, 1);
                let thin_group = { bind(thin, &[(9, &self.recs), (10, b.rows), (11, &self.list)]) };
                pass.set_pipeline(thin);
                pass.set_bind_group(0, &thin_group, &[]);
                pass.dispatch_workgroups_indirect(&self.list, generation::THIN_ARGS as u64 * 4);
                {
                    let g = bind(
                        generate,
                        &[
                            (0, &eye_buf),
                            (1, b.clipmap),
                            (2, b.ground),
                            (5, b.shards),
                            (6, b.objects),
                            (7, b.pool),
                            (9, &self.recs),
                            (10, b.rows),
                            (11, &self.list),
                        ],
                    );
                    pass.set_pipeline(generate);
                    pass.set_bind_group(0, &g, &[]);
                    pass.dispatch_workgroups_indirect(
                        &self.list,
                        generation::GENERATE_ARGS as u64 * 4,
                    );
                }
            }
            let cnt_group = {
                bind(
                    cnt,
                    &[
                        (0, &eye_buf),
                        (1, b.clipmap),
                        (9, &self.recs),
                        (13, &drawing),
                        (14, &self.regions),
                    ],
                )
            };
            pass.set_pipeline(cnt);
            pass.set_bind_group(0, &cnt_group, &[]);
            pass.dispatch_workgroups(self.workgroups, 1, 1);
            let args_group = bind(
                args,
                &[
                    (0, b.meshes),
                    (1, &self.regions),
                    (2, &self.args),
                    (3, &self.uniforms),
                    (15, &self.instances),
                ],
            );
            pass.set_pipeline(args);
            pass.set_bind_group(0, &args_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
            let compact_group = {
                bind(
                    compact,
                    &[
                        (1, b.clipmap),
                        (9, &self.recs),
                        (10, b.rows),
                        (13, &drawing),
                        (14, &self.regions),
                        (15, &self.instances),
                    ],
                )
            };
            pass.set_pipeline(compact);
            pass.set_bind_group(0, &compact_group, &[]);
            pass.dispatch_workgroups(self.workgroups, 1, 1);
        }
        q.submit([e.finish()]);
        // The light's mark then lists the tiles the compacted decor stands on, reading its
        // lanes, under the head `mark_hdr` lays. None of the world's receivers' tiles is
        // marked. The mark's workgroups are those its one opening thread sets, with the placed
        // decor's after the characters'.
        let hdr = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytes(&mark_hdr()),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let list = buf(d, &vec![0u32; light::list_words(MAP_SIDE) as usize]);
        let light_args = buf(d, &vec![0u32; light::args_words(MAP_SIDE) as usize]);
        let mut receiver_tiles_words = vec![0u32; light::RECEIVER_TILES_WORDS as usize];
        receiver_tiles_words[..ray::LEFT_EMPTY.len()].copy_from_slice(&ray::LEFT_EMPTY);
        let stood = buf(d, &receiver_tiles_words);
        let binds = [
            (0, &eye_buf),
            (11, &hdr),
            (12, &list),
            (18, &drawing),
            (19, &self.regions),
            (20, &self.instances),
            (21, &light_args),
            (22, &stood),
        ];
        // The mark's dispatch lies in a buffer of its own, which the opening pass writes and
        // the mark does not bind (`light::DRAWS_DISPATCH_WORDS`).
        let draws = buf(d, &vec![0u32; light::DRAWS_DISPATCH_WORDS as usize]);
        let open = [binds[1], binds[4], binds[5], (24, &draws)];
        dispatch(d, q, self.mark_draws_open, &open, [1, 1, 1]);
        let opened = read(d, q, &draws, frame.name);
        let at = 0;
        let placed =
            read(d, q, &self.regions, frame.name)[2 * drawing::REGIONS as usize - 1].min(self.cap);
        let groups = if placed == 0 {
            0
        } else {
            light::RECEIVER_BOXES as u32 + placed.div_ceil(ROW_LANES)
        };
        assert_eq!(
            opened[at..at + 3],
            [groups, 1, 1],
            "{}: the drawn receivers' workgroups",
            frame.name
        );
        dispatch(d, q, self.mark_draws, &binds, [opened[at], 1, 1]);
        FrameLeft {
            recs: read(d, q, &self.recs, frame.name),
            regions: read(d, q, &self.regions, frame.name),
            args: read(d, q, &self.args, frame.name),
            uniforms: read(d, q, &self.uniforms, frame.name),
            instances: read(d, q, &self.instances, frame.name),
            list: read(d, q, &list, frame.name),
            light_args: read(d, q, &light_args, frame.name),
            stood: read(d, q, &stood, frame.name),
        }
    }
}

/// The words a frame leaves on the device.
struct FrameLeft {
    recs: Vec<u32>,
    regions: Vec<u32>,
    args: Vec<u32>,
    uniforms: Vec<u32>,
    instances: Vec<u32>,
    /// What `light_mark_draws_cs` leaves of the light's from the instances: its list, its args
    /// with their marks, and the stood list's args.
    list: Vec<u32>,
    light_args: Vec<u32>,
    stood: Vec<u32>,
}

/// Frames through all the passes by hand. Their records must match the Rust's frame, regions
/// and all, their draws must agree with their records, and the tiles their draws stand on must
/// be the Rust's. The frames are: all whole; the eye moved, with no mark; the placed decor gone,
/// so what it took comes back, with some heads stale and pairs whole and in part in one
/// dispatch; boxes over whole bricks; the decor back; a cut; a drawn corner left out. Some
/// draws lie out of view, and some in view past the light's reach.
/// `DECOR_RUNS` sets how many chains by hand run over each world, however their threads fall.
#[test]
fn hand_frames_answer_as_the_rust() {
    let Some((device, queue)) = device() else {
        return;
    };
    let runs: u32 = std::env::var("DECOR_RUNS")
        .ok()
        .and_then(|r| r.parse().ok())
        .unwrap_or(12);
    let hand_pipes = [
        hand_pipeline(&device, SHADER_GENERATION_MARK),
        hand_pipeline(&device, SHADER_GENERATION_THIN),
        hand_pipeline(&device, SHADER_GENERATION),
        hand_pipeline(&device, SHADER_GENERATION_CNT),
        hand_pipeline(&device, SHADER_GENERATION_ARGS),
        hand_pipeline(&device, SHADER_GENERATION_COMPACT),
    ];
    let mark_draws = pipeline(
        &device,
        "light_mark_draws by hand",
        src(SHADER_LIGHT_MARK_DRAWS),
        "light_mark_draws_cs",
    );
    let mark_draws_open = pipeline(
        &device,
        "light_mark_draws_open by hand",
        src(SHADER_LIGHT_MARK_DRAWS_OPEN),
        "light_mark_draws_open_cs",
    );
    let mut total = Tally::default();
    for (name, at) in [
        ("east of the seam", real_world::EYE),
        ("west of the seam", [(1 << 23) - 300, (40 << 16) + 30]),
    ] {
        let t0 = std::time::Instant::now();
        let built = real_world::build_with_decors(at, 1200);
        let unplaced = real_world::build(at).objects;
        let world = built.world();
        let without = World {
            objects: &unplaced,
            ..world
        };
        let make = |w: &[u32]| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytes(w),
                usage: wgpu::BufferUsages::STORAGE,
            })
        };
        let clip_flat: Vec<u32> = built.clipmap.iter().flatten().map(|&v| v as u32).collect();
        let shards_flat: Vec<u32> = built.shards.iter().flatten().copied().collect();
        // What the core shows: for each decor, a bit for each clip level it stands at.
        let mut shown = [0u32; ROWS.len()];
        for (k, d) in ROWS.iter().enumerate() {
            for cell_zoom in FIRST_CELL_ZOOM..=LEAF_CELL_ZOOM {
                if cell_zoom >= scatter::first_cell_zoom(d) {
                    shown[k] |= 1 << (cell_zoom - FIRST_CELL_ZOOM);
                }
            }
        }
        let meshes: Vec<u32> = (0..generation::GROUPS)
            .map(|g| if g % 3 == 2 { 0 } else { 36 + 6 * g })
            .collect();
        let (clipmap, ground, shards, placing, no_placing, pool, rows, shown_buf, meshes_buf) = (
            make(&clip_flat),
            make(&built.ground),
            make(&shards_flat),
            make(&built.objects),
            make(&unplaced),
            make(&built.pool),
            make(rows_words()),
            make(&shown),
            make(&meshes),
        );
        // Placed decor as the core hands it, five more than one workgroup's runs, so it spills
        // into a second workgroup.
        let placed = generation::RUNS + 5;
        let cap = generation::ENTRY_RECS + placed.next_power_of_two().max(64);
        let workgroups = generation::EYE_ENTRIES + placed.div_ceil(generation::RUNS);
        let mut init = vec![0u32; generation::rec_at(cap) as usize];
        init[generation::PLACED_HDR as usize] = placed;
        for n in 0..placed {
            let r = generation::rec_at(generation::ENTRY_RECS + n) as usize;
            let j = n % ROWS.len() as u32;
            let p = built.decors[n as usize];
            let (variant, turn) = generation::placed_pose(&ROWS[j as usize], p);
            init[r] = generation::rec_word(
                j,
                variant,
                turn,
                scatter::coarsest_stand(&ROWS[j as usize], variant),
            );
            init[r + 1] = p[0] as u32;
            init[r + 2] = p[1] as u32;
            init[r + 3] = p[2] as u32;
        }
        // The frames. The later frames take their boxes from what the first reported.
        let origin = [at[0] << 8, at[1] << 8, (built.top_at(at) + 30) << 8];
        let mut drawing = drawing_words(0.02, 0.0002);
        drawing[drawing::ANCHOR as usize] = at[0] as u32;
        drawing[drawing::ANCHOR as usize + 1] = at[1] as u32;
        let sizing = Sizing {
            name: "a frame",
            origin,
            range: [13, 32],
            drawing,
        };
        let first = Frame {
            name: "first, every entry whole",
            marking: true,
            every: true,
            boxes: Vec::new(),
            stale: Vec::new(),
            placed_decor: true,
            cuts: [Cut::NONE; 2],
            sizing: sizing.clone(),
        };
        let mut model = Model {
            recs: init.clone(),
            shown,
            workgroups,
        };
        let mut tally = Tally::default();
        let mut claims = Vec::new();
        let mut wants = Vec::new();
        let mut step =
            |model: &mut Model, frame: Frame, tally: &mut Tally, claims: &mut Vec<[i32; 3]>| {
                let w = if frame.placed_decor { world } else { without };
                let left = model.step(w, &frame, tally, claims);
                eprintln!("{name}, {}: {} pairs", frame.name, pairs_of(&left).len());
                wants.push((frame, left));
            };
        step(&mut model, first.clone(), &mut tally, &mut claims);
        let reach = |p: [i32; 3], r: i32| [p[0] - r, p[1] - r, p[0] + r, p[1] + r];
        let mut moved = sizing.clone();
        moved.origin[0] = moved.origin[0].wrapping_add(40 << 8);
        moved.origin[1] += 25 << 8;
        step(
            &mut model,
            Frame {
                name: "the eye moved",
                marking: false,
                every: false,
                sizing: moved,
                ..first.clone()
            },
            &mut tally,
            &mut claims,
        );
        let damage: Vec<([i32; 4], Zoom)> = claims
            .iter()
            .take(12)
            .map(|&p| (reach(p, 6), generation::EVERY))
            .collect();
        tally.add("frame: a placed decor took a record", !damage.is_empty());
        if damage.is_empty() {
            total.merge(tally);
            continue;
        }
        let stale: Vec<u32> = (0..generation::EYE_ENTRIES)
            .filter(|s| mix(*s, [11, 2]).is_multiple_of(40))
            .collect();
        step(
            &mut model,
            Frame {
                name: "the placed decor gone, some stale",
                every: false,
                boxes: damage.clone(),
                stale,
                placed_decor: false,
                ..first.clone()
            },
            &mut tally,
            &mut claims,
        );
        let row = |s: u32| built.clipmap[s as usize];
        let bricks: Vec<([i32; 4], Zoom)> = (0..9u32)
            .map(|k| {
                let cell_zoom = 22 + k % 3;
                let s = (cell_zoom - FIRST_CELL_ZOOM) * ENTRIES + k * 7;
                (
                    generation::brick_rect(cell_zoom, [row(s)[0], row(s)[1]]),
                    cell_zoom,
                )
            })
            .collect();
        step(
            &mut model,
            Frame {
                name: "bricks handed",
                every: false,
                boxes: bricks,
                placed_decor: false,
                ..first.clone()
            },
            &mut tally,
            &mut claims,
        );
        step(
            &mut model,
            Frame {
                name: "the placed decor back",
                every: false,
                boxes: damage.clone(),
                ..first.clone()
            },
            &mut tally,
            &mut claims,
        );
        // A carve over a placed decor that took a record, with a box over the cut.
        let p = claims[0];
        let cut_rect = object::cut_rect(20, object::ancestor_at(24, p, 4));
        step(
            &mut model,
            Frame {
                name: "a cut",
                every: false,
                boxes: vec![(cut_rect, generation::EVERY)],
                cuts: [
                    Cut {
                        zoom: 20,
                        at: object::ancestor_at(24, p, 4),
                    },
                    Cut::NONE,
                ],
                ..first.clone()
            },
            &mut tally,
            &mut claims,
        );
        // The corner left out is that of the first placed decor the frame before drew, so the
        // frame draws it as nothing.
        let mut hide = sizing.clone();
        let c = (0..placed)
            .find(|&n| {
                let r = generation::rec_at(generation::ENTRY_RECS + n) as usize;
                model.recs[r] >> drawing::REGION_SHIFT != drawing::NONE
            })
            .map(|n| built.decors[n as usize])
            .expect("a placed decor drawn");
        for (a, v) in c.iter().enumerate() {
            hide.drawing[drawing::HIDE as usize + a] = *v as u32;
        }
        hide.drawing[drawing::HIDE as usize + 3] = 1;
        step(
            &mut model,
            Frame {
                name: "a corner left out",
                marking: false,
                every: false,
                sizing: hide,
                ..first.clone()
            },
            &mut tally,
            &mut claims,
        );
        eprintln!("{name}: the Rust's frames in {:?}", t0.elapsed());
        // Each chain runs every frame on the device from the same start, and each frame's
        // words are compared with the Rust's.
        let chain_on = || {
            let usage = wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST;
            let buf = |w: &[u32], extra: wgpu::BufferUsages| {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytes(w),
                    usage: usage | extra,
                })
            };
            Chain {
                device: &device,
                queue: &queue,
                pipelines: &hand_pipes,
                mark_draws: &mark_draws,
                mark_draws_open: &mark_draws_open,
                recs: buf(&init, wgpu::BufferUsages::empty()),
                list: buf(
                    &vec![0; generation::LIST_WORDS as usize],
                    wgpu::BufferUsages::INDIRECT,
                ),
                regions: buf(
                    &vec![0; 2 * drawing::REGIONS as usize],
                    wgpu::BufferUsages::empty(),
                ),
                args: buf(
                    &vec![0xdead_beef; (drawing::ARGS_WORDS * drawing::REGIONS) as usize],
                    wgpu::BufferUsages::empty(),
                ),
                uniforms: buf(&fresh_uniforms(cap), wgpu::BufferUsages::empty()),
                instances: buf(
                    &vec![0; ((drawing::COLUMNS + 1) * cap) as usize],
                    wgpu::BufferUsages::empty(),
                ),
                cap,
                workgroups,
            }
        };
        for run in 0..runs {
            let chain = chain_on();
            let t1 = std::time::Instant::now();
            let mut apart = 0;
            for (frame, want) in &wants {
                let b = WorldBinding {
                    clipmap: &clipmap,
                    ground: &ground,
                    shards: &shards,
                    objects: if frame.placed_decor {
                        &placing
                    } else {
                        &no_placing
                    },
                    pool: &pool,
                    rows: &rows,
                    shown: &shown_buf,
                    meshes: &meshes_buf,
                };
                let got = chain.frame(frame, &b);
                let mut d = frame_differs(
                    &got,
                    want,
                    &built.clipmap,
                    workgroups,
                    &frame.sizing.drawing,
                    cap,
                    &meshes,
                );
                d.extend(stood_differs(&got, &frame.sizing, cap, &mut total));
                let on = u64::from(got.stood[light::RECEIVER_TILES_CNT as usize]);
                *total.0.entry("frame: tiles decor stands on").or_default() += on;
                for line in d.iter().take(8) {
                    eprintln!("{name}, {}, run {run}: {line}", frame.name);
                }
                apart += d.len() as u64;
            }
            eprintln!("{name}: run {run}: {apart} apart, {:?}", t1.elapsed());
            *total.0.entry("apart").or_default() += apart;
            *total.0.entry("chains").or_default() += 1;
        }
        total.merge(tally);
    }
    for (k, v) in &total.0 {
        if k.starts_with("frame") || k.starts_with("apart") || k.starts_with("chains") {
            eprintln!("{v:>12} {k}");
        }
    }
    assert_eq!(total.0["apart"], 0);
    assert!(
        total.0["frame: a record comes back"] > 0
            && total.0["frame: a record taken away"] > 0
            && total.0["frame: tiles decor stands on"] > 0
            && total.0["frame: draws out of view"] > 0
            && total.0["frame: draws in view past the light's reach"] > 0
            && total.0["frame: a placed decor took a record"] > 0
    );
}

/// seed.wgsl `halve` must halve as `halve::halve` does. The cases take counts, rooms and
/// weights at random, and add fractions that meet their draw exactly, which go to the second
/// half.
#[test]
fn hand_halve_as_the_rust() {
    let Some((device, queue)) = device() else {
        return;
    };
    let mut draw = real_world::Draw(0x4a1f_e5d3);
    let mut asks: Vec<[u32; 4]> = (0..4096)
        .map(|_| {
            let caps = [draw.next() % 3000, draw.next() % 3000];
            [
                draw.next() % (caps[0] + caps[1] + 1),
                caps[0],
                caps[1],
                draw.next(),
            ]
        })
        .collect();
    asks.extend([
        [1, 1, 1, 0x8000_8000],
        [3, 3, 3, 0x5555_ffff],
        [0, 0, 0, 0],
        [1, 1, 0, u32::MAX],
        [0xffff, 0xffff, 0xffff, u32::MAX],
        [0xffff, 0xffff, 0xffff, 0],
    ]);
    let source = [
        include_str!(concat!(env!("OUT_DIR"), "/shader/constant.wgsl")),
        include_str!(concat!(env!("OUT_DIR"), "/shader/world_word.wgsl")),
        include_str!(concat!(env!("OUT_DIR"), "/shader/decor_word.wgsl")),
        include_str!(concat!(env!("OUT_DIR"), "/shader/seed.wgsl")),
        r"
@group(0) @binding(12) var<storage, read> asked: array<vec4<u32>>;
@group(0) @binding(13) var<storage, read_write> told: array<u32>;

@compute @workgroup_size(64)
fn halve_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x < arrayLength(&asked)) {
    let a = asked[id.x];
    told[id.x] = halve(a.x, a.yz, a.w);
  }
}
",
    ]
    .join("\n");
    let p = pipeline(&device, "halve", &source, "halve_cs");
    let flat: Vec<u32> = asks.iter().flatten().copied().collect();
    let (asked, told) = (buf(&device, &flat), buf(&device, &vec![0; asks.len()]));
    dispatch(
        &device,
        &queue,
        &p,
        &[(12, &asked), (13, &told)],
        [(asks.len() as u32).div_ceil(64), 1, 1],
    );
    let got = read(&device, &queue, &told, "halve");
    for (k, &[n, cap_0, cap_1, w]) in asks.iter().enumerate() {
        assert_eq!(
            got[k],
            halve::halve(n, [cap_0, cap_1], w),
            "{n} {cap_0} {cap_1} {w:#x}"
        );
    }
}

/// decor_scatter.wgsl `scatter_cnt_of` must count as `scatter::cnt_of` does. The cases take
/// decors with seeds, chances and roots other than the catalog's, with means of many to a root
/// column so the rooms bind, at every zoom from the root to the leaves. They add root columns
/// whose draw lands on a count's rounding edge.
#[test]
fn hand_scatter_count_as_the_rust() {
    let Some((device, queue)) = device() else {
        return;
    };
    let mut draw = real_world::Draw(0x0dec_0c47);
    let mut asked: Vec<[u32; 6]> = Vec::new();
    // A chance at which a root column's mean holds at most `per_root` counts, in 16.16 fixed
    // point.
    let chance_for = |root: Zoom, per_root: u32| {
        let shift = 2 * (LEAF_CELL_ZOOM - root);
        (per_root >> shift.min(31)).clamp(1, 0xffff) << 16
    };
    for _ in 0..6000 {
        let root = FIRST_CELL_ZOOM + draw.next() % (LEAF_CELL_ZOOM - FIRST_CELL_ZOOM + 1);
        // Zooms from three over the root, where a column holds none, down to the leaves.
        let zoom = (root + draw.next() % (LEAF_CELL_ZOOM - root + 4)).saturating_sub(3);
        let per_root = [1 << 14, 1 << 16, 3 << 17, 1 << 20, 1 << 24][draw.next() as usize % 5];
        let chance = chance_for(root, per_root);
        let b = [
            draw.within(-(1 << 20), 1 << 20),
            draw.within(-(1 << 20), 1 << 20),
        ];
        asked.push([draw.next(), chance, root, zoom, b[0] as u32, b[1] as u32]);
    }
    // The rounding edge: `mean + (draw & 0xffff)` lands on a whole count with `draw` odd. Only
    // an odd mean reaches it, and only a root at the leaves has one.
    let mut edges = 0;
    for (seed, root, per_root) in [
        (0x7902_530c, 24, 2049),
        (0x1234_5678, 24, 0x4_0001),
        (0xb0a5_802d, 24, 777),
    ] {
        let chance = chance_for(root, per_root);
        let mean = (chance >> 16) << (2 * (LEAF_CELL_ZOOM - root));
        for k in 0..1 << 22 {
            let c = [k % 2048 - 1024, k / 2048 - 1024];
            let d = node_seed(seed ^ scatter::CNT_TAG, (root, c));
            if d & 1 == 1 && (mean + (d & 0xffff)).is_multiple_of(1 << 16) {
                for zoom in [root, LEAF_CELL_ZOOM] {
                    let b = [c[0] << (zoom - root), c[1] << (zoom - root)];
                    asked.push([seed, chance, root, zoom, b[0] as u32, b[1] as u32]);
                }
                edges += 1;
            }
        }
    }
    let source = [
        include_str!(concat!(env!("OUT_DIR"), "/shader/constant.wgsl")),
        include_str!(concat!(env!("OUT_DIR"), "/shader/walk_word.wgsl")),
        include_str!(concat!(env!("OUT_DIR"), "/shader/world_word.wgsl")),
        include_str!(concat!(env!("OUT_DIR"), "/shader/decor_word.wgsl")),
        include_str!(concat!(env!("OUT_DIR"), "/shader/seed.wgsl")),
        include_str!(concat!(env!("OUT_DIR"), "/shader/decor_row.wgsl")),
        include_str!(concat!(env!("OUT_DIR"), "/shader/decor_scatter.wgsl")),
        r"
@group(0) @binding(12) var<storage, read> asked: array<u32>;
@group(0) @binding(13) var<storage, read_write> told: array<u32>;

@compute @workgroup_size(64)
fn count_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let a = 6u * id.x;
  if (a < arrayLength(&asked)) {
    let b = vec2<i32>(i32(asked[a + 4u]), i32(asked[a + 5u]));
    told[id.x] = scatter_cnt_of(asked[a], asked[a + 1u], asked[a + 2u], asked[a + 3u], b);
  }
}
",
    ]
    .join("\n");
    let p = pipeline(&device, "count", &source, "count_cs");
    let flat: Vec<u32> = asked.iter().flatten().copied().collect();
    let (asked_buf, told) = (buf(&device, &flat), buf(&device, &vec![0; asked.len()]));
    dispatch(
        &device,
        &queue,
        &p,
        &[(12, &asked_buf), (13, &told)],
        [(asked.len() as u32).div_ceil(64), 1, 1],
    );
    let got = read(&device, &queue, &told, "count");
    let (mut bound, mut many, mut under) = (0, 0, 0);
    for (k, a) in asked.iter().enumerate() {
        let b = [a[4] as i32, a[5] as i32];
        let want = scatter::cnt_of([a[0], a[1], a[2]], a[3], b);
        assert_eq!(got[k], want, "{a:?}");
        many += (want > 1) as u32;
        // A leaf of a root column that holds four or more, where the rooms bound the count on
        // the way down.
        if a[3] == LEAF_CELL_ZOOM {
            let t = a[3] - a[2];
            let at_root = scatter::cnt_of([a[0], a[1], a[2]], a[2], [b[0] >> t, b[1] >> t]);
            bound += (at_root >= 4) as u32;
        }
        under += (a[3] < a[2]) as u32;
    }
    eprintln!(
        "count: {} asked, {edges} on a rounding edge, {many} past one, {bound} under a full root, {under} over a root",
        asked.len()
    );
    assert!(edges > 0 && many > 0 && bound > 0 && under > 0);
}
