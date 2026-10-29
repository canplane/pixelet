// pxl-kernel/domain/query/model/registry.rs

//! The registered models. A model's mesh is one heap block read lane by lane
//! (positions, normals, UVs, joints, weights, indices), in glTF's Y-up scaled
//! to its registered height; what draws it turns it Z-up. A model poses its
//! joints at a clip's time and writes their palette for whatever plays it.

use pxl_render::asset::gltf::{MAX_SCENE_IDXS, MAX_SCENE_VERTICES};
use pxl_render::frame::draw::{DrawList, MODEL_VERTEX_WORDS};
use xpute_guest::mem;
use xpute_guest::mem::heap::alloc;

// The most a parse lets one model draw fits a block's size.
const _: () = assert!(
    (MAX_SCENE_VERTICES as u64 * MODEL_VERTEX_WORDS as u64 + MAX_SCENE_IDXS as u64) * 4
        <= u32::MAX as u64
);

/// Translation, rotation; anything else is scale.
const T: u8 = 0;
const R: u8 = 1;

pub struct Channel {
    pub joint: u32,
    pub path: u8,
    /// Step keys hold a value until the next; linear keys interpolate.
    pub step: bool,
    pub times: Vec<f32>,
    /// 3 or 4 floats a key.
    pub vals: Vec<f32>,
}

struct Clip {
    duration: f32,
    channels: Vec<Channel>,
    /// The farthest any point of the mesh moves over it, unscaled; set when
    /// the model is sealed (`Model::moved`).
    motion: f32,
}

#[derive(Clone, Copy)]
pub struct MeshRef {
    pub at: usize,
    pub handle: u32,
    pub vertex_cnt: u32,
    pub idx_cnt: u32,
    pub radius: f32,
    pub scale: f32,
}

pub struct Model {
    at: usize,
    bytes: u32,
    handle: u32,
    vertex_cnt: u32,
    idx_cnt: u32,
    parent: Vec<i32>,
    /// A joint's rest pose: translation (3), rotation (4, xyzw), scale (3).
    rest: Vec<[f32; 10]>,
    /// The transform above a root joint that is not a joint itself, column-major.
    root_pre: Vec<[f32; 16]>,
    inverse_bind: Vec<[f32; 16]>,
    clips: Vec<Clip>,
    /// Joints in an order where a parent comes before its children.
    order: Vec<u32>,
    scale: f32,
    /// Unscaled bounding sphere for a hit test to reject by.
    radius: f32,
}

#[derive(Default)]
pub struct Models {
    /// Never let go: a model is a build asset and stays while the build runs.
    models: Vec<Model>,
}

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
        let th = libm::acosf(d);
        let s = libm::sinf(th);
        (libm::sinf((1.0 - t) * th) / s, libm::sinf(t * th) / s)
    };
    let q = [
        a[0] * wa + b[0] * wb,
        a[1] * wa + b[1] * wb,
        a[2] * wa + b[2] * wb,
        a[3] * wa + b[3] * wb,
    ];
    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3])
        .sqrt()
        .max(1e-12);
    [q[0] / n, q[1] / n, q[2] / n, q[3] / n]
}

/// Floats a key: four for a rotation, three otherwise.
fn width(path: u8) -> usize {
    if path == R {
        4
    } else {
        3
    }
}

fn sample(ch: &Channel, time: f32, out: &mut [f32; 10]) {
    let n = ch.times.len();
    if n == 0 {
        return;
    }
    let width = width(ch.path);
    let at = |k: usize| &ch.vals[k * width..k * width + width];
    let (k0, k1, t) = if time <= ch.times[0] {
        (0, 0, 0.0)
    } else if time >= ch.times[n - 1] {
        (n - 1, n - 1, 0.0)
    } else {
        let k = ch.times.partition_point(|&x| x <= time) - 1;
        let span = ch.times[k + 1] - ch.times[k];
        (
            k,
            k + 1,
            if span > 0.0 {
                (time - ch.times[k]) / span
            } else {
                0.0
            },
        )
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

/// One local transform eased toward another, rotations along the sphere.
pub fn blend(a: &[f32; 10], b: &[f32; 10], w: f32) -> [f32; 10] {
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
    /// None when the heap refused.
    pub fn new_model(
        &mut self,
        vertex_cnt: u32,
        idx_cnt: u32,
        joint_cnt: u32,
    ) -> Option<(u32, usize)> {
        let bytes = vertex_cnt
            .checked_mul(MODEL_VERTEX_WORDS)
            .and_then(|words| words.checked_add(idx_cnt))
            .and_then(|words| words.checked_mul(4))?;
        let (at, bytes) = alloc(bytes)?;
        let j = joint_cnt as usize;
        let model = Model {
            at,
            bytes,
            handle: 0,
            vertex_cnt,
            idx_cnt,
            parent: vec![-1; j],
            rest: vec![[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]; j],
            root_pre: vec![
                [
                    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0
                ];
                j
            ],
            inverse_bind: vec![
                [
                    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0
                ];
                j
            ],
            clips: Vec::new(),
            order: Vec::new(),
            scale: 1.0,
            radius: 0.0,
        };
        self.models.push(model);
        Some(((self.models.len() - 1) as u32, at))
    }

    /// `parent` is -1 for a root; `root_pre` is the transform above a root.
    pub fn joint(
        &mut self,
        model: u32,
        joint: u32,
        parent: i32,
        rest: [f32; 10],
        root_pre: [f32; 16],
        inverse_bind: [f32; 16],
    ) {
        let Some(m) = self.models.get_mut(model as usize) else {
            return;
        };
        let j = joint as usize;
        if j >= m.parent.len() {
            return;
        }
        m.parent[j] = parent;
        m.rest[j] = rest;
        m.root_pre[j] = root_pre;
        m.inverse_bind[j] = inverse_bind;
    }

    pub fn clip(&mut self, model: u32, duration: f32) -> u32 {
        let Some(m) = self.models.get_mut(model as usize) else {
            return u32::MAX;
        };
        m.clips.push(Clip {
            duration,
            channels: Vec::new(),
            motion: 0.0,
        });
        (m.clips.len() - 1) as u32
    }

    /// `ch` into `model`'s `clip`, its values a key's each, past them dropped.
    pub fn ch(&mut self, model: u32, clip: u32, mut ch: Channel) {
        let Some(m) = self.models.get_mut(model as usize) else {
            return;
        };
        let Some(c) = m.clips.get_mut(clip as usize) else {
            return;
        };
        ch.vals.truncate(ch.times.len() * width(ch.path));
        c.channels.push(ch);
    }

    /// # Safety
    /// The model's block has been written.
    pub unsafe fn seal(&mut self, draw: &mut DrawList, model: u32, scale: f32) {
        let Some(m) = self.models.get_mut(model as usize) else {
            return;
        };
        m.scale = scale;
        m.handle = draw.upload(m.at as u32, m.bytes, m.vertex_cnt, m.idx_cnt);
        let positions =
            core::slice::from_raw_parts(mem::at::<f32>(m.at), m.vertex_cnt as usize * 3);
        m.radius = positions
            .as_chunks::<3>()
            .0
            .iter()
            .map(|p| p[0] * p[0] + p[1] * p[1] + p[2] * p[2])
            .fold(0.0f32, f32::max)
            .sqrt();
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
        for c in 0..m.clips.len() {
            let motion = m.moved(c);
            m.clips[c].motion = motion;
        }
    }

    pub fn mesh(&self, model: u32) -> Option<MeshRef> {
        let m = self.models.get(model as usize)?;
        Some(MeshRef {
            at: m.at,
            handle: m.handle,
            vertex_cnt: m.vertex_cnt,
            idx_cnt: m.idx_cnt,
            radius: m.radius,
            scale: m.scale,
        })
    }

    pub fn get(&self, model: u32) -> Option<&Model> {
        self.models.get(model as usize)
    }
}

impl Model {
    pub fn joints(&self) -> usize {
        self.parent.len()
    }

    pub fn handle(&self) -> u32 {
        self.handle
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Its bounding sphere about its origin, unscaled.
    pub fn radius(&self) -> f32 {
        self.radius
    }

    /// A clip's length, where it has one.
    pub fn duration(&self, clip: u32) -> Option<f32> {
        self.clips.get(clip as usize).map(|c| c.duration)
    }

    /// The farthest any point of its mesh moves over a clip, unscaled.
    pub fn motion(&self, clip: u32) -> f32 {
        self.clips.get(clip as usize).map_or(0.0, |c| c.motion)
    }

    /// A clip's motion, off its own pose and palette at each of its keys: a
    /// vertex is its joints' palettes blended, so it moves no farther than
    /// the farthest joint's palette moves a point within the radius — its
    /// translation's change and its linear part's (Frobenius) times the
    /// radius. Twice the farthest from the first key bounds any two.
    fn moved(&self, clip: usize) -> f32 {
        let c = &self.clips[clip];
        let mut times: Vec<f32> = c
            .channels
            .iter()
            .flat_map(|ch| ch.times.iter().copied())
            .chain([0.0, c.duration])
            .collect();
        times.sort_by(f32::total_cmp);
        times.dedup();
        let n = self.joints();
        let (mut local, mut global) = (vec![[0.0; 10]; n], vec![[0.0; 16]; n]);
        let (mut first, mut palette) = (vec![0.0; n * 16], vec![0.0; n * 16]);
        self.pose(Some((clip as u32, times[0])), &mut local);
        self.palette(&local, &mut global, &mut first);
        let mut far = 0.0f32;
        for &t in &times[1..] {
            self.pose(Some((clip as u32, t)), &mut local);
            self.palette(&local, &mut global, &mut palette);
            for (a, b) in palette
                .as_chunks::<16>()
                .0
                .iter()
                .zip(first.as_chunks::<16>().0)
            {
                let d: [f32; 16] = core::array::from_fn(|i| a[i] - b[i]);
                let linear = [0, 1, 2, 4, 5, 6, 8, 9, 10]
                    .iter()
                    .map(|&i| d[i] * d[i])
                    .sum::<f32>()
                    .sqrt();
                let moved = (d[12] * d[12] + d[13] * d[13] + d[14] * d[14]).sqrt();
                far = far.max(moved + linear * self.radius);
            }
        }
        2.0 * far
    }

    /// The joints' local transforms at a clip's time, or at rest.
    pub fn pose(&self, at: Option<(u32, f32)>, out: &mut [[f32; 10]]) {
        out.copy_from_slice(&self.rest);
        let Some(c) = at.and_then(|(clip, _)| self.clips.get(clip as usize)) else {
            return;
        };
        let time = at.map_or(0.0, |(_, t)| t);
        for ch in &c.channels {
            if let Some(slot) = out.get_mut(ch.joint as usize) {
                sample(ch, time, slot);
            }
        }
    }

    /// The joint palette from local transforms, parents first: one mat4 a joint.
    pub fn palette(&self, local: &[[f32; 10]], global: &mut [[f32; 16]], palette: &mut [f32]) {
        for &j in &self.order {
            let j = j as usize;
            let l = mat_trs(&local[j]);
            let p = self.parent[j];
            global[j] = if p < 0 {
                mat_mul(&self.root_pre[j], &l)
            } else {
                mat_mul(&global[p as usize], &l)
            };
        }
        for j in 0..self.joints() {
            palette[j * 16..j * 16 + 16]
                .copy_from_slice(&mat_mul(&global[j], &self.inverse_bind[j]));
        }
    }
}
