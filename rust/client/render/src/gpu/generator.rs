// pxl-render/gpu/generator.rs

//! The decor the GPU generates and draws (pxl-content `scatter`): what the
//! ground generates in the eye's clip levels over the cast's world
//! (`decor_cs`), placed decor handed as records beside it — a candidate
//! generated again only where what it reads changed (`decor_mark_cs`,
//! `Generation`), its record standing otherwise, and a brick thinned again only
//! where it is another; then each frame each record taken where the rays ask
//! the leaf cell's zoom or finer and sized for the screen, counted a draw
//! region (`decor_cnt_cs`), the regions laid out with their indirect args
//! (`decor_args_cs`), and each record drawn compacted into the decor's
//! instances in the layout the decor and impostor shaders read
//! (`decor_compact_cs`). The CPU hands what only it knows and never reads what
//! is generated.

use crate::frame::shader::access;
use crate::frame::shader::sequence;
use pxl_content::decor::DECORS;
use pxl_content::generation::{self, drawn, GROUPS};
use pxl_engine::{clipmap, eye, object, Zoom};
use pxl_spec::space::coord::q3::Q3;

use crate::frame::shader::group::{
    ARGS_GROUP, CNT_GROUP, COMPACT_GROUP, CULLED_LIGHT_ARGS, CULLED_SCREEN_ARGS, GENERATE_GROUP,
    GENERATION_ARGS, GENERATION_ARGS_OF_BINDS, GENERATION_BINDS, GENERATION_LIST,
    GENERATION_MARK_BINDS, GENERATION_THIN_BINDS, MARK_GROUP, MASK_GROUP, MASK_LIGHT_BINDS,
    MASK_SCREEN_BINDS, THIN_GROUP,
};
use crate::frame::shader::record::PIPELINE_DECOR;
use crate::frame::shader::record::{
    COMPUTE_DECOR_CULL, COMPUTE_DECOR_CULL_ARGS, COMPUTE_GENERATION, COMPUTE_GENERATION_ARGS,
    COMPUTE_GENERATION_CNT, COMPUTE_GENERATION_COMPACT, COMPUTE_GENERATION_MARK,
    COMPUTE_GENERATION_THIN,
};
use crate::frame::shader::word::{instance, DRAW_FLOATS};
use crate::gpu::api::{
    buf_entry, buf_range_entry, buffer_usage, bytes, release, word_bytes, Entry, Gpu, Layout,
    RenderContext,
};
use crate::gpu::pipeline::{Pipeline, Pipelines};
use crate::gpu::read::Read;
use crate::gpu::tiles::Tiles;

/// What the CPU hands the GPU's decor this frame.
pub struct DecorFrame<'a> {
    /// Per kind, the clip level zooms it is generated at, a bit each over the
    /// first clip level's (`decor_cs`).
    pub zooms: &'a [u32],
    /// pxl-content `generation::drawn`'s drawing words, `DRAWING_WORDS` of them.
    pub drawing: &'a [u32],
    /// Placed decor, a record each as `decor_cs` writes one: its kind,
    /// variant and turn, then its box's min corner in leaf cells, its base
    /// sunk under it by none, as it stands where it is placed; handed where
    /// they lie, their writer asking `held` (gpu/api.rs) before it writes
    /// them again.
    pub placed_records: &'a [[u32; generation::RECORD_WORDS as usize]],
    /// The anchor the lanes are about, the unit the eye stands in, as a
    /// draw's Q3 (gpu/draw.rs `Position`): where the decor's instances are
    /// drawn, against the frame's eye (word.rs `frame::EYE`).
    pub anchor: Q3,
    /// What overwrites wrote since the last frame, `object::EDITS` boxes
    /// of leaf cells at most (pxl-content `generation::meets`): the slots
    /// whose decor they reach generate again.
    pub edits: &'a [[i32; 4]],
}

/// What decor's generation reads besides the frame's decor, none of it the
/// eye's place: the clip levels' table, a preview's cuts, and the bricks filled
/// this frame. A slot generates whole where its brick is another; else it
/// generates again only the records a box reaches, each by what its own
/// candidate reads (`generation::reaches`): an edit, a cut as it was or is, or a
/// brick filled at its clip level's zoom or finer. The overwrites' every write
/// names its box (the frame's `edits`), and the service area's index is made
/// once, before any brick is held, so neither generates a slot by itself. A
/// brick filled reaches its own clip level's zoom and every coarser one: a
/// coarser clip level's decor faces as the leaf cells vote, each read off the
/// finest clip level holding it (pxl-content `generation::asks`).
pub struct Generation<'a> {
    pub clipmap: &'a [i32],
    pub cuts: &'a [u32; eye::CUTS],
    /// The bricks handed this frame, each its cells' zoom and the brick.
    pub fresh: &'a [(Zoom, [i32; 2])],
    /// The eye as the cast's (pxl-engine `eye`), of which the count pass
    /// reads its place and the zooms it asks (`ORIGIN`, `RANGE`).
    pub eye: &'a [u32; eye::WORDS],
}

pub struct Generator {
    mark: Pipeline,
    thin: Pipeline,
    generate: Pipeline,
    cnt: Pipeline,
    args_pipeline: Pipeline,
    compact: Pipeline,
    groups: [u32; 6],
    /// What `decor_mark_cs` reads and lists for `decor_cs`'s dispatch
    /// (`generation::LIST_AT`).
    list: u32,
    zooms: u32,
    drawing: u32,
    rows: u32,
    meshes: u32,
    regions: u32,
    args: u32,
    /// Each region's draw uniform (`drawn::UNIFORM_WORDS` apart), as the
    /// decor and impostor shaders read one, its index set by the args pass.
    uniforms: u32,
    /// The records, laid by slot (`generation::record_at`); then the decor's
    /// instances, `drawn::COLUMNS` lanes and the index list, `cap` slots each.
    records: u32,
    instances: u32,
    /// The draws' bind groups over the uniforms, a region's at a dynamic
    /// offset, and over the instances, in the layouts the decor and impostor
    /// draws share (record.rs `DRAW`, `UPLOAD`): made with their buffers.
    draw_layouts: [Layout; 2],
    uniform_group: u32,
    instances_group: u32,
    /// What a frame through a gate draws in their place (`mask`), each gate's its own
    /// (`Mask`), and the passes that make it.
    culled: [Culled; 2],
    cull_args: Pipeline,
    mask: Pipeline,
    cap: u32,
    /// The placed decor's records, as last handed.
    placed_cnt: u32,
    /// The world its groups bind: grouped again exactly when it is another.
    bindings: World,
    on: bool,
    /// The kinds' zooms as last written, a constant of the table the core
    /// hands once; the clip levels' table, the cuts and the placed decor as
    /// last handed; and whether this frame marks slots to generate.
    zooms_read: Read,
    clipmap: Read,
    cuts: [u32; eye::CUTS],
    placed: Read,
    marking: bool,
    /// The boxes the slots they meet generate again in, held until a frame
    /// that marks by them is drawn: one dropped, or handed and never drawn,
    /// hands them again, as generating a slot twice generates it the same.
    /// Past `object::RECTS`, the rest joined into the last.
    pending_rects: [[i32; 4]; object::RECTS as usize],
    holding: usize,
    /// How many of `pending_rects` this frame marks by, and how many boxes it marks
    /// by in all, the bricks handed with them (`marked_rects`).
    marked: usize,
    rects: u32,
    /// Whether the slots' records were not kept, decor having been off: the
    /// next frame drawn with decor generates every slot.
    stale: bool,
    /// What the count, args and compact passes read past the records, as they
    /// last ran: the eye's place and zooms, the drawing words, the meshes,
    /// the anchor and the records' capacity; and whether this frame runs them,
    /// which it does where that moved or the records did.
    count_read: Read,
    counting: bool,
}

/// The world `decor_cs` asks: the cast's eye, clip level, the tiles' frames,
/// the service area's index, the overwrites and the ways' pool, as the cast
/// asks it.
pub type World = [u32; 6];

impl Generator {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines) -> Generator {
        let buf = |size: u32, usage: u32| g.buf(size, usage);
        let storage = buffer_usage::STORAGE | buffer_usage::COPY_DST;
        let rows = buf(size_of_val(&DECORS) as u32, storage);
        let table = DECORS;
        // SAFETY: the rows are `repr(C)` words alone (pxl-content decor.spec.rs), read while `table` lives.
        let words = unsafe {
            core::slice::from_raw_parts(table.as_ptr().cast::<u32>(), size_of_val(&table) / 4)
        };
        g.write_buf(rows, 0, bytes(words));
        let decor_pipeline = pipelines.build_world(g, PIPELINE_DECOR);
        let draw_layouts = [decor_pipeline.layouts[1], decor_pipeline.layouts[3]];
        let uniforms = buf(
            word_bytes((drawn::UNIFORM_WORDS * drawn::REGIONS) as usize),
            storage | buffer_usage::UNIFORM,
        );
        Generator {
            mark: pipelines.build_compute(g, COMPUTE_GENERATION_MARK, &MARK_GROUP),
            thin: pipelines.build_compute(g, COMPUTE_GENERATION_THIN, &THIN_GROUP),
            generate: pipelines.build_compute(g, COMPUTE_GENERATION, &GENERATE_GROUP),
            cnt: pipelines.build_compute(g, COMPUTE_GENERATION_CNT, &CNT_GROUP),
            args_pipeline: pipelines.build_compute(g, COMPUTE_GENERATION_ARGS, &ARGS_GROUP),
            compact: pipelines.build_compute(g, COMPUTE_GENERATION_COMPACT, &COMPACT_GROUP),
            groups: [0; 6],
            list: buf(
                word_bytes(generation::LIST_WORDS as usize),
                storage | buffer_usage::INDIRECT,
            ),
            zooms: buf(word_bytes(DECORS.len()), storage),
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
            culled: [Mask::Screen, Mask::Light].map(|_| Culled {
                list: 0,
                args: buf(
                    word_bytes((drawn::ARGS_WORDS * drawn::REGIONS) as usize),
                    storage | buffer_usage::INDIRECT,
                ),
                group: 0,
            }),
            cull_args: pipelines.build_compute(g, COMPUTE_DECOR_CULL_ARGS, &MASK_GROUP),
            mask: pipelines.build_compute(g, COMPUTE_DECOR_CULL, &MASK_GROUP),
            cap: 0,
            placed_cnt: 0,
            bindings: [0; 6],
            on: false,
            zooms_read: Read::default(),
            clipmap: Read::default(),
            cuts: [0; eye::CUTS],
            placed: Read::default(),
            count_read: Read::default(),
            counting: false,
            marking: false,
            pending_rects: [[0; 4]; object::RECTS as usize],
            holding: 0,
            marked: 0,
            rects: 0,
            stale: true,
        }
    }

    /// The frame's decor handed over and what its generation reads (`generation`);
    /// `meshes` each group's index count, none while its model loads.
    /// Grouped again where `world` is not the one its groups bind or the
    /// records' capacity grew, so a frame without a cast misses no change.
    pub fn prepare(
        &mut self,
        g: &Gpu,
        f: DecorFrame,
        generation: Generation,
        meshes: &[u32; GROUPS as usize],
        world: World,
    ) {
        self.on = true;
        let need =
            generation::SLOT_RECORDS + (f.placed_records.len() as u32).next_power_of_two().max(64);
        let grew = need > self.cap;
        if grew {
            // The slots' records carried over: the placed tail is laid anew.
            let (records, instances) = (self.records, self.instances);
            self.records = g.buf(
                word_bytes(generation::record_at(need) as usize),
                buffer_usage::STORAGE | buffer_usage::COPY_DST | buffer_usage::COPY_SRC,
            );
            self.instances = g.buf(
                word_bytes(((drawn::COLUMNS + 1) * need) as usize),
                buffer_usage::STORAGE,
            );
            if records != 0 {
                let e = g.encoder();
                g.copy_buf(
                    e,
                    (records, 0),
                    (self.records, 0),
                    word_bytes(generation::record_at(generation::SLOT_RECORDS) as usize),
                );
                g.submit(e);
            }
            release(records);
            release(instances);
            release(self.instances_group);
            self.instances_group = g.bind_group(
                self.draw_layouts[1],
                &[buf_entry(0, self.instances), buf_entry(1, self.instances)],
            );
            for culled in &mut self.culled {
                release(culled.group);
                release(culled.list);
                culled.list = g.buf(
                    word_bytes(((drawn::COLUMNS + 1) * need) as usize),
                    buffer_usage::STORAGE,
                );
                culled.group = g.bind_group(
                    self.draw_layouts[1],
                    &[buf_entry(0, self.instances), buf_entry(1, culled.list)],
                );
            }
            self.cap = need;
            self.placed.forget();
        }
        let regroup = grew || world != self.bindings;
        if regroup {
            self.regroup(g, world);
            self.bindings = world;
        }
        let s = &mut self.zooms_read;
        s.begin();
        s.read(f.zooms);
        if !s.end() {
            // What each slot generated was generated at the zooms it was
            // shown at: every slot generates again.
            g.write_buf(self.zooms, 0, bytes(f.zooms));
            self.stale = true;
        }
        // The boxes the slots they meet generate again in: the edits, and each
        // cut that moved as it was and as it is, held with any a frame not
        // drawn handed.
        for &b in f.edits {
            hold(&mut self.pending_rects, &mut self.holding, b);
        }
        cut_rects(&self.cuts, generation.cuts, |b| {
            hold(&mut self.pending_rects, &mut self.holding, b);
        });
        self.cuts = *generation.cuts;
        let w = &mut self.clipmap;
        w.begin();
        w.read(generation.clipmap);
        let changed = !w.end();
        self.marked = self.holding;
        // A brick filled at any clip level generates decor there and coarser.
        let fresh = !generation.fresh.is_empty();
        self.marking = self.stale || changed || fresh || self.marked != 0;
        self.rects = 0;
        if self.marking {
            // The boxes held, at every zoom, then each brick handed, at
            // its clip level's and coarser.
            let mut rects = [0u32; (generation::RECT_CAP * generation::RECT_WORDS) as usize];
            let fresh = generation
                .fresh
                .iter()
                .map(|&(cell_zoom, q)| (generation::brick_rect(cell_zoom, q), cell_zoom));
            let pending_rects = self.pending_rects[..self.marked]
                .iter()
                .map(|&b| (b, generation::EVERY));
            let mut n = 0;
            for (b, zoom) in pending_rects
                .chain(fresh)
                .take(generation::RECT_CAP as usize)
            {
                let at = n * generation::RECT_WORDS as usize;
                rects[at..at + 4].copy_from_slice(&b.map(|v| v as u32));
                rects[at + 4] = zoom;
                n += 1;
            }
            self.rects = n as u32;
            // The list's head (pxl-content `generation::THIN_ARGS`): each
            // pass's workgroups across and up, the pairs listed 0 until
            // `decor_mark_cs` lists them; whether every entry generates
            // whole, the boxes, and no pair yet in part.
            let mut header = [0u32; generation::RECTS_AT as usize];
            let mut put = |at: u32, val: u32| header[at as usize] = val;
            put(
                generation::THIN_ARGS,
                clipmap::BRICK_CELLS.div_ceil(generation::LANES),
            );
            put(generation::THIN_ARGS + 1, 1);
            put(
                generation::GENERATE_ARGS,
                generation::KIND_RECORDS.div_ceil(generation::LANES),
            );
            put(generation::GENERATE_ARGS + 1, 1);
            put(generation::LIST_EVERY, u32::from(self.stale));
            put(generation::LIST_RECTS, n as u32);
            put(generation::LIST_PARTS, 0);
            g.write_buf(
                self.list,
                sequence::OPEN_GENERATION_LIST.to_at(),
                bytes(&header),
            );
            g.write_buf(
                self.list,
                sequence::OPEN_GENERATION_RECTS.to_at(),
                bytes(&rects[..n * generation::RECT_WORDS as usize]),
            );
        }
        let p = &mut self.placed;
        p.begin();
        p.read(f.placed_records);
        let placed_changed = !p.end();
        self.placed_cnt = f.placed_records.len() as u32;
        if placed_changed {
            g.write_buf(
                self.records,
                generation::PLACED_HEADER * 4,
                bytes(&[f.placed_records.len() as u32, 0, 0, 0]),
            );
            if !f.placed_records.is_empty() {
                // One record however many.
                g.upload_buf(
                    self.records,
                    generation::record_at(generation::SLOT_RECORDS) * 4,
                    bytes(f.placed_records.as_flattened()),
                );
            }
        }
        let c = &mut self.count_read;
        c.begin();
        c.read(&generation.eye[eye::ORIGIN..eye::ORIGIN + 3]);
        c.read(&[
            generation.eye[eye::RANGE],
            generation.eye[eye::RANGE + 1],
            self.cap,
        ]);
        c.read(f.drawing);
        c.read(meshes);
        c.read(&f.anchor);
        c.mark(self.marking || placed_changed || regroup);
        self.counting = !c.end();
        if !self.counting {
            return;
        }
        g.write_buf(self.drawing, 0, bytes(f.drawing));
        g.write_buf(self.meshes, 0, bytes(meshes));
        g.write_buf(self.regions, 0, bytes(&[0u32; 2 * drawn::REGIONS as usize]));
        // Each region's uniform, its index the lanes' base until the args
        // pass adds its first slot: a model's group drawn about the anchor,
        // on Q3 and so with no remainder, an impostor's its group's layer too.
        let mut uniforms = [0u32; (drawn::UNIFORM_WORDS * drawn::REGIONS) as usize];
        for r in 0..drawn::REGIONS {
            let u = &mut uniforms[(r * drawn::UNIFORM_WORDS) as usize..][..DRAW_FLOATS];
            u[instance::Q..instance::Q + 3].copy_from_slice(&f.anchor.map(|v| v as u32));
            u[instance::STRIDE] = self.cap;
            u[instance::OPACITY] = 1.0f32.to_bits();
            u[instance::LAYER] = if r % 2 == 1 { r / 2 } else { 0 };
            u[instance::IDX] = drawn::COLUMNS * self.cap;
        }
        g.write_buf(self.uniforms, 0, bytes(&uniforms));
    }

    /// The frame's buffer `buf` (frame/shader/group.rs `BUFFERS`) the generator holds, 0 for
    /// another's.
    fn buf_of(&self, buf: u32) -> u32 {
        match buf {
            GENERATION_LIST => self.list,
            GENERATION_ARGS => self.args,
            CULLED_SCREEN_ARGS => self.culled[Mask::Screen as usize].args,
            CULLED_LIGHT_ARGS => self.culled[Mask::Light as usize].args,
            _ => 0,
        }
    }

    fn regroup(&mut self, g: &Gpu, world: World) {
        let entries = |ids: &[(u32, u32)]| -> Vec<Entry> {
            ids.iter()
                .map(|&(binding, b)| buf_entry(binding, b))
                .collect()
        };
        let group = |pipeline: &Pipeline, ids: &[(u32, u32)]| {
            g.bind_group(pipeline.layouts[0], &entries(ids))
        };
        let framed = |pipeline: &Pipeline, table: &access::Group, ids: &[(u32, u32)]| {
            g.frame_group(
                pipeline.layouts[0],
                table,
                |b| self.buf_of(b),
                &entries(ids),
            )
        };
        for id in self.groups {
            release(id);
        }
        let [eye, clipmap, ground, shards, objects, pool] = world;
        let groups = [
            framed(
                &self.mark,
                &GENERATION_MARK_BINDS,
                &[(1, clipmap), (9, self.records), (12, self.zooms)],
            ),
            framed(
                &self.thin,
                &GENERATION_THIN_BINDS,
                &[(9, self.records), (10, self.rows)],
            ),
            framed(
                &self.generate,
                &GENERATION_BINDS,
                &[
                    (0, eye),
                    (1, clipmap),
                    (2, ground),
                    (5, shards),
                    (6, objects),
                    (7, pool),
                    (9, self.records),
                    (10, self.rows),
                ],
            ),
            group(
                &self.cnt,
                &[
                    (0, eye),
                    (1, clipmap),
                    (9, self.records),
                    (13, self.drawing),
                    (14, self.regions),
                ],
            ),
            framed(
                &self.args_pipeline,
                &GENERATION_ARGS_OF_BINDS,
                &[
                    (0, self.meshes),
                    (1, self.regions),
                    (3, self.uniforms),
                    (15, self.instances),
                ],
            ),
            group(
                &self.compact,
                &[
                    (1, clipmap),
                    (9, self.records),
                    (10, self.rows),
                    (13, self.drawing),
                    (14, self.regions),
                    (15, self.instances),
                ],
            ),
        ];
        self.groups = groups;
    }

    /// No decor this frame: none generated, none drawn, and the next generates
    /// every slot (`stale`).
    pub fn off(&mut self) {
        self.on = false;
        self.rects = 0;
        self.stale = true;
        self.count_read.forget();
    }

    /// The frame was drawn: the boxes it marked by and its every-slot
    /// generation are done with.
    pub fn on_drawn(&mut self) {
        if self.marking {
            self.pending_rects.copy_within(self.marked..self.holding, 0);
            self.holding -= self.marked;
            self.stale = false;
        }
        self.marked = 0;
    }

    /// The frame was dropped, nothing of it recorded: what it wrote is
    /// written again, the slots it marked marked again by the boxes still
    /// held, the clip level's table and the placed decor read anew.
    pub fn on_dropped(&mut self) {
        self.marked = 0;
        self.clipmap.forget();
        self.placed.forget();
        self.zooms_read.forget();
        self.count_read.forget();
    }

    /// Whether it marks or counts this frame: its passes run, and the frame
    /// draws what they leave.
    pub fn works(&self) -> bool {
        self.on && self.groups[0] != 0 && (self.marking || self.counting)
    }

    /// A frame's decor handed and never drawn: its edits held for the next.
    pub fn carry_edits(&mut self, edits: &[[i32; 4]]) {
        for &b in edits {
            hold(&mut self.pending_rects, &mut self.holding, b);
        }
    }

    /// Within a compute pass over the cast's world, after the cast: the slots
    /// to generate again marked, their candidates thinned a clip level cell a
    /// thread and generated a record a thread where any may be, then every
    /// record counted, laid out and compacted.
    pub fn dispatch(&self, g: &Gpu, pass: u32) {
        if !self.on || self.groups[0] == 0 {
            return;
        }
        if self.marking {
            g.set_pipeline(pass, self.mark.id);
            g.set_bind_group(pass, 0, self.groups[0], &[]);
            g.dispatch(
                pass,
                generation::EYE_ENTRIES.div_ceil(generation::LANES),
                1,
                1,
            );
            let list_of = [
                (&self.thin, self.groups[1], &sequence::THIN),
                (&self.generate, self.groups[2], &sequence::GENERATE),
            ];
            for (pipeline, group, step) in list_of {
                g.set_pipeline(pass, pipeline.id);
                g.set_bind_group(pass, 0, group, &[]);
                g.dispatch_indirect(pass, self.list, step.at());
            }
        }
        if !self.counting {
            return;
        }
        // A workgroup a slot, reading the records its decor laid, then
        // placed decor's, `generation::RUNS` a workgroup (decor_run.wgsl
        // `run_of`): the records held, not the capacity.
        let records = generation::EYE_ENTRIES + self.placed_cnt.div_ceil(generation::RUNS);
        let passes = [
            (&self.cnt, records),
            (&self.args_pipeline, 1),
            (&self.compact, records),
        ];
        for (k, (pipeline, x)) in passes.into_iter().enumerate() {
            g.set_pipeline(pass, pipeline.id);
            g.set_bind_group(pass, 0, self.groups[3 + k], &[]);
            g.dispatch(pass, x, 1, 1);
        }
    }

    /// The boxes this frame marks slots by to generate again: the edits and
    /// cuts held and the bricks handed, `generation::RECT_CAP` at most; 0
    /// where it marks none.
    pub fn marked_rects(&self) -> u32 {
        self.rects
    }

    /// Whether decor is compacted again this frame, so stands elsewhere.
    pub fn counts(&self) -> bool {
        self.on && self.counting
    }

    /// The decor compacted, while decor is on (`Instances`).
    pub fn instances(&self) -> Option<Instances> {
        (self.on && self.instances != 0).then_some(Instances {
            drawing: self.drawing,
            regions: self.regions,
            instances: self.instances,
            cap: self.cap,
        })
    }

    /// What the draws read, while decor is on: the decor's instances, their slots,
    /// the regions' indirect args, and where they are drawn.
    pub fn indirect_draw(&self) -> Option<IndirectDraw> {
        (self.on && self.instances != 0).then_some(IndirectDraw {
            instances_group: self.instances_group,
            args: self.args,
            uniform_group: self.uniform_group,
            culled: self.culled.map(|culled| (culled.group, culled.args)),
            mask: None,
        })
    }

    /// The instances a frame draws through `mask`, those whose ball meets the box of the
    /// claimed tiles of `tiles` (damage_rect.wgsl, decor_cull.wgsl) as the frame's words
    /// `frame` see it, once the tiles' marks are made: timed with the marks.
    pub fn mask(&self, ctx: &mut RenderContext, mask: Mask, frame: u32, tiles: &Tiles) {
        if !(self.on && self.instances != 0) {
            return;
        }
        let damage_rect = tiles.dispatch_damage_rect(ctx);
        let culled = &self.culled[mask as usize];
        let g = ctx.g;
        let table = match mask {
            Mask::Screen => &MASK_SCREEN_BINDS,
            Mask::Light => &MASK_LIGHT_BINDS,
        };
        let group = g.frame_group(
            self.mask.layouts[0],
            table,
            |b| self.buf_of(b),
            &[
                buf_entry(0, frame),
                buf_entry(1, self.uniforms),
                buf_entry(3, self.instances),
                buf_entry(4, culled.list),
                buf_entry(6, damage_rect),
                buf_entry(7, self.regions),
                buf_entry(31, tiles.this_frame().1),
            ],
        );
        let pass = g.compute_pass(ctx.encoder, ctx.stamp(tiles.span()).pass());
        g.set_bind_group(pass, 0, group, &[]);
        g.set_pipeline(pass, self.cull_args.id);
        g.dispatch(pass, drawn::REGIONS.div_ceil(64), 1, 1);
        g.set_pipeline(pass, self.mask.id);
        g.dispatch(pass, self.cap.div_ceil(64), 1, 1);
        g.end(pass);
        release(group);
    }
}

/// The gate a frame draws decor through (`Generator::mask`): the screen's claimed tiles, or
/// the light map's. Both are drawn in one frame, so each keeps its own list.
#[derive(Clone, Copy)]
pub enum Mask {
    Screen,
    Light,
}

/// What a frame draws through a gate in place of the instances: the instances that meet its
/// claimed tiles, listed at each region's place, the regions' args with them, and the draws'
/// group over that list.
#[derive(Clone, Copy)]
struct Culled {
    list: u32,
    args: u32,
    group: u32,
}

/// The decor compacted as a reader beside the draws reads it
/// (light_mark_draws.wgsl): its drawing words, its regions and its instances,
/// and the slots they hold.
#[derive(Clone, Copy)]
pub struct Instances {
    pub drawing: u32,
    pub regions: u32,
    pub instances: u32,
    pub cap: u32,
}

/// The decor's instances as the draws read them (`Generator::drawn`): their
/// group, the regions' indirect args and their draw uniforms' group.
#[derive(Clone, Copy)]
pub struct IndirectDraw {
    pub instances_group: u32,
    pub args: u32,
    pub uniform_group: u32,
    /// The group and the args a frame draws by through each gate (`Generator::mask`).
    pub culled: [(u32, u32); 2],
    /// The gate it is drawn through, none where it is drawn whole.
    pub mask: Option<Mask>,
}

impl IndirectDraw {
    /// The draw a frame makes through `mask`: its list and args in place of the instances'
    /// own (`Generator::mask`).
    pub fn through_mask(self, mask: Mask) -> IndirectDraw {
        let (instances_group, args) = self.culled[mask as usize];
        IndirectDraw {
            instances_group,
            args,
            mask: Some(mask),
            ..self
        }
    }

    /// Where region `region`'s indirect args lie, in bytes.
    pub fn args_at(&self, region: u32) -> u32 {
        drawn::ARGS_WORDS * 4 * region
    }

    /// Where region `region`'s draw uniform lies, a dynamic offset in bytes.
    pub fn uniform_at(&self, region: u32) -> u32 {
        drawn::UNIFORM_WORDS * 4 * region
    }
}

/// Box `b` held among the `holding` boxes of `pending_rects`, past the room joined
/// into the last: what decor's generation (`Generator`) and the node cache's
/// forgetting (ray/cast.rs) are asked over.
pub(crate) fn hold(pending_rects: &mut [[i32; 4]], holding: &mut usize, b: [i32; 4]) {
    if *holding < pending_rects.len() {
        pending_rects[*holding] = b;
        *holding += 1;
    } else {
        let last = &mut pending_rects[*holding - 1];
        *last = [
            last[0].min(b[0]),
            last[1].min(b[1]),
            last[2].max(b[2]),
            last[3].max(b[3]),
        ];
    }
}

/// The boxes a preview's cuts changed, `was` against `is`: each cut that
/// moved as it was and as it is (pxl-engine `object::cut_rect`).
pub(crate) fn cut_rects(
    was: &[u32; eye::CUTS],
    is: &[u32; eye::CUTS],
    mut each: impl FnMut([i32; 4]),
) {
    for k in [0, eye::CUT_WORDS] {
        let (was, is) = (&was[k..k + eye::CUT_WORDS], &is[k..k + eye::CUT_WORDS]);
        if was != is {
            for c in [was, is].into_iter().filter(|c| c[3] != eye::NO_CUT) {
                each(object::cut_rect(
                    c[3],
                    [c[0] as i32, c[1] as i32, c[2] as i32],
                ));
            }
        }
    }
}
