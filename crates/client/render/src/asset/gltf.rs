// pxl-render/asset/gltf.rs

//! A glTF 2.0 parser for what the shipped models use, output in the file's own
//! frame. It awaits `pause` between units (an accessor slice, a node, a skin, a
//! channel), so a file's size never bounds how long one poll runs.

use core::future::Future;

use pxl_spec::space::coord::mat4::{mat4, mat4_mul, Mat4};

use xpute_core::codec::json::{read_json, JsonValue};
use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;

#[derive(Clone, Debug, PartialEq)]
pub struct GltfPrimitive {
    pub position: Vec<f32>,
    pub normal: Option<Vec<f32>>,
    pub uv: Option<Vec<f32>>,
    /// Four joint indices a vertex, into the mesh's skin; None when unskinned.
    pub joints: Option<Vec<u16>>,
    pub weights: Option<Vec<f32>>,
    /// Triangle list; generated as 0..n when the primitive has none.
    pub index: Vec<u32>,
    pub material: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GltfNode {
    pub parent: i32,
    pub children: Vec<u32>,
    pub mesh: i32,
    pub skin: i32,
    pub local: Mat4,
    pub translation: [f64; 3],
    pub rotation: [f64; 4],
    pub scale: [f64; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct GltfSkin {
    pub joints: Vec<u32>,
    /// Sixteen floats a joint, column-major; identity when the file gives none.
    pub inverse_bind: Vec<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GltfPath {
    Translation = 0,
    Rotation = 1,
    Scale = 2,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GltfChannel {
    pub node: u32,
    pub path: GltfPath,
    pub step: bool,
    pub times: Vec<f32>,
    /// A cubic spline's tangents are dropped.
    pub values: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GltfAnimation {
    pub duration: f64,
    pub channels: Vec<GltfChannel>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Gltf {
    pub nodes: Vec<GltfNode>,
    pub meshes: Vec<Vec<GltfPrimitive>>,
    pub skins: Vec<GltfSkin>,
    pub animations: Vec<GltfAnimation>,
    pub roots: Vec<u32>,
}

const GLB_MAGIC: u32 = 0x4654_6c67;
const CHUNK_JSON: u32 = 0x4e4f_534a;
const CHUNK_BIN: u32 = 0x004e_4942;

fn component_size(t: u32) -> usize {
    match t {
        5120 | 5121 => 1,
        5122 | 5123 => 2,
        _ => 4,
    }
}

fn type_size(t: &str) -> usize {
    match t {
        "SCALAR" => 1,
        "VEC2" => 2,
        "VEC3" => 3,
        "VEC4" | "MAT2" => 4,
        "MAT3" => 9,
        "MAT4" => 16,
        _ => 1,
    }
}

fn u32_le(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// Bounds one accessor slice between pauses.
const ELEMENTS_PER_PAUSE: usize = 1 << 12;

pub async fn parse_gltf<P: Future<Output = ()>>(bytes: &[u8], external_buffers: &[&[u8]], pause: impl Fn() -> P) -> Result<Gltf, MarshalError> {
    if bytes.len() >= 12 && u32_le(bytes, 0) == GLB_MAGIC {
        let length = (u32_le(bytes, 8) as usize).min(bytes.len());
        let mut at = 12;
        let mut text: Option<&str> = None;
        let mut bin: Option<&[u8]> = None;
        while at + 8 <= length {
            let chunk_length = u32_le(bytes, at) as usize;
            let chunk_type = u32_le(bytes, at + 4);
            let end = (at + 8 + chunk_length).min(bytes.len());
            let data = &bytes[at + 8..end];
            if chunk_type == CHUNK_JSON {
                text = core::str::from_utf8(data).ok();
            } else if chunk_type == CHUNK_BIN && bin.is_none() {
                bin = Some(data);
            }
            at += 8 + chunk_length;
        }
        let text = text.ok_or(MarshalError::new(Errno::EBADMSG))?;
        let json = read_json(text).ok_or(MarshalError::new(Errno::EBADMSG))?;
        let mut buffers: Vec<&[u8]> = bin.into_iter().collect();
        buffers.extend_from_slice(external_buffers);
        read_document(json, &buffers, &pause).await
    } else {
        let text = core::str::from_utf8(bytes).map_err(|_| MarshalError::new(Errno::EBADMSG))?;
        let json = read_json(text).ok_or(MarshalError::new(Errno::EBADMSG))?;
        read_document(json, external_buffers, &pause).await
    }
}

fn num(v: Option<JsonValue>) -> Option<f64> {
    v?.as_f64()
}

fn arr<'a>(v: Option<JsonValue<'a>>) -> impl Iterator<Item = JsonValue<'a>> {
    v.into_iter().flat_map(|v| v.items())
}

fn plain<'a>(v: Option<JsonValue<'a>>) -> Option<&'a str> {
    v?.as_str()?.as_plain()
}

fn floats<const N: usize>(v: Option<JsonValue>, default: [f64; N]) -> [f64; N] {
    if arr(v).count() < N {
        return default;
    }
    let mut out = default;
    for (o, item) in out.iter_mut().zip(arr(v)) {
        *o = item.as_f64().unwrap_or(*o);
    }
    out
}

struct Reader<'a, F> {
    json: JsonValue<'a>,
    buffers: &'a [&'a [u8]],
    pause: &'a F,
}

impl<P: Future<Output = ()>, F: Fn() -> P> Reader<'_, F> {
    async fn read<T: Copy + Default>(&self, index: usize, convert: fn(f64) -> T) -> Result<Vec<T>, MarshalError> {
        (self.pause)().await;
        let a = self.json.get("accessors").and_then(|x| x.at(index)).ok_or_else(|| MarshalError::new(Errno::ENOENT))?;
        let count = num(a.get("count")).unwrap_or(0.0) as usize;
        let width = type_size(plain(a.get("type")).unwrap_or("SCALAR"));
        let mut out = vec![T::default(); count * width];
        if let Some(bv_index) = num(a.get("bufferView")) {
            let bv = self.json.get("bufferViews").and_then(|x| x.at(bv_index as usize)).ok_or_else(|| MarshalError::new(Errno::ENOENT))?;
            let buffer_index = num(bv.get("buffer")).unwrap_or(0.0) as usize;
            let buffer = *self.buffers.get(buffer_index).ok_or_else(|| MarshalError::new(Errno::ENOENT))?;
            let component_type = num(a.get("componentType")).unwrap_or(5126.0) as u32;
            let comp = component_size(component_type);
            let stride = num(bv.get("byteStride")).map_or(comp * width, |s| s as usize);
            let base = num(bv.get("byteOffset")).unwrap_or(0.0) as usize + num(a.get("byteOffset")).unwrap_or(0.0) as usize;
            let norm = a.get("normalized").and_then(|n| n.as_bool()) == Some(true);
            for e in 0..count {
                if e > 0 && e % ELEMENTS_PER_PAUSE == 0 {
                    (self.pause)().await;
                }
                for c in 0..width {
                    let at = base + e * stride + c * comp;
                    let v = match component_type {
                        5120 => {
                            let v = buffer[at] as i8 as f64;
                            if norm {
                                (v / 127.0).max(-1.0)
                            } else {
                                v
                            }
                        }
                        5121 => {
                            let v = buffer[at] as f64;
                            if norm {
                                v / 255.0
                            } else {
                                v
                            }
                        }
                        5122 => {
                            let v = i16::from_le_bytes([buffer[at], buffer[at + 1]]) as f64;
                            if norm {
                                (v / 32767.0).max(-1.0)
                            } else {
                                v
                            }
                        }
                        5123 => {
                            let v = u16::from_le_bytes([buffer[at], buffer[at + 1]]) as f64;
                            if norm {
                                v / 65535.0
                            } else {
                                v
                            }
                        }
                        5125 => u32_le(buffer, at) as f64,
                        _ => f32::from_le_bytes([buffer[at], buffer[at + 1], buffer[at + 2], buffer[at + 3]]) as f64,
                    };
                    out[e * width + c] = convert(v);
                }
            }
        }
        // Sparse accessors read as their dense base; the shipped models have none.
        Ok(out)
    }

    async fn f32s(&self, index: usize) -> Result<Vec<f32>, MarshalError> {
        self.read(index, |v| v as f32).await
    }

    async fn u32s(&self, index: usize) -> Result<Vec<u32>, MarshalError> {
        self.read(index, |v| v as u32).await
    }

    async fn u16s(&self, index: usize) -> Result<Vec<u16>, MarshalError> {
        self.read(index, |v| v as u16).await
    }

    async fn f32s_of(&self, index: Option<usize>) -> Result<Option<Vec<f32>>, MarshalError> {
        match index {
            Some(i) => self.f32s(i).await.map(Some),
            None => Ok(None),
        }
    }
}

async fn read_document<P: Future<Output = ()>, F: Fn() -> P>(json: JsonValue<'_>, buffers: &[&[u8]], pause: &F) -> Result<Gltf, MarshalError> {
    let r = Reader { json, buffers, pause };

    let mut meshes = Vec::new();
    for m in arr(json.get("meshes")) {
        let mut prims = Vec::new();
        for p in arr(m.get("primitives")) {
            let attributes = p.get("attributes");
            let attr = |name: &str| attributes.and_then(|a| num(a.get(name))).map(|v| v as usize);
            if num(p.get("mode")).unwrap_or(4.0) != 4.0 {
                continue;
            }
            let Some(position_at) = attr("POSITION") else { continue };
            let position = r.f32s(position_at).await?;
            let count = position.len() / 3;
            let index = match num(p.get("indices")) {
                Some(i) => r.u32s(i as usize).await?,
                None => (0..count as u32).collect(),
            };
            let joints = match attr("JOINTS_0") {
                Some(i) => Some(r.u16s(i).await?),
                None => None,
            };
            prims.push(GltfPrimitive {
                normal: r.f32s_of(attr("NORMAL")).await?,
                uv: r.f32s_of(attr("TEXCOORD_0")).await?,
                joints,
                weights: r.f32s_of(attr("WEIGHTS_0")).await?,
                position,
                index,
                material: num(p.get("material")).map_or(-1, |v| v as i32),
            });
        }
        meshes.push(prims);
    }

    let raw_nodes: Vec<JsonValue> = arr(json.get("nodes")).collect();
    let mut parents = vec![-1i32; raw_nodes.len()];
    for (i, n) in raw_nodes.iter().enumerate() {
        for c in arr(n.get("children")) {
            if let Some(c) = c.as_f64() {
                parents[c as usize] = i as i32;
            }
        }
    }
    let mut nodes = Vec::with_capacity(raw_nodes.len());
    for (i, n) in raw_nodes.iter().enumerate() {
        pause().await;
        let translation = floats(n.get("translation"), [0.0; 3]);
        let rotation = floats(n.get("rotation"), [0.0, 0.0, 0.0, 1.0]);
        let scale = floats(n.get("scale"), [1.0; 3]);
        let mut local = mat4();
        if n.get("matrix").is_some() {
            local = floats(n.get("matrix"), local);
        } else {
            compose_trs(&mut local, &translation, &rotation, &scale);
        }
        nodes.push(GltfNode {
            parent: parents[i],
            children: arr(n.get("children")).filter_map(|c| c.as_f64()).map(|c| c as u32).collect(),
            mesh: num(n.get("mesh")).map_or(-1, |v| v as i32),
            skin: num(n.get("skin")).map_or(-1, |v| v as i32),
            local,
            translation,
            rotation,
            scale,
        });
    }

    let mut skins = Vec::new();
    for s in arr(json.get("skins")) {
        pause().await;
        let joints: Vec<u32> = arr(s.get("joints")).filter_map(|j| j.as_f64()).map(|j| j as u32).collect();
        let inverse_bind = match num(s.get("inverseBindMatrices")) {
            Some(i) => r.f32s(i as usize).await?,
            None => joints.iter().flat_map(|_| [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]).collect(),
        };
        skins.push(GltfSkin { joints, inverse_bind });
    }

    let mut animations = Vec::new();
    for a in arr(json.get("animations")) {
        let mut duration = 0.0f64;
        let mut channels = Vec::new();
        for c in arr(a.get("channels")) {
            pause().await;
            let Some(target) = c.get("target") else { continue };
            let Some(node) = num(target.get("node")) else { continue };
            let path = match plain(target.get("path")) {
                Some("translation") => GltfPath::Translation,
                Some("rotation") => GltfPath::Rotation,
                Some("scale") => GltfPath::Scale,
                _ => continue,
            };
            let Some(sampler) = a.get("samplers").and_then(|s| s.at(num(c.get("sampler")).unwrap_or(0.0) as usize)) else {
                continue;
            };
            let mut times = r.f32s(num(sampler.get("input")).unwrap_or(0.0) as usize).await?;
            let mut values = r.f32s(num(sampler.get("output")).unwrap_or(0.0) as usize).await?;
            let width = if path == GltfPath::Rotation { 4 } else { 3 };
            let interpolation = plain(sampler.get("interpolation")).unwrap_or("LINEAR");
            if interpolation == "CUBICSPLINE" {
                // In-tangent, value, out-tangent: keep the value.
                values = (0..times.len())
                    .filter_map(|t| values.get(t * width * 3 + width..t * width * 3 + width * 2))
                    .flatten()
                    .copied()
                    .collect();
            }
            // An output shorter than its input is read for the keys it has, not past them.
            let keys = times.len().min(values.len() / width);
            times.truncate(keys);
            values.truncate(keys * width);
            if let Some(&last) = times.last() {
                duration = duration.max(last as f64);
            }
            channels.push(GltfChannel {
                node: node as u32,
                path,
                step: interpolation == "STEP",
                times,
                values,
            });
        }
        animations.push(GltfAnimation { duration, channels });
    }

    let scene_index = num(json.get("scene")).unwrap_or(0.0) as usize;
    let scene = json.get("scenes").and_then(|s| s.at(scene_index));
    let roots = match scene.and_then(|s| s.get("nodes")) {
        Some(n) => arr(Some(n)).filter_map(|v| v.as_f64()).map(|v| v as u32).collect(),
        None => (0..raw_nodes.len() as u32).filter(|&i| parents[i as usize] < 0).collect(),
    };
    Ok(Gltf {
        nodes,
        meshes,
        skins,
        animations,
        roots,
    })
}

/// T × R × S, column-major, from a translation, a unit quaternion and a scale.
pub fn compose_trs<'a>(out: &'a mut Mat4, t: &[f64; 3], q: &[f64; 4], s: &[f64; 3]) -> &'a mut Mat4 {
    let [x, y, z, w] = *q;
    let (xx, yy, zz, xy, xz, yz, wx, wy, wz) = (x * x, y * y, z * z, x * y, x * z, y * z, w * x, w * y, w * z);
    *out = [
        (1.0 - 2.0 * (yy + zz)) * s[0],
        2.0 * (xy + wz) * s[0],
        2.0 * (xz - wy) * s[0],
        0.0,
        2.0 * (xy - wz) * s[1],
        (1.0 - 2.0 * (xx + zz)) * s[1],
        2.0 * (yz + wx) * s[1],
        0.0,
        2.0 * (xz + wy) * s[2],
        2.0 * (yz - wx) * s[2],
        (1.0 - 2.0 * (xx + yy)) * s[2],
        0.0,
        t[0],
        t[1],
        t[2],
        1.0,
    ];
    out
}

/// Every node's world matrix at rest, parents before children.
pub fn world_matrices(gltf: &Gltf) -> Vec<Mat4> {
    fn visit(gltf: &Gltf, out: &mut [Mat4], i: usize, parent: Option<Mat4>) {
        match parent {
            Some(p) => {
                let mut m = mat4();
                mat4_mul(&mut m, &p, &gltf.nodes[i].local);
                out[i] = m;
            }
            None => out[i] = gltf.nodes[i].local,
        }
        let here = out[i];
        for &c in &gltf.nodes[i].children {
            visit(gltf, out, c as usize, Some(here));
        }
    }
    let mut out = vec![mat4(); gltf.nodes.len()];
    for &r in &gltf.roots {
        visit(gltf, &mut out, r as usize, None);
    }
    out
}

/// The nodes reachable from the scene's roots, parents before children.
pub fn scene_nodes(gltf: &Gltf) -> Vec<u32> {
    fn visit(gltf: &Gltf, out: &mut Vec<u32>, i: u32) {
        out.push(i);
        for &c in &gltf.nodes[i as usize].children {
            visit(gltf, out, c);
        }
    }
    let mut out = Vec::new();
    for &r in &gltf.roots {
        visit(gltf, &mut out, r);
    }
    out
}
