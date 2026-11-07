// pxl-kernel/domain/scene/model/asset.rs

//! A glTF file parsed and registered as a model, the one way any model
//! enters the world. A skinned model's joints are its skins' joints; an
//! unskinned one animates by nodes, every node a joint with identity inverse
//! bind (a pet's parts walk this way). A still model (no skin, no clip) has
//! its node transforms baked into the vertices, since the decor pass reads no
//! joint palette and a wheel would otherwise sit at the model's origin.
//!
//! A file's bytes, as the loader read them (sys/io/loader.rs `take_gltf`),
//! are parsed once, whole, as they are taken (pxl-render `parse_gltf`), and
//! the parse is kept for every registration of the file, the bytes gone. A
//! registration runs in its own task, and yields between units where the
//! turn's deadline has passed: a primitive, a run of `STEPS_PER_PAUSE`
//! vertices, the joints, a clip, and the seal's passes (registry.rs
//! `Model::seal`). The model joins the registry, and the table here, only
//! once it is whole.

use core::future::Future;
use core::ops::Range;
use core::pin::pin;
use core::task::{Context, Poll, Waker};
use std::rc::Rc;

use fnv::FnvHashMap;

use pxl_render::asset::gltf::{parse_gltf, scene_nodes, world_matrices, Gltf, GltfPrimitive};
use pxl_render::gpu::draw::MODEL_VERTEX_WORDS;
use pxl_spec::space::coord::mat4::{self, Mat4};
use xpute_core::global::Global;
use xpute_core::mem;
use xpute_core::sched::edge::yield_if_spent;
use xpute_core::sched::task;

use crate::domain::scene::model;
use crate::domain::scene::model::registry::{Channel, Model, STEPS_PER_PAUSE};
use crate::sys::io::loader::{load_of, take_gltf};
use crate::sys::io::room::Refusal;
use crate::sys::load::Load;
use crate::sys::sched::edge::{request_world_tick, world_edge};
use pxl_spec::asset::catalog::{Slot, SLOTS};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

/// The models that can register at once, each in its own task: one a slot,
/// the most `model_of` starts.
pub const REGISTRATIONS: usize = SLOTS;

/// None while its task runs, then the model as registered, by its number,
/// or as the heap refused it room.
type Registration = Option<Result<u32, Refusal>>;

/// Each file, by its number below the asset root, by the height it was
/// asked at, its bits.
static REGISTRY: Global<Vec<(u32, Option<u64>, Registration)>> = Global::new();

/// The file as parsed, or why its bytes do not parse, for good.
type Parse = Result<Rc<Gltf>, MarshalError>;

/// Each file's parse, by its number below the asset root.
static PARSED: Global<Vec<(u32, Parse)>> = Global::new();

/// The file `slot` names registered `height_u` units tall, or at its own
/// size when None, once, by its model's number: two slots naming one file
/// reach one. None while it loads or registers, while the heap refuses it
/// room, and for good where its bytes do not parse: a refusal is a wait, not
/// a failure, asked again once the heap gives some back. No more than
/// `REGISTRATIONS` run at once; past them a model waits its turn here.
pub fn model_of(slot: Slot, height_u: Option<f64>) -> Option<u32> {
    let load = load_of(slot)?;
    let file = load.id();
    let key = height_u.map(f64::to_bits);
    let registered = REGISTRY.get(Vec::new);
    let at = registered
        .iter()
        .position(|(a, h, _)| *a == file && *h == key);
    match at.map(|at| &registered[at].2) {
        Some(Some(Ok(model))) => return Some(*model),
        Some(None) => return None,
        Some(Some(Err(refused))) if !refused.over() => return None,
        _ => {}
    }
    let gltf = gltf_of(load)?;
    if registered.iter().filter(|(_, _, r)| r.is_none()).count() >= REGISTRATIONS {
        return None;
    }
    match at {
        Some(at) => registered[at].2 = None,
        None => registered.push((file, key, None)),
    }
    let spawned = task::spawn(registration(file, height_u, gltf), 0);
    xpute_kit::ensure!(spawned, ENOSPC, file);
    None
}

/// `load`'s file as parsed; none while it loads, and for good where it does
/// not parse. The first to ask once it is read takes its bytes from the
/// loader and parses them.
fn gltf_of(load: Load) -> Option<Rc<Gltf>> {
    let file = load.id();
    let parsed = PARSED.get(Vec::new);
    if !parsed.iter().any(|(a, _)| *a == file) {
        let bytes = take_gltf(load)?;
        parsed.push((file, parse(&bytes).map(Rc::new)));
    }
    let (_, gltf) = parsed.iter().find(|(a, _)| *a == file)?;
    gltf.as_ref().ok().cloned()
}

/// `bytes` parsed whole, at once: its pause is always ready.
fn parse(bytes: &[u8]) -> Result<Gltf, MarshalError> {
    let mut parsing = pin!(parse_gltf(bytes, &[], || core::future::ready(())));
    match parsing
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(parsed) => parsed,
        Poll::Pending => None.or_bug(Errno::ENOTRECOVERABLE),
    }
}

/// `slot`'s model as `model_of` registered it at `height_u`, asking
/// nothing: what a reader that must not load, such as the query, finds;
/// none while it is not registered.
pub fn registered_model(slot: Slot, height_u: Option<f64>) -> Option<u32> {
    let file = load_of(slot)?.id();
    let key = height_u.map(f64::to_bits);
    REGISTRY
        .get(Vec::new)
        .iter()
        .find(|(a, h, _)| *a == file && *h == key)
        .and_then(|(_, _, entry)| (*entry)?.ok())
}

/// The file numbered `file`'s model, `gltf`, registered at `height_u`, its
/// entry left registering by `model_of` and set once it is whole or
/// refused. A landing wakes a turn.
async fn registration(file: u32, height_u: Option<f64>, gltf: Rc<Gltf>) {
    let entry = register_model(&gltf, height_u, || yield_if_spent(world_edge))
        .await
        .ok_or_else(Refusal::now);
    let key = height_u.map(f64::to_bits);
    if let Some((_, _, at)) = REGISTRY
        .get(Vec::new)
        .iter_mut()
        .find(|(a, h, _)| *a == file && *h == key)
    {
        *at = Some(entry);
    }
    request_world_tick();
}

/// A primitive as its model lays it out: from vertex `first` on, by its
/// node's world transform where the model is still, its joints remapped into
/// the model's where skinned, else every vertex on its node's joint.
struct Part<'a> {
    prim: &'a GltfPrimitive,
    first: usize,
    still: bool,
    world_mat: &'a Mat4,
    remap: Option<Vec<u32>>,
    own_joint: u32,
}

/// `part`'s vertices `run` written into the block's `words`, columns
/// `vertex_cnt` apart, their lowest and highest Y as placed folded into `ys`.
fn lay_out(
    words: &mut [u32],
    vertex_cnt: usize,
    part: &Part,
    run: Range<usize>,
    ys: &mut [f64; 2],
) {
    let Part {
        prim,
        still,
        world_mat,
        own_joint,
        ..
    } = *part;
    let remap = &part.remap;
    let column = |k: usize| k * vertex_cnt;
    let put_f = |words: &mut [u32], at: usize, v: f32| words[at] = v.to_bits();
    let mut p = [0.0; 3];
    for i in run {
        let v = part.first + i;
        let (mut x, mut y, mut z) = (prim.pos[i * 3], prim.pos[i * 3 + 1], prim.pos[i * 3 + 2]);
        let mut normal = prim.normal.as_ref().map_or([0.0, 1.0, 0.0], |nm| {
            [nm[i * 3], nm[i * 3 + 1], nm[i * 3 + 2]]
        });
        if still {
            let at = mat4::transform_point(&mut p, world_mat, x as f64, y as f64, z as f64);
            (x, y, z) = (at[0] as f32, at[1] as f32, at[2] as f32);
            let n = mat4::transform_dir(
                &mut p,
                world_mat,
                normal[0] as f64,
                normal[1] as f64,
                normal[2] as f64,
            );
            normal = [n[0] as f32, n[1] as f32, n[2] as f32];
        }
        put_f(words, column(0) + v * 3, x);
        put_f(words, column(0) + v * 3 + 1, y);
        put_f(words, column(0) + v * 3 + 2, z);
        for (c, val) in normal.into_iter().enumerate() {
            put_f(words, column(3) + v * 3 + c, val);
        }
        let uv = prim
            .uv
            .as_ref()
            .map_or([0.0, 0.0], |uv| [uv[i * 2], uv[i * 2 + 1]]);
        put_f(words, column(6) + v * 2, uv[0]);
        put_f(words, column(6) + v * 2 + 1, uv[1]);
        for c in 0..4 {
            match (remap, &prim.joints) {
                (Some(remap), Some(joints)) => {
                    words[column(8) + v * 4 + c] =
                        *remap.get(joints[i * 4 + c] as usize).unwrap_or(&0);
                    let w = prim
                        .weights
                        .as_ref()
                        .map_or(if c == 0 { 1.0 } else { 0.0 }, |w| w[i * 4 + c]);
                    put_f(words, column(12) + v * 4 + c, w);
                }
                _ => {
                    words[column(8) + v * 4 + c] = if c == 0 { own_joint } else { 0 };
                    put_f(
                        words,
                        column(12) + v * 4 + c,
                        if c == 0 { 1.0 } else { 0.0 },
                    );
                }
            }
        }
        let at_y = if remap.is_some() || still {
            y as f64
        } else {
            mat4::transform_point(&mut p, world_mat, x as f64, y as f64, z as f64)[1]
        };
        ys[0] = ys[0].min(at_y);
        ys[1] = ys[1].max(at_y);
    }
}

/// Its model's number, at its own size when `height_u` is None; None when
/// it has no mesh or the heap refused. Awaits `pause` first and between
/// units.
async fn register_model<P: Future<Output = ()>>(
    gltf: &Gltf,
    height_u: Option<f64>,
    pause: impl Fn() -> P,
) -> Option<u32> {
    pause().await;
    let order = scene_nodes(gltf);
    let worlds = world_matrices(gltf);
    let skinned = !gltf.skins.is_empty()
        && order
            .iter()
            .any(|&n| gltf.nodes[n as usize].skin >= 0 && gltf.nodes[n as usize].mesh >= 0);
    let still = !skinned && gltf.animations.is_empty();

    let mut joint_nodes: Vec<u32> = Vec::new();
    let mut joint_of: FnvHashMap<u32, u32> = FnvHashMap::default();
    let mut add_joint = |node: u32| {
        if let std::collections::hash_map::Entry::Vacant(e) = joint_of.entry(node) {
            e.insert(joint_nodes.len() as u32);
            joint_nodes.push(node);
        }
    };
    if skinned {
        for &n in &order {
            let node = &gltf.nodes[n as usize];
            if node.skin >= 0 && node.mesh >= 0 {
                for &j in &gltf.skins[node.skin as usize].joints {
                    add_joint(j);
                }
            }
        }
    } else {
        for &n in &order {
            add_joint(n);
        }
    }
    let joint_cnt = joint_nodes.len().max(1) as u32;

    let mut parts: Vec<(u32, usize, usize)> = Vec::new();
    let (mut vertex_cnt, mut idx_cnt) = (0u32, 0u32);
    for &n in &order {
        let node = &gltf.nodes[n as usize];
        if node.mesh < 0 || (skinned && node.skin < 0) {
            continue;
        }
        for (k, prim) in gltf.meshes[node.mesh as usize].iter().enumerate() {
            parts.push((n, node.mesh as usize, k));
            vertex_cnt = vertex_cnt.checked_add(u32::try_from(prim.pos.len() / 3).ok()?)?;
            idx_cnt = idx_cnt.checked_add(u32::try_from(prim.idx.len()).ok()?)?;
        }
    }
    if vertex_cnt == 0 {
        return None;
    }

    let mut m = Model::new(vertex_cnt, idx_cnt, joint_cnt)?;
    let n = (vertex_cnt * MODEL_VERTEX_WORDS + idx_cnt) as usize;
    // SAFETY: the model's block, `n` words, ours alone: no number names the
    // model until it joins the registry, sealed.
    let words = unsafe { core::slice::from_raw_parts_mut(mem::at::<u32>(m.at()), n) };
    let mut ys = [f64::INFINITY, f64::NEG_INFINITY];
    let (mut v0, mut i0) = (0usize, 0usize);
    for &(node_idx, mesh, prim_idx) in &parts {
        pause().await;
        let node = &gltf.nodes[node_idx as usize];
        let prim = &gltf.meshes[mesh][prim_idx];
        let cnt = prim.pos.len() / 3;
        let part = Part {
            prim,
            first: v0,
            still,
            world_mat: &worlds[node_idx as usize],
            remap: (skinned && node.skin >= 0).then(|| {
                gltf.skins[node.skin as usize]
                    .joints
                    .iter()
                    .map(|j| *joint_of.get(j).unwrap_or(&0))
                    .collect()
            }),
            own_joint: *joint_of.get(&node_idx).unwrap_or(&0),
        };
        for from in (0..cnt).step_by(STEPS_PER_PAUSE) {
            if from > 0 {
                pause().await;
            }
            let run = from..cnt.min(from + STEPS_PER_PAUSE);
            lay_out(words, vertex_cnt as usize, &part, run, &mut ys);
        }
        // An index is a word where a vertex is `MODEL_VERTEX_WORDS`.
        let run = STEPS_PER_PAUSE * MODEL_VERTEX_WORDS as usize;
        let idxs_at = vertex_cnt as usize * MODEL_VERTEX_WORDS as usize + i0;
        for (dst, src) in words[idxs_at..idxs_at + prim.idx.len()]
            .chunks_mut(run)
            .zip(prim.idx.chunks(run))
        {
            pause().await;
            for (w, &idx) in dst.iter_mut().zip(src) {
                *w = v0 as u32 + idx;
            }
        }
        v0 += cnt;
        i0 += prim.idx.len();
    }

    pause().await;
    let mut inverse_bind_of: FnvHashMap<u32, [f32; 16]> = FnvHashMap::default();
    if skinned {
        for skin in &gltf.skins {
            for (k, &j) in skin.joints.iter().enumerate() {
                let mut mat = [0.0; 16];
                mat.copy_from_slice(&skin.inverse_bind[k * 16..k * 16 + 16]);
                inverse_bind_of.insert(j, mat);
            }
        }
    }
    for j in 0..joint_cnt {
        let Some(&n) = joint_nodes.get(j as usize) else {
            m.joint(
                j,
                -1,
                [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
                IDENTITY,
                IDENTITY,
            );
            continue;
        };
        let node = &gltf.nodes[n as usize];
        let parent_node = node.parent;
        let parent = if parent_node >= 0 {
            joint_of
                .get(&(parent_node as u32))
                .map_or(-1, |&p| p as i32)
        } else {
            -1
        };
        if still {
            m.joint(
                j,
                parent,
                [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
                IDENTITY,
                IDENTITY,
            );
            continue;
        }
        let t = node.translation;
        let r = node.rot;
        let s = node.scale;
        let rest = [t[0], t[1], t[2], r[0], r[1], r[2], r[3], s[0], s[1], s[2]].map(|v| v as f32);
        // A root joint under a non-joint node carries that node's world transform.
        let root_pre = if parent < 0 && parent_node >= 0 {
            worlds[parent_node as usize].map(|v| v as f32)
        } else {
            IDENTITY
        };
        m.joint(
            j,
            parent,
            rest,
            root_pre,
            *inverse_bind_of.get(&n).unwrap_or(&IDENTITY),
        );
    }

    for anim in &gltf.animations {
        pause().await;
        let c = m.clip(anim.duration as f32);
        for ch in &anim.channels {
            let Some(&joint) = joint_of.get(&ch.node) else {
                continue;
            };
            let ch = Channel {
                joint,
                path: ch.path as u8,
                step: ch.step,
                times: ch.times.to_vec(),
                vals: ch.vals.to_vec(),
            };
            m.ch(c, ch);
        }
    }

    let raw = ys[1] - ys[0];
    let scale = match height_u {
        Some(h) if raw > 0.0 => (h / raw) as f32,
        _ => 1.0,
    };
    // SAFETY: every vertex and index of the block written above.
    unsafe { m.seal(scale, &pause).await };
    Some(model::models().add(model::draw_list(), m))
}
