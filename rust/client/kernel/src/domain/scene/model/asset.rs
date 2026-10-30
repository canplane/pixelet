// pxl-kernel/domain/scene/model/asset.rs

//! A parsed glTF registered as a model, the one way any model enters the
//! world. A skinned model's joints are its skins' joints; an unskinned one
//! animates by nodes, every node a joint with identity inverse bind (a pet's
//! parts walk this way). A still model (no skin, no clip) has its node
//! transforms baked into the vertices, since the decor pass reads no joint
//! palette and a wheel would otherwise sit at the model's origin.

use fnv::FnvHashMap;

use pxl_render::asset::gltf::{scene_nodes, world_matrices, Gltf};
use pxl_render::frame::draw::MODEL_VERTEX_WORDS;
use pxl_spec::space::coord::mat4;
use xpute_guest::global::Global;
use xpute_guest::mem;

use crate::domain::scene::model;
use crate::domain::scene::model::registry::Channel;
use crate::sys::io::loader::{load_gltf, load_of};
use crate::sys::io::room::Refused;
use crate::sys::mem::section;
use pxl_spec::asset::catalog::Slot;

#[derive(Clone, Debug, PartialEq)]
pub struct ModelHandle {
    pub model: u32,
    /// One per animation in file order, so a clip's number is its animation's index.
    pub clip_cnt: u32,
    /// What it is drawn at to stand the height it was registered at.
    pub scale: f32,
}

const IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

/// A model as registered, or as the heap refused it room.
type Registration = Result<ModelHandle, Refused>;

/// Each file, by its number below the asset root, by the height it was
/// asked at, its bits.
static REGISTERED: Global<Vec<(u32, Option<u64>, Registration)>> = Global::new();

/// The file `slot` names registered `height_u` units tall, or at its own
/// size when None, once: two slots naming one file reach one. None while it
/// loads, and while the heap refuses it room: a refusal is a wait, not a
/// failure, asked again once the heap gives some back.
pub fn model_of(slot: Slot, height_u: Option<f64>) -> Option<ModelHandle> {
    let load = load_of(slot)?;
    let file = load.number();
    let key = height_u.map(f64::to_bits);
    let registered = REGISTERED.get(Vec::new);
    let at = registered
        .iter()
        .position(|(a, h, _)| *a == file && *h == key);
    match at.map(|at| &registered[at].2) {
        Some(Ok(handle)) => return Some(handle.clone()),
        Some(Err(refused)) if !refused.over() => return None,
        _ => {}
    }
    let gltf = load_gltf(load)?;
    let entry = register_model(&gltf, height_u).ok_or_else(Refused::now);
    match at {
        Some(at) => registered[at].2 = entry.clone(),
        None => registered.push((file, key, entry.clone())),
    }
    entry.ok()
}

/// `slot`'s model as `model_of` registered it at `height_u`, asking
/// nothing: what a reader that must not load, such as the query, finds;
/// none while it is not registered.
pub fn registered(slot: Slot, height_u: Option<f64>) -> Option<ModelHandle> {
    let file = load_of(slot)?.number();
    let key = height_u.map(f64::to_bits);
    REGISTERED
        .get(Vec::new)
        .iter()
        .find(|(a, h, _)| *a == file && *h == key)
        .and_then(|(_, _, entry)| entry.clone().ok())
}

/// At its own size when `height_u` is None; None when it has no mesh or the
/// heap refused.
fn register_model(gltf: &Gltf, height_u: Option<f64>) -> Option<ModelHandle> {
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

    section::map().and_then(|_| {
        let models = model::models();
        let (model, block) = models.new_model(vertex_cnt, idx_cnt, joint_cnt)?;
        let n = (vertex_cnt * MODEL_VERTEX_WORDS + idx_cnt) as usize;
        // SAFETY: the model's block, `n` words, ours until sealed.
        let words = unsafe { core::slice::from_raw_parts_mut(mem::at::<u32>(block), n) };
        let lane = |k: usize| k * vertex_cnt as usize;
        let put_f = |words: &mut [u32], at: usize, v: f32| words[at] = v.to_bits();

        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        let (mut v0, mut i0) = (0usize, 0usize);
        let mut p = [0.0; 3];
        for &(node_idx, mesh, prim_idx) in &parts {
            let node = &gltf.nodes[node_idx as usize];
            let prim = &gltf.meshes[mesh][prim_idx];
            let cnt = prim.pos.len() / 3;
            // A skinned vertex is already in the skin's space; an unskinned one is
            // placed by its node's joint.
            let remap: Option<Vec<u32>> = (skinned && node.skin >= 0).then(|| {
                gltf.skins[node.skin as usize]
                    .joints
                    .iter()
                    .map(|j| *joint_of.get(j).unwrap_or(&0))
                    .collect()
            });
            let own_joint = *joint_of.get(&node_idx).unwrap_or(&0);
            let world_mat = &worlds[node_idx as usize];
            for i in 0..cnt {
                let v = v0 + i;
                let (mut x, mut y, mut z) =
                    (prim.pos[i * 3], prim.pos[i * 3 + 1], prim.pos[i * 3 + 2]);
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
                put_f(words, lane(0) + v * 3, x);
                put_f(words, lane(0) + v * 3 + 1, y);
                put_f(words, lane(0) + v * 3 + 2, z);
                for (c, val) in normal.into_iter().enumerate() {
                    put_f(words, lane(3) + v * 3 + c, val);
                }
                let uv = prim
                    .uv
                    .as_ref()
                    .map_or([0.0, 0.0], |uv| [uv[i * 2], uv[i * 2 + 1]]);
                put_f(words, lane(6) + v * 2, uv[0]);
                put_f(words, lane(6) + v * 2 + 1, uv[1]);
                for c in 0..4 {
                    match (&remap, &prim.joints) {
                        (Some(remap), Some(joints)) => {
                            words[lane(8) + v * 4 + c] =
                                *remap.get(joints[i * 4 + c] as usize).unwrap_or(&0);
                            let w = prim
                                .weights
                                .as_ref()
                                .map_or(if c == 0 { 1.0 } else { 0.0 }, |w| w[i * 4 + c]);
                            put_f(words, lane(12) + v * 4 + c, w);
                        }
                        _ => {
                            words[lane(8) + v * 4 + c] = if c == 0 { own_joint } else { 0 };
                            put_f(words, lane(12) + v * 4 + c, if c == 0 { 1.0 } else { 0.0 });
                        }
                    }
                }
                let at_y = if remap.is_some() || still {
                    y as f64
                } else {
                    mat4::transform_point(&mut p, world_mat, x as f64, y as f64, z as f64)[1]
                };
                lo = lo.min(at_y);
                hi = hi.max(at_y);
            }
            for (i, &idx) in prim.idx.iter().enumerate() {
                words[lane(MODEL_VERTEX_WORDS as usize) + i0 + i] = v0 as u32 + idx;
            }
            v0 += cnt;
            i0 += prim.idx.len();
        }

        let mut inverse_bind_of: FnvHashMap<u32, [f32; 16]> = FnvHashMap::default();
        if skinned {
            for skin in &gltf.skins {
                for (k, &j) in skin.joints.iter().enumerate() {
                    let mut m = [0.0; 16];
                    m.copy_from_slice(&skin.inverse_bind[k * 16..k * 16 + 16]);
                    inverse_bind_of.insert(j, m);
                }
            }
        }
        for j in 0..joint_cnt {
            let Some(&n) = joint_nodes.get(j as usize) else {
                models.joint(
                    model,
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
                models.joint(
                    model,
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
            let rest =
                [t[0], t[1], t[2], r[0], r[1], r[2], r[3], s[0], s[1], s[2]].map(|v| v as f32);
            // A root joint under a non-joint node carries that node's world transform.
            let root_pre = if parent < 0 && parent_node >= 0 {
                worlds[parent_node as usize].map(|v| v as f32)
            } else {
                IDENTITY
            };
            models.joint(
                model,
                j,
                parent,
                rest,
                root_pre,
                *inverse_bind_of.get(&n).unwrap_or(&IDENTITY),
            );
        }

        for anim in &gltf.animations {
            let c = models.clip(model, anim.duration as f32);
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
                models.ch(model, c, ch);
            }
        }

        let raw = hi - lo;
        let scale = match height_u {
            Some(h) if raw > 0.0 => (h / raw) as f32,
            _ => 1.0,
        };
        // SAFETY: the model's block is in the volumes' memory.
        unsafe { models.seal(model::draw_list(), model, scale) };
        Some(ModelHandle {
            model,
            clip_cnt: gltf.animations.len() as u32,
            scale,
        })
    })
}
