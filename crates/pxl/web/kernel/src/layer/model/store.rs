// pxl-kernel/layer/model/store.rs

//! Models and the actors that play them: a glTF asset as the core holds it,
//! and the instances placed in the world that animate it.
//!
//! A model is parsed from bytes the shell fetches — the core does not read
//! files. Its mesh is one heap block in a layout a draw reads
//! lane by lane (positions, normals, UVs, four joints and four weights a
//! vertex, then the indices); its skeleton is each joint's parent, its rest
//! pose and its inverse bind matrix, the vertices already in the space the
//! bind matrices expect; its clips are channels of keyframes, one joint and
//! one of translation, rotation or scale each. A model with no joints is
//! drawn as it lies. The mesh stands in glTF's Y-up with its height scaled
//! to the height it is registered with; an actor turns it to this world's Z-up.
//!
//! An actor is a model at a place, facing a way, playing a clip. Each tick
//! the core advances every actor's clips, samples them — linear keys, the
//! rotations slerped — blends a clip fading in over the one fading out, as
//! three's mixer did, walks the joints parent first, and writes the joint
//! palette into the actor's block, which the renderer is told to rewrite.

use pxl_render::frame::draw::{DrawList, MODEL_VERTEX_WORDS, PIPELINE_SKINNED};
use pxl_render::gpu::api::word_bytes;
use pxl_spec::space::v3::V3;
use xpute_runtime::abi::handle::Slots;
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::heap::{alloc, free};

use crate::layer::model::occupancy::Occupancy;

/// A channel's path: translation, rotation; anything else is scale.
const T: u8 = 0;
const R: u8 = 1;

struct Channel {
    joint: u32,
    path: u8,
    /// Step keys hold a value until the next; linear keys interpolate.
    step: bool,
    times: Vec<f32>,
    /// 3 or 4 floats a key.
    values: Vec<f32>,
}

struct Clip {
    duration: f32,
    channels: Vec<Channel>,
}

/// Where a model's mesh is and how big, for a hit test (layer/decor/hit.rs).
#[derive(Clone, Copy)]
pub struct MeshRef {
    pub at: usize,
    /// The renderer's buffer, once sealed.
    pub handle: u32,
    pub vertex_cnt: u32,
    pub index_cnt: u32,
    pub radius: f32,
    pub scale: f32,
}

pub struct Model {
    at: usize,
    bytes: u32,
    handle: u32,
    vertex_cnt: u32,
    index_cnt: u32,
    parent: Vec<i32>,
    /// A joint's rest pose: translation (3), rotation (4, xyzw), scale (3).
    rest: Vec<[f32; 10]>,
    /// The transform above a root joint that is not a joint itself, column-major.
    root_pre: Vec<[f32; 16]>,
    inverse_bind: Vec<[f32; 16]>,
    clips: Vec<Clip>,
    /// Joints in an order where a parent comes before its children.
    order: Vec<u32>,
    /// What the mesh is scaled by to stand as tall as it is registered.
    scale: f32,
    /// How far a vertex reaches from the model's origin, unscaled: a
    /// bounding sphere for a hit test to reject by.
    radius: f32,
    /// What it fills, in its own frame and units.
    occupancy: Option<Occupancy>,
}

#[derive(Clone, Copy)]
struct Playing {
    clip: u32,
    time: f32,
}

pub struct Actor {
    model: u32,
    /// Where it stands, in units, and its turn about Z.
    pub at: V3,
    pub rotation_z: f32,
    pub visible: bool,
    current: Option<Playing>,
    /// The clip fading out, and how far the fade has gone of its length.
    previous: Option<Playing>,
    fade: f32,
    fade_len: f32,
    palette_at: usize,
    palette_bytes: u32,
    palette_handle: u32,
    /// Scratch: each joint's local and global transform this tick.
    local: Vec<[f32; 10]>,
    global: Vec<[f32; 16]>,
}

#[derive(Default)]
pub struct Models {
    /// Every model registered, by number: none is let go, since a model is
    /// an asset of the build and the build's assets stay while it runs.
    models: Vec<Model>,
    actors: Slots<Actor>,
    /// The pose a cross-fade blends from, worked in a turn at a time.
    from: Vec<[f32; 10]>,
}

// ---- small math over column-major f32 matrices ----

fn mat_mul(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut o = [0.0; 16];
    for c in 0..4 {
        for r in 0..4 {
            let mut s = 0.0;
            for k in 0..4 {
                s += a[k * 4 + r] * b[c * 4 + k];
            }
            o[c * 4 + r] = s;
        }
    }
    o
}

fn mat_trs(p: &[f32; 10]) -> [f32; 16] {
    let (x, y, z, w) = (p[3], p[4], p[5], p[6]);
    let (sx, sy, sz) = (p[7], p[8], p[9]);
    let (x2, y2, z2) = (x + x, y + y, z + z);
    let (xx, xy, xz) = (x * x2, x * y2, x * z2);
    let (yy, yz, zz) = (y * y2, y * z2, z * z2);
    let (wx, wy, wz) = (w * x2, w * y2, w * z2);
    [
        (1.0 - (yy + zz)) * sx,
        (xy + wz) * sx,
        (xz - wy) * sx,
        0.0,
        (xy - wz) * sy,
        (1.0 - (xx + zz)) * sy,
        (yz + wx) * sy,
        0.0,
        (xz + wy) * sz,
        (yz - wx) * sz,
        (1.0 - (xx + yy)) * sz,
        0.0,
        p[0],
        p[1],
        p[2],
        1.0,
    ]
}

fn slerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let mut d = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    let mut b = b;
    if d < 0.0 {
        d = -d;
        b = [-b[0], -b[1], -b[2], -b[3]];
    }
    let (wa, wb) = if d > 0.9995 {
        (1.0 - t, t)
    } else {
        let th = d.acos();
        let s = th.sin();
        (((1.0 - t) * th).sin() / s, (t * th).sin() / s)
    };
    let q = [a[0] * wa + b[0] * wb, a[1] * wa + b[1] * wb, a[2] * wa + b[2] * wb, a[3] * wa + b[3] * wb];
    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt().max(1e-12);
    [q[0] / n, q[1] / n, q[2] / n, q[3] / n]
}

fn sample(ch: &Channel, time: f32, out: &mut [f32; 10]) {
    let n = ch.times.len();
    if n == 0 {
        return;
    }
    let width = if ch.path == R { 4 } else { 3 };
    let at = |k: usize| &ch.values[k * width..k * width + width];
    let (k0, k1, t) = if time <= ch.times[0] {
        (0, 0, 0.0)
    } else if time >= ch.times[n - 1] {
        (n - 1, n - 1, 0.0)
    } else {
        let k = ch.times.partition_point(|&x| x <= time) - 1;
        let span = ch.times[k + 1] - ch.times[k];
        (k, k + 1, if span > 0.0 { (time - ch.times[k]) / span } else { 0.0 })
    };
    let t = if ch.step { 0.0 } else { t };
    let (a, b) = (at(k0), at(k1));
    match ch.path {
        T => {
            for i in 0..3 {
                out[i] = a[i] + (b[i] - a[i]) * t;
            }
        }
        R => {
            let q = slerp([a[0], a[1], a[2], a[3]], [b[0], b[1], b[2], b[3]], t);
            out[3..7].copy_from_slice(&q);
        }
        _ => {
            for i in 0..3 {
                out[7 + i] = a[i] + (b[i] - a[i]) * t;
            }
        }
    }
}

fn blend(a: &[f32; 10], b: &[f32; 10], w: f32) -> [f32; 10] {
    let mut o = [0.0; 10];
    for i in 0..3 {
        o[i] = a[i] + (b[i] - a[i]) * w;
        o[7 + i] = a[7 + i] + (b[7 + i] - a[7 + i]) * w;
    }
    let q = slerp([a[3], a[4], a[5], a[6]], [b[3], b[4], b[5], b[6]], w);
    o[3..7].copy_from_slice(&q);
    o
}

impl Models {
    /// A model of `vertex_cnt` vertices, `index_cnt` indices and `joint_cnt`
    /// joints, its mesh block acquired: its id and the block's address, for
    /// the caller to write; none when the heap refused.
    pub fn new_model(&mut self, vertex_cnt: u32, index_cnt: u32, joint_cnt: u32) -> Option<(u32, usize)> {
        xpute_core::ensure!(self.models.len() < crate::mem::fixed::MODELS_MAX, ENOTRECOVERABLE, self.models.len());
        let bytes = (vertex_cnt * MODEL_VERTEX_WORDS + index_cnt) * 4;
        let (at, bytes) = alloc(bytes)?;
        let j = joint_cnt as usize;
        let model = Model {
            at,
            bytes,
            handle: 0,
            vertex_cnt,
            index_cnt,
            parent: vec![-1; j],
            rest: vec![[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]; j],
            root_pre: vec![[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]; j],
            inverse_bind: vec![[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]; j],
            clips: Vec::new(),
            order: Vec::new(),
            scale: 1.0,
            radius: 0.0,
            occupancy: None,
        };
        self.models.push(model);
        Some(((self.models.len() - 1) as u32, at))
    }

    /// A joint: its parent (-1 for a root), its rest pose, the transform
    /// above it when it is a root, and its inverse bind matrix.
    pub fn joint(&mut self, model: u32, joint: u32, parent: i32, rest: [f32; 10], root_pre: [f32; 16], inverse_bind: [f32; 16]) {
        let Some(m) = self.models.get_mut(model as usize) else { return };
        let j = joint as usize;
        if j >= m.parent.len() {
            return;
        }
        m.parent[j] = parent;
        m.rest[j] = rest;
        m.root_pre[j] = root_pre;
        m.inverse_bind[j] = inverse_bind;
    }

    /// A clip; its index in the model.
    pub fn clip(&mut self, model: u32, duration: f32) -> u32 {
        let Some(m) = self.models.get_mut(model as usize) else { return u32::MAX };
        m.clips.push(Clip { duration, channels: Vec::new() });
        (m.clips.len() - 1) as u32
    }

    /// A channel of a clip: keys' times and values, `values` holding the
    /// keys' values end to end — four for a rotation and three otherwise.
    #[allow(clippy::too_many_arguments)]
    pub fn channel(&mut self, model: u32, clip: u32, joint: u32, path: u8, step: bool, times: &[f32], values: &[f32]) {
        let Some(m) = self.models.get_mut(model as usize) else { return };
        let Some(c) = m.clips.get_mut(clip as usize) else { return };
        let width = if path == R { 4 } else { 3 };
        let values = values[..times.len() * width].to_vec();
        let times = times.to_vec();
        c.channels.push(Channel { joint, path, step, times, values });
    }

    /// The model is complete: its block goes to the GPU and its joints are
    /// ordered parent first.
    ///
    /// # Safety
    /// The model's block has been written.
    pub unsafe fn seal(&mut self, draw: &mut DrawList, model: u32, scale: f32) {
        let Some(m) = self.models.get_mut(model as usize) else { return };
        m.scale = scale;
        m.handle = draw.upload(m.at as u32, m.bytes, m.vertex_cnt, m.index_cnt);
        let positions = core::slice::from_raw_parts(ptr_at::<f32>(m.at), m.vertex_cnt as usize * 3);
        m.radius = positions.as_chunks::<3>().0.iter().map(|p| p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).fold(0.0f32, f32::max).sqrt();
        let indices = core::slice::from_raw_parts(ptr_at::<u32>(m.at).add(m.vertex_cnt as usize * MODEL_VERTEX_WORDS as usize), m.index_cnt as usize);
        m.occupancy = Occupancy::of(positions, indices);
        let n = m.parent.len();
        let mut placed = vec![false; n];
        m.order.clear();
        while m.order.len() < n {
            let before = m.order.len();
            for j in 0..n {
                let p = m.parent[j];
                if !placed[j] && (p < 0 || placed[p as usize]) {
                    placed[j] = true;
                    m.order.push(j as u32);
                }
            }
            if m.order.len() == before {
                // A cycle: the rest are roots.
                for (j, done) in placed.iter_mut().enumerate() {
                    if !*done {
                        *done = true;
                        m.order.push(j as u32);
                    }
                }
            }
        }
    }

    /// A model's mesh, for a hit test: where its block is, its counts, its
    /// reach from the origin and its scale.
    pub fn mesh(&self, model: u32) -> Option<MeshRef> {
        let m = self.models.get(model as usize)?;
        Some(MeshRef {
            at: m.at,
            handle: m.handle,
            vertex_cnt: m.vertex_cnt,
            index_cnt: m.index_cnt,
            radius: m.radius,
            scale: m.scale,
        })
    }

    /// What a model fills, in its own frame and units; none for one with no
    /// extent.
    pub fn occupancy(&self, model: u32) -> Option<&Occupancy> {
        self.models.get(model as usize)?.occupancy.as_ref()
    }

    pub fn new_actor(&mut self, draw: &mut DrawList, model: u32) -> Option<u32> {
        xpute_core::ensure!(self.actors.iter().count() < crate::mem::fixed::ACTORS_MAX, ENOTRECOVERABLE);
        let joints = self.models.get(model as usize)?.parent.len();
        let palette_bytes = word_bytes(joints.max(1) * 16);
        let (palette_at, palette_bytes) = alloc(palette_bytes)?;
        // SAFETY: the block just acquired; identity joints until the first tick.
        unsafe {
            let p = core::slice::from_raw_parts_mut(ptr_at::<f32>(palette_at), palette_bytes as usize / 4);
            p.fill(0.0);
            for j in 0..joints.max(1) {
                for d in 0..4 {
                    p[j * 16 + d * 5] = 1.0;
                }
            }
        }
        let palette_handle = draw.upload(palette_at as u32, palette_bytes, 0, 0);
        let actor = Actor {
            model,
            at: [0.0; 3],
            rotation_z: 0.0,
            visible: false,
            current: None,
            previous: None,
            fade: 0.0,
            fade_len: 0.0,
            palette_at,
            palette_bytes,
            palette_handle,
            local: vec![[0.0; 10]; joints],
            global: vec![[0.0; 16]; joints],
        };
        Some(self.actors.insert(actor))
    }

    pub fn actor(&mut self, id: u32) -> Option<&mut Actor> {
        self.actors.get_mut(id)
    }

    pub fn actor_ref(&self, id: u32) -> Option<&Actor> {
        self.actors.get(id)
    }

    /// Starts `clip` from its beginning, fading over `fade_s` from what
    /// played before; a clip already playing is left alone.
    pub fn play(&mut self, id: u32, clip: u32, fade_s: f32) {
        let Some(a) = self.actor(id) else { return };
        if a.current.is_some_and(|c| c.clip == clip) {
            return;
        }
        a.previous = a.current;
        a.current = Some(Playing { clip, time: 0.0 });
        a.fade = 0.0;
        a.fade_len = if a.previous.is_some() { fade_s.max(0.0) } else { 0.0 };
    }

    pub fn remove_actor(&mut self, draw: &mut DrawList, id: u32) {
        if let Some(a) = self.actors.remove(id) {
            draw.free(a.palette_handle);
            // SAFETY: the block this asset took and has not given back.
            unsafe { free((a.palette_at, a.palette_bytes)) };
        }
    }

    /// Advances every actor's clips by `dt` seconds and writes the palettes;
    /// whether any
    /// actor is playing a clip of some length, which is what asks for the
    /// next frame — nothing else says an actor moved.
    pub fn tick(&mut self, draw: &mut DrawList, dt: f32) -> bool {
        let mut animating = false;
        let Models { models, actors, from } = self;
        for a in actors.values_mut() {
            let Some(m) = models.get(a.model as usize) else { continue };
            animating |= a.current.is_some_and(|p| m.clips.get(p.clip as usize).is_some_and(|c| c.duration > 0.0));
            let wrap = |p: &mut Playing| {
                p.time += dt;
                if let Some(c) = m.clips.get(p.clip as usize) {
                    if c.duration > 0.0 {
                        p.time %= c.duration;
                    }
                }
            };
            if let Some(p) = a.current.as_mut() {
                wrap(p);
            }
            if let Some(p) = a.previous.as_mut() {
                wrap(p);
            }
            if a.previous.is_some() {
                a.fade += dt;
                if a.fade >= a.fade_len {
                    a.previous = None;
                }
            }
            let joints = m.parent.len();
            if joints == 0 {
                continue;
            }
            let pose = |p: Option<Playing>, out: &mut [[f32; 10]]| {
                out.copy_from_slice(&m.rest);
                if let Some(p) = p {
                    if let Some(c) = m.clips.get(p.clip as usize) {
                        for ch in &c.channels {
                            if let Some(slot) = out.get_mut(ch.joint as usize) {
                                sample(ch, p.time, slot);
                            }
                        }
                    }
                }
            };
            pose(a.current, &mut a.local);
            if let Some(prev) = a.previous {
                from.resize(joints, [0.0; 10]);
                pose(Some(prev), from);
                let w = if a.fade_len > 0.0 { (a.fade / a.fade_len).clamp(0.0, 1.0) } else { 1.0 };
                for (to, from) in a.local.iter_mut().zip(from.iter()) {
                    *to = blend(from, to, w);
                }
            }
            for &j in &m.order {
                let j = j as usize;
                let local = mat_trs(&a.local[j]);
                let p = m.parent[j];
                a.global[j] = if p < 0 { mat_mul(&m.root_pre[j], &local) } else { mat_mul(&a.global[p as usize], &local) };
            }
            // SAFETY: the actor's own palette block, one mat4 a joint.
            let palette = unsafe { core::slice::from_raw_parts_mut(ptr_at::<f32>(a.palette_at), joints * 16) };
            for j in 0..joints {
                palette[j * 16..j * 16 + 16].copy_from_slice(&mat_mul(&a.global[j], &m.inverse_bind[j]));
            }
            draw.rewrite(a.palette_handle, a.palette_at as u32, a.palette_bytes);
        }
        animating
    }

    /// Every visible actor's draw, placed against the frame's origin.
    pub fn draw(&self, draw: &mut DrawList, eye: V3) {
        for (_, a) in self.actors.iter() {
            if !a.visible {
                continue;
            }
            let Some(m) = self.models.get(a.model as usize) else { continue };
            if m.handle == 0 {
                continue;
            }
            let t = DrawList::translation(a.at, eye);
            draw.draw_scaled(PIPELINE_SKINNED, m.handle, a.palette_handle, a.model, t, a.rotation_z, 1.0, m.scale);
        }
    }
}
