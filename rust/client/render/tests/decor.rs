//! Decor's passes by hand (frame/shader/decor_pose.wgsl, decor_args.wgsl) answer as the
//! Rust does: over the real world of tests/real_world, each record a thread leaves as
//! `generation::placement` stands it through the real walk (`support::column_surface`),
//! word for word (doc/experiment/20261104-taking-rust-gpu-out/decor). Skipped where no
//! adapter answers.

mod real_world;

use std::collections::BTreeMap;

use pxl_content::decor::{Decor, Orientation, DECORS};
use pxl_content::generation::{self, drawn, DecorQuery, BRICK_RECORDS, RECORD_WORDS};
use pxl_content::scatter;
use pxl_engine::answer::{self, In};
use pxl_engine::cast::{Detail, End, Query, Spacing};
use pxl_engine::clipmap::{self as clip, Lookup, BRICK, BRICK_CELLS, ENTRIES, FIRST_CELL};
use pxl_engine::grid;
use pxl_engine::ground::Resident;
use pxl_engine::light::{self, Light};
use pxl_engine::overwrite::{self, Cut};
use pxl_engine::seed::node_seed;
use pxl_engine::support::{self, Ancestors};
use pxl_engine::way::tree;
use pxl_engine::world::{self, Inheritance, PlacedCursor};
use pxl_engine::{area, eye, value, wrap_idx, Zoom, LEAF_CELL};
use pxl_render::frame::shader::record::{
    src, SHADER_GENERATION, SHADER_GENERATION_ARGS, SHADER_GENERATION_CNT,
    SHADER_GENERATION_COMPACT, SHADER_GENERATION_MARK, SHADER_GENERATION_THIN,
    SHADER_LIGHT_MARK_DRAWS,
};
use wgpu::util::DeviceExt;

use real_world::{mix, Built, World};

/// Why the answers fell as they did, a kind of answer each.
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

/// The world unplaced as decor's generation walks it, the test's reference, its visits told.
struct Ground<'w> {
    w: World<'w>,
    cuts: [Cut; 2],
    visits: u64,
}

impl Query for Ground<'_> {
    type At = Inheritance;

    fn root(&mut self, _: [i32; 3]) -> Inheritance {
        Inheritance::UNPLACED
    }

    fn child(&mut self, at: &Inheritance, zoom: Zoom, node: [i32; 3]) -> Inheritance {
        at.child(&self.w, &tree::Read(self.w.pool), (zoom, node))
    }

    fn node(
        &mut self,
        at: &mut Inheritance,
        zoom: Zoom,
        c: [i32; 3],
        zooms: [Zoom; 2],
        _: f32,
    ) -> In<[u32; 2]> {
        self.visits += 1;
        let mut held = self.w;
        let pool = tree::Read(self.w.pool);
        world::node(
            &mut held,
            &self.w,
            &self.w,
            &pool,
            at,
            (zoom, c),
            zooms,
            self.cuts,
        )
    }
}

/// The line a cell is asked off (`DecorQuery::asset`): the column surface's, or, as edited,
/// the cell's root's alone.
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
    fn zoom(&self) -> Zoom {
        if self.edited {
            0
        } else {
            self.line.zoom()
        }
    }
    fn ancestor(&self, z: Zoom) -> Inheritance {
        if self.edited {
            self.root
        } else {
            self.line.ancestor(z)
        }
    }
}

/// `generation::DecorQuery` over the real world, the test's reference, what it answers tallied.
struct Generating<'w, 't> {
    w: World<'w>,
    cuts: [Cut; 2],
    tally: &'t mut Tally,
    walk_visits: u64,
    ask_visits: u64,
    /// `next_placed`'s `&self`: whether it was asked, a decor and another thing met, a
    /// decor left out met; and the last decor met.
    placed: std::cell::Cell<[u64; 4]>,
    last_decor: std::cell::Cell<[i32; 3]>,
    /// The decors whose boxes took a record.
    claims: Vec<[i32; 3]>,
    /// Whether the column last asked no clip level holds.
    last_lacks: bool,
}

impl DecorQuery for Generating<'_, '_> {
    type ColumnSurface = support::ColumnSurface<Inheritance>;

    fn rows(&self) -> &[Decor] {
        &DECORS
    }

    fn column_surface(
        &mut self,
        cell: Zoom,
        g: [i32; 2],
    ) -> (Self::ColumnSurface, (i32, f32), bool) {
        let mut ground = Ground {
            w: self.w,
            cuts: [Cut::NONE; 2],
            visits: 0,
        };
        let f = support::column_surface(&mut ground, cell, g);
        self.walk_visits += ground.visits;
        self.tally.add("surface: columns walked", true);
        assert!(
            f.ends != End::Lacks,
            "a listed pair's brick holds its leaves' columns"
        );
        self.tally
            .add("surface: nothing fills", f.ends == End::Miss);
        let met = f.ends == End::Hit;
        let mut held = self.w;
        let wrapped = wrap_idx(cell, g[0]);
        let shard = area::Shard::UNKNOWN.at(&self.w, (cell, [wrapped, g[1]]));
        self.tally.add("surface: past the area", shard.past());
        if met && !shard.past() {
            let col = held.cell(shard, Lookup::NONE, cell, [wrapped, g[1], f.top.0]);
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
                col.palette_idx == value::VOID,
            );
            self.tally.add("surface: a part of its cube", f.top.1 < 1.0);
            self.tally
                .add("surface: a wall", col.palette_idx == value::UNKNOWN);
            // What the hand pass asks of the world: where ways bound the column, its cells
            // down to the surface; else none.
            let (b, _, _) = clip::bound_over(
                &held,
                &mut Lookup::NONE.clone(),
                [cell, cell],
                (cell, [wrapped, g[1]]),
            );
            let [lo, hi] = b[1];
            if lo < hi {
                let start = ((hi - 1) >> (LEAF_CELL - cell)).max(col.top - 1);
                self.tally.add("hand: columns walked", true);
                *self.tally.0.entry("hand: cells walked").or_default() +=
                    (start - f.top.0 + 1) as u64;
                // The cell it met, as the walk asked it.
                let mut ground = Ground {
                    w: self.w,
                    cuts: [Cut::NONE; 2],
                    visits: 0,
                };
                let top = [wrapped, g[1], f.top.0];
                let kept = answer::fill_eighths(support::asset(&mut ground, &f.line, cell, top));
                self.tally
                    .add("walk: met a part of its cube", kept < answer::EIGHTHS);
                self.tally
                    .add("walk: met an odd part of its cube", kept % 2 == 1);
            }
        }
        if f.ends == End::Miss && !shard.past() {
            let col = held.cell(shard, Lookup::NONE, cell, [wrapped, g[1], 0]);
            self.tally.add(
                "surface: void column, no way",
                col.palette_idx == value::VOID,
            );
        }
        (f, f.top, met)
    }

    fn finest(&mut self, cell: Zoom, g: [i32; 2]) -> Zoom {
        let (f, _) = clip::step(&self.w, Lookup::NONE, LEAF_CELL, g);
        assert!(
            f.here() && f.from() >= cell,
            "a listed pair's brick holds its leaf columns, at its clip level or finer"
        );
        self.tally.add(
            "finest: a finer clip level holds the leaf column",
            f.from() > cell,
        );
        self.tally
            .add("finest: its own clip level is the finest", f.from() == cell);
        f.from()
    }

    fn asset(&mut self, f: &Self::ColumnSurface, zoom: Zoom, c: [i32; 3], edited: bool) -> u32 {
        let root_at = overwrite::ancestor_at(zoom, c, zoom);
        let (cuts, root) = if edited {
            (self.cuts, Inheritance::of(&self.w, (0, root_at)))
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
            w: self.w,
            cuts,
            visits: 0,
        };
        let v = support::asset(&mut ground, &line, zoom, c);
        self.ask_visits += ground.visits;
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
            self.tally.add(
                "edit: part of its cube",
                answer::fill_eighths(v) < answer::EIGHTHS,
            );
            self.tally.add("edit: at a coarse zoom", zoom < LEAF_CELL);
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

    fn next_placed(&self, p: &mut PlacedCursor) -> ([i32; 3], u32, bool) {
        let (c, word, more) = p.next(&self.w);
        let mut t = self.placed.get();
        t[0] += 1;
        if more {
            let (_, decor) = generation::row_of(&DECORS, word & 0xffff);
            t[if decor { 1 } else { 2 }] += 1;
            t[3] += (decor && c == self.cuts[1].at && self.cuts[1].zoom == LEAF_CELL) as u64;
            if decor {
                self.last_decor.set(c);
            }
        }
        self.placed.set(t);
        (c, word, more)
    }
}

/// What a thread leaves of a record.
fn stand(q: &mut Generating, j: u32, cell: Zoom, leaf: [i32; 2]) -> (u32, u32, u32) {
    let decor = &DECORS[j as usize];
    let c = scatter::candidate_at(decor, leaf).0;
    let (g, has_footing, generates) = generation::placement(q, decor, cell, &c, true);
    let word = if generates {
        generation::record_word(j, g.variant, g.turn, cell)
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
        q.tally.add("generates: at the leaves", cell == LEAF_CELL);
        q.tally
            .add("generates: at a coarser clip level", cell < LEAF_CELL);
        let along = decor.orientation == Orientation::along_transport;
        q.tally.add("vote: across", along && g.turn & 1 == 1);
        q.tally.add("vote: along", along && g.turn & 1 == 0);
        q.tally.add("vote: flipped", along && g.turn >= 2);
        q.tally.add("generates: sunk under its corner", g.sink != 0);
    }
    (word, g.at[2] as u32, g.sink)
}

/// A dispatch's pairs, each an entry, a decor and whether in part, the frame's boxes and cuts.
struct Round {
    name: &'static str,
    pairs: Vec<(u32, u32, bool)>,
    boxes: Vec<([i32; 4], Zoom)>,
    cuts: [Cut; 2],
}

/// The records a pair holds (`decor_thin_cs`'s), and how many it says it laid.
#[derive(Default, Clone)]
struct PairLeaves {
    leaves: Vec<[i32; 2]>,
    cnt: u32,
}

/// What each pair holds: what `decor_thin_cs` lays, its candidates off the hashes, then more
/// wherever something asks a case, any leaf whose variant stands at the clip level: a
/// thread asks only what stands.
fn pairs_about(built: &Built, edges: &[[i32; 2]]) -> BTreeMap<(u32, u32), PairLeaves> {
    let mut pairs: BTreeMap<(u32, u32), PairLeaves> = BTreeMap::new();
    let cells = [22u32, 23, 24];
    for cell in cells {
        for e in 0..ENTRIES {
            let s = (cell - FIRST_CELL) * ENTRIES + e;
            let row = built.clipmap[s as usize];
            if row[2] == 0 {
                continue;
            }
            for (j, decor) in DECORS.iter().enumerate() {
                if !scatter::any_stands_at(decor, cell) {
                    continue;
                }
                let [x0, y0, _, _] = generation::brick_box(cell, [row[0], row[1]]);
                let t = LEAF_CELL - cell;
                let mut leaves = Vec::new();
                for i in 0..BRICK_CELLS {
                    let corner = [
                        x0 + ((i % BRICK) << t) as i32,
                        y0 + ((i / BRICK) << t) as i32,
                    ];
                    if scatter::count(j, cell, [corner[0] >> t, corner[1] >> t]) == 0 {
                        continue;
                    }
                    let side = 1u32 << t;
                    for n in 0..side * side {
                        let leaf = [corner[0] + (n % side) as i32, corner[1] + (n / side) as i32];
                        let (c, here) = scatter::candidate_at(decor, leaf);
                        if here
                            && scatter::stands_at(decor, scatter::variant(decor, &c), cell)
                            && scatter::retained(decor, &c)
                        {
                            leaves.push(leaf);
                        }
                    }
                }
                pairs.insert((s, j as u32), PairLeaves { leaves, cnt: 0 });
            }
        }
    }
    // A leaf and those about it, in whichever pair may hold it.
    let mut near = |at: [i32; 2], reach: i32, step: i32| {
        for cell in cells {
            let k = clip::BRICK_SHIFT + LEAF_CELL - cell;
            let mut dy = -reach;
            while dy <= reach {
                let mut dx = -reach;
                while dx <= reach {
                    let leaf = [real_world::wrap(at[0] + dx), at[1] + dy];
                    let q = [
                        wrap_idx(clip::bricks_zoom(cell), leaf[0] >> k),
                        leaf[1] >> k,
                    ];
                    let s = (cell - FIRST_CELL) * ENTRIES + clip::entry(q);
                    let row = built.clipmap[s as usize];
                    if row[2] != 0 && [row[0], row[1]] == q {
                        for (j, decor) in DECORS.iter().enumerate() {
                            let c = scatter::candidate_at(decor, leaf).0;
                            let stands =
                                scatter::stands_at(decor, scatter::variant(decor, &c), cell);
                            if let Some(p) = pairs.get_mut(&(s, j as u32)) {
                                if stands
                                    && (p.leaves.len() as u32) < decor.cap()
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
    for (n, p) in built.folds.iter().enumerate() {
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

/// Word `at`'s bytes.
fn bytes(words: &[u32]) -> &[u8] {
    // SAFETY: every u32's bytes are u8s, in the slice's own lifetime.
    unsafe { std::slice::from_raw_parts(words.as_ptr().cast(), words.len() * 4) }
}

/// What a dispatch may take, past which its label says which never ended.
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
    let done = device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(DISPATCH_LIMIT),
    });
    assert!(
        done.is_ok(),
        "{label}: not done in {DISPATCH_LIMIT:?}: {done:?}"
    );
    let v = staging.slice(..).get_mapped_range().unwrap();
    v.as_chunks::<4>()
        .0
        .iter()
        .map(|b| u32::from_le_bytes(*b))
        .collect()
}

/// A compute pipeline of `source`, refused where its module is.
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

/// Shader `kind` by hand, as the renderer makes its pipeline: over the group it states
/// (gpu/generator.rs), not one read off the module.
fn hand_pipeline(device: &wgpu::Device, kind: u32) -> wgpu::ComputePipeline {
    use pxl_render::api_spec::record::{STORAGE, STORAGE_RW, UNIFORM};
    use pxl_render::gpu::generator::{
        ARGS_GROUP, CNT_GROUP, COMPACT_GROUP, GENERATE_GROUP, MARK_GROUP, THIN_GROUP,
    };
    let (group, entry): (&[u32], &str) = match kind {
        SHADER_GENERATION => (&GENERATE_GROUP, "decor_cs"),
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
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
        eprintln!("no adapter: skipped");
        return None;
    };
    Some(
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .unwrap(),
    )
}

/// The first word of entry `s`'s `i`-th record of decor `j`.
fn word_of(s: u32, j: u32, i: u32) -> usize {
    generation::record_at(generation::entry_record(&DECORS, s, j, i)) as usize
}

/// The GPU's side of a world: its buffers, and `decor_cs` over them.
struct Gpu<'d> {
    device: &'d wgpu::Device,
    queue: &'d wgpu::Queue,
    clipmap: wgpu::Buffer,
    ground: wgpu::Buffer,
    shards: wgpu::Buffer,
    overwrites: wgpu::Buffer,
    pool: wgpu::Buffer,
    rows: wgpu::Buffer,
    hand: &'d wgpu::ComputePipeline,
}

impl Gpu<'_> {
    /// `pairs` run over `init`.
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
        let records = make(init, store);
        let list = make(list, store);
        let eye_buf = make(eye_words, wgpu::BufferUsages::UNIFORM);
        let x = generation::KIND_RECORDS.div_ceil(generation::LANES);
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
                    (6, &self.overwrites),
                    (7, &self.pool),
                    (9, &records),
                    (10, &self.rows),
                    (11, &list),
                ],
            );
            pass.set_pipeline(self.hand);
            pass.set_bind_group(0, &g, &[]);
            pass.dispatch_workgroups(x, 1, pairs);
        }
        self.queue.submit([e.finish()]);
        read(d, self.queue, &records, label)
    }
}

/// Whether a box of the round's reaches the leaf (`is_changed`).
fn is_changed(boxes: &[([i32; 4], Zoom)], cell: Zoom, leaf: [i32; 2]) -> bool {
    let at = [leaf[0], leaf[1], leaf[0] + 1, leaf[1] + 1];
    boxes
        .iter()
        .any(|&(b, z)| generation::reaches(b, z, cell, at))
}

/// A round's expected words: `want` over `init`, and those a test does not hold a thread to.
struct Expected {
    init: Vec<u32>,
    want: Vec<u32>,
    free: Vec<usize>,
    asked: u64,
    walk_visits: u64,
    ask_visits: u64,
    tally: Tally,
    claims: Vec<[i32; 3]>,
}

/// The words a round starts from: each pair's count, and its records as `decor_thin_cs` leaves
/// a whole pair's or a frame before left a pair's in part.
fn init_of(laid: &BTreeMap<(u32, u32), PairLeaves>, round: &Round) -> Vec<u32> {
    let words = generation::record_at(generation::SLOT_RECORDS) as usize;
    let mut init = vec![0u32; words];
    for (&(s, j), p) in laid {
        init[generation::cnt_at(s, j) as usize] = if p.cnt != 0 {
            p.cnt
        } else {
            p.leaves.len() as u32
        };
    }
    for &(s, j, part) in &round.pairs {
        let cell = clip::eye_cell(s / ENTRIES);
        for (i, leaf) in laid[&(s, j)].leaves.iter().enumerate() {
            let r = word_of(s, j, i as u32);
            let h = mix(i as u32 + 7, *leaf);
            // A whole pair's first words as a frame before left them, which thin keeps.
            init[r] = if !part {
                h
            } else if h.is_multiple_of(5) {
                generation::UNGENERATED
            } else {
                generation::record_word(j, h % 3, h >> 8 & 3, cell)
                    | (h >> 12 & 0x7f) << drawn::REGION_SHIFT
            };
            init[r + 1] = leaf[0] as u32;
            init[r + 2] = leaf[1] as u32;
            init[r + 3] = h >> 3;
            init[r + 4] = h >> 6;
        }
    }
    init
}

fn expect(built: &Built, laid: &BTreeMap<(u32, u32), PairLeaves>, round: &Round) -> Expected {
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
                    let (mut asked, mut walks, mut asks) = (0, 0, 0);
                    let mut claims = Vec::new();
                    for &(s, j, part) in pairs {
                        let cell = clip::eye_cell(s / ENTRIES);
                        let p = &laid[&(s, j)];
                        let cap = DECORS[j as usize].cap();
                        let cnt = if p.cnt != 0 {
                            p.cnt
                        } else {
                            p.leaves.len() as u32
                        };
                        tally.add("pair: more laid than its room", cnt > cap);
                        for (i, leaf) in p.leaves.iter().enumerate() {
                            if i as u32 >= cnt.min(cap) {
                                continue;
                            }
                            if part && !is_changed(&round.boxes, cell, *leaf) {
                                tally.add("part: not reached, it stands", true);
                                continue;
                            }
                            tally.add("part: reached", part);
                            let mut q = Generating {
                                w,
                                cuts: round.cuts,
                                tally: &mut tally,
                                walk_visits: 0,
                                ask_visits: 0,
                                placed: Default::default(),
                                last_decor: Default::default(),
                                claims: Vec::new(),
                                last_lacks: false,
                            };
                            let (word, z, sink) = stand(&mut q, j, cell, *leaf);
                            walks += q.walk_visits;
                            asks += q.ask_visits;
                            claims.append(&mut q.claims);
                            asked += 1;
                            out.push((word_of(s, j, i as u32), word, z, sink));
                        }
                    }
                    (out, tally, asked, walks, asks, claims)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut want = init.clone();
    let mut free = Vec::new();
    let mut tally = Tally::default();
    let (mut asked, mut walk_visits, mut ask_visits) = (0, 0, 0);
    let mut claims = Vec::new();
    for (out, t, a, wv, av, mut by) in results {
        claims.append(&mut by);
        tally.merge(t);
        asked += a;
        walk_visits += wv;
        ask_visits += av;
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
        walk_visits,
        ask_visits,
        tally,
        claims,
    }
}

/// The list a dispatch reads: its boxes, the pairs whole from its start, in part from its end.
fn list_of(round: &Round, pairs: &[(u32, u32, bool)]) -> (Vec<u32>, u32) {
    let mut list = vec![0u32; generation::LIST_WORDS as usize];
    let whole: Vec<_> = pairs.iter().filter(|p| !p.2).collect();
    list[generation::THIN_ARGS as usize + 2] = whole.len() as u32;
    list[generation::LIST_BOXES as usize] = round.boxes.len() as u32;
    for (k, &(b, z)) in round.boxes.iter().enumerate() {
        let at = generation::BOXES_AT as usize + k * generation::BOX_WORDS as usize;
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

/// Each word against what is expected; how many differ, the first few told.
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
                let (s, at) = if (k as u32) < generation::SLOTS_AT {
                    (
                        k as u32 / generation::HEAD_WORDS,
                        k as u32 % generation::HEAD_WORDS,
                    )
                } else {
                    let rec = (k as u32 - generation::SLOTS_AT) / RECORD_WORDS;
                    (
                        rec / BRICK_RECORDS,
                        (k as u32 - generation::SLOTS_AT) % RECORD_WORDS,
                    )
                };
                eprintln!(
                    "{name} {by}: word {k}, entry {s}, at {at}: got {g:#x} want {w:#x} (was {:#x}), leaf {:?}",
                    e.init[k],
                    if (k as u32) >= generation::SLOTS_AT {
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

/// Some leaf bricks' east neighbours held by no clip level, their rows cleared at every zoom: a
/// box from the west runs onto a column not held. What is about those edges, for records.
fn lack_columns(built: &mut Built) -> Vec<[i32; 2]> {
    let mut edges = Vec::new();
    let k = |cell: Zoom| clip::BRICK_SHIFT + LEAF_CELL - cell;
    for e in (5..ENTRIES).step_by(13) {
        let row = built.clipmap[((LEAF_CELL - FIRST_CELL) * ENTRIES + e) as usize];
        if row[2] == 0 {
            continue;
        }
        let east = [(row[0] + 1) << k(LEAF_CELL), row[1] << k(LEAF_CELL)];
        for cell in FIRST_CELL..=LEAF_CELL {
            let q = [
                wrap_idx(clip::bricks_zoom(cell), east[0] >> k(cell)),
                east[1] >> k(cell),
            ];
            built.clipmap[((cell - FIRST_CELL) * ENTRIES + clip::entry(q)) as usize][2] = 0;
        }
        edges.push([real_world::wrap(east[0] - 1), east[1] + 20]);
        if edges.len() == 4 {
            break;
        }
    }
    edges
}

/// Leaf cells round the world, X's lap.
const LAP: i32 = 1 << LEAF_CELL;

/// A round's cases about a point: cuts and part boxes over the pairs near it.
fn pairs_near(
    laid: &BTreeMap<(u32, u32), PairLeaves>,
    built: &Built,
    p: [i32; 2],
    reach: i32,
) -> Vec<(u32, u32, bool)> {
    laid.iter()
        .filter(|((s, _), l)| {
            let row = built.clipmap[*s as usize];
            let cell = clip::eye_cell(s / ENTRIES);
            let b = generation::brick_box(cell, [row[0], row[1]]);
            !l.leaves.is_empty()
                && wrap_idx(LEAF_CELL, p[0] - b[0]) + reach >= 0
                && wrap_idx(LEAF_CELL, p[0] - b[2]) - reach < 0
                && p[1] + reach >= b[1]
                && p[1] - reach < b[3]
        })
        .map(|(&(s, j), _)| (s, j, false))
        .collect()
}

/// The rounds of a world: all of it whole; then cuts and boxes about what stood there.
fn rounds(
    built: &Built,
    laid: &BTreeMap<(u32, u32), PairLeaves>,
    stood: &[(u32, u32, [i32; 2], i32)],
    claims: &[[i32; 3]],
) -> Vec<Round> {
    let none = [Cut::NONE; 2];
    let mut out = Vec::new();
    let leaf_of = |n: usize| stood[n % stood.len()];
    // A cut of the leaves over a record's box and at its base, and a coarser one over it.
    for n in 0..12 {
        let (_, _, leaf, z) = leaf_of(n * 37 + 3);
        let at = |zoom: Zoom, dz: i32| Cut {
            zoom,
            at: {
                let t = LEAF_CELL - zoom;
                [wrap_idx(zoom, leaf[0] >> t), leaf[1] >> t, (z + dz) >> t]
            },
        };
        let cut = match n % 4 {
            0 => at(LEAF_CELL, 0),
            1 => at(LEAF_CELL, -1),
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
    // Hidden: a placed decor still claims; another placed thing does not stand in the way.
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
                    zoom: LEAF_CELL,
                    at: p,
                },
            ],
        });
    }
    for n in 0..12 {
        let (zoom, c) = built.placed[n * 97 % built.placed.len()];
        let t = zoom.saturating_sub(LEAF_CELL);
        out.push(Round {
            name: "a placed leaf left out",
            pairs: pairs_near(laid, built, [c[0] >> t, c[1] >> t], 8),
            boxes: Vec::new(),
            cuts: [Cut::NONE, Cut { zoom, at: c }],
        });
    }
    // Whole and in part in one dispatch, boxes of every kind.
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
            // Laps of X: the box a lap west, and one the whole way round.
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
            // A brick filled at a coarser clip level alone: no candidate finer reads it.
            5 => {
                let k = clip::BRICK_SHIFT + LEAF_CELL - 22;
                let q = [leaf[0] >> k, leaf[1] >> k];
                vec![(generation::brick_box(22, q), 22)]
            }
            // A lap east, across the seam into the region; and one ending at its west edge a
            // lap round, and one past it.
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

/// A world's buffers on the device, and `decor_cs` over them.
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
        overwrites: make(&built.overwrites),
        pool: make(&built.pool),
        rows: make(rows_words()),
        hand,
    }
}

/// A world as `run_world` asks it: built with `extra` placed, its rows not held cleared, and
/// what its pairs lay.
fn world_of(eye: [i32; 2], extra: &[Leaf]) -> (Built, BTreeMap<(u32, u32), PairLeaves>) {
    let mut built = real_world::build_with(eye, 600, extra);
    let edges = lack_columns(&mut built);
    let mut laid = pairs_about(&built, &edges);
    // A pair that laid past its room, as many as its room read.
    if let Some((_, p)) = laid
        .iter_mut()
        .find(|(k, p)| k.1 == 3 && p.leaves.len() as u32 >= DECORS[3].cap() - 8)
    {
        p.cnt = DECORS[3].cap() + 3;
    }
    (built, laid)
}

/// What a round asks, against the Rust: each dispatch `decor_cs`'s, its words compared.
fn run_round(
    gpu: &Gpu,
    built: &Built,
    laid: &BTreeMap<(u32, u32), PairLeaves>,
    round: &Round,
    label: &str,
    totals: &mut Tally,
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
    eprintln!(
        "{label}, {}: {} asked, {} walk visits, {} ask visits; {d} differ",
        round.name, e.asked, e.walk_visits, e.ask_visits
    );
    *totals.0.entry("records asked").or_default() += e.asked;
    *totals.0.entry("walk visits").or_default() += e.walk_visits;
    *totals.0.entry("ask visits").or_default() += e.ask_visits;
    *totals.0.entry("words differing").or_default() += d;
    (e, hand)
}

/// A leaf placed, its zoom, its node and its word.
type Leaf = (Zoom, [i32; 3], u32);

/// A record that stood: its pair, its leaf, its box's height and its first word.
type StoodRecord = (u32, u32, [i32; 2], i32, u32);

/// What is placed in the boxes of records that stand at the leaves, and the records: a cube
/// over the base of each of `cubed`; and of each of `folded`, a wide one, a cube a sixty-fourth
/// of its cell in its first column's top cell, and a cube at its second column's base.
fn placed_in_boxes(stood: &[StoodRecord]) -> (Vec<Leaf>, Vec<StoodRecord>, Vec<StoodRecord>) {
    let object = overwrite::leaf_word(value::OBJECTS[0], 0);
    let at_leaves: Vec<StoodRecord> = stood
        .iter()
        .copied()
        .filter(|r| clip::eye_cell(r.0 / ENTRIES) == LEAF_CELL)
        .collect();
    let mut extra = Vec::new();
    let cubed: Vec<StoodRecord> = at_leaves.iter().step_by(37).take(12).copied().collect();
    for &(_, _, leaf, z, _) in &cubed {
        extra.push((LEAF_CELL, [leaf[0], leaf[1], z], object));
    }
    let mut folded = Vec::new();
    for &r in at_leaves.iter().skip(5) {
        let (j, variant, turn, _) = generation::record_of(r.4);
        let [w, d, h] = DECORS[j as usize].cells(variant, turn);
        if w * d < 2 || folded.iter().any(|f: &StoodRecord| f.2 == r.2) {
            continue;
        }
        let (leaf, base) = (r.2, r.3 - 1);
        let top = base + h as i32;
        let fine = LEAF_CELL + 2;
        extra.push((fine, [leaf[0] << 2, leaf[1] << 2, top << 2], object));
        let next = if w >= 2 {
            [leaf[0] + 1, leaf[1]]
        } else {
            [leaf[0], leaf[1] + 1]
        };
        extra.push((LEAF_CELL, [next[0], next[1], base], object));
        folded.push(r);
        if folded.len() == 12 {
            break;
        }
    }
    (extra, cubed, folded)
}

/// The rounds over records something is placed in the boxes of (`placed_in_boxes`): them all
/// whole; then each of the first six cubed, its cube left out.
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
                    zoom: LEAF_CELL,
                    at: [r.2[0], r.2[1], r.3],
                },
            ],
        });
    }
    out
}

/// Records whose boxes something is placed in (`placed_in_boxes`), on a world with it placed:
/// a cube over the base takes a record, and does not where it is left out; a sixty-fourth of
/// a cell in a box is no edit, nor a cube at a base, so those records stand.
fn run_placed(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    hand: &wgpu::ComputePipeline,
    eye: [i32; 2],
    label: &str,
    totals: &mut Tally,
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
    let (e, _) = run_round(&gpu, &built, &laid, &rounds[0], label, totals);
    let taken = cubed
        .iter()
        .filter(|r| word(&e, r, 0) == generation::UNGENERATED)
        .count();
    let standing = folded
        .iter()
        .filter(|r| word(&e, r, 0) != generation::UNGENERATED)
        .count();
    *totals
        .0
        .entry("box: a cube over its base takes it")
        .or_default() += taken as u64;
    *totals
        .0
        .entry("box: a fold under an eighth and a cube at a base leave it")
        .or_default() += standing as u64;
    totals.merge(e.tally);
    for (r, round) in cubed.iter().zip(&rounds[1..]) {
        let (e, _) = run_round(&gpu, &built, &laid, round, label, totals);
        let back = word(&e, r, 0) != generation::UNGENERATED;
        *totals
            .0
            .entry("box: a cube left out leaves it")
            .or_default() += back as u64;
        totals.merge(e.tally);
    }
}

fn run_world(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    hand: &wgpu::ComputePipeline,
    eye: [i32; 2],
    label: &str,
    totals: &mut Tally,
) -> Vec<StoodRecord> {
    let t0 = std::time::Instant::now();
    let (built, laid) = world_of(eye, &[]);
    let all_records: usize = laid.values().map(|p| p.leaves.len()).sum();
    eprintln!(
        "{label}: {all_records} records in {} pairs, built in {:?}",
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
    let run = |round: &Round, totals: &mut Tally| -> (Vec<u32>, Vec<[i32; 3]>) {
        let (e, hand) = run_round(&gpu, &built, &laid, round, label, totals);
        totals.merge(e.tally);
        (hand, e.claims)
    };
    let (hand, claims) = run(&first, totals);
    // What stood, for the cuts and boxes about it.
    let mut stood = Vec::new();
    let mut words = Vec::new();
    for (&(s, j), p) in &laid {
        for i in 0..p.leaves.len().min(DECORS[j as usize].cap() as usize) {
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
            run(&round, totals);
        }
    }
    eprintln!("{label}: {:?}", t0.elapsed());
    words
}

/// `decor_cs` by hand against `generation::placement`.
#[test]
fn hand_decor_answers_as_placement() {
    let Some((device, queue)) = device() else {
        return;
    };
    let hand = hand_pipeline(&device, SHADER_GENERATION);
    let mut totals = Tally::default();
    // East of the seam over the service area's south edge; west of it; south of the world's
    // equator, X and Y negative, all past the area.
    for (eye, label) in [
        (real_world::EYE, "east of the seam"),
        ([(1 << 23) - 300, (40 << 16) + 30], "west of the seam"),
        ([300, -50_670], "past the area"),
    ] {
        let stood = run_world(&device, &queue, &hand, eye, label, &mut totals);
        if !stood.is_empty() {
            run_placed(&device, &queue, &hand, eye, label, &mut totals, &stood);
        }
    }
    for (k, v) in &totals.0 {
        eprintln!("{v:>12} {k}");
    }
    assert_eq!(totals.0["words differing"], 0);
    // A box's column is asked from its lowest cell up; on a column no clip level holds that
    // reads void, on which no decor stands, so it never comes to an edit there.
    assert!(DECORS.iter().all(|d| !d.allows(value::VOID)));
    assert_eq!(totals.0["edit: asked over a column not held"], 0);
    let zero: Vec<_> = totals
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

/// A dispatch over `at`'s buffers, its wait bounded (`read`).
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

/// A buffer of `w`, read and written as a pass does.
fn buffer(device: &wgpu::Device, w: &[u32]) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytes(w),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
    })
}

fn rows_words() -> &'static [u32] {
    // SAFETY: the rows are `repr(C)` words alone (pxl-content decor.spec.rs).
    unsafe { std::slice::from_raw_parts(DECORS.as_ptr().cast::<u32>(), size_of_val(&DECORS) / 4) }
}

/// The candidates of decor `j` in brick `q` at `cell` that stand clear there, by `scatter`'s
/// rules: the test's reference for `decor_thin_cs`.
fn thin_leaves(j: usize, cell: Zoom, q: [i32; 2]) -> Vec<[i32; 2]> {
    let decor = &DECORS[j];
    let [x0, y0, _, _] = generation::brick_box(cell, q);
    let t = LEAF_CELL - cell;
    let mut leaves = Vec::new();
    for i in 0..BRICK_CELLS {
        let corner = [
            x0 + ((i % BRICK) << t) as i32,
            y0 + ((i / BRICK) << t) as i32,
        ];
        if scatter::count(j, cell, [corner[0] >> t, corner[1] >> t]) == 0 {
            continue;
        }
        let side = 1u32 << t;
        for n in 0..side * side {
            let leaf = [corner[0] + (n % side) as i32, corner[1] + (n / side) as i32];
            let (c, here) = scatter::candidate_at(decor, leaf);
            if here
                && scatter::stands_at(decor, scatter::variant(decor, &c), cell)
                && scatter::retained(decor, &c)
            {
                leaves.push(leaf);
            }
        }
    }
    leaves
}

/// `decor_thin_cs` by hand lays what the hashes hold, each pair's records in no order, however
/// its threads fall: pairs at every clip level a decor stands at, and one past them, none;
/// bricks either side of the seam, X and Y negative.
#[test]
fn hand_thin_lays_as_scatter() {
    let Some((device, queue)) = device() else {
        return;
    };
    let hand = hand_pipeline(&device, SHADER_GENERATION_THIN);
    let words = generation::record_at(generation::SLOT_RECORDS) as usize;
    let size = |pairs: u32| [BRICK_CELLS.div_ceil(generation::LANES), 1, pairs];
    let (mut laid_total, mut pairs_total) = (0u64, 0u64);
    let (mut none_past, mut full_rooms, mut stale_heads) = (0u64, 0u64, 0u64);
    for (name, eye) in [
        ("east of the seam", real_world::EYE),
        ("west of the seam", [(1 << 23) - 300, (40 << 16) + 30]),
        ("south", [300, -50_670]),
    ] {
        let built = real_world::build(eye);
        let rows = buffer(&device, rows_words());
        let mut pairs = Vec::new();
        let mut init = vec![0u32; words];
        let mut want = BTreeMap::new();
        // Every decor at the clip levels about those it stands at.
        for cell in 21..=24u32 {
            for e in 0..ENTRIES {
                let s = (cell - FIRST_CELL) * ENTRIES + e;
                let row = built.clipmap[s as usize];
                if row[2] == 0 {
                    continue;
                }
                let h = generation::head_at(s) as usize;
                init[h] = row[0] as u32;
                init[h + 1] = row[1] as u32;
                init[h + 2] = 1;
                for j in 0..DECORS.len() {
                    let leaves = thin_leaves(j, cell, [row[0], row[1]]);
                    none_past += (cell == 21 && leaves.is_empty()) as u64;
                    want.insert((s, j as u32), leaves);
                    pairs.push((s, j as u32));
                }
            }
        }
        let read_laid = |got: &[u32], s: u32, j: u32| -> (u32, Vec<[i32; 2]>) {
            let cnt = got[generation::cnt_at(s, j) as usize];
            let mut leaves = Vec::new();
            for i in 0..cnt.min(DECORS[j as usize].cap()) {
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
            let list = buffer(&device, &list);
            {
                let records = buffer(&device, &init);
                {
                    let at = [(9, &records), (10, &rows), (11, &list)];
                    dispatch(&device, &queue, &hand, &at, size(chunk.len() as u32));
                }
                let got = read(&device, &queue, &records, name);
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
                // By hand it lays a record's cell and nothing else of it.
                {
                    for &(s, j) in chunk {
                        for i in 0..DECORS[j as usize].cap() {
                            let r = word_of(s, j, i);
                            assert_eq!(
                                [got[r], got[r + 3], got[r + 4]],
                                [0; 3],
                                "{name}, {s} {j} {i}"
                            );
                        }
                    }
                }
            }
            pairs_total += chunk.len() as u64;
        }
        // A pair whose room is near full before thin runs: no record past its room, whatever
        // the count, each pair alone so any word past it is thin's.
        let full: Vec<(u32, u32)> = pairs
            .iter()
            .copied()
            .filter(|p| want[p].len() >= 3)
            .step_by(97)
            .take(4)
            .collect();
        for (s, j) in full {
            let cap = DECORS[j as usize].cap();
            let mut before = init.clone();
            before[generation::cnt_at(s, j) as usize] = cap - 2;
            let mut list = vec![0u32; generation::LIST_WORDS as usize];
            list[generation::THIN_ARGS as usize + 2] = 1;
            list[generation::LIST_AT as usize] = generation::pair_word(s, j);
            let (list, records) = (buffer(&device, &list), buffer(&device, &before));
            let at = [(9, &records), (10, &rows), (11, &list)];
            dispatch(&device, &queue, &hand, &at, size(1));
            let got = read(&device, &queue, &records, name);
            let n = want[&(s, j)].len() as u32;
            assert_eq!(
                got[generation::cnt_at(s, j) as usize],
                cap - 2 + n,
                "{name}, {s} {j}: its count"
            );
            let mut written = Vec::new();
            for i in cap - 2..cap {
                let r = word_of(s, j, i);
                written.push([got[r + 1] as i32, got[r + 2] as i32]);
                before[r + 1] = got[r + 1];
                before[r + 2] = got[r + 2];
            }
            for leaf in &written {
                assert!(
                    want[&(s, j)].contains(leaf),
                    "{name}, {s} {j}: {leaf:?} no candidate"
                );
            }
            assert!(
                written[0] != written[1],
                "{name}, {s} {j}: one candidate twice"
            );
            before[generation::cnt_at(s, j) as usize] = cap - 2 + n;
            assert!(got == before, "{name}, {s} {j}: a word past its room");
            full_rooms += 1;
        }
        // A head naming the brick east of the one its entry's row holds, as no mark of the
        // frame writes it: thin does not read the row, and lays the head's brick's candidates.
        let stale: Vec<(u32, u32)> = pairs
            .iter()
            .copied()
            .filter(|&(s, j)| {
                let row = built.clipmap[s as usize];
                let cell = FIRST_CELL + s / ENTRIES;
                let east = thin_leaves(j as usize, cell, [row[0] + 1, row[1]]);
                !east.is_empty() && east != want[&(s, j)]
            })
            .step_by(53)
            .take(4)
            .collect();
        for &(s, j) in &stale {
            let row = built.clipmap[s as usize];
            let cell = FIRST_CELL + s / ENTRIES;
            let mut before = init.clone();
            before[generation::head_at(s) as usize] = (row[0] + 1) as u32;
            let mut list = vec![0u32; generation::LIST_WORDS as usize];
            list[generation::THIN_ARGS as usize + 2] = 1;
            list[generation::LIST_AT as usize] = generation::pair_word(s, j);
            let (list, records) = (buffer(&device, &list), buffer(&device, &before));
            let at = [(9, &records), (10, &rows), (11, &list)];
            dispatch(&device, &queue, &hand, &at, size(1));
            let got = read(&device, &queue, &records, name);
            let mut east = thin_leaves(j as usize, cell, [row[0] + 1, row[1]]);
            east.sort();
            assert_eq!(
                read_laid(&got, s, j),
                (east.len() as u32, east),
                "{name}, {s} {j}: a head naming another brick"
            );
            stale_heads += 1;
        }
        eprintln!("{name}: {} pairs", pairs.len());
    }
    eprintln!(
        "thin: {pairs_total} pairs, {laid_total} laid, {none_past} past where they stand, {full_rooms} rooms filled, {stale_heads} heads naming another brick"
    );
    assert!(laid_total > 0 && none_past > 0 && full_rooms > 0 && stale_heads > 0);
}

/// The test's reference for `decor_mark_cs`, over words, one entry after another, by
/// `generation`'s rules; its list in the order of its entries, a pair's place not its meaning.
fn mark_rust(clipmap: &[[i32; 4]], records: &mut [u32], list: &mut [u32], shown: &[u32]) {
    let boxes: Vec<([i32; 4], Zoom)> = (0..list[generation::LIST_BOXES as usize])
        .map(|k| {
            let b = (generation::BOXES_AT + generation::BOX_WORDS * k) as usize;
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
        let held = clipmap[s as usize];
        if held[2] == 0 {
            continue;
        }
        let (cell, q, h) = (
            clip::eye_cell(s / ENTRIES),
            [held[0], held[1]],
            generation::head_at(s) as usize,
        );
        let head = [records[h], records[h + 1], records[h + 2]];
        let whole =
            list[generation::LIST_EVERY as usize] != 0 || !generation::generated_for(head, q);
        let at = generation::brick_box(cell, q);
        if !whole
            && !boxes
                .iter()
                .any(|&(b, z)| generation::reaches(b, z, cell, at))
        {
            continue;
        }
        if whole {
            records[h] = q[0] as u32;
            records[h + 1] = q[1] as u32;
            records[h + 2] = 1;
            for k in 0..DECORS.len() as u32 {
                records[generation::cnt_at(s, k) as usize] = 0;
            }
        }
        for j in 0..DECORS.len() as u32 {
            if shown[j as usize] >> (cell - FIRST_CELL) & 1 != 0 {
                let counted = if whole {
                    generation::THIN_ARGS + 2
                } else {
                    generation::LIST_PARTS
                } as usize;
                let n = list[counted];
                list[counted] += 1;
                list[generation::GENERATE_ARGS as usize + 2] += 1;
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

/// The list's words as a set: its pairs whole and in part sorted, as threads leave them in any order.
fn list_sorted(list: &[u32]) -> Vec<u32> {
    let mut out = list.to_vec();
    let whole = out[generation::THIN_ARGS as usize + 2] as usize;
    let parts = out[generation::LIST_PARTS as usize] as usize;
    out[generation::LIST_AT as usize..generation::LIST_AT as usize + whole].sort_unstable();
    let end = generation::LIST_WORDS as usize;
    out[end - parts..end].sort_unstable();
    out
}

/// One mark's case: `every` whole, which heads hold their bricks, the boxes, what is shown.
struct MarkCase {
    name: &'static str,
    every: bool,
    stale_one_in: u32,
    boxes: Vec<([i32; 4], Zoom)>,
    shown: [u32; DECORS.len()],
}

/// `decor_mark_cs` by hand as the Rust, and as generated, over heads held and stale, entries not
/// held, boxes of every kind and what the kernel shows; its threads' order any, `RUNS_OF_MARK`
/// a case.
#[test]
fn hand_mark_lists_as_the_rust() {
    const RUNS_OF_MARK: u32 = 12;
    let Some((device, queue)) = device() else {
        return;
    };
    let hand = hand_pipeline(&device, SHADER_GENERATION_MARK);
    let words = generation::record_at(generation::SLOT_RECORDS) as usize;
    // What `shown_by` hands: a decor's zooms where any variant stands; and fewer, as an extent asks.
    let mut stands = [0u32; DECORS.len()];
    for (k, d) in DECORS.iter().enumerate() {
        for cell in FIRST_CELL..=LEAF_CELL {
            if scatter::any_stands_at(d, cell) {
                stands[k] |= 1 << (cell - FIRST_CELL);
            }
        }
    }
    let mut leaves_only = [0u32; DECORS.len()];
    leaves_only[0] = 1 << (LEAF_CELL - FIRST_CELL);
    leaves_only[2] = 1 << (LEAF_CELL - FIRST_CELL);
    let mut tally = Tally::default();
    for (name, eye) in [
        ("east of the seam", real_world::EYE),
        ("west of the seam", [(1 << 23) - 300, (40 << 16) + 30]),
    ] {
        let built = real_world::build(eye);
        let clip_flat: Vec<u32> = built.clipmap.iter().flatten().map(|&v| v as u32).collect();
        let (clip_buf, stale) = (buffer(&device, &clip_flat), real_world::wrap(eye[0]));
        // Boxes on what the entries hold.
        let row = |cell: Zoom, e: u32| built.clipmap[((cell - FIRST_CELL) * ENTRIES + e) as usize];
        let brick = |cell: Zoom, e: u32| {
            let r = row(cell, e);
            generation::brick_box(cell, [r[0], r[1]])
        };
        let on = brick(24, 27);
        // Boxes about a brick at the edges of what its candidates read: just in and just out of
        // its reach on each side, a lap round, and at a zoom coarser and finer than its own.
        let about = |cell: Zoom, e: u32| -> Vec<([i32; 4], Zoom)> {
            let b = brick(cell, e);
            let r = generation::READ_REACH << (LEAF_CELL - cell);
            let (lo, hi) = ([b[0] - r, b[1] - r], [b[2] + r, b[3] + r]);
            let mid = (b[1] + b[3]) / 2;
            let lap = 1 << LEAF_CELL;
            vec![
                ([hi[0] - 1, mid, hi[0] + 4, mid + 1], generation::EVERY),
                (
                    [hi[0] + lap, mid, hi[0] + lap + 4, mid + 1],
                    generation::EVERY,
                ),
                ([lo[0] - 4, mid, lo[0], mid + 1], generation::EVERY),
                (
                    [lo[0] - 4 - lap, mid, lo[0] + 1 - lap, mid + 1],
                    generation::EVERY,
                ),
                ([b[0], hi[1], b[0] + 1, hi[1] + 3], generation::EVERY),
                ([b[0], lo[1] - 3, b[0] + 1, lo[1]], generation::EVERY),
                (b, cell - 1),
                ([hi[0] - 1, mid, hi[0], mid + 1], (cell + 1).min(LEAF_CELL)),
            ]
        };
        // Each of a brick's edge boxes about a brick of its own, held and apart from the others,
        // so its verdict alone lists that brick's pairs.
        let one_a_brick = |cell: Zoom, about: &dyn Fn(Zoom, u32) -> Vec<([i32; 4], Zoom)>| {
            let held: Vec<u32> = (0..ENTRIES)
                .filter(|&e| row(cell, e)[2] != 0)
                .step_by(5)
                .collect();
            (0..8)
                .map(|i| about(cell, held[i % held.len()])[i])
                .collect::<Vec<_>>()
        };
        let cases = [
            MarkCase {
                name: "all held, no box",
                every: false,
                stale_one_in: 0,
                boxes: vec![],
                shown: stands,
            },
            MarkCase {
                name: "every entry whole",
                every: true,
                stale_one_in: 0,
                boxes: vec![],
                shown: stands,
            },
            MarkCase {
                name: "some stale",
                every: false,
                stale_one_in: 5,
                boxes: vec![],
                shown: stands,
            },
            MarkCase {
                name: "a little shown",
                every: false,
                stale_one_in: 3,
                boxes: vec![],
                shown: leaves_only,
            },
            MarkCase {
                name: "an edit",
                every: false,
                stale_one_in: 0,
                boxes: vec![(
                    [on[0] + 3, on[1] + 3, on[0] + 5, on[1] + 5],
                    generation::EVERY,
                )],
                shown: stands,
            },
            MarkCase {
                name: "a brick filled, every kind",
                every: false,
                stale_one_in: 9,
                boxes: (0..21u32)
                    .map(|k| {
                        let cell = 13 + k % 12;
                        (
                            brick(cell, k * 3 % ENTRIES),
                            if k % 5 == 0 { generation::EVERY } else { cell },
                        )
                    })
                    .collect(),
                shown: stands,
            },
            MarkCase {
                name: "about bricks, at their reach's edges",
                every: false,
                stale_one_in: 0,
                boxes: one_a_brick(24, &about),
                shown: stands,
            },
            MarkCase {
                name: "about bricks coarser, at their reach's edges",
                every: false,
                stale_one_in: 0,
                boxes: [one_a_brick(16, &about), one_a_brick(22, &about)].concat(),
                shown: stands,
            },
            MarkCase {
                name: "round the world",
                every: false,
                stale_one_in: 0,
                boxes: vec![
                    (
                        [on[0] - (1 << 24), on[1], on[2] - (1 << 24), on[3]],
                        generation::EVERY,
                    ),
                    ([stale - 4, on[1] - 1, stale + (1 << 24) + 4, on[1] + 1], 24),
                ],
                shown: stands,
            },
        ];
        for case in cases {
            let mut records = vec![0u32; words];
            for s in 0..generation::EYE_ENTRIES {
                let held = built.clipmap[s as usize];
                let h = generation::head_at(s) as usize;
                let m = mix(s, [3, 9]);
                let is_stale = case.stale_one_in != 0 && m.is_multiple_of(case.stale_one_in);
                // A brick stale on X or on Y.
                let on_y = is_stale && m & 128 != 0;
                records[h] = (held[0] + (is_stale && !on_y) as i32) as u32;
                records[h + 1] = (held[1] + on_y as i32) as u32;
                records[h + 2] = if is_stale && m & 64 != 0 { 0 } else { 1 };
                for k in 0..DECORS.len() as u32 {
                    records[generation::cnt_at(s, k) as usize] = 40 + k + (s & 3);
                }
            }
            let mut list = vec![0u32; generation::LIST_WORDS as usize];
            list[generation::LIST_EVERY as usize] = case.every as u32;
            list[generation::LIST_BOXES as usize] = case.boxes.len() as u32;
            for (k, &(b, z)) in case.boxes.iter().enumerate() {
                let at = generation::BOXES_AT as usize + k * generation::BOX_WORDS as usize;
                for a in 0..4 {
                    list[at + a] = b[a] as u32;
                }
                list[at + 4] = z;
            }
            // Past the boxes counted, a frame before's, which reach every brick.
            for k in case.boxes.len()..generation::BOX_CAP as usize {
                let at = generation::BOXES_AT as usize + k * generation::BOX_WORDS as usize;
                list[at..at + 4].copy_from_slice(&[0, (-1i32 << 30) as u32, 1 << 24, 1 << 30]);
                list[at + 4] = generation::EVERY;
            }
            let (mut want_records, mut want_list) = (records.clone(), list.clone());
            mark_rust(
                &built.clipmap,
                &mut want_records,
                &mut want_list,
                &case.shown,
            );
            let want_sorted = list_sorted(&want_list);
            let whole = want_list[generation::THIN_ARGS as usize + 2];
            let parts = want_list[generation::LIST_PARTS as usize];
            eprintln!("{name}, {}: {whole} whole, {parts} in part", case.name);
            *tally.0.entry("pairs whole").or_default() += whole as u64;
            *tally.0.entry("pairs in part").or_default() += parts as u64;
            let shown = buffer(&device, &case.shown);
            for run in 0..RUNS_OF_MARK {
                {
                    let (r_buf, l_buf) = (buffer(&device, &records), buffer(&device, &list));
                    let size = [generation::EYE_ENTRIES.div_ceil(generation::LANES), 1, 1];
                    {
                        let at = [(1, &clip_buf), (9, &r_buf), (11, &l_buf), (12, &shown)];
                        dispatch(&device, &queue, &hand, &at, size);
                    }
                    let got_records = read(&device, &queue, &r_buf, case.name);
                    let got_list = read(&device, &queue, &l_buf, case.name);
                    assert!(
                        got_records == want_records,
                        "{name}, {}: records, run {run}",
                        case.name
                    );
                    let same = list_sorted(&got_list) == want_sorted;
                    assert!(same, "{name}, {}: the list, run {run}", case.name);
                    *tally.0.entry("runs").or_default() += 1;
                }
            }
        }
    }
    eprintln!("mark: {:?}", tally.0);
    assert!(tally.0["pairs whole"] > 0 && tally.0["pairs in part"] > 0);
}

/// Whether workgroup `g` of a pass over the records reads them: the test's reference for
/// decor_run.wgsl `is_held`.
fn is_held(records: &[u32], clipmap: &[[i32; 4]], g: u32) -> bool {
    if g >= generation::EYE_ENTRIES {
        return true;
    }
    let (h, q) = (generation::head_at(g) as usize, clipmap[g as usize]);
    q[2] != 0
        && generation::generated_for([records[h], records[h + 1], records[h + 2]], [q[0], q[1]])
}

/// The `k`-th run of records workgroup `g` reads: the test's reference for decor_run.wgsl
/// `run_of`.
fn run_of(records: &[u32], g: u32, k: u32) -> (u32, u32) {
    if g < generation::EYE_ENTRIES {
        let at_run = k * generation::RUN;
        let (mut j, mut first) = (0, 0);
        while j + 1 < DECORS.len() as u32 && at_run >= first + DECORS[j as usize].cap() {
            first += DECORS[j as usize].cap();
            j += 1;
        }
        let cap = DECORS[j as usize].cap();
        let laid = records[generation::cnt_at(g, j) as usize].min(cap);
        let past = at_run - first;
        let cnt = laid.saturating_sub(past);
        (
            generation::entry_record(&DECORS, g, j, past),
            cnt.min(generation::RUN),
        )
    } else {
        let n = (g - generation::EYE_ENTRIES) * generation::RUNS + k;
        (
            generation::SLOT_RECORDS + n,
            u32::from(n < records[generation::PLACED_HEAD as usize]),
        )
    }
}

/// Each word of a pass's records, a workgroup at a time, as `decor_cnt_cs` and
/// `decor_compact_cs` take them.
fn each_record(records: &[u32], clipmap: &[[i32; 4]], workgroups: u32) -> Vec<usize> {
    let mut out = Vec::new();
    for g in 0..workgroups {
        if is_held(records, clipmap, g) {
            for k in 0..generation::RUNS {
                let (first, cnt) = run_of(records, g, k);
                for n in 0..cnt {
                    out.push(generation::record_at(first + n) as usize);
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
        // What `light_mark_draws_cs` reads of it: where it looks, and the light's box.
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
        let light = Light::of(
            [
                [900.0, 0.0, 0.0],
                [0.0, 900.0, 0.0],
                [0.0, 0.0, -900.0],
                [0.0, 0.0, 450.0],
            ],
            MAP_SIDE,
            [0, 0],
            1.0,
        );
        words[eye::LIGHT..eye::LIGHT + eye::LIGHT_WORDS].copy_from_slice(&light.words());
        words
    }
}

/// The map's side `light_mark_draws_cs` is run at, and its cast's number.
const MAP_SIDE: u32 = 4096;
const CAST_NUMBER: u32 = 7;

/// `cnt_one`'s across and its floors: how far a record's span lies from either, and what it
/// asks of the zooms.
fn across_of(s: &Sizing, group: u32, d2: f32) -> f32 {
    let d = libm::sqrtf(d2);
    let d = if d > drawn::NEAREST_U {
        d
    } else {
        drawn::NEAREST_U
    };
    f32::from_bits(s.drawing[(drawn::EXTENTS + group) as usize]) / d
}

/// Floats within this many steps of each other a GPU's rounding may part: WGSL's `sqrt` and `/`
/// are not bit-stable, and a contraction moves `distance2`.
const WITHIN_ULPS: i64 = 8;

fn ulps_apart(a: f32, b: f32) -> i64 {
    (a.to_bits() as i64 - b.to_bits() as i64).abs()
}

/// Whether a record's region may fall either way by a float: its span or its zoom at an edge.
fn at_an_edge(s: &Sizing, group: u32, d2: f32) -> bool {
    let across = across_of(s, group, d2);
    let floor = |at: u32| f32::from_bits(s.drawing[at as usize]);
    let near = 16.0 * f32::EPSILON;
    let zoom = |r2: f32| grid::cell_zoom_at(r2);
    ulps_apart(across, floor(drawn::MODEL_ANGLE)) <= WITHIN_ULPS
        || ulps_apart(across, floor(drawn::IMPOSTOR_ANGLE)) <= WITHIN_ULPS
        || zoom(d2 * (1.0 - near)) != zoom(d2 * (1.0 + near))
}

/// `cnt_one` over words, its region's count, and whether a float may move it.
fn cnt_one(s: &Sizing, world: &World, records: &mut [u32], regions: &mut [u32], i: usize) -> bool {
    if records[i] == generation::UNGENERATED {
        return false;
    }
    let w = records[i];
    let (decor, variant, _, cell) = generation::record_of(w);
    let corner = [
        records[i + 1] as i32,
        records[i + 2] as i32,
        records[i + 3] as i32,
    ];
    let d2 = generation::distance2(s.origin, corner);
    let placed = i >= generation::record_at(generation::SLOT_RECORDS) as usize;
    let detail = Detail {
        range: s.range,
        spacing: Spacing::NONE,
    };
    let at = detail.at(d2);
    let at = if placed {
        at
    } else {
        generation::held_at(world, at, [corner[0], corner[1]])
    };
    let here = generation::drawn_at(at, cell, placed);
    let group = generation::first_group(&DECORS, decor) + variant;
    let hide = drawn::HIDE as usize;
    let hidden = s.drawing[hide + 3] != 0
        && corner[0] == s.drawing[hide] as i32
        && corner[1] == s.drawing[hide + 1] as i32
        && corner[2] == s.drawing[hide + 2] as i32;
    let floor = |at: u32| f32::from_bits(s.drawing[at as usize]);
    let tier = drawn::tier(
        across_of(s, group, d2),
        floor(drawn::MODEL_ANGLE),
        floor(drawn::IMPOSTOR_ANGLE),
    );
    let region = if hidden || !here || tier == drawn::CULLED {
        drawn::NONE
    } else {
        2 * group + tier
    };
    records[i] = w & ((1 << drawn::REGION_SHIFT) - 1) | region << drawn::REGION_SHIFT;
    if region != drawn::NONE {
        regions[region as usize] += 1;
    }
    at_an_edge(s, group, d2)
}

/// Records as generation leaves them, placed decor's after: an entry's heads held or stale, each
/// decor none, a few, its room or past it, each a place in its brick, a variant and a turn, or
/// nothing generated.
fn records_to_size(built: &Built, eye: [i32; 2], placed: u32) -> (Vec<u32>, u32) {
    let need = generation::SLOT_RECORDS + placed.next_power_of_two().max(64);
    let mut records = vec![0u32; generation::record_at(need) as usize];
    let mut draw = real_world::Draw(0x0bad_cafe);
    for s in 0..generation::EYE_ENTRIES {
        let row = built.clipmap[s as usize];
        let cell = clip::eye_cell(s / ENTRIES);
        let h = generation::head_at(s) as usize;
        let stale = mix(s, [5, 5]).is_multiple_of(9);
        records[h] = (row[0] + stale as i32) as u32;
        records[h + 1] = row[1] as u32;
        records[h + 2] = 1;
        let b = generation::brick_box(cell, [row[0], row[1]]);
        for (j, d) in DECORS.iter().enumerate() {
            let cap = d.cap();
            let laid = match draw.next() % 16 {
                0..=5 => 0,
                6 => cap,
                7 => cap + 5,
                8 => generation::RUN,
                9 => generation::RUN + 1,
                _ => draw.next() % 40,
            }
            .min(cap + 5);
            records[generation::cnt_at(s, j as u32) as usize] = laid;
            let z = built.top_at([b[0], b[1]]);
            for i in 0..laid.min(cap) {
                let r = word_of(s, j as u32, i);
                let h = draw.next();
                let leaf = [
                    b[0] + (h % (b[2] - b[0]) as u32) as i32,
                    b[1] + ((h >> 8) % (b[3] - b[1]) as u32) as i32,
                ];
                // Each with the region the frame before gave it, which cnt sizes anew.
                records[r] = if h.is_multiple_of(8) {
                    generation::UNGENERATED
                } else {
                    generation::record_word(
                        j as u32,
                        draw.next() % d.variants,
                        draw.next() & 3,
                        cell,
                    ) | (draw.next() % (drawn::REGIONS + 1)) << drawn::REGION_SHIFT
                };
                records[r + 1] = real_world::wrap(leaf[0]) as u32;
                records[r + 2] = leaf[1] as u32;
                records[r + 3] = (z + draw.within(-4, 5)) as u32;
                records[r + 4] = draw.next() % 256;
            }
        }
    }
    records[generation::PLACED_HEAD as usize] = placed;
    for n in 0..placed {
        let r = generation::record_at(generation::SLOT_RECORDS + n) as usize;
        let j = n % DECORS.len() as u32;
        let d = &DECORS[j as usize];
        let v = draw.next() % d.variants;
        records[r] = generation::record_word(j, v, draw.next() & 3, scatter::coarsest_stand(d, v))
            | (draw.next() % (drawn::REGIONS + 1)) << drawn::REGION_SHIFT;
        // Most about the eye, where they are drawn; one in seven farther, where it is not.
        let reach = if n % 7 == 3 { 900 } else { 40 };
        let at = [
            eye[0] + draw.within(-reach, reach),
            eye[1] + draw.within(-reach, reach),
        ];
        records[r + 1] = real_world::wrap(at[0]) as u32;
        records[r + 2] = at[1] as u32;
        records[r + 3] = (built.top_at(at) + draw.within(0, 3)) as u32;
        // The second beside the first on its row, a few cells along X.
        if n == 1 {
            let first = generation::record_at(generation::SLOT_RECORDS) as usize;
            records[r + 1] = real_world::wrap(records[first + 1] as i32 + 3) as u32;
            records[r + 2] = records[first + 2];
            records[r + 3] = records[first + 3];
        }
    }
    (
        records,
        generation::EYE_ENTRIES + placed.div_ceil(generation::RUNS),
    )
}

/// A record's first word as hand and Rust must agree on it: exact, or, where a float may move
/// it, its bits under the region.
fn record_agrees(got: u32, want: u32, free: bool) -> bool {
    got == want
        || (free
            && got & ((1 << drawn::REGION_SHIFT) - 1) == want & ((1 << drawn::REGION_SHIFT) - 1))
}

/// The drawing words a frame hands, its floors as angles and each group's extent.
fn drawing_words(model: f32, impostor: f32) -> Vec<u32> {
    let mut d = vec![0u32; drawn::DRAWING_WORDS as usize];
    d[drawn::MODEL_ANGLE as usize] = model.to_bits();
    d[drawn::IMPOSTOR_ANGLE as usize] = impostor.to_bits();
    for g in 0..generation::GROUPS {
        d[(drawn::EXTENTS + g) as usize] = (1.0 + g as f32 * 0.37).to_bits();
    }
    d
}

/// `decor_cnt_cs` by hand as the Rust, and as generated, its threads falling any way,
/// `RUNS_OF_CNT` a case: where the rays ask and hold, tiers, a hidden corner, the eye on a
/// record, heads stale, runs of each decor, placed decor over a workgroup's runs.
#[test]
fn hand_cnt_sizes_as_the_rust() {
    const RUNS_OF_CNT: u32 = 6;
    let Some((device, queue)) = device() else {
        return;
    };
    let hand = hand_pipeline(&device, SHADER_GENERATION_CNT);
    let mut tally = Tally::default();
    for (name, at, placed) in [
        ("east of the seam", real_world::EYE, 0),
        ("east of the seam, placed", real_world::EYE, 80),
        ("west of the seam", [(1 << 23) - 300, (40 << 16) + 30], 500),
    ] {
        let built = real_world::build(at);
        let world = built.world();
        let (records, workgroups) = records_to_size(&built, at, placed);
        let clip_flat: Vec<u32> = built.clipmap.iter().flatten().map(|&v| v as u32).collect();
        let clip_buf = buffer(&device, &clip_flat);
        let origin = [at[0] << 8, at[1] << 8, (built.top_at(at) + 30) << 8];
        let base = Sizing {
            name: "as a frame asks",
            origin,
            range: [13, 32],
            drawing: drawing_words(0.02, 0.004),
        };
        // A record the frame draws, its corner for the cases that ask of one.
        let order = each_record(&records, &built.clipmap, workgroups);
        let mut scratch = records.clone();
        let mut regions = vec![0u32; 2 * drawn::REGIONS as usize];
        let drawn_at = order
            .iter()
            .copied()
            .find(|&i| {
                cnt_one(&base, &world, &mut scratch, &mut regions, i);
                records[i] != generation::UNGENERATED
                    && scratch[i] >> drawn::REGION_SHIFT != drawn::NONE
            })
            .expect("a record drawn");
        let corner = [
            records[drawn_at + 1] as i32,
            records[drawn_at + 2] as i32,
            records[drawn_at + 3] as i32,
        ];
        let group = {
            let (d, v, _, _) = generation::record_of(records[drawn_at]);
            generation::first_group(&DECORS, d) + v
        };
        let mut hidden = base.clone();
        hidden.name = "a corner left out";
        for (a, v) in corner.iter().enumerate() {
            hidden.drawing[drawn::HIDE as usize + a] = *v as u32;
        }
        hidden.drawing[drawn::HIDE as usize + 3] = 1;
        let mut none_hidden = hidden.clone();
        none_hidden.name = "a corner given, none left out";
        none_hidden.drawing[drawn::HIDE as usize + 3] = 0;
        // The first placed record left out, the second sharing its Y and Z.
        let first_placed = generation::record_at(generation::SLOT_RECORDS) as usize;
        let mut placed_hidden = hidden.clone();
        placed_hidden.name = "a placed corner left out, another on its row";
        for a in 0..3 {
            placed_hidden.drawing[drawn::HIDE as usize + a] = records[first_placed + 1 + a];
        }
        let mut on_it = base.clone();
        on_it.name = "the eye on a record";
        on_it.origin = [
            (corner[0] << 8).wrapping_add(128),
            (corner[1] << 8) + 128,
            corner[2] << 8,
        ];
        let mut near = base.clone();
        near.name = "a far range";
        near.range = [24, 24];
        let mut coarse = base.clone();
        coarse.name = "a coarse range";
        coarse.range = [16, 22];
        // The floors eight steps either side of the record's span.
        let span = across_of(&base, group, generation::distance2(origin, corner));
        let steps = |n: i64| f32::from_bits((span.to_bits() as i64 + n) as u32);
        let mut over = base.clone();
        over.name = "the model's floor just over its span";
        over.drawing[drawn::MODEL_ANGLE as usize] = steps(WITHIN_ULPS + 1).to_bits();
        let mut under = base.clone();
        under.name = "the model's floor just under its span";
        under.drawing[drawn::MODEL_ANGLE as usize] = steps(-WITHIN_ULPS - 1).to_bits();
        let mut impostor_over = base.clone();
        impostor_over.name = "the impostor's floor just over its span";
        impostor_over.drawing[drawn::MODEL_ANGLE as usize] = 1e9f32.to_bits();
        impostor_over.drawing[drawn::IMPOSTOR_ANGLE as usize] = steps(WITHIN_ULPS + 1).to_bits();
        // The zoom's edge either side of a record at the leaves: the eye moved along Z to the
        // distance where they end, a step of Q3 short and past, only `here` deciding.
        let split = grid::CELLS_A_RAD / grid::SPANS;
        let edge = split * split * 4.0;
        let leaf_record = order
            .iter()
            .copied()
            .find(|&i| {
                let at_xy = [origin[0], origin[1], (records[i + 3] as i32) << 8];
                let c = [
                    records[i + 1] as i32,
                    records[i + 2] as i32,
                    records[i + 3] as i32,
                ];
                records[i] != generation::UNGENERATED
                    && generation::record_of(records[i]).3 == LEAF_CELL
                    && generation::distance2(at_xy, c) * 2.0 + 4.0 < edge
            })
            .expect("a record at the leaves near the eye");
        let c = [
            records[leaf_record + 1] as i32,
            records[leaf_record + 2] as i32,
            records[leaf_record + 3] as i32,
        ];
        let dxy2 = generation::distance2([origin[0], origin[1], c[2] << 8], c);
        let oz = (c[2] << 8) + (libm::sqrtf(edge - dxy2) * 256.0) as i32;
        let mut short = base.clone();
        short.name = "the eye a step short of the leaves' edge";
        short.drawing[drawn::MODEL_ANGLE as usize] = 1e9f32.to_bits();
        short.drawing[drawn::IMPOSTOR_ANGLE as usize] = 1e-9f32.to_bits();
        short.origin[2] = oz - 1;
        let mut past = short.clone();
        past.name = "the eye a step past the leaves' edge";
        past.origin[2] = oz + 2;
        let region_at = |s: &Sizing| {
            let (mut w, mut r) = (records.clone(), vec![0u32; 2 * drawn::REGIONS as usize]);
            let free = cnt_one(s, &world, &mut w, &mut r, leaf_record);
            assert!(
                !free,
                "{name}, {}: a step of Q3 is past the floats' edge",
                s.name
            );
            w[leaf_record] >> drawn::REGION_SHIFT
        };
        assert!(
            region_at(&short) != drawn::NONE && region_at(&past) == drawn::NONE,
            "{name}: the leaves' edge is between the two"
        );
        for sizing in [
            base,
            hidden,
            none_hidden,
            placed_hidden,
            on_it,
            near,
            coarse,
            over,
            under,
            impostor_over,
            short,
            past,
        ] {
            let mut want = records.clone();
            let mut want_regions = vec![0u32; 2 * drawn::REGIONS as usize];
            let mut free = vec![false; records.len()];
            for &i in &order {
                free[i] = cnt_one(&sizing, &world, &mut want, &mut want_regions, i);
            }
            let free_n = free.iter().filter(|f| **f).count() as u64;
            let (drawing, eye_buf) = (
                buffer(&device, &sizing.drawing),
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytes(&sizing.eye()),
                    usage: wgpu::BufferUsages::UNIFORM,
                }),
            );
            for run in 0..RUNS_OF_CNT {
                {
                    let (r_buf, regions) = (
                        buffer(&device, &records),
                        buffer(&device, &vec![0u32; 2 * drawn::REGIONS as usize]),
                    );
                    {
                        let at = [
                            (0, &eye_buf),
                            (1, &clip_buf),
                            (9, &r_buf),
                            (13, &drawing),
                            (14, &regions),
                        ];
                        dispatch(&device, &queue, &hand, &at, [workgroups, 1, 1]);
                    }
                    let got = read(&device, &queue, &r_buf, sizing.name);
                    let got_regions = read(&device, &queue, &regions, sizing.name);
                    let mut counted = vec![0u32; 2 * drawn::REGIONS as usize];
                    for (k, (&g, &w)) in got.iter().zip(&want).enumerate() {
                        assert!(
                            record_agrees(g, w, free[k]),
                            "{name}, {}: word {k}: got {g:#x}, want {w:#x}, run {run}",
                            sizing.name
                        );
                    }
                    for &i in &order {
                        let region = got[i] >> drawn::REGION_SHIFT;
                        if got[i] != generation::UNGENERATED && region != drawn::NONE {
                            counted[region as usize] += 1;
                        }
                    }
                    assert_eq!(got_regions, counted, "{name}, {}: counts", sizing.name);
                    if free_n == 0 {
                        assert_eq!(got_regions, want_regions, "{name}, {}", sizing.name);
                    }
                }
            }
            let drawn_n: u64 = want_regions.iter().map(|&n| n as u64).sum();
            eprintln!(
                "{name}, {}: {} records, {drawn_n} drawn, {free_n} at an edge",
                sizing.name,
                order.len()
            );
            *tally.0.entry("records sized").or_default() += order.len() as u64;
            *tally.0.entry("drawn").or_default() += drawn_n;
            *tally.0.entry("at an edge").or_default() += free_n;
            let at_on = |models: bool| {
                want_regions
                    .iter()
                    .enumerate()
                    .filter(|(r, n)| (r % 2 == 0) == models && **n > 0)
                    .count() as u64
            };
            *tally.0.entry("cases with a model").or_default() += (at_on(true) > 0) as u64;
            *tally.0.entry("cases with an impostor").or_default() += (at_on(false) > 0) as u64;
        }
    }
    eprintln!("cnt: {:?}", tally.0);
    assert!(
        tally.0["drawn"] > 0
            && tally.0["cases with a model"] > 0
            && tally.0["cases with an impostor"] > 0
    );
}

/// What `compact_one` writes of the record at word `i`, its lanes' bits, and its region.
fn lanes_of(drawing: &[u32], records: &[u32], i: usize) -> Option<(u32, [u32; 5])> {
    let w = records[i];
    let region = w >> drawn::REGION_SHIFT;
    if region == drawn::NONE {
        return None;
    }
    let (decor, variant, turn, _) = generation::record_of(w);
    let [across, deep, _] = DECORS[decor as usize].cells(variant, turn);
    let anchor = |axis: usize| {
        let d = (records[i + 1 + axis] as i32)
            .wrapping_sub(drawing[drawn::ANCHOR as usize + axis] as i32);
        (if axis == 0 { wrap_idx(LEAF_CELL, d) } else { d }) as f32
    };
    let sink = records[i + 4] as f32 / (1 << pxl_engine::FRAC_W) as f32;
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

/// `decor_compact_cs` by hand as the Rust, and as generated: each region's slots hold its
/// records' lanes, in no order, its threads falling any way, `RUNS_OF_COMPACT` a case; past
/// the instances' slots, its count goes on and no slot holds what is not a record's.
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
        let world = built.world();
        let (records, workgroups) = records_to_size(&built, at, placed);
        let clip_flat: Vec<u32> = built.clipmap.iter().flatten().map(|&v| v as u32).collect();
        let (clip_buf, rows) = (buffer(&device, &clip_flat), buffer(&device, rows_words()));
        let origin = [at[0] << 8, at[1] << 8, (built.top_at(at) + 30) << 8];
        // Records sized as a far range asks, for many drawn; about an anchor a little off the
        // eye, and one across the seam.
        for anchor_at in [
            [at[0] + 40, at[1] - 7, 12],
            [real_world::wrap(at[0] + (1 << 23)), at[1], -300],
        ] {
            let mut drawing = drawing_words(0.02, 0.0001);
            for a in 0..3 {
                drawing[drawn::ANCHOR as usize + a] = anchor_at[a] as u32;
            }
            let sizing = Sizing {
                name: "sized",
                origin,
                range: [13, 32],
                drawing,
            };
            let order = each_record(&records, &built.clipmap, workgroups);
            let (mut sized, mut regions) =
                (records.clone(), vec![0u32; 2 * drawn::REGIONS as usize]);
            for &i in &order {
                cnt_one(&sizing, &world, &mut sized, &mut regions, i);
            }
            // The regions laid out, as `decor_args_cs` leaves them.
            let mut first = 0;
            let mut firsts = vec![0u32; drawn::REGIONS as usize];
            for r in 0..drawn::REGIONS as usize {
                regions[drawn::REGIONS as usize + r] = first;
                firsts[r] = first;
                first += regions[r];
            }
            let total = first;
            let mut wanted: Vec<(u32, [u32; 5])> = order
                .iter()
                .filter(|&&i| sized[i] != generation::UNGENERATED)
                .filter_map(|&i| lanes_of(&sizing.drawing, &sized, i))
                .collect();
            wanted.sort_unstable();
            assert_eq!(wanted.len() as u32, total);
            *tally.0.entry("instances laid").or_default() += total as u64;
            // Room for all and to spare, and room for fewer than are drawn.
            for cap in [total + 64, total / 2 + 1] {
                let drawing_buf = buffer(&device, &sizing.drawing);
                let r_buf = buffer(&device, &sized);
                for run in 0..RUNS_OF_COMPACT {
                    {
                        let regions_buf = buffer(&device, &regions);
                        let instances = buffer(
                            &device,
                            &vec![0xffff_ffffu32; ((drawn::COLUMNS + 1) * cap) as usize],
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
                        // Each region's next slot past its end, what `Instances::cnt` reads.
                        for r in 0..drawn::REGIONS as usize {
                            assert_eq!(
                                got_regions[drawn::REGIONS as usize + r],
                                firsts[r] + regions[r],
                                "{name}, region {r}"
                            );
                            assert_eq!(got_regions[r], regions[r], "{name}, region {r}'s count");
                        }
                        let mut have = Vec::new();
                        for r in 0..drawn::REGIONS as usize {
                            for slot in firsts[r]..(firsts[r] + regions[r]).min(cap) {
                                let (c, k) = (cap as usize, slot as usize);
                                let lanes = [
                                    got[k],
                                    got[c + k],
                                    got[2 * c + k],
                                    got[3 * c + k],
                                    got[4 * c + k],
                                ];
                                assert_eq!(
                                    got[drawn::COLUMNS as usize * c + k],
                                    slot,
                                    "{name}: a slot's own index"
                                );
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
                            // Which records took the slots is the threads'; each is a record's.
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

/// `decor_args_cs` by hand as the Rust lays regions, over uniforms fresh as a frame writes.
#[test]
fn hand_args_answers_as_the_rust() {
    let Some((device, queue)) = device() else {
        return;
    };
    let hand = hand_pipeline(&device, SHADER_GENERATION_ARGS);
    let (regions_n, groups) = (drawn::REGIONS as usize, generation::GROUPS as usize);
    let store = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC;
    let make = |w: &[u32]| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytes(w),
            usage: store,
        })
    };
    for round in 0..4u32 {
        // A mesh still loading is 0.
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
        // Each region's uniform as `Generator::prepare` writes it: the columns' base at its index.
        let u = drawn::UNIFORM_WORDS as usize;
        let mut uniforms = vec![0u32; u * regions_n];
        let base = drawn::COLUMNS * 2048;
        for r in 0..regions_n {
            uniforms[r * u] = 1.5f32.to_bits();
            uniforms[r * u + drawn::UNIFORM_IDX as usize] = base;
        }
        let args0 = vec![0xdead_beefu32; drawn::ARGS_WORDS as usize * regions_n];
        // The regions laid end to end, as `generation::drawn` lays them.
        let (mut want_regions, mut want_args, mut want_uniforms) =
            (regions.clone(), args0.clone(), uniforms.clone());
        let mut first = 0;
        for r in 0..regions_n {
            let n = regions[r];
            want_regions[regions_n + r] = first;
            want_uniforms[r * u + drawn::UNIFORM_IDX as usize] += first;
            let a = drawn::ARGS_WORDS as usize * r;
            want_args[a] = if r % 2 == 0 {
                meshes[r / 2]
            } else {
                drawn::IMPOSTOR_CORNERS
            };
            want_args[a + 1] = n;
            want_args[a + 2] = 0;
            want_args[a + 3] = 0;
            want_args[a + 4] = 0;
            first += n;
        }
        {
            let pipe = &hand;
            let (mb, rb, ab, ub) = (make(&meshes), make(&regions), make(&args0), make(&uniforms));
            let entries =
                [(0, &mb), (1, &rb), (2, &ab), (3, &ub)].map(|(binding, b)| wgpu::BindGroupEntry {
                    binding,
                    resource: b.as_entire_binding(),
                });
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &pipe.get_bind_group_layout(0),
                entries: &entries,
            });
            let mut e = device.create_command_encoder(&Default::default());
            {
                let mut pass = e.begin_compute_pass(&Default::default());
                pass.set_pipeline(pipe);
                pass.set_bind_group(0, &group, &[]);
                pass.dispatch_workgroups(1, 1, 1);
            }
            queue.submit([e.finish()]);
            assert_eq!(
                read(&device, &queue, &rb, "regions"),
                want_regions,
                "regions, round {round}"
            );
            assert_eq!(
                read(&device, &queue, &ab, "args"),
                want_args,
                "args, round {round}"
            );
            assert_eq!(
                read(&device, &queue, &ub, "uniforms"),
                want_uniforms,
                "uniforms, round {round}"
            );
        }
    }
}

/// A frame of decor's passes, as `Generator::prepare` and `dispatch` run them.
#[derive(Clone)]
struct Frame {
    name: &'static str,
    /// Whether it marks at all; every entry whole; the boxes; heads a brick left.
    marking: bool,
    every: bool,
    boxes: Vec<([i32; 4], Zoom)>,
    stale: Vec<u32>,
    /// Which of the world's two placings it reads, the second without the placed decor.
    placed_decor: bool,
    cuts: [Cut; 2],
    sizing: Sizing,
}

/// A pair's count and, by leaf, its records' first word, height and sink.
type PairRecords = (u32, BTreeMap<[i32; 2], [u32; 3]>);

/// What a frame leaves, as both sides are read: each pair's records by their leaf, in no order.
struct FrameWords {
    /// `(entry, decor)` to what it laid.
    pairs: BTreeMap<(u32, u32), PairRecords>,
    regions: Vec<u32>,
    args: Vec<u32>,
    uniforms: Vec<u32>,
    /// A region and a slot's lanes, sorted.
    lanes: Vec<(u32, [u32; 5])>,
    /// What `Instances::cnt` reads.
    placed_cnt: u32,
    /// The tiles `light_mark_draws_cs` stood on off the instances.
    stood: Vec<u32>,
}

fn words_of(
    records: &[u32],
    regions: Vec<u32>,
    args: Vec<u32>,
    uniforms: Vec<u32>,
    instances: &[u32],
    cap: u32,
) -> FrameWords {
    let mut pairs = BTreeMap::new();
    for s in 0..generation::EYE_ENTRIES {
        for j in 0..DECORS.len() as u32 {
            let cnt = records[generation::cnt_at(s, j) as usize];
            if cnt == 0 {
                continue;
            }
            let mut by_leaf = BTreeMap::new();
            for i in 0..cnt.min(DECORS[j as usize].cap()) {
                let r = word_of(s, j, i);
                by_leaf.insert(
                    [records[r + 1] as i32, records[r + 2] as i32],
                    [records[r], records[r + 3], records[r + 4]],
                );
            }
            pairs.insert((s, j), (cnt, by_leaf));
        }
    }
    let (n, c) = (drawn::REGIONS as usize, cap as usize);
    let mut lanes = Vec::new();
    if !instances.is_empty() {
        for r in 0..n {
            let first = regions[n + r] - regions[r];
            for slot in first..regions[n + r] {
                let k = slot as usize;
                assert_eq!(instances[drawn::COLUMNS as usize * c + k], slot);
                lanes.push((
                    r as u32,
                    [
                        instances[k],
                        instances[c + k],
                        instances[2 * c + k],
                        instances[3 * c + k],
                        instances[4 * c + k],
                    ],
                ));
            }
        }
        lanes.sort_unstable();
    }
    FrameWords {
        pairs,
        placed_cnt: regions[2 * n - 1].min(cap),
        stood: Vec::new(),
        regions,
        args,
        uniforms,
        lanes,
    }
}

/// The Rust's frame over its own records, what a float may move in it, what it said.
struct Model {
    records: Vec<u32>,
    list: Vec<u32>,
    workgroups: u32,
    cap: u32,
    shown: [u32; DECORS.len()],
    meshes: Vec<u32>,
}

/// What a frame's uniforms hold before its args add to them (`Generator::prepare`).
fn fresh_uniforms(cap: u32) -> Vec<u32> {
    let u = drawn::UNIFORM_WORDS as usize;
    let mut uniforms = vec![0u32; u * drawn::REGIONS as usize];
    for r in 0..drawn::REGIONS as usize {
        let layer = if r % 2 == 1 { r as u32 / 2 } else { 0 };
        uniforms[r * u..r * u + 8].copy_from_slice(&[
            1.5f32.to_bits(),
            2.5f32.to_bits(),
            3.5f32.to_bits(),
            0,
            cap,
            1.0f32.to_bits(),
            layer,
            drawn::COLUMNS * cap,
        ]);
    }
    uniforms
}

/// A list's head and boxes as `Generator::prepare` writes them, before a mark.
fn list_head(frame: &Frame) -> Vec<u32> {
    let mut list = vec![0u32; generation::LIST_WORDS as usize];
    list[generation::THIN_ARGS as usize] = BRICK_CELLS.div_ceil(generation::LANES);
    list[generation::THIN_ARGS as usize + 1] = 1;
    list[generation::GENERATE_ARGS as usize] = generation::KIND_RECORDS.div_ceil(generation::LANES);
    list[generation::GENERATE_ARGS as usize + 1] = 1;
    list[generation::LIST_EVERY as usize] = frame.every as u32;
    list[generation::LIST_BOXES as usize] = frame.boxes.len() as u32;
    for (k, &(b, z)) in frame.boxes.iter().enumerate() {
        let at = generation::BOXES_AT as usize + k * generation::BOX_WORDS as usize;
        for a in 0..4 {
            list[at + a] = b[a] as u32;
        }
        list[at + 4] = z;
    }
    list
}

impl Model {
    /// The frame through the Rust's passes, a pair's records in `thin_leaves`'s order.
    fn step(
        &mut self,
        world: World,
        frame: &Frame,
        tally: &mut Tally,
        claims: &mut Vec<[i32; 3]>,
    ) -> (FrameWords, Vec<u32>, u64) {
        for &s in &frame.stale {
            self.records[generation::head_at(s) as usize + 2] = 0;
        }
        if frame.marking {
            let mut list = list_head(frame);
            mark_rust(world.clipmap, &mut self.records, &mut list, &self.shown);
            // Thin, then what each listed record stands as.
            let whole = list[generation::THIN_ARGS as usize + 2] as usize;
            let parts = list[generation::LIST_PARTS as usize] as usize;
            let at_whole = generation::LIST_AT as usize;
            let at_part = generation::LIST_WORDS as usize;
            let mut asked: Vec<(u32, u32, u32, [i32; 2])> = Vec::new();
            for &w in &list[at_whole..at_whole + whole] {
                let (s, j) = generation::pair_of(w);
                let cell = clip::eye_cell(s / ENTRIES);
                let h = generation::head_at(s) as usize;
                let q = [self.records[h] as i32, self.records[h + 1] as i32];
                let leaves = thin_leaves(j as usize, cell, q);
                let cap = DECORS[j as usize].cap();
                self.records[generation::cnt_at(s, j) as usize] = leaves.len() as u32;
                for (i, leaf) in leaves.iter().take(cap as usize).enumerate() {
                    let r = word_of(s, j, i as u32);
                    self.records[r + 1] = leaf[0] as u32;
                    self.records[r + 2] = leaf[1] as u32;
                    asked.push((s, j, i as u32, *leaf));
                }
            }
            for &w in &list[at_part - parts..at_part] {
                let (s, j) = generation::pair_of(w);
                let cell = clip::eye_cell(s / ENTRIES);
                let n =
                    self.records[generation::cnt_at(s, j) as usize].min(DECORS[j as usize].cap());
                for i in 0..n {
                    let r = word_of(s, j, i);
                    let leaf = [self.records[r + 1] as i32, self.records[r + 2] as i32];
                    if is_changed(&frame.boxes, cell, leaf) {
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
                                let cell = clip::eye_cell(s / ENTRIES);
                                let mut q = Generating {
                                    w: world,
                                    cuts: frame.cuts,
                                    tally: &mut tally,
                                    walk_visits: 0,
                                    ask_visits: 0,
                                    placed: Default::default(),
                                    last_decor: Default::default(),
                                    claims: Vec::new(),
                                    last_lacks: false,
                                };
                                let stood = stand(&mut q, j, cell, leaf);
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
                    let was = self.records[r];
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
                    self.records[r] = word;
                    if word != generation::UNGENERATED {
                        self.records[r + 3] = z;
                        self.records[r + 4] = sink;
                    }
                }
            }
            self.list = list;
        }
        // Sized, laid out, compacted.
        let order = each_record(&self.records, world.clipmap, self.workgroups);
        let mut regions = vec![0u32; 2 * drawn::REGIONS as usize];
        let mut free_n = 0;
        let mut free_words = Vec::new();
        for &i in &order {
            if cnt_one(&frame.sizing, &world, &mut self.records, &mut regions, i) {
                free_n += 1;
                free_words.push(i as u32);
            }
        }
        let n = drawn::REGIONS as usize;
        let mut uniforms = fresh_uniforms(self.cap);
        let mut args = vec![0u32; drawn::ARGS_WORDS as usize * n];
        let mut first = 0;
        for r in 0..n {
            regions[n + r] = first;
            uniforms[r * drawn::UNIFORM_WORDS as usize + drawn::UNIFORM_IDX as usize] += first;
            let a = drawn::ARGS_WORDS as usize * r;
            args[a] = if r % 2 == 0 {
                self.meshes[r / 2]
            } else {
                drawn::IMPOSTOR_CORNERS
            };
            args[a + 1] = regions[r];
            first += regions[r];
        }
        let mut lanes: Vec<(u32, [u32; 5])> = order
            .iter()
            .filter(|&&i| self.records[i] != generation::UNGENERATED)
            .filter_map(|&i| lanes_of(&frame.sizing.drawing, &self.records, i))
            .collect();
        lanes.sort_unstable();
        for r in 0..n {
            regions[n + r] += regions[r];
        }
        let mut left = words_of(&self.records, regions, args, uniforms, &[], self.cap);
        left.lanes = lanes;
        (left, free_words, free_n)
    }
}

/// Words apart between what a frame left and what the Rust's did: records exact, or, where a
/// float may move them, under their region; counts exact where none may.
fn frame_differs(
    got: &FrameWords,
    want: &FrameWords,
    free: &BTreeMap<[i32; 2], ()>,
    free_n: u64,
) -> Vec<String> {
    let mut out = Vec::new();
    if got.pairs.len() != want.pairs.len() {
        out.push(format!(
            "pairs {} against {}",
            got.pairs.len(),
            want.pairs.len()
        ));
    }
    for (k, (cnt, leaves)) in &want.pairs {
        let Some((got_cnt, got_leaves)) = got.pairs.get(k) else {
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
            let mask = (1u32 << drawn::REGION_SHIFT) - 1;
            let same = if w[0] == generation::UNGENERATED {
                g[0] == w[0]
            } else if free.contains_key(leaf) {
                g[0] & mask == w[0] & mask && g[1..] == w[1..]
            } else {
                g == w
            };
            if !same {
                out.push(format!("{k:?} {leaf:?}: got {g:x?}, want {w:x?}"));
            }
        }
    }
    if free_n == 0 {
        if got.regions != want.regions {
            out.push("regions".to_string());
        }
        if got.args != want.args {
            out.push("args".to_string());
        }
        if got.uniforms != want.uniforms {
            out.push("uniforms".to_string());
        }
        if got.lanes != want.lanes {
            out.push(format!(
                "lanes, {} against {}",
                got.lanes.len(),
                want.lanes.len()
            ));
        }
        if got.placed_cnt != want.placed_cnt {
            out.push("what `Instances::cnt` reads".to_string());
        }
    } else {
        let apart = got
            .lanes
            .iter()
            .filter(|l| want.lanes.binary_search(l).is_err())
            .count() as u64;
        if apart > free_n {
            out.push(format!("{apart} lanes apart with {free_n} at an edge"));
        }
    }
    out
}

/// The buffers one chain of passes runs over, and its pipelines.
struct Chain<'d> {
    device: &'d wgpu::Device,
    queue: &'d wgpu::Queue,
    pipelines: &'d [wgpu::ComputePipeline; 6],
    /// `light_mark_draws_cs`, generated, a reader beside the draws of what compact lays.
    mark_draws: &'d wgpu::ComputePipeline,
    records: wgpu::Buffer,
    list: wgpu::Buffer,
    regions: wgpu::Buffer,
    args: wgpu::Buffer,
    uniforms: wgpu::Buffer,
    instances: wgpu::Buffer,
    cap: u32,
    workgroups: u32,
}

/// What a frame's world binds: the clip levels, the ground, the area, what is placed, the ways.
struct WorldBuffers<'b> {
    clipmap: &'b wgpu::Buffer,
    ground: &'b wgpu::Buffer,
    shards: &'b wgpu::Buffer,
    overwrites: &'b wgpu::Buffer,
    pool: &'b wgpu::Buffer,
    rows: &'b wgpu::Buffer,
    shown: &'b wgpu::Buffer,
    meshes: &'b wgpu::Buffer,
}

impl Chain<'_> {
    /// One frame, as `Generator::dispatch` records it: one compute pass, what marks by the list
    /// dispatched off it.
    fn frame(&self, frame: &Frame, b: &WorldBuffers) -> FrameWords {
        let (d, q) = (self.device, self.queue);
        let store = wgpu::BufferUsages::STORAGE;
        for &s in &frame.stale {
            q.write_buffer(
                &self.records,
                (generation::head_at(s) as u64 + 2) * 4,
                bytes(&[0]),
            );
        }
        if frame.marking {
            q.write_buffer(
                &self.list,
                0,
                bytes(&list_head(frame)[..generation::LIST_AT as usize]),
            );
        }
        q.write_buffer(
            &self.regions,
            0,
            bytes(&vec![0u32; 2 * drawn::REGIONS as usize]),
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
                let mark_group = {
                    bind(
                        mark,
                        &[
                            (1, b.clipmap),
                            (9, &self.records),
                            (11, &self.list),
                            (12, b.shown),
                        ],
                    )
                };
                pass.set_pipeline(mark);
                pass.set_bind_group(0, &mark_group, &[]);
                pass.dispatch_workgroups(generation::EYE_ENTRIES.div_ceil(generation::LANES), 1, 1);
                let thin_group =
                    { bind(thin, &[(9, &self.records), (10, b.rows), (11, &self.list)]) };
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
                            (6, b.overwrites),
                            (7, b.pool),
                            (9, &self.records),
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
                        (9, &self.records),
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
                        (9, &self.records),
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
        // The tiles the decor compacted stands on, as the light's mark reads its lanes: the
        // head laid out for the map's side as the renderer lays it (gpu/ray/cast.rs).
        let mut head = vec![0u32; light::HEAD_WORDS];
        head[light::HEAD_CAST] = CAST_NUMBER;
        for (k, v) in light::head_layout(MAP_SIDE) {
            head[k] = v;
        }
        let head = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytes(&head),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let tiles = light::tiles(MAP_SIDE);
        let stood = buffer(
            d,
            &vec![0u32; (light::stands_at(MAP_SIDE) + tiles * tiles) as usize],
        );
        dispatch(
            d,
            q,
            self.mark_draws,
            &[
                (0, &eye_buf),
                (11, &head),
                (12, &stood),
                (18, &drawing),
                (19, &self.regions),
                (20, &self.instances),
            ],
            [
                (light::ACTORS as u32 + self.cap).div_ceil(generation::LANES),
                1,
                1,
            ],
        );
        let stood = read(d, q, &stood, frame.name);
        let records = read(d, q, &self.records, frame.name);
        let regions = read(d, q, &self.regions, frame.name);
        let args = read(d, q, &self.args, frame.name);
        let uniforms = read(d, q, &self.uniforms, frame.name);
        let instances = read(d, q, &self.instances, frame.name);
        let mut left = words_of(&records, regions, args, uniforms, &instances, self.cap);
        left.stood = stood;
        left
    }
}

/// Frames through all the passes by hand, against the Rust's frame:
/// all whole; the eye moved, no mark; the placed decor gone, what it took coming back, some
/// bricks gone stale, whole and in part in one dispatch; bricks handed; the decor back; a cut;
/// a corner left out. `DECOR_RUNS` chains by hand a world, threads falling any way.
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
    let mut totals = Tally::default();
    for (name, at) in [
        ("east of the seam", real_world::EYE),
        ("west of the seam", [(1 << 23) - 300, (40 << 16) + 30]),
    ] {
        let t0 = std::time::Instant::now();
        let built = real_world::build_with_decors(at, 600);
        let unplaced = real_world::build(at).overwrites;
        let world = built.world();
        let without = World {
            overwrites: &unplaced,
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
        // What the kernel shows, a decor's zooms where it stands.
        let mut shown = [0u32; DECORS.len()];
        for (k, d) in DECORS.iter().enumerate() {
            for cell in FIRST_CELL..=LEAF_CELL {
                if scatter::any_stands_at(d, cell) {
                    shown[k] |= 1 << (cell - FIRST_CELL);
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
            make(&built.overwrites),
            make(&unplaced),
            make(&built.pool),
            make(rows_words()),
            make(&shown),
            make(&meshes),
        );
        // Placed decor, as the kernel hands it: over a workgroup's runs.
        let placed = generation::RUNS + 5;
        let cap = generation::SLOT_RECORDS + placed.next_power_of_two().max(64);
        let workgroups = generation::EYE_ENTRIES + placed.div_ceil(generation::RUNS);
        let mut init = vec![0u32; generation::record_at(cap) as usize];
        init[generation::PLACED_HEAD as usize] = placed;
        for n in 0..placed {
            let r = generation::record_at(generation::SLOT_RECORDS + n) as usize;
            let j = n % DECORS.len() as u32;
            let p = built.decors[n as usize];
            let (variant, turn) = generation::placed_pose(&DECORS[j as usize], p);
            init[r] = generation::record_word(
                j,
                variant,
                turn,
                scatter::coarsest_stand(&DECORS[j as usize], variant),
            );
            init[r + 1] = p[0] as u32;
            init[r + 2] = p[1] as u32;
            init[r + 3] = p[2] as u32;
        }
        // The frames, the later ones off what the first's said.
        let origin = [at[0] << 8, at[1] << 8, (built.top_at(at) + 30) << 8];
        let mut drawing = drawing_words(0.02, 0.0002);
        drawing[drawn::ANCHOR as usize] = at[0] as u32;
        drawing[drawn::ANCHOR as usize + 1] = at[1] as u32;
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
            records: init.clone(),
            list: Vec::new(),
            workgroups,
            cap,
            shown,
            meshes: meshes.clone(),
        };
        let mut tally = Tally::default();
        let mut claims = Vec::new();
        let mut wants = Vec::new();
        let mut step =
            |model: &mut Model, frame: Frame, tally: &mut Tally, claims: &mut Vec<[i32; 3]>| {
                let w = if frame.placed_decor { world } else { without };
                let (left, free_words, free_n) = model.step(w, &frame, tally, claims);
                let mut free = BTreeMap::new();
                for i in free_words {
                    let i = i as usize;
                    free.insert(
                        [model.records[i + 1] as i32, model.records[i + 2] as i32],
                        (),
                    );
                }
                eprintln!(
                    "{name}, {}: {} pairs, {} drawn, {free_n} at an edge",
                    frame.name,
                    left.pairs.len(),
                    left.lanes.len()
                );
                wants.push((frame, left, free, free_n));
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
        let edits: Vec<([i32; 4], Zoom)> = claims
            .iter()
            .take(12)
            .map(|&p| (reach(p, 6), generation::EVERY))
            .collect();
        assert!(!edits.is_empty(), "{name}: no placed decor took a record");
        let stale: Vec<u32> = (0..generation::EYE_ENTRIES)
            .filter(|s| mix(*s, [11, 2]).is_multiple_of(40))
            .collect();
        step(
            &mut model,
            Frame {
                name: "the placed decor gone, some stale",
                every: false,
                boxes: edits.clone(),
                stale,
                placed_decor: false,
                ..first.clone()
            },
            &mut tally,
            &mut claims,
        );
        let row = |s: u32| built.clipmap[s as usize];
        let handed: Vec<([i32; 4], Zoom)> = (0..9u32)
            .map(|k| {
                let cell = 22 + k % 3;
                let s = (cell - FIRST_CELL) * ENTRIES + k * 7;
                (generation::brick_box(cell, [row(s)[0], row(s)[1]]), cell)
            })
            .collect();
        step(
            &mut model,
            Frame {
                name: "bricks handed",
                every: false,
                boxes: handed,
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
                boxes: edits.clone(),
                ..first.clone()
            },
            &mut tally,
            &mut claims,
        );
        // A cut over what stands, its box asked.
        let p = claims[0];
        let cut_box = overwrite::cut_box(20, overwrite::ancestor_at(24, p, 4));
        step(
            &mut model,
            Frame {
                name: "a cut",
                every: false,
                boxes: vec![(cut_box, generation::EVERY)],
                cuts: [
                    Cut {
                        zoom: 20,
                        at: overwrite::ancestor_at(24, p, 4),
                    },
                    Cut::NONE,
                ],
                ..first.clone()
            },
            &mut tally,
            &mut claims,
        );
        let mut hide = sizing.clone();
        let c = built.decors[0];
        for (a, v) in c.iter().enumerate() {
            hide.drawing[drawn::HIDE as usize + a] = *v as u32;
        }
        hide.drawing[drawn::HIDE as usize + 3] = 1;
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
        // The chains.
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
                records: buf(&init, wgpu::BufferUsages::empty()),
                list: buf(
                    &vec![0; generation::LIST_WORDS as usize],
                    wgpu::BufferUsages::INDIRECT,
                ),
                regions: buf(
                    &vec![0; 2 * drawn::REGIONS as usize],
                    wgpu::BufferUsages::empty(),
                ),
                args: buf(
                    &vec![0xdead_beef; (drawn::ARGS_WORDS * drawn::REGIONS) as usize],
                    wgpu::BufferUsages::empty(),
                ),
                uniforms: buf(&fresh_uniforms(cap), wgpu::BufferUsages::empty()),
                instances: buf(
                    &vec![0; ((drawn::COLUMNS + 1) * cap) as usize],
                    wgpu::BufferUsages::empty(),
                ),
                cap,
                workgroups,
            }
        };
        let mut stood_first: Vec<Vec<u32>> = Vec::new();
        for run in 0..runs {
            let chain = chain_on();
            let t1 = std::time::Instant::now();
            let mut apart = 0;
            for (frame, want, free, free_n) in &wants {
                let b = WorldBuffers {
                    clipmap: &clipmap,
                    ground: &ground,
                    shards: &shards,
                    overwrites: if frame.placed_decor {
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
                let mut d = frame_differs(&got, want, free, *free_n);
                let k = wants.iter().position(|w| w.0.name == frame.name).unwrap();
                if stood_first.len() <= k {
                    let on = got.stood.iter().filter(|&&w| w == CAST_NUMBER).count() as u64;
                    *totals.0.entry("frame: tiles decor stands on").or_default() += on;
                    stood_first.push(got.stood.clone());
                } else if *free_n == 0 && got.stood != stood_first[k] {
                    d.push("the tiles `light_mark_draws_cs` stood on".to_string());
                }
                for line in d.iter().take(8) {
                    eprintln!("{name}, {}, run {run}: {line}", frame.name);
                }
                apart += d.len() as u64;
            }
            eprintln!("{name}: run {run}: {apart} apart, {:?}", t1.elapsed());
            *totals.0.entry("apart").or_default() += apart;
            *totals.0.entry("chains").or_default() += 1;
        }
        totals.merge(tally);
    }
    for (k, v) in &totals.0 {
        if k.starts_with("frame") || k.starts_with("apart") || k.starts_with("chains") {
            eprintln!("{v:>12} {k}");
        }
    }
    assert_eq!(totals.0["apart"], 0);
    assert!(
        totals.0["frame: a record comes back"] > 0
            && totals.0["frame: a record taken away"] > 0
            && totals.0["frame: tiles decor stands on"] > 0
    );
}

/// decor_row.wgsl `mul_hi` is the high word of a product: random pairs, either side past 2^16,
/// and their ends.
#[test]
fn hand_mul_hi_is_the_high_word() {
    let Some((device, queue)) = device() else {
        return;
    };
    let mut draw = real_world::Draw(0x2545_f491);
    let mut pairs: Vec<[u32; 2]> = (0..4096).map(|_| [draw.next(), draw.next()]).collect();
    for a in [0, 1, 0xffff, 0x1_0000, u32::MAX] {
        for b in [0, 1, 0xffff, 0x1_0000, 9, u32::MAX] {
            pairs.push([a, b]);
        }
    }
    let source = [
        include_str!(concat!(env!("OUT_DIR"), "/shader/world_word.wgsl")),
        include_str!(concat!(env!("OUT_DIR"), "/shader/decor_word.wgsl")),
        include_str!(concat!(env!("OUT_DIR"), "/shader/seed.wgsl")),
        include_str!(concat!(env!("OUT_DIR"), "/shader/decor_row.wgsl")),
        r"
@group(0) @binding(12) var<storage, read> asked: array<vec2<u32>>;
@group(0) @binding(13) var<storage, read_write> told: array<u32>;

@compute @workgroup_size(64)
fn mul_hi_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x < arrayLength(&asked)) {
    told[id.x] = mul_hi(asked[id.x].x, asked[id.x].y);
  }
}
",
    ]
    .join("\n");
    let p = pipeline(&device, "mul_hi", &source, "mul_hi_cs");
    let flat: Vec<u32> = pairs.iter().flatten().copied().collect();
    let (asked, told) = (
        buffer(&device, &flat),
        buffer(&device, &vec![0; pairs.len()]),
    );
    dispatch(
        &device,
        &queue,
        &p,
        &[(12, &asked), (13, &told)],
        [(pairs.len() as u32).div_ceil(64), 1, 1],
    );
    let got = read(&device, &queue, &told, "mul_hi");
    for (k, [a, b]) in pairs.iter().enumerate() {
        assert_eq!(
            got[k],
            ((*a as u64 * *b as u64) >> 32) as u32,
            "{a:#x} × {b:#x}"
        );
    }
}

/// decor_scatter.wgsl `scatter_count_of` counts as `scatter::count_of`: decors of seeds, chances
/// and roots other than the catalog's, means of many a root column so the rooms bind, every zoom
/// from the root to the leaves; and root columns whose draw lands on a count's rounding edge.
#[test]
fn hand_scatter_count_as_the_rust() {
    let Some((device, queue)) = device() else {
        return;
    };
    let mut draw = real_world::Draw(0x0dec_0c47);
    let mut asked: Vec<[u32; 6]> = Vec::new();
    // A chance whose mean a root column holds `per_root` 16.16 counts at most.
    let chance_for = |root: Zoom, per_root: u32| {
        let shift = 2 * (LEAF_CELL - root);
        (per_root >> shift.min(31)).clamp(1, 0xffff) << 16
    };
    for _ in 0..6000 {
        let root = FIRST_CELL + draw.next() % (LEAF_CELL - FIRST_CELL + 1);
        // From three over the root, where a column holds none, to the leaves.
        let zoom = (root + draw.next() % (LEAF_CELL - root + 4)).saturating_sub(3);
        let per_root = [1 << 14, 1 << 16, 3 << 17, 1 << 20, 1 << 24][draw.next() as usize % 5];
        let chance = chance_for(root, per_root);
        let b = [
            draw.within(-(1 << 20), 1 << 20),
            draw.within(-(1 << 20), 1 << 20),
        ];
        asked.push([draw.next(), chance, root, zoom, b[0] as u32, b[1] as u32]);
    }
    // The rounding edge: `mean + (draw & 0xffff)` a whole count with `draw` odd, which an odd
    // mean alone reaches, a root at the leaves.
    let mut edges = 0;
    for (seed, root, per_root) in [
        (0x7902_530c, 24, 2049),
        (0x1234_5678, 24, 0x4_0001),
        (0xb0a5_802d, 24, 777),
    ] {
        let chance = chance_for(root, per_root);
        let mean = (chance >> 16) << (2 * (LEAF_CELL - root));
        for k in 0..1 << 22 {
            let c = [k % 2048 - 1024, k / 2048 - 1024];
            let d = node_seed(seed ^ scatter::COUNT, (root, c));
            if d & 1 == 1 && (mean + (d & 0xffff)).is_multiple_of(1 << 16) {
                for zoom in [root, LEAF_CELL] {
                    let b = [c[0] << (zoom - root), c[1] << (zoom - root)];
                    asked.push([seed, chance, root, zoom, b[0] as u32, b[1] as u32]);
                }
                edges += 1;
            }
        }
    }
    let source = [
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
    told[id.x] = scatter_count_of(asked[a], asked[a + 1u], asked[a + 2u], asked[a + 3u], b);
  }
}
",
    ]
    .join("\n");
    let p = pipeline(&device, "count", &source, "count_cs");
    let flat: Vec<u32> = asked.iter().flatten().copied().collect();
    let (asked_buf, told) = (
        buffer(&device, &flat),
        buffer(&device, &vec![0; asked.len()]),
    );
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
        let want = scatter::count_of([a[0], a[1], a[2]], a[3], b);
        assert_eq!(got[k], want, "{a:?}");
        many += (want > 1) as u32;
        // A leaf of a root column holding four or more: its rooms bound the count on the way down.
        if a[3] == LEAF_CELL {
            let t = a[3] - a[2];
            let at_root = scatter::count_of([a[0], a[1], a[2]], a[2], [b[0] >> t, b[1] >> t]);
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
