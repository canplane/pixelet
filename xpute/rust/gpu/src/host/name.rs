// xpute-gpu/host/name.rs

//! Each enumeration's number as wgpu's value, by its WebGPU name.

use serde::de::value::{Error as NameError, StrDeserializer};
use serde::de::{DeserializeOwned, IntoDeserializer};

use crate::enums::{
    ADDRESS_MODE, BLEND_FACTOR, BLEND_OPERATION, COMPARE_FUNCTION, FILTER_MODE, FRONT_FACE,
    INDEX_FORMAT, MIPMAP_FILTER_MODE, PRIMITIVE_TOPOLOGY, SAMPLER_BINDING_TYPE, STENCIL_OPERATION,
    STORAGE_TEXTURE_ACCESS, TEXTURE_ASPECT, TEXTURE_DIMENSION, TEXTURE_FORMAT,
    TEXTURE_VIEW_DIMENSION, VERTEX_FORMAT, VERTEX_STEP_MODE,
};
use crate::spec::{BUFFER_USAGE, COLOR_WRITE, SHADER_STAGE, TEXTURE_USAGE};

/// By place in the spec's list, read through wgpu's serde names; none for a
/// name wgpu gives no value of.
pub(super) struct Names {
    pub(super) texture_format: Vec<Option<wgpu::TextureFormat>>,
    pub(super) vertex_format: Vec<Option<wgpu::VertexFormat>>,
    pub(super) topology: Vec<Option<wgpu::PrimitiveTopology>>,
    pub(super) index_format: Vec<Option<wgpu::IndexFormat>>,
    pub(super) front_face: Vec<Option<wgpu::FrontFace>>,
    pub(super) compare: Vec<Option<wgpu::CompareFunction>>,
    pub(super) stencil: Vec<Option<wgpu::StencilOperation>>,
    pub(super) blend_operation: Vec<Option<wgpu::BlendOperation>>,
    pub(super) blend_factor: Vec<Option<wgpu::BlendFactor>>,
    pub(super) step: Vec<Option<wgpu::VertexStepMode>>,
    pub(super) address: Vec<Option<wgpu::AddressMode>>,
    pub(super) filter: Vec<Option<wgpu::FilterMode>>,
    pub(super) mipmap: Vec<Option<wgpu::MipmapFilterMode>>,
    pub(super) dimension: Vec<Option<wgpu::TextureDimension>>,
    pub(super) view_dimension: Vec<Option<wgpu::TextureViewDimension>>,
    pub(super) aspect: Vec<Option<wgpu::TextureAspect>>,
    pub(super) sampler_binding: Vec<Option<wgpu::SamplerBindingType>>,
    pub(super) storage_access: Vec<Option<wgpu::StorageTextureAccess>>,
}

fn named<T: DeserializeOwned>(names: &[&str]) -> Vec<Option<T>> {
    names
        .iter()
        .map(|n| {
            let d: StrDeserializer<NameError> = n.into_deserializer();
            T::deserialize(d).ok()
        })
        .collect()
}

impl Names {
    pub(super) fn new() -> Names {
        Names {
            texture_format: named(TEXTURE_FORMAT),
            vertex_format: named(VERTEX_FORMAT),
            topology: named(PRIMITIVE_TOPOLOGY),
            index_format: named(INDEX_FORMAT),
            front_face: named(FRONT_FACE),
            compare: named(COMPARE_FUNCTION),
            stencil: named(STENCIL_OPERATION),
            blend_operation: named(BLEND_OPERATION),
            blend_factor: named(BLEND_FACTOR),
            step: named(VERTEX_STEP_MODE),
            address: named(ADDRESS_MODE),
            filter: named(FILTER_MODE),
            mipmap: named(MIPMAP_FILTER_MODE),
            dimension: named(TEXTURE_DIMENSION),
            view_dimension: named(TEXTURE_VIEW_DIMENSION),
            aspect: named(TEXTURE_ASPECT),
            sampler_binding: named(SAMPLER_BINDING_TYPE),
            storage_access: named(STORAGE_TEXTURE_ACCESS),
        }
    }
}

/// A guest's word is the union of the flags of the members whose bits it
/// sets, whatever bits wgpu gives them (its trunk splits them over two words).
pub(super) const BUFFER_USAGES: [(u32, wgpu::BufferUsages); 10] = [
    (BUFFER_USAGE::MAP_READ, wgpu::BufferUsages::MAP_READ),
    (BUFFER_USAGE::MAP_WRITE, wgpu::BufferUsages::MAP_WRITE),
    (BUFFER_USAGE::COPY_SRC, wgpu::BufferUsages::COPY_SRC),
    (BUFFER_USAGE::COPY_DST, wgpu::BufferUsages::COPY_DST),
    (BUFFER_USAGE::INDEX, wgpu::BufferUsages::INDEX),
    (BUFFER_USAGE::VERTEX, wgpu::BufferUsages::VERTEX),
    (BUFFER_USAGE::UNIFORM, wgpu::BufferUsages::UNIFORM),
    (BUFFER_USAGE::STORAGE, wgpu::BufferUsages::STORAGE),
    (BUFFER_USAGE::INDIRECT, wgpu::BufferUsages::INDIRECT),
    (
        BUFFER_USAGE::QUERY_RESOLVE,
        wgpu::BufferUsages::QUERY_RESOLVE,
    ),
];
pub(super) const TEXTURE_USAGES: [(u32, wgpu::TextureUsages); 5] = [
    (TEXTURE_USAGE::COPY_SRC, wgpu::TextureUsages::COPY_SRC),
    (TEXTURE_USAGE::COPY_DST, wgpu::TextureUsages::COPY_DST),
    (
        TEXTURE_USAGE::TEXTURE_BINDING,
        wgpu::TextureUsages::TEXTURE_BINDING,
    ),
    (
        TEXTURE_USAGE::STORAGE_BINDING,
        wgpu::TextureUsages::STORAGE_BINDING,
    ),
    (
        TEXTURE_USAGE::RENDER_ATTACHMENT,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
    ),
];
pub(super) const SHADER_STAGES: [(u32, wgpu::ShaderStages); 3] = [
    (SHADER_STAGE::VERTEX, wgpu::ShaderStages::VERTEX),
    (SHADER_STAGE::FRAGMENT, wgpu::ShaderStages::FRAGMENT),
    (SHADER_STAGE::COMPUTE, wgpu::ShaderStages::COMPUTE),
];
pub(super) const COLOR_WRITES: [(u32, wgpu::ColorWrites); 5] = [
    (COLOR_WRITE::RED, wgpu::ColorWrites::RED),
    (COLOR_WRITE::GREEN, wgpu::ColorWrites::GREEN),
    (COLOR_WRITE::BLUE, wgpu::ColorWrites::BLUE),
    (COLOR_WRITE::ALPHA, wgpu::ColorWrites::ALPHA),
    (COLOR_WRITE::ALL, wgpu::ColorWrites::ALL),
];
// A member the spec gains is one these name, or the build stops here.
const _: () = assert!(
    BUFFER_USAGES.len() == BUFFER_USAGE::KEYS.len()
        && TEXTURE_USAGES.len() == TEXTURE_USAGE::KEYS.len()
        && SHADER_STAGES.len() == SHADER_STAGE::KEYS.len()
        && COLOR_WRITES.len() == COLOR_WRITE::KEYS.len()
);

pub(super) fn flags<F: Copy + core::ops::BitOr<Output = F>>(
    of: &[(u32, F)],
    none: F,
    word: u32,
) -> F {
    of.iter()
        .filter(|(bit, _)| word & bit == *bit)
        .fold(none, |all, (_, f)| all | *f)
}

/// A name wgpu gives no value of is the guest asking for what this device
/// cannot be: the program's bug, not the host's.
pub(super) fn of<T: Copy>(of: &[Option<T>], n: u32, what: &str) -> T {
    of.get(n as usize)
        .copied()
        .flatten()
        .unwrap_or_else(|| panic!("[gpu] {what} {n} has no wgpu value"))
}
