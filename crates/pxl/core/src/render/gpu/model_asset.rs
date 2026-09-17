// pxl_core/render/gpu/model_asset.rs

//! A parsed glTF (render/gltf/gltf.rs) handed to the scheduler's models as a
//! model (crates/pxl/core/src/module/world/gltf_cache.rs): the one place a parsed asset
//! becomes the kernel's lanes, so a companion, a decor kind or an asset a player
//! brings all enter the world the same way.
//!
//! A skinned model's joints are its skins' joint nodes, every primitive merged
//! onto that one list: its vertices are in the skin's space, as the spec has
//! them, and a joint's palette entry is its global transform times its inverse
//! bind matrix. A model with no skin is animated by its nodes instead, so every
//! node in the scene is a joint with an identity inverse bind, and a mesh's
//! vertices ride the joint of the node that holds it — which is what plays a
//! pet's walk, where each part is a node of its own. A root joint whose parent
//! is not a joint carries that parent's transform above it. Each clip's
//! channels on joints go over by node.
//!
//! The height the model is scaled to is measured over its vertices at rest,
//! through their nodes, as the box three drew around the scene.

use std::collections::HashMap;

use xpute_core::kit::mat4::{mat4_transform_point, Vec3};

use crate::render::gltf::gltf::{scene_nodes, world_matrices, Gltf};
use crate::sys::kernel::with_world;

#[derive(Clone, Debug, PartialEq)]
pub struct ModelHandle {
    pub model: u32,
    /// Clip index by the name the glTF gives it.
    pub clips: HashMap<String, u32>,
    /// Triangles, for the ledger.
    pub triangles: u32,
}

const IDENTITY: [f32; 16] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

/// Registers `gltf` as a model standing `height_u` tall, or at its own size
/// when None; None when it has no mesh or the pool refused its block.
pub fn register_model(gltf: &Gltf, height_u: Option<f64>) -> Option<ModelHandle> {
    let order = scene_nodes(gltf);
    let worlds = world_matrices(gltf);
    let skinned = !gltf.skins.is_empty() && order.iter().any(|&n| gltf.nodes[n as usize].skin >= 0 && gltf.nodes[n as usize].mesh >= 0);

    // The joints: the skins' joint nodes in first-seen order, or every node.
    let mut joint_nodes: Vec<u32> = Vec::new();
    let mut joint_of: HashMap<u32, u32> = HashMap::new();
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

    // The meshes: every primitive of every node that holds one, skinned or not
    // as the model is.
    let mut parts: Vec<(u32, usize, usize)> = Vec::new();
    let (mut vertex_cnt, mut index_cnt) = (0u32, 0u32);
    for &n in &order {
        let node = &gltf.nodes[n as usize];
        if node.mesh < 0 || (skinned && node.skin < 0) {
            continue;
        }
        for (k, prim) in gltf.meshes[node.mesh as usize].iter().enumerate() {
            parts.push((n, node.mesh as usize, k));
            vertex_cnt += (prim.position.len() / 3) as u32;
            index_cnt += prim.index.len() as u32;
        }
    }
    if vertex_cnt == 0 {
        return None;
    }

    with_world(|world, sched| {
        let (model, block) = sched.models.new_model(vertex_cnt, index_cnt, joint_cnt)?;
        let mem = world.volumes.mem();
        let n = (vertex_cnt * 16 + index_cnt) as usize;
        // SAFETY: the model's block is `n` words in the volumes' memory, ours
        // until the model is sealed.
        let words = unsafe { core::slice::from_raw_parts_mut(mem.add(block) as *mut u32, n) };
        let lane = |k: usize| k * vertex_cnt as usize;
        let put_f = |words: &mut [u32], at: usize, v: f32| words[at] = v.to_bits();

        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        let (mut v0, mut i0) = (0usize, 0usize);
        let mut p = Vec3 { x: 0.0, y: 0.0, z: 0.0 };
        for &(node_index, mesh, prim_index) in &parts {
            let node = &gltf.nodes[node_index as usize];
            let prim = &gltf.meshes[mesh][prim_index];
            let count = prim.position.len() / 3;
            // A skinned vertex is in the skin's space already, and its joints
            // are the skin's; an unskinned one is the node's, whose joint
            // places it.
            let remap: Option<Vec<u32>> = (skinned && node.skin >= 0).then(|| gltf.skins[node.skin as usize].joints.iter().map(|j| *joint_of.get(j).unwrap_or(&0)).collect());
            let own_joint = *joint_of.get(&node_index).unwrap_or(&0);
            let world_m = &worlds[node_index as usize];
            for i in 0..count {
                let v = v0 + i;
                let (x, y, z) = (prim.position[i * 3], prim.position[i * 3 + 1], prim.position[i * 3 + 2]);
                put_f(words, lane(0) + v * 3, x);
                put_f(words, lane(0) + v * 3 + 1, y);
                put_f(words, lane(0) + v * 3 + 2, z);
                let normal = prim.normal.as_ref().map_or([0.0, 1.0, 0.0], |nm| [nm[i * 3], nm[i * 3 + 1], nm[i * 3 + 2]]);
                for (c, value) in normal.into_iter().enumerate() {
                    put_f(words, lane(3) + v * 3 + c, value);
                }
                let uv = prim.uv.as_ref().map_or([0.0, 0.0], |uv| [uv[i * 2], uv[i * 2 + 1]]);
                put_f(words, lane(6) + v * 2, uv[0]);
                put_f(words, lane(6) + v * 2 + 1, uv[1]);
                for c in 0..4 {
                    match (&remap, &prim.joints) {
                        (Some(remap), Some(joints)) => {
                            words[lane(8) + v * 4 + c] = *remap.get(joints[i * 4 + c] as usize).unwrap_or(&0);
                            let w = prim.weights.as_ref().map_or(if c == 0 { 1.0 } else { 0.0 }, |w| w[i * 4 + c]);
                            put_f(words, lane(12) + v * 4 + c, w);
                        }
                        _ => {
                            words[lane(8) + v * 4 + c] = if c == 0 { own_joint } else { 0 };
                            put_f(words, lane(12) + v * 4 + c, if c == 0 { 1.0 } else { 0.0 });
                        }
                    }
                }
                // The height, measured where the vertex stands at rest.
                let at_y = if remap.is_some() {
                    y as f64
                } else {
                    mat4_transform_point(&mut p, world_m, x as f64, y as f64, z as f64).y
                };
                lo = lo.min(at_y);
                hi = hi.max(at_y);
            }
            for (i, &index) in prim.index.iter().enumerate() {
                words[lane(16) + i0 + i] = v0 as u32 + index;
            }
            v0 += count;
            i0 += prim.index.len();
        }

        // Joints: parent, rest pose, what stands above a root, inverse bind.
        let mut inverse_bind_of: HashMap<u32, [f32; 16]> = HashMap::new();
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
                sched.models.joint(model, j, -1, [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0], IDENTITY, IDENTITY);
                continue;
            };
            let node = &gltf.nodes[n as usize];
            let parent_node = node.parent;
            let parent = if parent_node >= 0 { joint_of.get(&(parent_node as u32)).map_or(-1, |&p| p as i32) } else { -1 };
            let t = node.translation;
            let r = node.rotation;
            let s = node.scale;
            let rest = [t[0], t[1], t[2], r[0], r[1], r[2], r[3], s[0], s[1], s[2]].map(|v| v as f32);
            // A root joint under a node that is not one carries that node's
            // world transform; a scene root carries nothing.
            let root_pre = if parent < 0 && parent_node >= 0 {
                worlds[parent_node as usize].map(|v| v as f32)
            } else {
                IDENTITY
            };
            sched.models.joint(model, j, parent, rest, root_pre, *inverse_bind_of.get(&n).unwrap_or(&IDENTITY));
        }

        // Clips: the channels on joints.
        let mut clips = HashMap::new();
        for anim in &gltf.animations {
            let c = sched.models.clip(model, anim.duration as f32);
            clips.insert(anim.name.clone(), c);
            for channel in &anim.channels {
                let Some(&joint) = joint_of.get(&channel.node) else { continue };
                let keys = channel.times.len() as u32;
                // SAFETY: the keys' floats are this channel's own vectors, read
                // before this returns; a null base makes their addresses absolute.
                unsafe {
                    sched.models.channel(
                        core::ptr::null(),
                        model,
                        c,
                        joint,
                        channel.path as u8,
                        channel.step,
                        keys,
                        channel.times.as_ptr() as usize,
                        channel.values.as_ptr() as usize,
                    );
                }
            }
        }

        let raw = hi - lo;
        let scale = match height_u {
            Some(h) if raw > 0.0 => (h / raw) as f32,
            _ => 1.0,
        };
        // SAFETY: the model's block is in the volumes' memory.
        unsafe { sched.models.seal(&mut sched.draw, mem, model, scale) };
        Some(ModelHandle {
            model,
            clips,
            triangles: index_cnt / 3,
        })
    })
    .flatten()
}
