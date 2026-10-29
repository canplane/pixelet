// pxl-render/gpu/grown.rs

//! The decor the GPU grows and draws (pxl-spec-core `scatter`): what the
//! ground grows in the eye's windows over the cast's world (`decor_cs`),
//! placed decor handed as records beside it — a candidate grown again only
//! where what it reads changed (`decor_mark_cs`, `Grows`), its record
//! standing otherwise; then each frame each
//! record taken where the rays ask the leaf cell's zoom or finer and sized
//! for the screen, counted a draw region (`decor_cnt_cs`), the regions laid
//! out with their indirect args (`decor_args_cs`), and each record drawn
//! into the decor's instances in the layout the decor and impostor shaders read
//! (`decor_place_cs`). The CPU hands what only it knows and never reads
//! what is grown.

use pxl_spec_core::decor::{COUNTS, DECORS};
use pxl_spec_core::scatter::{self, drawn, BRICK_PATCHES, GROUPS};
use pxl_spec_core::{eye, overwrite, window};

use crate::frame::draw::PIPELINE_DECOR;
use crate::frame::shader::record::compute;
use crate::frame::shader::record::{
    SHADER_GROWN, SHADER_GROWN_ARGS, SHADER_GROWN_CNT, SHADER_GROWN_MARK, SHADER_GROWN_PLACE,
    SHADER_GROWN_STAND,
};
use crate::frame::shader::word::DRAW_FLOATS;
use crate::gpu::api::{buf_entry, buf_range_entry, buffer_usage, bytes, release, word_bytes, Gpu};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::read::Read;
use crate::gpu::surface::WORLD_COLOR;

/// What the CPU hands the GPU's decor this frame.
pub struct DecorFrame<'a> {
    /// Per kind, the window zooms it is grown at, a bit each over the first
    /// window's (`decor_cs`).
    pub shown: &'a [u32],
    /// pxl-spec-core `scatter::drawn`'s drawing words, `DRAWING_WORDS` of them.
    pub drawing: &'a [u32],
    /// Placed decor, a record each as `decor_cs` writes one: its kind,
    /// variant and turn, then its box's min corner in leaf cells, its base
    /// sunk under it by none, as it stands where it is placed; read where
    /// it lies, unwritten until the page replays the turn.
    pub placed: &'a [[u32; scatter::GROWN_WORDS as usize]],
    /// The anchor the lanes are about, less the frame's eye: where the decor's
    /// instances are drawn.
    pub translation: [f32; 3],
    /// What overwrites wrote since the last frame, `overwrite::EDITS` boxes
    /// of leaf cells at most (pxl-spec-core `scatter::meets`): the slots
    /// whose decor they reach grow again.
    pub edits: &'a [[i32; 4]],
}

/// What decor's growth reads besides the frame's decor, none of it the
/// eye's place: the windows' table, a preview's cuts, and the bricks filled
/// this frame. A slot grows whole where its brick is another; else a
/// candidate grows again only where a box meets what it reads
/// (`scatter::reads`): an edit, a cut as it was or is, or a brick filled at
/// its window's zoom. The overwrites' every write names its box (the
/// frame's `edits`), and the service area's index is made once, before any
/// brick is held, so neither grows a slot by itself. A brick filled
/// reaches its own window's zoom alone: a finer candidate reading it where
/// its own window lacks a brick grows again when that brick is filled, at
/// its own zoom, and is drawn off the coarser one until then.
pub struct Grows<'a> {
    pub window: &'a [i32],
    pub cuts: &'a [u32; eye::CUTS],
    /// The bricks handed this frame (pxl-spec-core `window::FRESH_WORDS`).
    pub fresh: &'a [[u32; window::FRESH_WORDS as usize]],
    /// The eye as the cast's (pxl-spec-core `eye`), of which the count pass
    /// reads its place and the zooms it asks (`ORIGIN`, `RANGE`).
    pub eye: &'a [u32; eye::WORDS],
}

pub struct Grown {
    mark: Built,
    stand: Built,
    grow: Built,
    cnt: Built,
    args_built: Built,
    place: Built,
    groups: [u32; 6],
    /// What `decor_mark_cs` reads and lists for `decor_cs`'s dispatch
    /// (`scatter::GROWS_AT`).
    grows: u32,
    shown: u32,
    drawing: u32,
    rows: u32,
    meshes: u32,
    regions: u32,
    args: u32,
    /// Each region's draw uniform (`drawn::UNIFORM_WORDS` apart), as the
    /// decor and impostor shaders read one, its index set by the args pass.
    uniforms: u32,
    /// The records, laid by slot (`scatter::record_at`); then the decor's
    /// instances, `drawn::LANES` lanes and the index list, `room` slots each.
    records: u32,
    instances: u32,
    /// The draws' bind groups over the uniforms, a region's at a dynamic
    /// offset, and over the instances, in the layouts the decor and impostor
    /// draws share (record.rs `DRAW`, `UPLOAD`): made with their buffers.
    draw_layouts: [u32; 2],
    uniform_group: u32,
    instances_group: u32,
    room: u32,
    /// The placed decor's records, as last handed.
    placed_cnt: u32,
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
    /// Past `overwrite::BOXES`, the rest joined into the last.
    held: [[i32; 4]; overwrite::BOXES as usize],
    holding: usize,
    /// How many of `held` this frame marks by.
    marked: usize,
    /// Whether the slots' records were not kept, decor having been off: the
    /// next frame drawn with decor grows every slot.
    stale: bool,
    /// What the count, args and place passes read past the records, as they
    /// last ran: the eye's place and zooms, the drawing words, the meshes,
    /// the anchor and the records' room; and whether this frame runs them,
    /// which it does where that moved or the records did.
    counted: Read,
    counting: bool,
}

/// The world `decor_cs` asks: the cast's eye, window, the tiles' frames, the
/// service area's index, the overwrites and the ways' pool, as the cast
/// asks it.
pub type World = [u32; 6];

impl Grown {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines) -> Grown {
        let buf = |size: u32, usage: u32| g.buf(size, usage);
        let storage = buffer_usage::STORAGE | buffer_usage::COPY_DST;
        let rows = buf(core::mem::size_of_val(&DECORS) as u32, storage);
        let table = DECORS;
        // SAFETY: the rows are `repr(C)` words alone (pxl-spec-core decor.spec.rs), read while `table` lives.
        let words = unsafe {
            core::slice::from_raw_parts(
                table.as_ptr().cast::<u32>(),
                core::mem::size_of_val(&table) / 4,
            )
        };
        g.write_buf(rows, 0, bytes(words));
        let drawn_by = pipelines.build(g, PIPELINE_DECOR, WORLD_COLOR);
        let draw_layouts = [drawn_by.layouts[1], drawn_by.layouts[3]];
        let uniforms = buf(
            word_bytes((drawn::UNIFORM_WORDS * drawn::REGIONS) as usize),
            storage | buffer_usage::UNIFORM,
        );
        Grown {
            mark: pipelines.build_compute(g, SHADER_GROWN_MARK, &compute::DECOR_MARK_GROUP),
            stand: pipelines.build_compute(g, SHADER_GROWN_STAND, &compute::DECOR_STAND_GROUP),
            grow: pipelines.build_compute(g, SHADER_GROWN, &compute::DECOR_GROUP),
            cnt: pipelines.build_compute(g, SHADER_GROWN_CNT, &compute::DECOR_CNT_GROUP),
            args_built: pipelines.build_compute(g, SHADER_GROWN_ARGS, &compute::DECOR_ARGS_GROUP),
            place: pipelines.build_compute(g, SHADER_GROWN_PLACE, &compute::DECOR_PLACE_GROUP),
            groups: [0; 6],
            grows: buf(
                word_bytes(scatter::GROWS_WORDS as usize),
                storage | buffer_usage::INDIRECT,
            ),
            shown: buf(word_bytes(DECORS.len()), storage),
            drawing: buf(word_bytes(drawn::DRAWING_WORDS as usize), storage),
            rows,
            meshes: buf(word_bytes(GROUPS as usize), storage),
            regions: buf(word_bytes(2 * drawn::REGIONS as usize), storage),
            args: buf(
                word_bytes((drawn::ARGS_WORDS * drawn::REGIONS) as usize),
                storage | buffer_usage::INDIRECT,
            ),
            uniforms,
            records: 0,
            instances: 0,
            draw_layouts,
            uniform_group: g.bind_group(
                draw_layouts[0],
                &[buf_range_entry(0, uniforms, 0, word_bytes(DRAW_FLOATS))],
            ),
            instances_group: 0,
            room: 0,
            placed_cnt: 0,
            bound: [0; 6],
            on: false,
            shown_read: Read::default(),
            window: Read::default(),
            cuts: [0; eye::CUTS],
            placed: Read::default(),
            counted: Read::default(),
            counting: false,
            marking: false,
            held: [[0; 4]; overwrite::BOXES as usize],
            holding: 0,
            marked: 0,
            stale: true,
        }
    }

    /// The frame's decor handed over and what its growth reads (`grows`);
    /// `meshes` each group's index count, none while its model loads.
    /// Grouped again where `world` is not the one its groups bind or the
    /// records' room grew, so a frame without a cast misses no change.
    pub fn prepare(
        &mut self,
        g: &Gpu,
        f: DecorFrame,
        grows: Grows,
        meshes: &[u32; GROUPS as usize],
        world: World,
    ) {
        self.on = true;
        let need = scatter::SLOT_RECORDS + (f.placed.len() as u32).next_power_of_two().max(64);
        let grew = need > self.room;
        if grew {
            // The slots' records carried over: the placed tail is laid anew.
            let (records, instances) = (self.records, self.instances);
            self.records = g.buf(
                word_bytes(scatter::record_at(need) as usize),
                buffer_usage::STORAGE | buffer_usage::COPY_DST | buffer_usage::COPY_SRC,
            );
            self.instances = g.buf(
                word_bytes(((drawn::LANES + 1) * need) as usize),
                buffer_usage::STORAGE,
            );
            if records != 0 {
                let e = g.encoder();
                g.copy_buf(
                    e,
                    (records, 0),
                    (self.records, 0),
                    word_bytes(scatter::record_at(scatter::SLOT_RECORDS) as usize),
                );
                g.submit(e);
            }
            release(records);
            release(instances);
            release(self.instances_group);
            self.instances_group =
                g.bind_group(self.draw_layouts[1], &[buf_entry(0, self.instances)]);
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
            // What each slot grew was grown at the zooms it was shown at:
            // every slot grows again.
            g.write_buf(self.shown, 0, bytes(f.shown));
            self.stale = true;
        }
        // The boxes the slots they meet grow again in: the edits, and each
        // cut that moved as it was and as it is, held with any a frame not
        // drawn handed.
        for &b in f.edits {
            hold(&mut self.held, &mut self.holding, b);
        }
        cut_boxes(&self.cuts, grows.cuts, |b| {
            hold(&mut self.held, &mut self.holding, b)
        });
        self.cuts = *grows.cuts;
        let w = &mut self.window;
        w.begin();
        w.read(grows.window);
        let moved = !w.end();
        self.marked = self.holding;
        // A brick filled at a coarser window than the leaves' grows no decor.
        let fresh = grows
            .fresh
            .iter()
            .any(|f| f[window::FRESH_CELL as usize] == pxl_spec_core::LEAF_CELL);
        self.marking = self.stale || moved || fresh || self.marked != 0;
        if self.marking {
            // The boxes held, at every zoom, then each brick handed, at
            // its window's alone.
            let mut boxes = [0u32; (scatter::MARKED * scatter::BOX_WORDS) as usize];
            let fresh = grows.fresh.iter().map(|f| {
                let cell = f[window::FRESH_CELL as usize];
                let q = window::fresh_brick([
                    f[window::FRESH_FIRST as usize],
                    f[window::FRESH_FIRST as usize + 1],
                ]);
                (scatter::brick_box(cell, q), cell)
            });
            let held = self.held[..self.marked]
                .iter()
                .map(|&b| (b, scatter::EVERY));
            let mut n = 0;
            for (b, zoom) in held.chain(fresh).take(scatter::MARKED as usize) {
                let at = n * scatter::BOX_WORDS as usize;
                boxes[at..at + 4].copy_from_slice(&b.map(|v| v as u32));
                boxes[at + 4] = zoom;
                n += 1;
            }
            let (s, w) = (compute::DECOR_STAND_WORKGROUP, compute::DECOR_WORKGROUP);
            g.write_buf(
                self.grows,
                0,
                bytes(&[
                    BRICK_PATCHES.div_ceil(s[0]),
                    1,
                    0,
                    0,
                    (COUNTS as u32).div_ceil(w[0]),
                    BRICK_PATCHES.div_ceil(w[1]),
                    0,
                    0,
                    u32::from(self.stale),
                    n as u32,
                    0,
                    0,
                ]),
            );
            g.write_buf(
                self.grows,
                scatter::BOXES_AT * 4,
                bytes(&boxes[..n * scatter::BOX_WORDS as usize]),
            );
        }
        let p = &mut self.placed;
        p.begin();
        p.read(f.placed);
        let placed_moved = !p.end();
        self.placed_cnt = f.placed.len() as u32;
        if placed_moved {
            g.write_buf(
                self.records,
                scatter::PLACED_HEAD * 4,
                bytes(&[f.placed.len() as u32, 0, 0, 0]),
            );
            if !f.placed.is_empty() {
                // Read where the kernel holds them, one record however many.
                g.upload_buf(
                    self.records,
                    scatter::record_at(scatter::SLOT_RECORDS) * 4,
                    bytes(f.placed.as_flattened()),
                );
            }
        }
        let c = &mut self.counted;
        c.begin();
        c.read(&grows.eye[eye::ORIGIN..eye::ORIGIN + 3]);
        c.read(&[grows.eye[eye::RANGE], grows.eye[eye::RANGE + 1], self.room]);
        c.read(f.drawing);
        c.read(meshes);
        c.read(&f.translation);
        c.mark(self.marking || placed_moved || regroup);
        self.counting = !c.end();
        if !self.counting {
            return;
        }
        g.write_buf(self.drawing, 0, bytes(f.drawing));
        g.write_buf(self.meshes, 0, bytes(meshes));
        g.write_buf(self.regions, 0, bytes(&[0u32; 2 * drawn::REGIONS as usize]));
        // Each region's uniform, its index the lanes' base until the args
        // pass adds its first slot: a model's group drawn about the anchor,
        // a sprite's its group's layer too.
        let mut uniforms = [0u32; (drawn::UNIFORM_WORDS * drawn::REGIONS) as usize];
        let [x, y, z] = f.translation;
        for r in 0..drawn::REGIONS {
            let u = (r * drawn::UNIFORM_WORDS) as usize;
            let layer = if r % 2 == 1 { r / 2 } else { 0 };
            uniforms[u..u + DRAW_FLOATS].copy_from_slice(&[
                x.to_bits(),
                y.to_bits(),
                z.to_bits(),
                0,
                self.room,
                1.0f32.to_bits(),
                layer,
                drawn::LANES * self.room,
            ]);
        }
        g.write_buf(self.uniforms, 0, bytes(&uniforms));
    }

    fn regroup(&mut self, g: &Gpu, world: World) {
        let group = |built: &Built, ids: &[u32]| {
            let mut entries = [buf_entry(0, 0); 9];
            for (k, &b) in ids.iter().enumerate() {
                entries[k] = buf_entry(k as u32, b);
            }

            g.bind_group(built.layouts[0], &entries[..ids.len()])
        };
        for id in self.groups {
            release(id);
        }
        let [eye, window, ground, shards, overwrites, pool] = world;
        self.groups = [
            group(
                &self.mark,
                &[window, self.records, self.grows, self.rows, self.shown],
            ),
            group(&self.stand, &[window, self.records, self.rows, self.grows]),
            group(
                &self.grow,
                &[
                    eye,
                    window,
                    ground,
                    shards,
                    overwrites,
                    self.records,
                    self.rows,
                    self.grows,
                    pool,
                ],
            ),
            group(
                &self.cnt,
                &[
                    eye,
                    self.drawing,
                    self.rows,
                    self.records,
                    self.regions,
                    window,
                ],
            ),
            group(
                &self.args_built,
                &[self.meshes, self.regions, self.args, self.uniforms],
            ),
            group(
                &self.place,
                &[
                    self.drawing,
                    self.rows,
                    self.records,
                    self.regions,
                    self.instances,
                    window,
                ],
            ),
        ];
    }

    /// No decor this frame: none grown, none drawn, and the next grows
    /// every slot (`stale`).
    pub fn off(&mut self) {
        self.on = false;
        self.stale = true;
        self.counted.forget();
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
    pub fn on_dropped(&mut self) {
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
    pub fn carry_edits(&mut self, edits: &[[i32; 4]]) {
        for &b in edits {
            hold(&mut self.held, &mut self.holding, b);
        }
    }

    /// Within a compute pass over the cast's world, after the cast: the
    /// slots to grow again marked, their candidates stood a patch a thread
    /// and grown a record a thread where any may be, then every
    /// record counted, laid out and placed.
    pub fn dispatch(&self, g: &Gpu, pass: u32) {
        if !self.on || self.groups[0] == 0 {
            return;
        }
        if self.marking {
            g.set_pipeline(pass, self.mark.pipeline);
            g.set_bind_group(pass, 0, self.groups[0], &[]);
            g.dispatch(
                pass,
                scatter::EYE_ENTRIES.div_ceil(compute::DECOR_MARK_WORKGROUP[0]),
                1,
                1,
            );
            let stood = [
                (&self.stand, scatter::STAND_ARGS),
                (&self.grow, scatter::GROW_ARGS),
            ];
            for (k, (built, args)) in stood.into_iter().enumerate() {
                g.set_pipeline(pass, built.pipeline);
                g.set_bind_group(pass, 0, self.groups[1 + k], &[]);
                g.dispatch_indirect(pass, self.grows, args * 4);
            }
        }
        if !self.counting {
            return;
        }
        // A workgroup a slot, reading the records its patches bear, then
        // placed decor's, `scatter::RUNS` a workgroup (pxl-shader
        // `records`): the records held, not the room.
        let records = scatter::EYE_ENTRIES + self.placed_cnt.div_ceil(scatter::RUNS);
        let passes = [
            (&self.cnt, records),
            (&self.args_built, 1),
            (&self.place, records),
        ];
        for (k, (built, x)) in passes.into_iter().enumerate() {
            g.set_pipeline(pass, built.pipeline);
            g.set_bind_group(pass, 0, self.groups[3 + k], &[]);
            g.dispatch(pass, x, 1, 1);
        }
    }

    /// Whether decor is placed again this frame, so stands elsewhere.
    pub fn counts(&self) -> bool {
        self.on && self.counting
    }

    /// The decor placed, while decor is on (`Instances`).
    pub fn placed(&self) -> Option<Instances> {
        (self.on && self.instances != 0).then_some(Instances {
            drawing: self.drawing,
            regions: self.regions,
            instances: self.instances,
            room: self.room,
        })
    }

    /// What the draws read, while decor is on: the decor's instances, their slots,
    /// the regions' indirect args, and where they are drawn.
    pub fn drawn(&self) -> Option<Drawn> {
        (self.on && self.instances != 0).then_some(Drawn {
            instances_group: self.instances_group,
            args: self.args,
            uniform_group: self.uniform_group,
        })
    }
}

/// The decor placed as a reader beside the draws reads it (pxl-shader
/// decor.rs `Instances`, read by sun.rs `sun_mark_drawn_cs`): its drawing
/// words, its regions and its instances, and the slots they hold.
#[derive(Clone, Copy)]
pub struct Instances {
    pub drawing: u32,
    pub regions: u32,
    pub instances: u32,
    pub room: u32,
}

/// The decor's instances as the draws read them (`Grown::drawn`): their
/// group, the regions' indirect args and their draw uniforms' group.
#[derive(Clone, Copy)]
pub struct Drawn {
    pub instances_group: u32,
    pub args: u32,
    pub uniform_group: u32,
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

/// Box `b` held among the `holding` boxes of `held`, past the room joined
/// into the last: what decor's growth (`Grown`) and the observed pyramid's
/// forgetting (ray/cast.rs) are asked over.
pub(crate) fn hold(held: &mut [[i32; 4]], holding: &mut usize, b: [i32; 4]) {
    if *holding < held.len() {
        held[*holding] = b;
        *holding += 1;
    } else {
        let last = &mut held[*holding - 1];
        *last = [
            last[0].min(b[0]),
            last[1].min(b[1]),
            last[2].max(b[2]),
            last[3].max(b[3]),
        ];
    }
}

/// The boxes a preview's cuts changed, `was` against `is`: each cut that
/// moved as it was and as it is (pxl-spec-core `overwrite::cut_box`).
pub(crate) fn cut_boxes(
    was: &[u32; eye::CUTS],
    is: &[u32; eye::CUTS],
    mut each: impl FnMut([i32; 4]),
) {
    for k in [0, eye::CUT_WORDS] {
        let (was, is) = (&was[k..k + eye::CUT_WORDS], &is[k..k + eye::CUT_WORDS]);
        if was != is {
            for c in [was, is].into_iter().filter(|c| c[3] != eye::NO_CUT) {
                each(overwrite::cut_box(
                    c[3],
                    [c[0] as i32, c[1] as i32, c[2] as i32],
                ));
            }
        }
    }
}
