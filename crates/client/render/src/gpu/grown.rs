// pxl-render/gpu/grown.rs

//! The decor the GPU grows and draws (pxl-portable `scatter`): what the
//! ground grows in the eye's windows over the cast's world (`decor_cs`),
//! placed decor handed as records beside it — grown where what it reads
//! changed, the records standing otherwise (`Grows`); then each frame each
//! record taken where the rays ask the zoom it was grown at and sized for
//! the screen, counted a draw region (`decor_count_cs`), the regions laid
//! out with their indirect args (`decor_args_cs`), and each record drawn
//! into one decor block in the layout the decor and impostor shaders read
//! (`decor_place_cs`). The CPU hands what only it knows and never reads
//! what is grown.

use pxl_portable::decor::{COUNTS, DECORS};
use pxl_portable::scatter::{drawn, GROUPS, GROWN_AT, GROWN_WORDS, SQUARE_BLOCKS, SQUARE_GROWN};
use pxl_portable::window::{EYE_WINDOWS, SLOTS};

use crate::frame::shader::record::{COMPUTE, STORAGE, STORAGE_RW, UNIFORM};
use crate::frame::shader::record::{SHADER_GROWN, SHADER_GROWN_ARGS, SHADER_GROWN_COUNT, SHADER_GROWN_PLACE};
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
}

/// What decor's growth reads besides the frame's decor, none of it the
/// eye's place: the windows' table and the served shards, what is placed
/// (the overwrites' revision) and a preview's cuts, and whether squares
/// were filled this frame. Where none of it changed, nor the decor handed,
/// the records stand and nothing is grown.
pub struct Grows<'a> {
    pub window: &'a [i32],
    pub shards: &'a [[i32; 4]],
    pub revision: u32,
    pub cuts: &'a [u32],
    pub filled: bool,
}

/// The most records `decor_cs` grows: every window's squares at their most
/// (pxl-portable `scatter::SQUARE_GROWN`).
const GROWN_ROOM: u32 = SQUARE_GROWN * EYE_WINDOWS * SLOTS;

/// The workgroups' sides: `decor_cs`'s, and a record a thread's.
const GROW_WORKGROUP: u32 = 8;
const RECORD_WORKGROUP: u32 = 64;

const GROW_GROUP: [u32; 24] = [
    0, COMPUTE, UNIFORM, 1, COMPUTE, STORAGE, 2, COMPUTE, STORAGE, 3, COMPUTE, STORAGE, 4, COMPUTE, STORAGE, 5, COMPUTE, STORAGE, 6, COMPUTE, STORAGE_RW, 7, COMPUTE, STORAGE,
];
const COUNT_GROUP: [u32; 15] = [0, COMPUTE, UNIFORM, 1, COMPUTE, STORAGE, 2, COMPUTE, STORAGE, 3, COMPUTE, STORAGE_RW, 4, COMPUTE, STORAGE_RW];
const ARGS_GROUP: [u32; 12] = [0, COMPUTE, STORAGE, 1, COMPUTE, STORAGE_RW, 2, COMPUTE, STORAGE_RW, 3, COMPUTE, STORAGE_RW];
const PLACE_GROUP: [u32; 15] = [0, COMPUTE, STORAGE, 1, COMPUTE, STORAGE, 2, COMPUTE, STORAGE, 3, COMPUTE, STORAGE_RW, 4, COMPUTE, STORAGE_RW];

pub struct Grown {
    grow: Built,
    count: Built,
    args_built: Built,
    place: Built,
    groups: [u32; 4],
    shown: u32,
    params: u32,
    rows: u32,
    meshes: u32,
    regions: u32,
    args: u32,
    /// Each region's draw uniform (`drawn::UNIFORM_WORDS` apart), as the
    /// decor and impostor shaders read one, its index set by the args pass.
    uniforms: u32,
    /// The records, the count first; then the decor block, `drawn::LANES`
    /// lanes and the index list, `room` slots each.
    records: u32,
    block: u32,
    room: u32,
    /// The world its groups bind: grouped again exactly when it is another.
    bound: World,
    on: bool,
    /// What the records were grown from (`Grows`), and whether this frame
    /// grows them again.
    read: Read,
    regrow: bool,
}

/// The world `decor_cs` asks: the cast's eye, window, the tiles' frames, the
/// served shards and the overwrites.
pub type World = [u32; 5];

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
            grow: pipelines.build_compute(g, SHADER_GROWN, &GROW_GROUP),
            count: pipelines.build_compute(g, SHADER_GROWN_COUNT, &COUNT_GROUP),
            args_built: pipelines.build_compute(g, SHADER_GROWN_ARGS, &ARGS_GROUP),
            place: pipelines.build_compute(g, SHADER_GROWN_PLACE, &PLACE_GROUP),
            groups: [0; 4],
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
            bound: [0; 5],
            on: false,
            read: Read::default(),
            regrow: false,
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
        let need = GROWN_ROOM + (f.placed.len() as u32).next_power_of_two().max(64);
        let grow = need > self.room;
        if grow {
            release(self.records);
            release(self.block);
            self.room = need;
            self.records = g.buffer(
                word_bytes((GROWN_AT + GROWN_WORDS * need) as usize),
                buffer_usage::STORAGE | buffer_usage::COPY_DST | buffer_usage::COPY_SRC,
            );
            self.block = g.buffer(word_bytes(((drawn::LANES + 1) * need) as usize), buffer_usage::STORAGE);
        }
        let regroup = grow || world != self.bound;
        if regroup {
            self.regroup(g, world);
            self.bound = world;
        }
        let r = &mut self.read;
        r.begin();
        r.read(grows.window);
        r.read(grows.shards);
        r.read(&[grows.revision]);
        r.read(grows.cuts);
        r.read(f.shown);
        r.read(f.placed);
        r.mark(grows.filled || regroup);
        self.regrow = !r.end();
        if self.regrow {
            g.write_buffer(self.shown, 0, bytes(f.shown));
            g.write_buffer(self.records, 0, bytes(&[f.placed.len() as u32, 0, 0, 0]));
            if !f.placed.is_empty() {
                g.write_buffer(self.records, GROWN_AT * 4, bytes(f.placed.as_flattened()));
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
            let mut entries = [buffer_entry(0, 0); 8];
            for (k, &b) in ids.iter().enumerate() {
                entries[k] = buffer_entry(k as u32, b);
            }

            g.bind_group(built.layouts[0], &entries[..ids.len()])
        };
        for id in self.groups {
            release(id);
        }
        let [eye, window, ground, shards, overwrites] = world;
        self.groups = [
            group(&self.grow, &[eye, window, ground, shards, overwrites, self.shown, self.records, self.rows]),
            group(&self.count, &[eye, self.params, self.rows, self.records, self.regions]),
            group(&self.args_built, &[self.meshes, self.regions, self.args, self.uniforms]),
            group(&self.place, &[self.params, self.rows, self.records, self.regions, self.block]),
        ];
    }

    /// No decor this frame: none grown, none drawn, and the next grows.
    pub fn off(&mut self) {
        self.on = false;
        self.read.forget();
    }

    /// The frame that grew was dropped: the next grows again.
    pub fn forget(&mut self) {
        self.read.forget();
    }

    /// Within a compute pass over the cast's world, after the cast: grown
    /// where what it reads changed, then counted, laid out and placed.
    pub fn dispatch(&self, g: &Gpu, pass: u32) {
        if !self.on || self.groups[0] == 0 {
            return;
        }
        let records = self.room.div_ceil(RECORD_WORKGROUP);
        let steps = [
            (
                &self.grow,
                (COUNTS as u32).div_ceil(GROW_WORKGROUP),
                SQUARE_BLOCKS.div_ceil(GROW_WORKGROUP),
                EYE_WINDOWS * SLOTS * DECORS.len() as u32,
            ),
            (&self.count, records, 1, 1),
            (&self.args_built, 1, 1, 1),
            (&self.place, records, 1, 1),
        ];
        for (k, (built, x, y, z)) in steps.into_iter().enumerate().skip(usize::from(!self.regrow)) {
            g.set_pipeline(pass, built.pipeline);
            g.set_bind_group(pass, 0, self.groups[k], &[]);
            g.dispatch(pass, x, y, z);
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
