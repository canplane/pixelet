// pxl-tool/asset/glb.rs

//! A model's extent, read from the POSITION accessors' `min`/`max` (which glTF
//! requires) under the node transforms; buffers are never decoded.

use serde::Deserialize;

use crate::mat4::{self, Mat4};

#[derive(Deserialize)]
struct Gltf {
    #[serde(default)]
    scenes: Vec<Scene>,
    /// Only the named scene is measured: other scenes are variants, and their
    /// union is not the model.
    scene: Option<usize>,
    #[serde(default)]
    nodes: Vec<Node>,
    #[serde(default)]
    meshes: Vec<Mesh>,
    #[serde(default)]
    accessors: Vec<Accessor>,
}

#[derive(Deserialize)]
struct Scene {
    #[serde(default)]
    nodes: Vec<usize>,
}

#[derive(Deserialize)]
struct Node {
    #[serde(default)]
    children: Vec<usize>,
    mesh: Option<usize>,
    matrix: Option<Mat4>,
    translation: Option<[f64; 3]>,
    rotation: Option<[f64; 4]>,
    scale: Option<[f64; 3]>,
}

#[derive(Deserialize)]
struct Mesh {
    #[serde(default)]
    primitives: Vec<Primitive>,
}

#[derive(Deserialize)]
struct Primitive {
    #[serde(default)]
    attributes: fnv::FnvHashMap<String, usize>,
}

#[derive(Deserialize)]
struct Accessor {
    min: Option<Vec<f64>>,
    max: Option<Vec<f64>>,
}

const GLB_MAGIC: u32 = 0x4654_6c67; // "glTF"
const CHUNK_JSON: u32 = 0x4e4f_534a; // "JSON"

fn u32_le(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn json_chunk(bytes: &[u8]) -> Result<&[u8], String> {
    if u32_le(bytes, 0) != Some(GLB_MAGIC) {
        return Err("not a glb".to_string());
    }
    let len = u32_le(bytes, 12).ok_or("truncated header")? as usize;
    if u32_le(bytes, 16) != Some(CHUNK_JSON) {
        return Err("first chunk is not JSON".to_string());
    }
    bytes
        .get(20..20 + len)
        .ok_or_else(|| "truncated JSON chunk".to_string())
}

fn local(node: &Node) -> Mat4 {
    if let Some(m) = node.matrix {
        return m;
    }
    let t = node.translation.unwrap_or([0.0; 3]);
    let q = node.rotation.unwrap_or([0.0, 0.0, 0.0, 1.0]);
    let s = node.scale.unwrap_or([1.0; 3]);
    let mut m = mat4::new();
    mat4::compose_trs(&mut m, &t, &q, &s);
    m
}

struct Aabb {
    lo: [f64; 3],
    hi: [f64; 3],
}

impl Aabb {
    fn new() -> Aabb {
        Aabb {
            lo: [f64::INFINITY; 3],
            hi: [f64::NEG_INFINITY; 3],
        }
    }
    fn add(&mut self, p: &[f64; 3]) {
        for (i, &v) in p.iter().enumerate() {
            self.lo[i] = self.lo[i].min(v);
            self.hi[i] = self.hi[i].max(v);
        }
    }
}

fn visit(gltf: &Gltf, idx: usize, parent: &Mat4, bounds: &mut Aabb, depth: usize) {
    // glTF does not forbid a cycle, which would not terminate.
    if depth > 64 {
        return;
    }
    let Some(node) = gltf.nodes.get(idx) else {
        return;
    };
    let mut world = mat4::new();
    mat4::mul(&mut world, parent, &local(node));

    if let Some(mesh) = node.mesh.and_then(|m| gltf.meshes.get(m)) {
        for primitive in &mesh.primitives {
            let Some(accessor) = primitive
                .attributes
                .get("POSITION")
                .and_then(|a| gltf.accessors.get(*a))
            else {
                continue;
            };
            let (Some(min), Some(max)) = (&accessor.min, &accessor.max) else {
                continue;
            };
            if min.len() < 3 || max.len() < 3 {
                continue;
            }
            // Every corner: a rotation takes a box's corners outside its min and max.
            for corner in 0..8u8 {
                let p = [
                    if corner & 1 != 0 { max[0] } else { min[0] },
                    if corner & 2 != 0 { max[1] } else { min[1] },
                    if corner & 4 != 0 { max[2] } else { min[2] },
                ];
                let mut at = [0.0; 3];
                bounds.add(mat4::transform_point(&mut at, &world, p[0], p[1], p[2]));
            }
        }
    }
    for child in &node.children {
        visit(gltf, *child, &world, bounds, depth + 1);
    }
}

/// glTF is y-up and the world z-up, so the extent is `(dx, dz, dy)`, rounded
/// to two decimals.
pub fn extent(bytes: &[u8]) -> Result<[f64; 3], String> {
    let gltf: Gltf = serde_json::from_slice(json_chunk(bytes)?).map_err(|e| e.to_string())?;
    let mut bounds = Aabb::new();

    let scenes: Vec<&Scene> = match gltf.scene.and_then(|i| gltf.scenes.get(i)) {
        Some(s) => vec![s],
        None => gltf.scenes.iter().collect(),
    };
    let roots: Vec<usize> = scenes
        .iter()
        .flat_map(|s| s.nodes.iter().copied())
        .collect();
    for r in roots {
        visit(&gltf, r, &mat4::new(), &mut bounds, 0);
    }

    if bounds.lo[0] > bounds.hi[0] {
        return Err("no POSITION accessor with bounds".to_string());
    }
    let round = |v: f64| (v * 100.0).round_ties_even() / 100.0;
    Ok([
        round(bounds.hi[0] - bounds.lo[0]),
        round(bounds.hi[2] - bounds.lo[2]),
        round(bounds.hi[1] - bounds.lo[1]),
    ])
}
