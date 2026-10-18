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

use crate::frame::shader::record::compute;
use crate::frame::shader::record::{SHADER_GROWN, SHADER_GROWN_ARGS, SHADER_GROWN_COUNT, SHADER_GROWN_MARK, SHADER_GROWN_PLACE};
use crate::frame::shader::words::DRAW_FLOATS;
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
/// eye's place: the windows' table, a preview's cuts, and the squares filled
/// this frame. A slot grows again only where something it read changed: its
/// square is another, it or one its boxes reach was filled
/// (`scatter::beside`), or an edit or a cut as it was or is meets it. The
/// overwrites' every write names its box (the frame's `edits`), and the
/// service area's index is made once, before any square is held, so neither
/// grows a slot by itself.
pub struct Grows<'a> {
    pub window: &'a [i32],
    pub cuts: &'a [u32; eye::CUTS],
    pub filled: u32,
    /// The eye as the cast's (pxl-portable `eye`), of which the count pass
    /// reads its place and the zooms it asks (`ORIGIN`, `TAU`, `RANGE`).
    pub eye: &'a [u32; eye::WORDS],
}

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
    /// The placed decor's records, as last handed.
    placed_count: u32,
    /// The world its groups bind: grouped again exactly when it is another.
    bound: World,
    on: bool,
    /// The kinds' zooms as last written, a constant of the table the kernel
    /// hands once; the windows' table, the cuts and the placed decor as last
    /// handed; and whether this frame marks slots to grow.
    shown_read: Read,
    window: Read,
    cuts: [u32; eye::CUTS],
    placed: Read,
    marking: bool,
    /// The boxes the slots they meet grow again in, held until a frame
    /// that marks by them is drawn: one dropped, or handed and never drawn,
    /// hands them again, as growing a slot twice grows it the same.
    /// Past `scatter::BOXES`, the rest joined into the last.
    held: [[i32; 4]; scatter::BOXES as usize],
    holding: usize,
    /// How many of `held` this frame marks by.
    marked: usize,
    /// Whether the slots' records were not kept, decor having been off: the
    /// next frame drawn with decor grows every slot.
    stale: bool,
    /// What the count, args and place passes read past the records, as they
    /// last ran: the eye's place and zooms, the frame's words, the meshes,
    /// the anchor and the records' room; and whether this frame runs them,
    /// which it does where that moved or the records did.
    counted: Read,
    counting: bool,
}

/// The world `decor_cs` asks: the cast's eye, window, the tiles' frames, the
/// service area's index and the overwrites, and the squares filled this
/// frame, which `decor_mark_cs` reads.
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
            mark: pipelines.build_compute(g, SHADER_GROWN_MARK, &compute::DECOR_MARK_GROUP),
            grow: pipelines.build_compute(g, SHADER_GROWN, &compute::DECOR_GROUP),
            count: pipelines.build_compute(g, SHADER_GROWN_COUNT, &compute::DECOR_COUNT_GROUP),
            args_built: pipelines.build_compute(g, SHADER_GROWN_ARGS, &compute::DECOR_ARGS_GROUP),
            place: pipelines.build_compute(g, SHADER_GROWN_PLACE, &compute::DECOR_PLACE_GROUP),
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
            placed_count: 0,
            bound: [0; 6],
            on: false,
            shown_read: Read::default(),
            window: Read::default(),
            cuts: [0; eye::CUTS],
            placed: Read::default(),
            counted: Read::default(),
            counting: false,
            marking: false,
            held: [[0; 4]; scatter::BOXES as usize],
            holding: 0,
            marked: 0,
            stale: true,
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
            // Nothing grows while decor is off, and what moved meanwhile is
            // not marked: the slots' records were not kept, so the next frame
            // with decor grows every one.
            self.stale = true;
            self.counted.forget();
            return;
        };
        let need = scatter::SLOT_RECORDS + (f.placed.len() as u32).next_power_of_two().max(64);
        let grew = need > self.room;
        if grew {
            // The slots' records carried over: the placed tail is laid anew.
            let (records, block) = (self.records, self.block);
            self.records = g.buffer(word_bytes(scatter::record_at(need).0 as usize), buffer_usage::STORAGE | buffer_usage::COPY_DST | buffer_usage::COPY_SRC);
            self.block = g.buffer(word_bytes(((drawn::LANES + 1) * need) as usize), buffer_usage::STORAGE);
            if records != 0 {
                let e = g.encoder();
                g.copy_buffer(e, (records, 0), (self.records, 0), word_bytes(scatter::record_at(scatter::SLOT_RECORDS).0 as usize));
                g.submit(e);
            }
            release(records);
            release(block);
            self.room = need;
            self.placed.forget();
        }
        let regroup = grew || world != self.bound;
        if regroup {
            self.regroup(g, world);
            self.bound = world;
        }
        let s = &mut self.shown_read;
        s.begin();
        s.read(f.shown);
        if !s.end() {
            g.write_buffer(self.shown, 0, bytes(f.shown));
        }
        // The boxes the slots they meet grow again in: the edits, and each
        // cut that moved as it was and as it is, held with any a frame not
        // drawn handed.
        for &b in f.edits {
            self.hold(b);
        }
        for k in [0, eye::CUT_WORDS] {
            let (was, is) = (self.cuts, grows.cuts);
            let (was, is) = (&was[k..k + eye::CUT_WORDS], &is[k..k + eye::CUT_WORDS]);
            if was != is {
                for c in [was, is].into_iter().filter(|c| c[3] != eye::NO_CUT) {
                    self.hold(scatter::cut_box(c[3], [c[0] as i32, c[1] as i32, c[2] as i32]));
                }
            }
        }
        self.cuts = *grows.cuts;
        let w = &mut self.window;
        w.begin();
        w.read(grows.window);
        let moved = !w.end();
        self.marked = self.holding;
        self.marking = self.stale || moved || grows.filled != 0 || self.marked != 0;
        if self.marking {
            g.write_buffer(self.marks, 0, bytes(&[u32::from(self.stale), grows.filled, self.marked as u32, 0]));
            g.write_buffer(self.marks, scatter::MARKS_AT * 4, bytes(self.held[..self.marked].as_flattened()));
            let w = compute::DECOR_WORKGROUP;
            g.write_buffer(self.grows, 0, bytes(&[(COUNTS as u32).div_ceil(w[0]), SQUARE_BLOCKS.div_ceil(w[1]), 0, 0]));
        }
        let p = &mut self.placed;
        p.begin();
        p.read(f.placed);
        let placed_moved = !p.end();
        self.placed_count = f.placed.len() as u32;
        if placed_moved {
            g.write_buffer(self.records, scatter::PLACED_HEAD * 4, bytes(&[f.placed.len() as u32, 0, 0, 0]));
            if !f.placed.is_empty() {
                g.write_buffer(self.records, scatter::record_at(scatter::SLOT_RECORDS).0 * 4, bytes(f.placed.as_flattened()));
            }
        }
        let c = &mut self.counted;
        c.begin();
        c.read(&grows.eye[eye::ORIGIN..eye::ORIGIN + 3]);
        c.read(&[grows.eye[eye::TAU], grows.eye[eye::RANGE], grows.eye[eye::RANGE + 1], self.room]);
        c.read(f.params);
        c.read(meshes);
        c.read(&f.translation);
        c.mark(self.marking || placed_moved || regroup);
        self.counting = !c.end();
        if !self.counting {
            return;
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
            uniforms[u..u + DRAW_FLOATS].copy_from_slice(&[x.to_bits(), y.to_bits(), z.to_bits(), 0, self.room, 1.0f32.to_bits(), layer, drawn::LANES * self.room]);
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

    /// No decor this frame: none grown, none drawn, and the next grows
    /// every slot (`stale`).
    pub fn off(&mut self) {
        self.on = false;
        self.stale = true;
        self.counted.forget();
    }

    /// Box `b` held for the slots it meets to grow again (`held`), past the
    /// room joined into the last.
    fn hold(&mut self, b: [i32; 4]) {
        if self.holding < self.held.len() {
            self.held[self.holding] = b;
            self.holding += 1;
        } else {
            let last = &mut self.held[self.holding - 1];
            *last = [last[0].min(b[0]), last[1].min(b[1]), last[2].max(b[2]), last[3].max(b[3])];
        }
    }

    /// The frame was drawn: the boxes it marked by and its every-slot
    /// growth are done with.
    pub fn drawn_frame(&mut self) {
        if self.marking {
            self.held.copy_within(self.marked..self.holding, 0);
            self.holding -= self.marked;
            self.stale = false;
        }
        self.marked = 0;
    }

    /// The frame was dropped, nothing of it recorded: what it wrote is
    /// written again, the slots it marked marked again by the boxes still
    /// held, the window's table and the placed decor read anew.
    pub fn dropped(&mut self) {
        self.marked = 0;
        self.window.forget();
        self.placed.forget();
        self.shown_read.forget();
        self.counted.forget();
    }

    /// Whether it marks or counts this frame: its passes run, and the frame
    /// draws what they leave.
    pub fn works(&self) -> bool {
        self.on && self.groups[0] != 0 && (self.marking || self.counting)
    }

    /// A frame's decor handed and never drawn: its edits held for the next.
    pub fn unconsumed(&mut self, edits: &[[i32; 4]]) {
        for &b in edits {
            self.hold(b);
        }
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
            g.dispatch(pass, scatter::EYE_SLOTS.div_ceil(compute::DECOR_MARK_WORKGROUP[0]), 1, 1);
            g.set_pipeline(pass, self.grow.pipeline);
            g.set_bind_group(pass, 0, self.groups[1], &[]);
            g.dispatch_indirect(pass, self.grows, 0);
        }
        if !self.counting {
            return;
        }
        // A workgroup a slot, reading the records its head holds, then the
        // placed decor's a workgroup's lanes each (gpu-query-shader
        // `records`): the records held, not the room.
        let records = |w: [u32; 3]| scatter::EYE_SLOTS + self.placed_count.div_ceil(w[0]);
        let passes = [
            (&self.count, records(compute::DECOR_COUNT_WORKGROUP)),
            (&self.args_built, 1),
            (&self.place, records(compute::DECOR_PLACE_WORKGROUP)),
        ];
        for (k, (built, x)) in passes.into_iter().enumerate() {
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
