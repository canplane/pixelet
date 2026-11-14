// xpute-gpu-native-host/name.rs

//! This maps each enumeration's number to its wgpu value. The spec lists each
//! enumeration's WebGPU names, which are wgpu's serde names too, so each
//! number maps to its value by name.

use serde::de::value::{Error as NameError, StrDeserializer};
use serde::de::{DeserializeOwned, IntoDeserializer};

use xpute_gpu_link::enums::{
    ADDRESS_MODES, BLEND_FACTORS, BLEND_OPERATIONS, COMPARE_FUNCTIONS, FILTER_MODES, FRONT_FACES,
    INDEX_FORMATS, MIPMAP_FILTER_MODES, PRIMITIVE_TOPOLOGIES, SAMPLER_BINDING_TYPES,
    STENCIL_OPERATIONS, STORAGE_TEXTURE_ACCESSES, TEXTURE_ASPECTS, TEXTURE_DIMENSIONS,
    TEXTURE_FORMATS, TEXTURE_VIEW_DIMENSIONS, VERTEX_FORMATS, VERTEX_STEP_MODES,
};
use xpute_gpu_link::spec::{buffer_usage, color_write, shader_stage, texture_usage};

/// These are each enumeration's wgpu values, indexed by place in the spec's
/// list. A name wgpu has no value for is None.
pub(super) struct EnumTable {
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
    pub(super) addr: Vec<Option<wgpu::AddressMode>>,
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

impl EnumTable {
    pub(super) fn new() -> EnumTable {
        EnumTable {
            texture_format: named(TEXTURE_FORMATS),
            vertex_format: named(VERTEX_FORMATS),
            topology: named(PRIMITIVE_TOPOLOGIES),
            index_format: named(INDEX_FORMATS),
            front_face: named(FRONT_FACES),
            compare: named(COMPARE_FUNCTIONS),
            stencil: named(STENCIL_OPERATIONS),
            blend_operation: named(BLEND_OPERATIONS),
            blend_factor: named(BLEND_FACTORS),
            step: named(VERTEX_STEP_MODES),
            addr: named(ADDRESS_MODES),
            filter: named(FILTER_MODES),
            mipmap: named(MIPMAP_FILTER_MODES),
            dimension: named(TEXTURE_DIMENSIONS),
            view_dimension: named(TEXTURE_VIEW_DIMENSIONS),
            aspect: named(TEXTURE_ASPECTS),
            sampler_binding: named(SAMPLER_BINDING_TYPES),
            storage_access: named(STORAGE_TEXTURE_ACCESSES),
        }
    }
}

/// A guest's usage word stands for the union of the flags whose spec bits it
/// sets. Each flag is mapped by name rather than by bit, since wgpu's bits
/// need not be WebGPU's: wgpu's trunk splits them over two words.
pub(super) const BUFFER_USAGES: [(u32, wgpu::BufferUsages); 10] = [
    (buffer_usage::MAP_READ, wgpu::BufferUsages::MAP_READ),
    (buffer_usage::MAP_WRITE, wgpu::BufferUsages::MAP_WRITE),
    (buffer_usage::COPY_SRC, wgpu::BufferUsages::COPY_SRC),
    (buffer_usage::COPY_DST, wgpu::BufferUsages::COPY_DST),
    (buffer_usage::INDEX, wgpu::BufferUsages::INDEX),
    (buffer_usage::VERTEX, wgpu::BufferUsages::VERTEX),
    (buffer_usage::UNIFORM, wgpu::BufferUsages::UNIFORM),
    (buffer_usage::STORAGE, wgpu::BufferUsages::STORAGE),
    (buffer_usage::INDIRECT, wgpu::BufferUsages::INDIRECT),
    (
        buffer_usage::QUERY_RESOLVE,
        wgpu::BufferUsages::QUERY_RESOLVE,
    ),
];
pub(super) const TEXTURE_USAGES: [(u32, wgpu::TextureUsages); 5] = [
    (texture_usage::COPY_SRC, wgpu::TextureUsages::COPY_SRC),
    (texture_usage::COPY_DST, wgpu::TextureUsages::COPY_DST),
    (
        texture_usage::TEXTURE_BINDING,
        wgpu::TextureUsages::TEXTURE_BINDING,
    ),
    (
        texture_usage::STORAGE_BINDING,
        wgpu::TextureUsages::STORAGE_BINDING,
    ),
    (
        texture_usage::RENDER_ATTACHMENT,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
    ),
];
pub(super) const SHADER_STAGES: [(u32, wgpu::ShaderStages); 3] = [
    (shader_stage::VERTEX, wgpu::ShaderStages::VERTEX),
    (shader_stage::FRAGMENT, wgpu::ShaderStages::FRAGMENT),
    (shader_stage::COMPUTE, wgpu::ShaderStages::COMPUTE),
];
pub(super) const COLOR_WRITES: [(u32, wgpu::ColorWrites); 5] = [
    (color_write::RED, wgpu::ColorWrites::RED),
    (color_write::GREEN, wgpu::ColorWrites::GREEN),
    (color_write::BLUE, wgpu::ColorWrites::BLUE),
    (color_write::ALPHA, wgpu::ColorWrites::ALPHA),
    (color_write::ALL, wgpu::ColorWrites::ALL),
];

pub(super) fn flags<F: Copy + core::ops::BitOr<Output = F>>(
    of: &[(u32, F)],
    none: F,
    word: u32,
) -> F {
    of.iter()
        .filter(|(bit, _)| word & bit == *bit)
        .fold(none, |all, (_, f)| all | *f)
}

/// A name wgpu has no value for means the guest asked for what this device
/// cannot be. That is the program's bug, not the host's.
pub(super) fn of<T: Copy>(of: &[Option<T>], n: u32, what: &str) -> T {
    of.get(n as usize)
        .copied()
        .flatten()
        .unwrap_or_else(|| panic!("[gpu] {what} {n} has no wgpu value"))
}
