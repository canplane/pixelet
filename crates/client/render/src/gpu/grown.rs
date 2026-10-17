// pxl-render/gpu/grown.rs

//! The decor the GPU grows and draws (pxl-portable `scatter`): what the
//! ground grows in the eye's windows over the cast's world (`decor_cs`),
//! placed decor handed as records beside it — grown again only in the eye
//! slots what it reads changed in (`decor_mark_cs`, `Grows`), the records
//! standing otherwise; then each frame each
//! record taken where the rays ask the zoom it was grown at and sized for
//! the screen, counted a draw region (`decor_count_cs`), the regions laid
//! out with their indirect args (`decor_args_cs`), and each record drawn
//! into one decor block in the layout the decor and impostor shaders read
//! (`decor_place_cs`). The CPU hands what only it knows and never reads
//! what is grown.

use pxl_portable::decor::{COUNTS, DECORS};
use pxl_portable::eye;
use pxl_portable::scatter::{self, drawn, GROUPS, SQUARE_BLOCKS};

use crate::frame::shader::record::{COMPUTE, STORAGE, STORAGE_RW, UNIFORM};
use crate::frame::shader::record::{SHADER_GROWN, SHADER_GROWN_ARGS, SHADER_GROWN_COUNT, SHADER_GROWN_MARK, SHADER_GROWN_PLACE};
use crate::gpu::api::{buffer_entry, buffer_usage, bytes, release, word_bytes, Gpu};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::renderer::Read;

/// What the CPU hands the GPU's decor this frame.
pub struct DecorFrame<'a> {
    /// Per kind, the window zooms it is grown at, a bit each over the first
    /// window's (`decor_cs`).
    pub shown: &'a [u32],
    /// pxl-portable `scatter::drawn`'s frame words, `PARAM_WORDS` of them.
    pub params: &'a [u32],
    /// Placed decor, a record each as `decor_cs` writes one: its kind,
    /// variant and turn, then its box's min corner in leaf cells.
    pub placed: &'a [[u32; 4]],
    /// The anchor the lanes are about, less the frame's eye: where the decor
    /// block is drawn.
    pub translation: [f32; 3],
    /// What overwrites wrote since the last frame, `scatter::EDITS` boxes
    /// of leaf cells at most (pxl-portable `scatter::meets`): the slots
    /// whose decor they reach grow again.
    pub edits: &'a [[i32; 4]],
}

/// What decor's growth reads besides the frame's decor, none of it the
/// eye's place: the windows' table and the served shards, what is placed
/// (the overwrites' revision, with the frame's `edits`) and a preview's
/// cuts, and the squares filled this frame. A slot grows again where its
/// square is another, one beside it was filled, or an edit or a cut as it
/// was or is meets it; every one where the kinds shown or the shards moved,
/// what is placed moved with no edit named, or the records were made again.
pub struct Grows<'a> {
    pub window: &'a [i32],
    pub shards: &'a [[i32; 4]],
    pub revision: u32,
    pub cuts: &'a [u32; eye::CUTS],
    pub filled: u32,
}

/// The workgroups' sides: `decor_cs`'s, and a record a thread's.
const GROW_WORKGROUP: u32 = 8;
const RECORD_WORKGROUP: u32 = 64;

const MARK_GROUP: [u32; 18] = [
    0, COMPUTE, STORAGE, 1, COMPUTE, STORAGE, 2, COMPUTE, STORAGE, 3, COMPUTE, STORAGE_RW, 4, COMPUTE, STORAGE_RW, 5, COMPUTE, STORAGE,
];
const GROW_GROUP: [u32; 27] = [
    0, COMPUTE, UNIFORM, 1, COMPUTE, STORAGE, 2, COMPUTE, STORAGE, 3, COMPUTE, STORAGE, 4, COMPUTE, STORAGE, 5, COMPUTE, STORAGE, 6, COMPUTE, STORAGE_RW, 7, COMPUTE, STORAGE, 8, COMPUTE, STORAGE,
];
const COUNT_GROUP: [u32; 18] = [
    0, COMPUTE, UNIFORM, 1, COMPUTE, STORAGE, 2, COMPUTE, STORAGE, 3, COMPUTE, STORAGE_RW, 4, COMPUTE, STORAGE_RW, 5, COMPUTE, STORAGE,
];
const ARGS_GROUP: [u32; 12] = [0, COMPUTE, STORAGE, 1, COMPUTE, STORAGE_RW, 2, COMPUTE, STORAGE_RW, 3, COMPUTE, STORAGE_RW];
const PLACE_GROUP: [u32; 18] = [
    0, COMPUTE, STORAGE, 1, COMPUTE, STORAGE, 2, COMPUTE, STORAGE, 3, COMPUTE, STORAGE_RW, 4, COMPUTE, STORAGE_RW, 5, COMPUTE, STORAGE,
];

pub struct Grown {
    mark: Built,
    grow: Built,
    count: Built,
    args_built: Built,
    place: Built,
    groups: [u32; 5],
    /// `decor_mark_cs`'s words (`scatter::MARKS_AT`), and what it lists for
    /// `decor_cs`'s dispatch (`scatter::GROWS_AT`).
    marks: u32,
    grows: u32,
    shown: u32,
    params: u32,
    rows: u32,
    meshes: u32,
    regions: u32,
    args: u32,
    /// Each region's draw uniform (`drawn::UNIFORM_WORDS` apart), as the
    /// decor and impostor shaders read one, its index set by the args pass.
    uniforms: u32,
    /// The records, laid by slot (`scatter::record_at`); then the decor
    /// block, `drawn::LANES` lanes and the index list, `room` slots each.
    records: u32,
    block: u32,
    room: u32,
    /// The world its groups bind: grouped again exactly when it is another.
    bound: World,
    on: bool,
    /// What every slot was grown from (`Grows`); the windows' table, the
    /// overwrites' revision, the cuts and the placed decor as last handed;
    /// and whether this frame marks slots to grow.
    read: Read,
    window: Read,
    revision: Option<u32>,
    cuts: [u32; eye::CUTS],
    placed: Read,
    marking: bool,
}

/// The world `decor_cs` asks: the cast's eye, window, the tiles' frames, the
/// served shards and the overwrites; and the squares filled this frame,
/// which `decor_mark_cs` reads.
pub type World = [u32; 6];

impl Grown {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines) -> Grown {
        let buffer = |size: u32, usage: u32| g.buffer(size, usage);
        let storage = buffer_usage::STORAGE | buffer_usage::COPY_DST;
        let rows = buffer(core::mem::size_of_val(&DECORS) as u32, storage);
        let table = DECORS;
        // SAFETY: the rows are `repr(C)` words alone (pxl-portable decor.spec.rs), read while `table` lives.
        let words = unsafe { core::slice::from_raw_parts(table.as_ptr().cast::<u32>(), core::mem::size_of_val(&table) / 4) };
        g.write_buffer(rows, 0, bytes(words));
        Grown {
            mark: pipelines.build_compute(g, SHADER_GROWN_MARK, &MARK_GROUP),
            grow: pipelines.build_compute(g, SHADER_GROWN, &GROW_GROUP),
            count: pipelines.build_compute(g, SHADER_GROWN_COUNT, &COUNT_GROUP),
            args_built: pipelines.build_compute(g, SHADER_GROWN_ARGS, &ARGS_GROUP),
            place: pipelines.build_compute(g, SHADER_GROWN_PLACE, &PLACE_GROUP),
            groups: [0; 5],
            marks: buffer(word_bytes(scatter::MARKS_WORDS as usize), storage),
            grows: buffer(word_bytes((scatter::GROWS_AT + scatter::EYE_SLOTS * DECORS.len() as u32) as usize), storage | buffer_usage::INDIRECT),
            shown: buffer(word_bytes(DECORS.len()), storage),
            params: buffer(word_bytes(drawn::PARAM_WORDS as usize), storage),
            rows,
            meshes: buffer(word_bytes(GROUPS as usize), storage),
            regions: buffer(word_bytes(2 * drawn::REGIONS as usize), storage),
            args: buffer(word_bytes((drawn::ARGS_WORDS * drawn::REGIONS) as usize), storage | buffer_usage::INDIRECT),
            uniforms: buffer(word_bytes((drawn::UNIFORM_WORDS * drawn::REGIONS) as usize), storage | buffer_usage::UNIFORM),
            records: 0,
            block: 0,
            room: 0,
            bound: [0; 6],
            on: false,
            read: Read::default(),
            window: Read::default(),
            revision: None,
            cuts: [0; eye::CUTS],
            placed: Read::default(),
            marking: false,
        }
    }

    /// The frame's decor handed over, none where decor is off, and what its
    /// growth reads (`grows`); `meshes` each group's index count, none while
    /// its model loads. Grouped again where `world` is not the one its
    /// groups bind or the records' room grew, so a frame without decor
    /// misses no change.
    pub fn prepare(&mut self, g: &Gpu, frame: Option<DecorFrame>, grows: Grows, meshes: &[u32; GROUPS as usize], world: World) {
        self.on = frame.is_some();
        let Some(f) = frame else {
            self.read.forget();
            return;
        };
        let need = scatter::SLOT_RECORDS + (f.placed.len() as u32).next_power_of_two().max(64);
        let grow = need > self.room;
        if grow {
            release(self.records);
            release(self.block);
            self.room = need;
            self.records = g.buffer(word_bytes(scatter::record_at(need).0 as usize), buffer_usage::STORAGE | buffer_usage::COPY_DST | buffer_usage::COPY_SRC);
            self.block = g.buffer(word_bytes(((drawn::LANES + 1) * need) as usize), buffer_usage::STORAGE);
            self.placed.forget();
        }
        let regroup = grow || world != self.bound;
        if regroup {
            self.regroup(g, world);
            self.bound = world;
        }
        // Every slot, where what every slot grows from moved.
        let unnamed = self.revision != Some(grows.revision) && f.edits.is_empty();
        let r = &mut self.read;
        r.begin();
        r.read(f.shown);
        r.read(grows.shards);
        r.mark(regroup || unnamed);
        let all = !r.end();
        self.revision = Some(grows.revision);
        // The boxes the slots they meet grow again in: the edits, and each
        // cut that moved as it was and as it is.
        let mut boxes = [[0i32; 4]; scatter::BOXES as usize];
        let mut n = 0;
        for &e in f.edits.iter().take(scatter::EDITS as usize) {
            (boxes[n], n) = (e, n + 1);
        }
        for k in [0, eye::CUT_WORDS] {
            let (was, is) = (&self.cuts[k..k + eye::CUT_WORDS], &grows.cuts[k..k + eye::CUT_WORDS]);
            if was != is {
                for c in [was, is].into_iter().filter(|c| c[3] != eye::NO_CUT) {
                    (boxes[n], n) = (scatter::cut_box(c[3], [c[0] as i32, c[1] as i32, c[2] as i32]), n + 1);
                }
            }
        }
        self.cuts = *grows.cuts;
        let w = &mut self.window;
        w.begin();
        w.read(grows.window);
        let moved = !w.end();
        self.marking = all || moved || grows.filled != 0 || n != 0;
        if self.marking {
            g.write_buffer(self.marks, 0, bytes(&[u32::from(all), grows.filled, n as u32, 0]));
            g.write_buffer(self.marks, scatter::MARKS_AT * 4, bytes(boxes[..n].as_flattened()));
            g.write_buffer(self.grows, 0, bytes(&[(COUNTS as u32).div_ceil(GROW_WORKGROUP), SQUARE_BLOCKS.div_ceil(GROW_WORKGROUP), 0, 0]));
            g.write_buffer(self.shown, 0, bytes(f.shown));
        }
        let p = &mut self.placed;
        p.begin();
        p.read(f.placed);
        if !p.end() {
            g.write_buffer(self.records, scatter::PLACED_HEAD * 4, bytes(&[f.placed.len() as u32, 0, 0, 0]));
            if !f.placed.is_empty() {
                g.write_buffer(self.records, scatter::record_at(scatter::SLOT_RECORDS).0 * 4, bytes(f.placed.as_flattened()));
            }
        }
        g.write_buffer(self.params, 0, bytes(f.params));
        g.write_buffer(self.meshes, 0, bytes(meshes));
        g.write_buffer(self.regions, 0, bytes(&[0u32; 2 * drawn::REGIONS as usize]));
        // Each region's uniform, its index the lanes' base until the args
        // pass adds its first slot: a model's group drawn about the anchor,
        // a sprite's its group's layer too.
        let mut uniforms = [0u32; (drawn::UNIFORM_WORDS * drawn::REGIONS) as usize];
        let [x, y, z] = f.translation;
        for r in 0..drawn::REGIONS {
            let u = (r * drawn::UNIFORM_WORDS) as usize;
            let layer = if r % 2 == 1 { r / 2 } else { 0 };
            uniforms[u..u + 8].copy_from_slice(&[x.to_bits(), y.to_bits(), z.to_bits(), 0, self.room, 1.0f32.to_bits(), layer, drawn::LANES * self.room]);
        }
        g.write_buffer(self.uniforms, 0, bytes(&uniforms));
    }

    fn regroup(&mut self, g: &Gpu, world: World) {
        let group = |built: &Built, ids: &[u32]| {
            let mut entries = [buffer_entry(0, 0); 9];
            for (k, &b) in ids.iter().enumerate() {
                entries[k] = buffer_entry(k as u32, b);
            }

            g.bind_group(built.layouts[0], &entries[..ids.len()])
        };
        for id in self.groups {
            release(id);
        }
        let [eye, window, ground, shards, overwrites, fresh] = world;
        self.groups = [
            group(&self.mark, &[self.marks, window, fresh, self.records, self.grows, self.rows]),
            group(&self.grow, &[eye, window, ground, shards, overwrites, self.shown, self.records, self.rows, self.grows]),
            group(&self.count, &[eye, self.params, self.rows, self.records, self.regions, window]),
            group(&self.args_built, &[self.meshes, self.regions, self.args, self.uniforms]),
            group(&self.place, &[self.params, self.rows, self.records, self.regions, self.block, window]),
        ];
    }

    /// No decor this frame: none grown, none drawn, and the next grows.
    pub fn off(&mut self) {
        self.on = false;
        self.read.forget();
    }

    /// The frame that grew was dropped, or never drawn after its decor was
    /// handed: the next grows every slot again.
    pub fn forget(&mut self) {
        self.read.forget();
    }

    /// Within a compute pass over the cast's world, after the cast: the
    /// slots to grow again marked and grown where any may be, then every
    /// record counted, laid out and placed.
    pub fn dispatch(&self, g: &Gpu, pass: u32) {
        if !self.on || self.groups[0] == 0 {
            return;
        }
        if self.marking {
            g.set_pipeline(pass, self.mark.pipeline);
            g.set_bind_group(pass, 0, self.groups[0], &[]);
            g.dispatch(pass, scatter::EYE_SLOTS.div_ceil(RECORD_WORKGROUP), 1, 1);
            g.set_pipeline(pass, self.grow.pipeline);
            g.set_bind_group(pass, 0, self.groups[1], &[]);
            g.dispatch_indirect(pass, self.grows, 0);
        }
        let records = self.room.div_ceil(RECORD_WORKGROUP);
        for (k, built) in [&self.count, &self.args_built, &self.place].into_iter().enumerate() {
            let x = if k == 1 { 1 } else { records };
            g.set_pipeline(pass, built.pipeline);
            g.set_bind_group(pass, 0, self.groups[2 + k], &[]);
            g.dispatch(pass, x, 1, 1);
        }
    }

    /// What the draws read, while decor is on: the decor block, its slots,
    /// the regions' indirect args, and where the block is drawn.
    pub fn drawn(&self) -> Option<Drawn> {
        (self.on && self.block != 0).then_some(Drawn {
            block: self.block,
            args: self.args,
            uniforms: self.uniforms,
        })
    }
}

/// The decor block as the draws read it (`Grown::drawn`): the block, the
/// regions' indirect args and their draw uniforms.
#[derive(Clone, Copy)]
pub struct Drawn {
    pub block: u32,
    pub args: u32,
    pub uniforms: u32,
}

impl Drawn {
    /// Where region `region`'s indirect args lie, in bytes.
    pub fn args_at(&self, region: u32) -> u32 {
        drawn::ARGS_WORDS * 4 * region
    }

    /// Where region `region`'s draw uniform lies, a dynamic offset in bytes.
    pub fn uniform_at(&self, region: u32) -> u32 {
        drawn::UNIFORM_WORDS * 4 * region
    }
}
