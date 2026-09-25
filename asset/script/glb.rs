// ingest-asset/glb.rs

//! A model's size, measured from the file rather than typed into a catalog.
//!
//! The extent is the axis-aligned bounds over every node of every scene, node
//! transforms applied, read from the POSITION accessors' own `min`/`max` —
//! which glTF requires a writer to provide. That is the same bounds a runtime
//! loader computes, so the number the range criterion reads is the one the
//! mesh actually has.
//!
//! glTF is y-up and the world is ENU (x, y, z-up), so the extent is recorded
//! as `(dx, dz, dy)`.
//!
//! Only the JSON chunk is read. The buffers are never decoded: an accessor's
//! `min`/`max` is what bounds the mesh, and reading a few hundred thousand
//! vertices to rediscover it would be slower and no more correct.

use pxl_spec::space::mat4::{mat4_identity, mat4_mul, mat4_transform_point, Mat4};
use pxl_spec::space::v3::V3;
use serde::Deserialize;

#[derive(Deserialize)]
struct Gltf {
    #[serde(default)]
    scenes: Vec<Scene>,
    /// The scene a viewer opens. Only this one is measured when it is named —
    /// a file with more than one scene carries variants of the same model,
    /// and taking them all would bound the union rather than the model.
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
    matrix: Option<Vec<f64>>,
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
    attributes: std::collections::HashMap<String, usize>,
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
    bytes.get(20..20 + len).ok_or_else(|| "truncated JSON chunk".to_string())
}

/// A node's local transform: its `matrix` when it has one, else its TRS.
/// glTF's rotation is a quaternion in `(x, y, z, w)`.
fn local(node: &Node) -> Mat4 {
    if let Some(m) = &node.matrix {
        let mut out = [0.0; 16];
        for (i, v) in m.iter().take(16).enumerate() {
            out[i] = *v;
        }
        return out;
    }
    let t = node.translation.unwrap_or([0.0; 3]);
    let [x, y, z, w] = node.rotation.unwrap_or([0.0, 0.0, 0.0, 1.0]);
    let s = node.scale.unwrap_or([1.0; 3]);
    let (xx, yy, zz) = (x * x, y * y, z * z);
    let (xy, xz, yz) = (x * y, x * z, y * z);
    let (wx, wy, wz) = (w * x, w * y, w * z);
    let mut m = [0.0; 16];
    m[0] = (1.0 - 2.0 * (yy + zz)) * s[0];
    m[1] = (2.0 * (xy + wz)) * s[0];
    m[2] = (2.0 * (xz - wy)) * s[0];
    m[4] = (2.0 * (xy - wz)) * s[1];
    m[5] = (1.0 - 2.0 * (xx + zz)) * s[1];
    m[6] = (2.0 * (yz + wx)) * s[1];
    m[8] = (2.0 * (xz + wy)) * s[2];
    m[9] = (2.0 * (yz - wx)) * s[2];
    m[10] = (1.0 - 2.0 * (xx + yy)) * s[2];
    m[12] = t[0];
    m[13] = t[1];
    m[14] = t[2];
    m[15] = 1.0;
    m
}

struct Bounds {
    lo: [f64; 3],
    hi: [f64; 3],
}

impl Bounds {
    fn new() -> Bounds {
        Bounds {
            lo: [f64::INFINITY; 3],
            hi: [f64::NEG_INFINITY; 3],
        }
    }
    fn add(&mut self, p: &V3) {
        for (i, &v) in p.iter().enumerate() {
            self.lo[i] = self.lo[i].min(v);
            self.hi[i] = self.hi[i].max(v);
        }
    }
}

fn visit(gltf: &Gltf, index: usize, parent: &Mat4, bounds: &mut Bounds, depth: usize) {
    // A scene graph is a tree in every file this reads, but glTF's own schema
    // does not forbid a cycle, and a cycle here would not terminate.
    if depth > 64 {
        return;
    }
    let Some(node) = gltf.nodes.get(index) else { return };
    let mut world = [0.0; 16];
    mat4_mul(&mut world, parent, &local(node));

    if let Some(mesh) = node.mesh.and_then(|m| gltf.meshes.get(m)) {
        for primitive in &mesh.primitives {
            let Some(accessor) = primitive.attributes.get("POSITION").and_then(|a| gltf.accessors.get(*a)) else {
                continue;
            };
            let (Some(min), Some(max)) = (&accessor.min, &accessor.max) else { continue };
            if min.len() < 3 || max.len() < 3 {
                continue;
            }
            // Every corner of the accessor's own box, because a rotation
            // takes a box's corners somewhere its min and max do not
            // describe.
            for corner in 0..8u8 {
                let p = [
                    if corner & 1 != 0 { max[0] } else { min[0] },
                    if corner & 2 != 0 { max[1] } else { min[1] },
                    if corner & 4 != 0 { max[2] } else { min[2] },
                ];
                let mut w = [0.0; 3];
                mat4_transform_point(&mut w, &world, p[0], p[1], p[2]);
                bounds.add(&w);
            }
        }
    }
    for child in &node.children {
        visit(gltf, *child, &world, bounds, depth + 1);
    }
}

/// The glb's extent in ENU world units: `(dx, dz, dy)` of its world-space
/// bounds, each rounded to two decimals the way a catalog reads it.
pub fn extent(bytes: &[u8]) -> Result<[f64; 3], String> {
    let gltf: Gltf = serde_json::from_slice(json_chunk(bytes)?).map_err(|e| e.to_string())?;
    let mut bounds = Bounds::new();
    let mut root = [0.0; 16];
    mat4_identity(&mut root);

    let scenes: Vec<&Scene> = match gltf.scene.and_then(|i| gltf.scenes.get(i)) {
        Some(s) => vec![s],
        None => gltf.scenes.iter().collect(),
    };
    let roots: Vec<usize> = scenes.iter().flat_map(|s| s.nodes.iter().copied()).collect();
    for r in roots {
        visit(&gltf, r, &root, &mut bounds, 0);
    }

    if bounds.lo[0] > bounds.hi[0] {
        return Err("no POSITION accessor with bounds".to_string());
    }
    let round = |v: f64| (v * 100.0).round() / 100.0;
    Ok([round(bounds.hi[0] - bounds.lo[0]), round(bounds.hi[2] - bounds.lo[2]), round(bounds.hi[1] - bounds.lo[1])])
}

#[cfg(test)]
#[path = "glb.test.rs"]
mod test;
