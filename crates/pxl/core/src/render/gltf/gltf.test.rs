// pxl_core/render/gltf/gltf.test.rs

use core::pin::pin;
use core::task::{Context, Poll, Waker};

use super::*;

fn read(path: &str) -> Vec<u8> {
    std::fs::read(format!("{}/../../../asset/{path}", env!("CARGO_MANIFEST_DIR"))).expect("the shipped asset")
}

/// The parse with pauses that never wait, so one poll runs it to the end.
fn parse(path: &str) -> Gltf {
    let bytes = read(path);
    let mut parse = pin!(parse_gltf(&bytes, &[], || core::future::ready(())));
    match parse.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(g) => g.unwrap(),
        Poll::Pending => unreachable!(),
    }
}

#[test]
fn gltf_a_rigged_character_comes_out_with_its_skins_weights_and_clips() {
    let g = parse("actor/kenney_mini_characters/male-f.glb");
    let prims: Vec<&GltfPrimitive> = g.meshes.iter().flatten().collect();
    assert_eq!(prims.len(), 2);
    assert_eq!(prims.iter().map(|p| p.position.len() / 3).sum::<usize>(), 1291);
    assert_eq!(g.skins.iter().map(|s| s.joints.len()).collect::<Vec<_>>(), vec![7, 7]);
    assert_eq!(g.skins[0].inverse_bind.len(), 7 * 16);
    for p in &prims {
        let (joints, weights) = (p.joints.as_ref().expect("joints"), p.weights.as_ref().expect("weights"));
        for w in weights.chunks_exact(4) {
            assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-3);
        }
        assert!(joints.iter().all(|&j| j < 7));
    }
    // The pack's clips are in one order in every file: static, idle, walk, sprint, …
    let idle = &g.animations[1];
    assert!((idle.duration - 1.3333).abs() < 1e-3);
    assert!(idle.channels.iter().all(|c| g.skins[0].joints.contains(&c.node)));
    assert!(idle.channels.iter().any(|c| c.path == GltfPath::Rotation && c.values.len() == c.times.len() * 4));
}

#[test]
fn gltf_an_unrigged_pet_is_animated_by_its_nodes_each_part_a_node_with_a_mesh() {
    let g = parse("actor/kenney_cube_pets/cat.glb");
    assert!(g.skins.is_empty());
    // static, idle, walk, run, …
    let walk = &g.animations[2];
    assert_eq!(walk.channels.len(), 24);
    let order = scene_nodes(&g);
    // Parents before children, every node reached once.
    assert_eq!(order.len(), g.nodes.len());
    for (k, &n) in order.iter().enumerate() {
        let parent = g.nodes[n as usize].parent;
        if parent >= 0 {
            assert!(order.iter().position(|&m| m == parent as u32).unwrap() < k);
        }
    }
    assert!(g.meshes.iter().flatten().all(|p| p.joints.is_none()));
}

#[test]
fn gltf_a_node_s_world_matrix_composes_its_parents_transforms() {
    let g = parse("decor/kenney_car_kit/taxi.glb");
    let worlds = world_matrices(&g);
    for n in scene_nodes(&g) {
        let node = &g.nodes[n as usize];
        if node.parent < 0 {
            assert_eq!(worlds[n as usize], node.local);
        }
    }
    // The taxi stands on its wheels: the lowest vertex, through its node, is
    // below the body's origin and the highest well above it.
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for n in scene_nodes(&g) {
        let node = &g.nodes[n as usize];
        if node.mesh < 0 {
            continue;
        }
        let w = &worlds[n as usize];
        for p in &g.meshes[node.mesh as usize] {
            for v in p.position.chunks_exact(3) {
                let y = w[1] * v[0] as f64 + w[5] * v[1] as f64 + w[9] * v[2] as f64 + w[13];
                lo = lo.min(y);
                hi = hi.max(y);
            }
        }
    }
    assert!(lo < 0.0 && hi > 1.0, "lo {lo} hi {hi}");
}
