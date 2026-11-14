// pxl-render/gpu/generator.rs

//! The decor the GPU generates and draws (pxl-content `scatter`). The GPU generates the decor
//! the ground holds in the eye's clip levels over the cast's world (`decor_pose_cs`), and the CPU
//! hands placed decor beside it as records. A candidate is generated again only where what it
//! reads changed (`decor_mark_cs`, `Generation`), and its record stands otherwise. A brick's
//! candidates are thinned again only where the brick is another. Then the count pass sizes for
//! the screen each record the rays ask for and counts it into a draw region (`decor_cnt_cs`),
//! the args pass lays the regions out with their indirect args (`decor_args_cs`), and the
//! compact pass writes each drawn record into the decor's instances, in the layout the decor
//! and impostor shaders read (`decor_compact_cs`). These three run again only where what they
//! read moved. The CPU hands what only it knows and never reads back what is generated.

use crate::frame::shader::access;
use crate::frame::shader::sequence;
use pxl_content::generation::{self, drawing, GROUP_CNT};
use pxl_engine::{eye, object};
use pxl_spec::asset::decor::ROWS;
use pxl_spec::brick;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::zoom::Zoom;

use crate::frame::shader::group::{
    CULLED_LIGHT_ARGS, CULLED_SCREEN_ARGS, CULL_LIGHT_BINDS, CULL_SCREEN_BINDS, GENERATION_ARGS,
    GENERATION_ARGS_OF_BINDS, GENERATION_BINDS, GENERATION_LIST, GENERATION_MARK_BINDS,
    GENERATION_THIN_BINDS,
};
use crate::frame::shader::record::PIPELINE_DECOR;
use crate::frame::shader::record::{
    COMPUTE_DECOR_CULL, COMPUTE_DECOR_CULL_ARGS, COMPUTE_GENERATION, COMPUTE_GENERATION_ARGS,
    COMPUTE_GENERATION_CNT, COMPUTE_GENERATION_COMPACT, COMPUTE_GENERATION_MARK,
    COMPUTE_GENERATION_THIN,
};
use crate::frame::shader::word::{instance, DRAW_FLOATS};
use crate::gpu::api::{
    buf_entry, buf_range_entry, buffer_usage, bytes, frame_entries, release, word_bytes, Entry,
    Gpu, Group, Layout, RenderContext,
};
use crate::gpu::pipeline::{Pipeline, PipelineCache};
use crate::gpu::read::Read;
use crate::gpu::tile::TileGrid;

/// What the CPU hands the GPU's decor this frame.
pub struct DecorFrame<'a> {
    /// For each decor, the clip level zooms it is generated at, a bit for each zoom
    /// counted from the first clip level's (`decor_pose_cs`).
    pub zooms: &'a [u32],
    /// The drawing words, `drawing::WORDS` of them, laid out as pxl-content
    /// `generation::drawing` says.
    pub drawing: &'a [u32],
    /// Placed decor, a record each in the layout `decor_pose_cs` writes: its decor, variant and
    /// turn, then its box's min corner in leaf cells, and a sink of none, as it stands where
    /// it is placed. The records are uploaded from where they lie, so their writer asks
    /// `in_flight` (gpu/api.rs) before it writes them again.
    pub placed_recs: &'a [[u32; generation::REC_WORDS as usize]],
    /// The unit the eye stands in, as a draw's Q3 (gpu/draw.rs `Position`). The instances'
    /// positions are about it, and from it they are drawn against the frame's eye (word.rs
    /// `frame::EYE`).
    pub anchor: Q3,
    /// The damage: the boxes of leaf cells the objects wrote since the last frame,
    /// `object::WRITE_DAMAGE_CAP` at most (pxl-content `generation::meets`). The entries whose
    /// decor they reach generate again.
    pub damage: &'a [[i32; 4]],
}

/// What decor's generation reads besides the frame's decor: the clip levels' table, a preview's
/// cuts, and the bricks filled this frame. None of it is the eye's place. An entry is a place
/// of the clip levels' table with the brick it holds. An entry generates whole where its brick
/// is another. Otherwise it generates again only the records that a box reaches by what the
/// record's own candidate reads (`generation::reaches`): a box of damage, a cut as it was or as
/// it is, or a brick filled at any clip level zoom, which may answer a read that a finer clip
/// level does not hold. Neither the objects nor the service area's index make an entry
/// generate by themselves: every write of the objects names its box (the frame's `damage`),
/// and the index is made once, before any brick is held. Each cell a candidate reads comes
/// from the finest clip level that holds it (pxl-content `generation::asks`).
pub struct Generation<'a> {
    pub clipmap: &'a [i32],
    pub cuts: &'a [u32; object::CUTS],
    /// The bricks handed this frame, each as its cells' zoom and the brick's place.
    pub fresh: &'a [(Zoom, [i32; 2])],
    /// The eye's words as the cast holds them (pxl-engine `eye`). The count pass reads the
    /// eye's place and the zooms it asks from them (`ORIGIN`, `RANGE`).
    pub eye: &'a [u32; eye::WORDS],
}

pub struct Generator {
    mark: Pipeline,
    thin: Pipeline,
    generate: Pipeline,
    cnt: Pipeline,
    args_pipeline: Pipeline,
    compact: Pipeline,
    groups: [Group; 6],
    /// What `decor_mark_cs` reads and the list it writes, by which `decor_thin_cs` and
    /// `decor_pose_cs` are dispatched, laid out as pxl-content `generation::THIN_ARGS` says.
    list: u32,
    zooms: u32,
    drawing: u32,
    rows: u32,
    meshes: u32,
    regions: u32,
    args: u32,
    /// Each region's draw uniform, `drawing::UNIFORM_WORDS` apart, as the decor and impostor
    /// shaders read one. The args pass sets each uniform's index.
    uniforms: u32,
    /// The records, laid out by entry (`generation::rec_at`). `instances` holds the decor's
    /// instances: `drawing::COLUMN_CNT` lanes and the index list, each `cap` words long.
    recs: u32,
    instances: u32,
    /// The draws' groups, in the layouts the decor and impostor draws share (record.rs
    /// `DRAW`, `UPLOAD`): one over the uniforms, with a region's at a dynamic offset, and one
    /// over the instances. Each is made with its buffers.
    draw_layouts: [Layout; 2],
    uniform_group: u32,
    instances_group: u32,
    /// What a frame drawn through a gate draws in place of the instances (`cull`), one for
    /// each gate (`Mask`), and the passes that make it.
    culled: [Culled; 2],
    cull_arg: Pipeline,
    cull: Pipeline,
    cap: u32,
    /// How many placed decor records were last handed.
    placed_cnt: u32,
    /// Whether decor is on this frame. Only `prepare` sets it, and it makes the buffers and
    /// the groups first, so while it is set they are made.
    on: bool,
    /// The decors' zooms as last written, which stay constant once the core has handed its
    /// table; the clip levels' table, the cuts and the placed decor as last handed; and
    /// whether this frame marks entries to generate.
    zooms_read: Read,
    clipmap: Read,
    cuts: [u32; object::CUTS],
    placed: Read,
    marking: bool,
    /// The boxes whose entries generate again, held until a frame that marks by them is drawn.
    /// A frame that is dropped, or handed and never drawn, hands them again; an entry
    /// generated twice generates the same. Past `object::DAMAGE_CAP` boxes, the rest are joined
    /// into the last.
    damage: [[i32; 4]; object::DAMAGE_CAP as usize],
    damage_cnt: usize,
    /// How many of `damage` this frame marks by, and how many boxes it marks by in
    /// all, counting the bricks handed (`rect_cnt`).
    mark_cnt: usize,
    rect_cnt: u32,
    /// Whether the entries' records may not hold what the entries generate, as after decor was
    /// off or the decors' zooms changed. The next frame drawn with decor then generates every
    /// entry.
    stale: bool,
    /// What the count, args and compact passes read besides the records, as they last ran:
    /// the eye's place and zooms, the drawing words, the meshes, the anchor and the records'
    /// capacity. `counting` says whether this frame runs them, which it does where any of
    /// that or the records moved.
    cnt_read: Read,
    counting: bool,
}

/// The buffers of the world `decor_pose_cs` asks, as the cast binds them: the cast's eye, the clip
/// levels' table, the bricks' words, the service area's index, the objects, and the ways'
/// pool.
pub type World = [u32; 6];

impl Generator {
    pub fn new(g: &Gpu, pipelines: &mut PipelineCache) -> Generator {
        let buf = |size: u32, usage: u32| g.buf(size, usage);
        let storage = buffer_usage::STORAGE | buffer_usage::COPY_DST;
        let rows = buf(size_of_val(&ROWS) as u32, storage);
        let table = ROWS;
        // SAFETY: the rows are `repr(C)` words alone (pxl-spec asset/decor.spec.rs), read while
        // `table` lives.
        let words = unsafe {
            core::slice::from_raw_parts(table.as_ptr().cast::<u32>(), size_of_val(&table) / 4)
        };
        g.write_buf(rows, 0, bytes(words));
        let decor_pipeline = pipelines.build_world(g, PIPELINE_DECOR);
        let draw_layouts = [decor_pipeline.layouts[1], decor_pipeline.layouts[3]];
        let uniforms = buf(
            word_bytes((drawing::UNIFORM_WORDS * drawing::REGION_CNT) as usize),
            storage | buffer_usage::UNIFORM,
        );
        Generator {
            mark: pipelines.build_compute(g, COMPUTE_GENERATION_MARK),
            thin: pipelines.build_compute(g, COMPUTE_GENERATION_THIN),
            generate: pipelines.build_compute(g, COMPUTE_GENERATION),
            cnt: pipelines.build_compute(g, COMPUTE_GENERATION_CNT),
            args_pipeline: pipelines.build_compute(g, COMPUTE_GENERATION_ARGS),
            compact: pipelines.build_compute(g, COMPUTE_GENERATION_COMPACT),
            groups: Default::default(),
            list: buf(
                word_bytes(generation::LIST_WORDS as usize),
                storage | buffer_usage::INDIRECT,
            ),
            zooms: buf(word_bytes(ROWS.len()), storage),
            drawing: buf(word_bytes(drawing::WORDS as usize), storage),
            rows,
            meshes: buf(word_bytes(GROUP_CNT as usize), storage),
            regions: buf(word_bytes(2 * drawing::REGION_CNT as usize), storage),
            args: buf(
                word_bytes((drawing::ARGS_WORDS * drawing::REGION_CNT) as usize),
                storage | buffer_usage::INDIRECT,
            ),
            uniforms,
            recs: 0,
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
                    word_bytes((drawing::ARGS_WORDS * drawing::REGION_CNT) as usize),
                    storage | buffer_usage::INDIRECT,
                ),
                group: 0,
            }),
            cull_arg: pipelines.build_compute(g, COMPUTE_DECOR_CULL_ARGS),
            cull: pipelines.build_compute(g, COMPUTE_DECOR_CULL),
            cap: 0,
            placed_cnt: 0,
            on: false,
            zooms_read: Read::default(),
            clipmap: Read::default(),
            cuts: [0; object::CUTS],
            placed: Read::default(),
            cnt_read: Read::default(),
            counting: false,
            marking: false,
            damage: [[0; 4]; object::DAMAGE_CAP as usize],
            damage_cnt: 0,
            mark_cnt: 0,
            rect_cnt: 0,
            stale: true,
        }
    }

    /// Takes the frame's decor `f` and what its generation reads (`generation`). `meshes`
    /// holds each group's index count, 0 while its model loads. The groups are set over
    /// `world` each frame (`api::Group`), so a frame without a cast misses no change.
    pub fn prepare(
        &mut self,
        g: &Gpu,
        f: DecorFrame,
        generation: Generation,
        meshes: &[u32; GROUP_CNT as usize],
        world: World,
    ) {
        self.on = true;
        let need =
            generation::ENTRY_REC_CAP + (f.placed_recs.len() as u32).next_power_of_two().max(64);
        let grew = need > self.cap;
        if grew {
            // The entries' records are copied into the new buffer. The placed records after them
            // are written anew.
            let (recs, instances) = (self.recs, self.instances);
            self.recs = g.buf(
                word_bytes(generation::rec_at(need) as usize),
                buffer_usage::STORAGE | buffer_usage::COPY_DST | buffer_usage::COPY_SRC,
            );
            self.instances = g.buf(
                word_bytes(((drawing::COLUMN_CNT + 1) * need) as usize),
                buffer_usage::STORAGE,
            );
            if recs != 0 {
                let e = g.encoder();
                g.copy_buf(
                    e,
                    (recs, 0),
                    (self.recs, 0),
                    word_bytes(generation::rec_at(generation::ENTRY_REC_CAP) as usize),
                );
                g.submit(e);
            }
            release(recs);
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
                    word_bytes(((drawing::COLUMN_CNT + 1) * need) as usize),
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
        let regroup = self.regroup(g, world);
        let s = &mut self.zooms_read;
        s.begin();
        s.read(f.zooms);
        if !s.end() {
            // Each entry's records were generated at the zooms the decors were shown at, so every
            // entry generates again.
            g.write_buf(self.zooms, 0, bytes(f.zooms));
            self.stale = true;
        }
        // The boxes whose entries generate again are held: the damage, and each cut that moved,
        // both as it was and as it is. They join any box a frame not drawn handed.
        for &b in f.damage {
            hold(&mut self.damage, &mut self.damage_cnt, b);
        }
        cut_rects(&self.cuts, generation.cuts, |b| {
            hold(&mut self.damage, &mut self.damage_cnt, b);
        });
        self.cuts = *generation.cuts;
        let w = &mut self.clipmap;
        w.begin();
        w.read(generation.clipmap);
        let changed = !w.end();
        self.mark_cnt = self.damage_cnt;
        // A brick filled at any clip level makes decor generate again wherever it may answer a
        // candidate's read.
        let fresh = !generation.fresh.is_empty();
        self.marking = self.stale || changed || fresh || self.mark_cnt != 0;
        self.rect_cnt = 0;
        if self.marking {
            // The boxes held come first, each reaching every zoom. Each brick handed follows,
            // reaching the candidates whose reads it may answer.
            let mut rects = [0u32; (generation::RECT_CAP * generation::RECT_WORDS) as usize];
            let fresh = generation
                .fresh
                .iter()
                .map(|&(cell_zoom, q)| (generation::brick_rect(cell_zoom, q), cell_zoom));
            let damage = self.damage[..self.mark_cnt]
                .iter()
                .map(|&b| (b, generation::EVERY));
            let mut n = 0;
            for (b, zoom) in damage.chain(fresh).take(generation::RECT_CAP as usize) {
                let at = n * generation::RECT_WORDS as usize;
                rects[at..at + 4].copy_from_slice(&b.map(|v| v as u32));
                rects[at + 4] = zoom;
                n += 1;
            }
            self.rect_cnt = n as u32;
            // The list's head (pxl-content `generation::THIN_ARGS`) holds each pass's
            // workgroups across and up, with the third count, the pairs listed, left 0 for
            // `decor_mark_cs` to fill. It then says whether every entry generates whole, how
            // many boxes there are, and that no pair generates in part yet.
            let mut hdr = [0u32; generation::RECTS_AT as usize];
            let mut put = |at: u32, val: u32| hdr[at as usize] = val;
            put(
                generation::THIN_ARGS,
                brick::BRICK_CELL_CNT.div_ceil(generation::LANE_CNT),
            );
            put(generation::THIN_ARGS + 1, 1);
            put(
                generation::GENERATE_ARGS,
                generation::DECOR_REC_CAP.div_ceil(generation::LANE_CNT),
            );
            put(generation::GENERATE_ARGS + 1, 1);
            put(generation::LIST_EVERY, u32::from(self.stale));
            put(generation::LIST_RECTS, n as u32);
            put(generation::LIST_PARTS, 0);
            g.write_buf(
                self.list,
                sequence::OPEN_GENERATION_LIST.to_at(),
                bytes(&hdr),
            );
            g.write_buf(
                self.list,
                sequence::OPEN_GENERATION_RECTS.to_at(),
                bytes(&rects[..n * generation::RECT_WORDS as usize]),
            );
        }
        let p = &mut self.placed;
        p.begin();
        p.read(f.placed_recs);
        let placed_changed = !p.end();
        self.placed_cnt = f.placed_recs.len() as u32;
        if placed_changed {
            g.write_buf(
                self.recs,
                generation::PLACED_HDR * 4,
                bytes(&[f.placed_recs.len() as u32, 0, 0, 0]),
            );
            if !f.placed_recs.is_empty() {
                // The upload is one stream record however many decor records there are.
                g.upload_buf(
                    self.recs,
                    generation::rec_at(generation::ENTRY_REC_CAP) * 4,
                    bytes(f.placed_recs.as_flattened()),
                );
            }
        }
        let c = &mut self.cnt_read;
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
        g.write_buf(
            self.regions,
            0,
            bytes(&[0u32; 2 * drawing::REGION_CNT as usize]),
        );
        // Each region's uniform is written here. Its index is the lanes' base until the args
        // pass adds the region's first instance. A region draws its group about the anchor,
        // which lies on Q3 and so has no remainder. An impostor's region, an odd one, also
        // names its group's layer.
        let mut uniforms = [0u32; (drawing::UNIFORM_WORDS * drawing::REGION_CNT) as usize];
        for r in 0..drawing::REGION_CNT {
            let u = &mut uniforms[(r * drawing::UNIFORM_WORDS) as usize..][..DRAW_FLOATS];
            u[instance::Q..instance::Q + 3].copy_from_slice(&f.anchor.map(|v| v as u32));
            u[instance::STRIDE] = self.cap;
            u[instance::OPACITY] = 1.0f32.to_bits();
            u[instance::LAYER] = if r % 2 == 1 { r / 2 } else { 0 };
            u[instance::IDX] = drawing::COLUMN_CNT * self.cap;
        }
        g.write_buf(self.uniforms, 0, bytes(&uniforms));
    }

    /// The generator's buffer for the frame's buffer `buf` (frame/shader/group.rs `BUFS`),
    /// or 0 for a buffer another part holds.
    fn buf_of(&self, buf: u32) -> u32 {
        match buf {
            GENERATION_LIST => self.list,
            GENERATION_ARGS => self.args,
            CULLED_SCREEN_ARGS => self.culled[Mask::Screen as usize].args,
            CULLED_LIGHT_ARGS => self.culled[Mask::Light as usize].args,
            _ => 0,
        }
    }

    /// Sets the groups over `world`, and says whether one was made anew: the world handed is
    /// another, or the records' capacity grew.
    fn regroup(&mut self, g: &Gpu, world: World) -> bool {
        let entries = |ids: &[(u32, u32)]| -> Vec<Entry> {
            ids.iter()
                .map(|&(binding, b)| buf_entry(binding, b))
                .collect()
        };
        let group = |pipeline: &Pipeline, ids: &[(u32, u32)]| (pipeline.layouts[0], entries(ids));
        let framed = |pipeline: &Pipeline, table: &access::Group, ids: &[(u32, u32)]| {
            let layout = pipeline.layouts[0];
            (
                layout,
                frame_entries(layout, table, |b| self.buf_of(b), &entries(ids)),
            )
        };
        let [eye, clipmap, ground, shards, objects, pool] = world;
        let made = [
            framed(
                &self.mark,
                &GENERATION_MARK_BINDS,
                &[(1, clipmap), (9, self.recs), (12, self.zooms)],
            ),
            framed(
                &self.thin,
                &GENERATION_THIN_BINDS,
                &[(9, self.recs), (10, self.rows)],
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
                    (9, self.recs),
                    (10, self.rows),
                ],
            ),
            group(
                &self.cnt,
                &[
                    (0, eye),
                    (1, clipmap),
                    (9, self.recs),
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
                    (9, self.recs),
                    (10, self.rows),
                    (13, self.drawing),
                    (14, self.regions),
                    (15, self.instances),
                ],
            ),
        ];
        let mut anew = false;
        for (group, (layout, e)) in self.groups.iter_mut().zip(made) {
            anew |= group.set(g, layout, &e);
        }
        anew
    }

    /// Turns decor off for this frame: nothing is generated or drawn, and the next frame with
    /// decor generates every entry (`stale`).
    pub fn off(&mut self) {
        self.on = false;
        self.rect_cnt = 0;
        self.stale = true;
        self.cnt_read.forget();
    }

    /// Called when the frame was drawn. The boxes it marked by, and its generation of every
    /// entry, are done.
    pub fn on_drawn(&mut self) {
        if self.marking {
            self.damage.copy_within(self.mark_cnt..self.damage_cnt, 0);
            self.damage_cnt -= self.mark_cnt;
            self.stale = false;
        }
        self.mark_cnt = 0;
    }

    /// Called when the frame was dropped and nothing of it was recorded. What it wrote is
    /// written again: the entries it marked are marked again by the boxes still held, and the
    /// clip levels' table and the placed decor are read anew.
    pub fn on_dropped(&mut self) {
        self.mark_cnt = 0;
        self.clipmap.forget();
        self.placed.forget();
        self.zooms_read.forget();
        self.cnt_read.forget();
    }

    /// Whether its passes run this frame, marking or counting. The frame then draws what they
    /// leave.
    pub fn works(&self) -> bool {
        self.on && (self.marking || self.counting)
    }

    /// Holds, for the next frame, the damage of a frame whose decor was handed and never drawn.
    pub fn carry_damage(&mut self, damage: &[[i32; 4]]) {
        for &b in damage {
            hold(&mut self.damage, &mut self.damage_cnt, b);
        }
    }

    /// Records the passes in the compute pass `pass` over the cast's world, after the cast.
    /// Where it marks, it marks the entries to generate again, thins their candidates a clip
    /// level cell a thread, and generates a record a thread where any may stand. Where it
    /// counts, it counts every record, lays the regions out and compacts the records.
    pub fn dispatch(&self, g: &Gpu, pass: u32) {
        if !self.on {
            return;
        }
        if self.marking {
            g.set_pipeline(pass, self.mark.id);
            g.set_bind_group(pass, 0, self.groups[0].id(), &[]);
            g.dispatch(
                pass,
                generation::EYE_ENTRY_CNT.div_ceil(generation::LANE_CNT),
                1,
                1,
            );
            let passes = [
                (
                    &self.thin,
                    self.groups[1].id(),
                    const { sequence::THIN.at() },
                ),
                (
                    &self.generate,
                    self.groups[2].id(),
                    const { sequence::GENERATE.at() },
                ),
            ];
            for (pipeline, group, at) in passes {
                g.set_pipeline(pass, pipeline.id);
                g.set_bind_group(pass, 0, group, &[]);
                g.dispatch_indirect(pass, self.list, at);
            }
        }
        if !self.counting {
            return;
        }
        // A workgroup reads the records an entry's decor laid, and past the entries each workgroup
        // reads `generation::RUN_CNT` placed records (decor_run.wgsl `run_of`). The dispatch
        // covers the records held, not the capacity.
        let recs = generation::EYE_ENTRY_CNT + self.placed_cnt.div_ceil(generation::RUN_CNT);
        let passes = [
            (&self.cnt, recs),
            (&self.args_pipeline, 1),
            (&self.compact, recs),
        ];
        for (k, (pipeline, x)) in passes.into_iter().enumerate() {
            g.set_pipeline(pass, pipeline.id);
            g.set_bind_group(pass, 0, self.groups[3 + k].id(), &[]);
            g.dispatch(pass, x, 1, 1);
        }
    }

    /// How many boxes this frame marks entries by to generate again: the damage and cuts held and
    /// the bricks handed, `generation::RECT_CAP` at most, or 0 where it marks none.
    pub fn rect_cnt(&self) -> u32 {
        self.rect_cnt
    }

    /// Whether decor is compacted again this frame, so its instances may stand elsewhere.
    pub fn counts(&self) -> bool {
        self.on && self.counting
    }

    /// The decor compacted, while decor is on (`DecorBuffer`).
    pub fn decor_buf(&self) -> Option<DecorBuffer> {
        self.on.then_some(DecorBuffer {
            drawing: self.drawing,
            regions: self.regions,
            instances: self.instances,
            cap: self.cap,
        })
    }

    /// What the draws read while decor is on: the instances' group, the regions' indirect
    /// args, the draw uniforms' group, and what a frame draws through each gate.
    pub fn indirect_draw(&self) -> Option<IndirectDraw> {
        self.on.then_some(IndirectDraw {
            instances_group: self.instances_group,
            args: self.args,
            uniform_group: self.uniform_group,
            culled: self.culled.map(|culled| (culled.group, culled.args)),
            mask: None,
        })
    }

    /// Lists the instances a frame draws through the gate `mask`: those whose bounding ball
    /// meets the box of the damaged tiles of `tiles`, as the frame's words `frame` see it
    /// (damage_rect.wgsl, decor_cull.wgsl). It runs once the tiles' marks are made, timed in
    /// the marks' span.
    pub fn cull(&self, ctx: &mut RenderContext, mask: Mask, frame: u32, tiles: &TileGrid) {
        if !self.on {
            return;
        }
        let damage_rect = tiles.dispatch_damage_rect(ctx);
        let culled = &self.culled[mask as usize];
        let g = ctx.g;
        let table = match mask {
            Mask::Screen => &CULL_SCREEN_BINDS,
            Mask::Light => &CULL_LIGHT_BINDS,
        };
        let group = g.frame_group(
            self.cull.layouts[0],
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
        g.set_pipeline(pass, self.cull_arg.id);
        g.dispatch(pass, drawing::REGION_CNT.div_ceil(64), 1, 1);
        g.set_pipeline(pass, self.cull.id);
        g.dispatch(pass, self.cap.div_ceil(64), 1, 1);
        g.end(pass);
        release(group);
    }
}

/// The gate a frame draws decor through (`Generator::cull`): the screen's damaged tiles, or
/// the light map's. Both are drawn in one frame, so each keeps its own list.
#[derive(Clone, Copy)]
pub enum Mask {
    Screen,
    Light,
}

/// What a frame draws through a gate in place of the instances: the instances that meet its
/// damaged tiles, listed at each region's place, the regions' args with them, and the draws'
/// group over that list.
#[derive(Clone, Copy)]
struct Culled {
    list: u32,
    args: u32,
    group: u32,
}

/// The compacted decor as a reader beside the draws reads it (light_mark_draws.wgsl): its
/// drawing words, its regions, its instances, and how many instances each lane holds
/// (`cap`).
#[derive(Clone, Copy)]
pub struct DecorBuffer {
    pub drawing: u32,
    pub regions: u32,
    pub instances: u32,
    pub cap: u32,
}

/// The decor's instances as the draws read them (`Generator::indirect_draw`): their group,
/// the regions' indirect args and the regions' draw uniforms' group.
#[derive(Clone, Copy)]
pub struct IndirectDraw {
    pub instances_group: u32,
    pub args: u32,
    pub uniform_group: u32,
    /// The group and the args a frame draws by through each gate (`Generator::cull`).
    pub culled: [(u32, u32); 2],
    /// The gate it is drawn through, none where it is drawn whole.
    pub mask: Option<Mask>,
}

impl IndirectDraw {
    /// The draw a frame makes through `mask`: its list and args in place of the instances'
    /// own (`Generator::cull`).
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
        drawing::ARGS_WORDS * 4 * region
    }

    /// Where region `region`'s draw uniform lies, a dynamic offset in bytes.
    pub fn uniform_at(&self, region: u32) -> u32 {
        drawing::UNIFORM_WORDS * 4 * region
    }
}

/// Appends box `b` after the `damage_cnt` boxes of `damage`, or, once `damage` is full,
/// joins it into the last box, so no box is lost however many come before they are taken.
/// The core's damage (pxl-core domain/scene/decor/gpu.rs `note_damage`), decor's generation
/// (`Generator`) and the node cache's forgetting (ray/cast.rs) all hold their boxes this way.
pub fn hold(damage: &mut [[i32; 4]], damage_cnt: &mut usize, b: [i32; 4]) {
    if *damage_cnt < damage.len() {
        damage[*damage_cnt] = b;
        *damage_cnt += 1;
    } else {
        let last = &mut damage[*damage_cnt - 1];
        *last = [
            last[0].min(b[0]),
            last[1].min(b[1]),
            last[2].max(b[2]),
            last[3].max(b[3]),
        ];
    }
}

/// Hands `each` the boxes a preview's cuts changed from `was` to `is`: each cut that moved,
/// both as it was and as it is (pxl-engine `object::cut_rect`).
pub(crate) fn cut_rects(
    was: &[u32; object::CUTS],
    is: &[u32; object::CUTS],
    mut each: impl FnMut([i32; 4]),
) {
    for k in [0, object::CUT_WORDS] {
        let (was, is) = (
            &was[k..k + object::CUT_WORDS],
            &is[k..k + object::CUT_WORDS],
        );
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
