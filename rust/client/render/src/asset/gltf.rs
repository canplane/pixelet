// pxl-render/asset/gltf.rs

//! A glTF 2.0 parser for what the shipped models use, output in the file's own
//! frame. It awaits `pause` between units (an accessor slice, a node, a skin, a
//! channel), so no decode runs long in one poll. The document is checked whole
//! before the first pause, so that poll is bounded only by the file's size,
//! which whatever accepts the file must bound.
//!
//! The bytes are not trusted. A file that reaches past its buffers, names an
//! index out of range, gives a node two parents or a cycle, would decode more
//! than `COMPS_PER_BYTE` of its size, or draws more than the `MAX_SCENE_*`
//! is refused as malformed or too large, and one that needs what is not read (a
//! sparse accessor, a required extension) as unsupported; none panics.

use core::cell::Cell;
use core::future::Future;

use pxl_spec::space::coord::mat4::{self, Mat4};

use xpute_kit::codec::json::{read_json, JsonValue};
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

/// Every attribute has the position's vertex count, and every index is below it.
#[derive(Clone, Debug, PartialEq)]
pub struct GltfPrimitive {
    pub pos: Vec<f32>,
    pub normal: Option<Vec<f32>>,
    pub uv: Option<Vec<f32>>,
    /// Four joint indices a vertex, into the mesh's skin; None when unskinned.
    pub joints: Option<Vec<u16>>,
    pub weights: Option<Vec<f32>>,
    /// Triangle list; generated as 0..n when the primitive has none.
    pub idx: Vec<u32>,
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
    pub vals: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GltfAnimation {
    pub duration: f64,
    pub channels: Vec<GltfChannel>,
}

/// Every index names an element that is there. The nodes are a forest: one
/// parent at most, no cycle, and `roots` distinct and parentless.
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

/// Decoded components a file may yield for each byte it and its buffers hold.
/// An accessor read twice is decoded twice, so without a bound a small file
/// names many times its size; the shipped models yield about one.
const COMPS_PER_BYTE: usize = 4;

/// The most the scene may draw, a mesh counted at every node that draws it: a
/// mesh is decoded once but laid out for each. Far above the shipped models
/// (3,836 vertices, 7,422 indices and 7 primitives at most); what an upload
/// may claim is for whatever accepts it to bound tighter.
pub const MAX_SCENE_VERTICES: u32 = 1 << 20;
pub const MAX_SCENE_IDXS: u32 = 1 << 22;
pub const MAX_SCENE_PRIMITIVES: u32 = 1 << 12;

fn comp_sz(t: usize) -> Option<usize> {
    match t {
        5120 | 5121 => Some(1),
        5122 | 5123 => Some(2),
        5125 | 5126 => Some(4),
        _ => None,
    }
}

fn type_sz(t: &str) -> Option<usize> {
    match t {
        "SCALAR" => Some(1),
        "VEC2" => Some(2),
        "VEC3" => Some(3),
        "VEC4" | "MAT2" => Some(4),
        "MAT3" => Some(9),
        "MAT4" => Some(16),
        _ => None,
    }
}

fn u32_le(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

#[track_caller]
fn malformed() -> MarshalError {
    MarshalError::new(Errno::EBADMSG)
}

/// Bounds one accessor slice between pauses.
const ELEMENTS_PER_PAUSE: usize = 1 << 12;

pub async fn parse_gltf<P: Future<Output = ()>>(
    bytes: &[u8],
    external_bufs: &[&[u8]],
    pause: impl Fn() -> P,
) -> Result<Gltf, MarshalError> {
    let budget = external_bufs
        .iter()
        .fold(bytes.len(), |n, b| n.saturating_add(b.len()))
        .saturating_mul(COMPS_PER_BYTE);
    if bytes.len() >= 12 && u32_le(bytes, 0) == GLB_MAGIC {
        let length = (u32_le(bytes, 8) as usize).min(bytes.len());
        let mut at = 12;
        let mut text: Option<&str> = None;
        let mut bin: Option<&[u8]> = None;
        while at + 8 <= length {
            let chunk_length = u32_le(bytes, at) as usize;
            let chunk_type = u32_le(bytes, at + 4);
            let end = (at + 8)
                .checked_add(chunk_length)
                .filter(|&end| end <= length)
                .ok_or_else(malformed)?;
            let data = &bytes[at + 8..end];
            if chunk_type == CHUNK_JSON {
                text = core::str::from_utf8(data).ok();
            } else if chunk_type == CHUNK_BIN && bin.is_none() {
                bin = Some(data);
            }
            at = end;
        }
        let text = text.ok_or_else(malformed)?;
        let json = read_json(text).ok_or_else(malformed)?;
        let mut bufs: Vec<&[u8]> = bin.into_iter().collect();
        bufs.extend_from_slice(external_bufs);
        read_document(json, &bufs, &pause, budget).await
    } else {
        let text = core::str::from_utf8(bytes).map_err(|_| malformed())?;
        let json = read_json(text).ok_or_else(malformed)?;
        read_document(json, external_bufs, &pause, budget).await
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

/// `v` as an index below `len`; none for a fraction, a negative, NaN or one
/// past it.
fn idx_below(v: f64, len: usize) -> Option<usize> {
    let i = v as usize;
    (v >= 0.0 && i as f64 == v && i < len).then_some(i)
}

/// A field that is an index below `len`, None when absent; malformed when it
/// is anything else.
fn idx_of(v: Option<JsonValue>, len: usize) -> Result<Option<usize>, MarshalError> {
    match v {
        None => Ok(None),
        Some(v) => v
            .as_f64()
            .and_then(|v| idx_below(v, len))
            .map(Some)
            .ok_or_else(malformed),
    }
}

/// A field that must be an index below `len`.
fn idx_in(v: Option<JsonValue>, len: usize) -> Result<usize, MarshalError> {
    idx_of(v, len)?.ok_or_else(malformed)
}

/// A count or a byte offset, `default` when absent.
fn sz(v: Option<JsonValue>, default: usize) -> Result<usize, MarshalError> {
    Ok(idx_of(v, usize::MAX)?.unwrap_or(default))
}

/// An accessor's fields, each looked up once: a lookup rescans the object, and
/// one accessor may be read for any number of primitives.
#[derive(Clone, Copy)]
struct AccessorFields<'a> {
    kind: Option<&'a str>,
    cnt: Option<JsonValue<'a>>,
    view: Option<JsonValue<'a>>,
    off: Option<JsonValue<'a>>,
    comp_type: Option<JsonValue<'a>>,
    normalized: bool,
    sparse: bool,
}

impl<'a> AccessorFields<'a> {
    fn of(a: JsonValue<'a>) -> Self {
        AccessorFields {
            kind: plain(a.get("type")),
            cnt: a.get("count"),
            view: a.get("bufferView"),
            off: a.get("byteOffset"),
            comp_type: a.get("componentType"),
            normalized: a.get("normalized").and_then(|n| n.as_bool()) == Some(true),
            sparse: a.get("sparse").is_some(),
        }
    }
}

/// A buffer view's fields, each looked up once, as an accessor's are.
#[derive(Clone, Copy)]
struct ViewFields<'a> {
    buf: Option<JsonValue<'a>>,
    off: Option<JsonValue<'a>>,
    length: Option<JsonValue<'a>>,
    stride: Option<JsonValue<'a>>,
}

impl<'a> ViewFields<'a> {
    fn of(v: JsonValue<'a>) -> Self {
        ViewFields {
            buf: v.get("buffer"),
            off: v.get("byteOffset"),
            length: v.get("byteLength"),
            stride: v.get("byteStride"),
        }
    }
}

/// A sampler's fields, each looked up once: any number of channels share one.
#[derive(Clone, Copy)]
struct SamplerFields<'a> {
    input: Option<JsonValue<'a>>,
    output: Option<JsonValue<'a>>,
    interpolation: &'a str,
}

impl<'a> SamplerFields<'a> {
    fn of(s: JsonValue<'a>) -> Self {
        SamplerFields {
            input: s.get("input"),
            output: s.get("output"),
            interpolation: plain(s.get("interpolation")).unwrap_or("LINEAR"),
        }
    }
}

struct Reader<'a, F> {
    accessors: Vec<AccessorFields<'a>>,
    views: Vec<ViewFields<'a>>,
    bufs: &'a [&'a [u8]],
    pause: &'a F,
    /// Components the file may still decode.
    budget: Cell<usize>,
}

impl<P: Future<Output = ()>, F: Fn() -> P> Reader<'_, F> {
    fn spend(&self, comps: usize) -> Result<(), MarshalError> {
        let left = self
            .budget
            .get()
            .checked_sub(comps)
            .ok_or_else(|| MarshalError::new(Errno::EFBIG))?;
        self.budget.set(left);
        Ok(())
    }

    /// Accessor `index`, which must be of `kind`, its components converted.
    async fn read<T: Copy + Default>(
        &self,
        idx: usize,
        kind: &str,
        convert: fn(f64) -> T,
    ) -> Result<Vec<T>, MarshalError> {
        (self.pause)().await;
        let a = *self.accessors.get(idx).ok_or_else(malformed)?;
        if a.sparse {
            return Err(MarshalError::new(Errno::ENOTSUP));
        }
        if a.kind != Some(kind) {
            return Err(malformed());
        }
        let width = type_sz(kind).ok_or_else(malformed)?;
        let cnt = idx_in(a.cnt, usize::MAX)?;
        let len = cnt.checked_mul(width).ok_or_else(malformed)?;
        self.spend(len)?;
        let mut out = vec![T::default(); len];
        // An accessor with no view is all zeros.
        let Some(bv_idx) = idx_of(a.view, self.views.len())? else {
            return Ok(out);
        };
        let bv = self.views[bv_idx];
        let buf = self.bufs[idx_in(bv.buf, self.bufs.len())?];
        let view_off = sz(bv.off, 0)?;
        let view_length = idx_in(bv.length, usize::MAX)?;
        let view = view_off
            .checked_add(view_length)
            .and_then(|end| buf.get(view_off..end))
            .ok_or_else(malformed)?;
        let comp_type = idx_in(a.comp_type, usize::MAX)?;
        let comp = comp_sz(comp_type).ok_or_else(malformed)?;
        let element = comp * width;
        let stride = sz(bv.stride, element)?;
        if stride < element {
            return Err(malformed());
        }
        let base = sz(a.off, 0)?;
        // The last element ends within the view, so no read below leaves it.
        if let Some(last) = cnt.checked_sub(1) {
            let end = last
                .checked_mul(stride)
                .and_then(|at| at.checked_add(base))
                .and_then(|at| at.checked_add(element));
            if end.is_none_or(|end| end > view.len()) {
                return Err(malformed());
            }
        }
        let norm = a.normalized;
        for e in 0..cnt {
            if e > 0 && e % ELEMENTS_PER_PAUSE == 0 {
                (self.pause)().await;
            }
            for c in 0..width {
                let at = base + e * stride + c * comp;
                let v = match comp_type {
                    5120 => {
                        let v = view[at] as i8 as f64;
                        if norm {
                            (v / 127.0).max(-1.0)
                        } else {
                            v
                        }
                    }
                    5121 => {
                        let v = view[at] as f64;
                        if norm {
                            v / 255.0
                        } else {
                            v
                        }
                    }
                    5122 => {
                        let v = i16::from_le_bytes([view[at], view[at + 1]]) as f64;
                        if norm {
                            (v / 32767.0).max(-1.0)
                        } else {
                            v
                        }
                    }
                    5123 => {
                        let v = u16::from_le_bytes([view[at], view[at + 1]]) as f64;
                        if norm {
                            v / 65535.0
                        } else {
                            v
                        }
                    }
                    5125 => u32_le(view, at) as f64,
                    _ => f32::from_le_bytes([view[at], view[at + 1], view[at + 2], view[at + 3]])
                        as f64,
                };
                out[e * width + c] = convert(v);
            }
        }
        Ok(out)
    }

    async fn f32s(&self, idx: usize, kind: &str) -> Result<Vec<f32>, MarshalError> {
        self.read(idx, kind, |v| v as f32).await
    }

    /// A vertex attribute, `count` elements as the position has.
    async fn attribute<T: Copy + Default>(
        &self,
        idx: Option<usize>,
        kind: &str,
        cnt: usize,
        convert: fn(f64) -> T,
    ) -> Result<Option<Vec<T>>, MarshalError> {
        let Some(idx) = idx else {
            return Ok(None);
        };
        let vals = self.read(idx, kind, convert).await?;
        if Some(vals.len()) != type_sz(kind).map(|w| w * cnt) {
            return Err(malformed());
        }
        Ok(Some(vals))
    }
}

async fn read_document<P: Future<Output = ()>, F: Fn() -> P>(
    json: JsonValue<'_>,
    bufs: &[&[u8]],
    pause: &F,
    budget: usize,
) -> Result<Gltf, MarshalError> {
    if arr(json.get("extensionsRequired")).next().is_some() {
        return Err(MarshalError::new(Errno::ENOTSUP));
    }
    let r = Reader {
        accessors: arr(json.get("accessors")).map(AccessorFields::of).collect(),
        views: arr(json.get("bufferViews")).map(ViewFields::of).collect(),
        bufs,
        pause,
        budget: Cell::new(budget),
    };

    let mut meshes = Vec::new();
    for m in arr(json.get("meshes")) {
        let mut prims = Vec::new();
        for p in arr(m.get("primitives")) {
            let attributes = p.get("attributes");
            let attr = |name: &str| idx_of(attributes.and_then(|a| a.get(name)), r.accessors.len());
            if num(p.get("mode")).unwrap_or(4.0) != 4.0 {
                continue;
            }
            let Some(pos_at) = attr("POSITION")? else {
                continue;
            };
            let pos = r.f32s(pos_at, "VEC3").await?;
            let cnt = pos.len() / 3;
            let idx = match idx_of(p.get("indices"), r.accessors.len())? {
                Some(i) => r.read(i, "SCALAR", |v| v as u32).await?,
                None => {
                    r.spend(cnt)?;
                    (0..u32::try_from(cnt).map_err(|_| malformed())?).collect()
                }
            };
            for (k, slice) in idx.chunks(ELEMENTS_PER_PAUSE).enumerate() {
                if k > 0 {
                    pause().await;
                }
                if slice.iter().any(|&i| i as usize >= cnt) {
                    return Err(malformed());
                }
            }
            prims.push(GltfPrimitive {
                normal: r
                    .attribute(attr("NORMAL")?, "VEC3", cnt, |v| v as f32)
                    .await?,
                uv: r
                    .attribute(attr("TEXCOORD_0")?, "VEC2", cnt, |v| v as f32)
                    .await?,
                joints: r
                    .attribute(attr("JOINTS_0")?, "VEC4", cnt, |v| v as u16)
                    .await?,
                weights: r
                    .attribute(attr("WEIGHTS_0")?, "VEC4", cnt, |v| v as f32)
                    .await?,
                pos,
                idx,
                material: num(p.get("material")).map_or(-1, |v| v as i32),
            });
        }
        meshes.push(prims);
    }

    let raw_nodes: Vec<JsonValue> = arr(json.get("nodes")).collect();
    let n = raw_nodes.len();
    if n > i32::MAX as usize {
        return Err(malformed());
    }
    let skin_cnt = arr(json.get("skins")).count();
    let mut parents = vec![-1i32; n];
    let mut children = Vec::with_capacity(n);
    for (i, node) in raw_nodes.iter().enumerate() {
        let mut own = Vec::new();
        for c in arr(node.get("children")) {
            let c = idx_in(Some(c), n)?;
            if c == i || parents[c] >= 0 {
                return Err(malformed());
            }
            parents[c] = i as i32;
            own.push(c as u32);
        }
        children.push(own);
    }
    // With one parent each, a node is on a cycle exactly when no walk down from
    // a parentless node reaches it.
    let mut reached = 0;
    let mut stack: Vec<usize> = (0..n).filter(|&i| parents[i] < 0).collect();
    while let Some(i) = stack.pop() {
        reached += 1;
        stack.extend(children[i].iter().map(|&c| c as usize));
    }
    if reached != n {
        return Err(malformed());
    }
    let mut nodes = Vec::with_capacity(n);
    for (i, (node, children)) in raw_nodes.iter().zip(children).enumerate() {
        pause().await;
        let translation = floats(node.get("translation"), [0.0; 3]);
        let rotation = floats(node.get("rotation"), [0.0, 0.0, 0.0, 1.0]);
        let scale = floats(node.get("scale"), [1.0; 3]);
        let mut local = mat4::new();
        if node.get("matrix").is_some() {
            local = floats(node.get("matrix"), local);
        } else {
            compose_trs(&mut local, &translation, &rotation, &scale);
        }
        nodes.push(GltfNode {
            parent: parents[i],
            children,
            mesh: idx_of(node.get("mesh"), meshes.len())?.map_or(-1, |m| m as i32),
            skin: idx_of(node.get("skin"), skin_cnt)?.map_or(-1, |s| s as i32),
            local,
            translation,
            rotation,
            scale,
        });
    }

    let mut skins = Vec::new();
    for s in arr(json.get("skins")) {
        pause().await;
        let joints = arr(s.get("joints"))
            .map(|j| idx_in(Some(j), n).map(|j| j as u32))
            .collect::<Result<Vec<u32>, _>>()?;
        let inverse_bind = match idx_of(s.get("inverseBindMatrices"), r.accessors.len())? {
            Some(i) => {
                let matrices = r.f32s(i, "MAT4").await?;
                // Each joint has its matrix; any past them are unread.
                if matrices.len() < joints.len() * 16 {
                    return Err(malformed());
                }
                matrices
            }
            None => {
                r.spend(joints.len() * 16)?;
                joints
                    .iter()
                    .flat_map(|_| {
                        [
                            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
                            0.0, 1.0,
                        ]
                    })
                    .collect()
            }
        };
        skins.push(GltfSkin {
            joints,
            inverse_bind,
        });
    }

    let mut animations = Vec::new();
    for a in arr(json.get("animations")) {
        let samplers: Vec<SamplerFields> = arr(a.get("samplers")).map(SamplerFields::of).collect();
        let mut duration = 0.0f64;
        let mut channels = Vec::new();
        for c in arr(a.get("channels")) {
            pause().await;
            let Some(target) = c.get("target") else {
                continue;
            };
            // A channel with no node animates what an extension names.
            let Some(node) = idx_of(target.get("node"), n)? else {
                continue;
            };
            let path = match plain(target.get("path")) {
                Some("translation") => GltfPath::Translation,
                Some("rotation") => GltfPath::Rotation,
                Some("scale") => GltfPath::Scale,
                _ => continue,
            };
            let sampler = samplers[idx_in(c.get("sampler"), samplers.len())?];
            let (width, kind) = if path == GltfPath::Rotation {
                (4, "VEC4")
            } else {
                (3, "VEC3")
            };
            let mut times = r
                .f32s(idx_in(sampler.input, r.accessors.len())?, "SCALAR")
                .await?;
            let mut vals = r
                .f32s(idx_in(sampler.output, r.accessors.len())?, kind)
                .await?;
            let interpolation = sampler.interpolation;
            if interpolation == "CUBICSPLINE" {
                // In-tangent, value, out-tangent: keep the value.
                vals = (0..times.len())
                    .filter_map(|t| vals.get(t * width * 3 + width..t * width * 3 + width * 2))
                    .flatten()
                    .copied()
                    .collect();
            }
            // An output shorter than its input is read for the keys it has, not past them.
            let keys = times.len().min(vals.len() / width);
            times.truncate(keys);
            vals.truncate(keys * width);
            if let Some(&last) = times.last() {
                duration = duration.max(last as f64);
            }
            channels.push(GltfChannel {
                node: node as u32,
                path,
                step: interpolation == "STEP",
                times,
                vals,
            });
        }
        animations.push(GltfAnimation { duration, channels });
    }

    let scenes: Vec<JsonValue> = arr(json.get("scenes")).collect();
    let scene = match idx_of(json.get("scene"), scenes.len())? {
        Some(s) => Some(scenes[s]),
        None => scenes.first().copied(),
    };
    let roots = match scene.and_then(|s| s.get("nodes")) {
        Some(list) => {
            let mut listed = vec![false; n];
            let mut roots = Vec::new();
            for v in arr(Some(list)) {
                let i = idx_in(Some(v), n)?;
                // Parentless and listed once, so no node is walked twice.
                if parents[i] >= 0 || listed[i] {
                    return Err(malformed());
                }
                listed[i] = true;
                roots.push(i as u32);
            }
            roots
        }
        None => (0..n as u32).filter(|&i| parents[i as usize] < 0).collect(),
    };
    let gltf = Gltf {
        nodes,
        meshes,
        skins,
        animations,
        roots,
    };
    // Each mesh summed once, so a node costs one addition however many
    // primitives its mesh has.
    let per_mesh: Vec<[usize; 3]> = gltf
        .meshes
        .iter()
        .map(|prims| {
            prims.iter().fold([0, 0, prims.len()], |[v, i, p], prim| {
                [v + prim.pos.len() / 3, i + prim.idx.len(), p]
            })
        })
        .collect();
    let mut drawn = [0usize; 3];
    for i in scene_nodes(&gltf) {
        let mesh = gltf.nodes[i as usize].mesh;
        if mesh >= 0 {
            for (sum, add) in drawn.iter_mut().zip(per_mesh[mesh as usize]) {
                *sum = sum.saturating_add(add);
            }
        }
    }
    let most = [MAX_SCENE_VERTICES, MAX_SCENE_IDXS, MAX_SCENE_PRIMITIVES];
    if drawn.iter().zip(most).any(|(&d, m)| d > m as usize) {
        return Err(MarshalError::new(Errno::EFBIG));
    }
    Ok(gltf)
}

/// T × R × S, column-major, from a translation, a unit quaternion and a scale.
pub fn compose_trs<'a>(
    out: &'a mut Mat4,
    t: &[f64; 3],
    q: &[f64; 4],
    s: &[f64; 3],
) -> &'a mut Mat4 {
    let [x, y, z, w] = *q;
    let (xx, yy, zz, xy, xz, yz, wx, wy, wz) = (
        x * x,
        y * y,
        z * z,
        x * y,
        x * z,
        y * z,
        w * x,
        w * y,
        w * z,
    );
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

/// Every node's world matrix at rest; identity for a node off the scene.
pub fn world_matrices(gltf: &Gltf) -> Vec<Mat4> {
    let mut out = vec![mat4::new(); gltf.nodes.len()];
    // Parents come first, so a parent's world is in place for its children.
    for i in scene_nodes(gltf) {
        let node = &gltf.nodes[i as usize];
        out[i as usize] = match node.parent {
            p if p >= 0 => {
                let mut m = mat4::new();
                mat4::mul(&mut m, &out[p as usize], &node.local);
                m
            }
            _ => node.local,
        };
    }
    out
}

/// The nodes reachable from the scene's roots, parents before children, each
/// before its next sibling's subtree. Walked on a stack of its own, so a deep
/// chain cannot overflow the module's fixed one.
pub fn scene_nodes(gltf: &Gltf) -> Vec<u32> {
    let mut out = Vec::new();
    let mut stack: Vec<u32> = gltf.roots.iter().rev().copied().collect();
    while let Some(i) = stack.pop() {
        out.push(i);
        stack.extend(gltf.nodes[i as usize].children.iter().rev());
    }
    out
}

#[cfg(test)]
#[path = "gltf.test.rs"]
mod test;
