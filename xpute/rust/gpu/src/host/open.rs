// xpute-gpu/host/open.rs

use crate::enums::{FEATURE_NAME, LIMIT_NAME};

/// What a program asks of the device it opens.
#[derive(Clone)]
pub struct Ask {
    pub backends: wgpu::Backends,
    pub power_preference: wgpu::PowerPreference,
    /// Asked for where the adapter offers them, and left out where it does
    /// not: what the device has, the guest reads from its block (DEVICE).
    pub features: wgpu::Features,
    /// What the device is asked to hold, from the adapter it is asked of,
    /// since an adapter refuses a device that asks past its own.
    pub limits: fn(&wgpu::Adapter) -> wgpu::Limits,
    pub memory_hints: wgpu::MemoryHints,
}

#[derive(Debug)]
pub enum OpenError {
    Adapter(wgpu::RequestAdapterError),
    Device(wgpu::RequestDeviceError),
}

/// A device opened as asked, and the instance a program makes its surfaces
/// with (`GpuHost::bind`).
pub struct Opened {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

/// Opens a device as `ask` says, for `GpuHost::new`, waiting out wgpu's
/// futures, which a native backend resolves at once. The instance is made
/// without a display handle: each surface brings its own (`SurfaceTargetUnsafe`).
pub fn open(ask: &Ask) -> Result<Opened, OpenError> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: ask.backends,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: ask.power_preference,
        ..Default::default()
    }))
    .map_err(OpenError::Adapter)?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: adapter.features() & ask.features,
        required_limits: (ask.limits)(&adapter),
        memory_hints: ask.memory_hints.clone(),
        ..Default::default()
    }))
    .map_err(OpenError::Device)?;
    Ok(Opened {
        instance,
        adapter,
        device,
        queue,
    })
}

/// The device's limits and features as the block lays them (DEVICE): each
/// limit `LIMIT_NAME` names, then a bit a feature `FEATURE_NAME` names.
pub(super) fn block(device: &wgpu::Device) -> (Vec<u32>, Vec<u32>) {
    let (limits, features) = (device.limits(), device.features());
    let words = LIMIT_NAME.iter().map(|name| limit(&limits, name)).collect();
    let mut bits = vec![0u32; FEATURE_NAME.len().div_ceil(32)];
    for (k, name) in FEATURE_NAME.iter().enumerate() {
        if feature(name).is_some_and(|f| features.contains(f)) {
            bits[k >> 5] |= 1 << (k & 31);
        }
    }
    (words, bits)
}

/// A limit's value as WebGPU names it, held to a word.
fn limit(l: &wgpu::Limits, name: &str) -> u32 {
    let v: u64 = match name {
        "maxTextureDimension1D" => l.max_texture_dimension_1d.into(),
        "maxTextureDimension2D" => l.max_texture_dimension_2d.into(),
        "maxTextureDimension3D" => l.max_texture_dimension_3d.into(),
        "maxTextureArrayLayers" => l.max_texture_array_layers.into(),
        "maxBindGroups" => l.max_bind_groups.into(),
        "maxBindGroupsPlusVertexBuffers" => l.max_bind_groups_plus_vertex_buffers.into(),
        "maxBindingsPerBindGroup" => l.max_bindings_per_bind_group.into(),
        "maxDynamicUniformBuffersPerPipelineLayout" => {
            l.max_dynamic_uniform_buffers_per_pipeline_layout.into()
        }
        "maxDynamicStorageBuffersPerPipelineLayout" => {
            l.max_dynamic_storage_buffers_per_pipeline_layout.into()
        }
        "maxSampledTexturesPerShaderStage" => l.max_sampled_textures_per_shader_stage.into(),
        "maxSamplersPerShaderStage" => l.max_samplers_per_shader_stage.into(),
        "maxStorageBuffersPerShaderStage" => l.max_storage_buffers_per_shader_stage.into(),
        "maxStorageTexturesPerShaderStage" => l.max_storage_textures_per_shader_stage.into(),
        "maxUniformBuffersPerShaderStage" => l.max_uniform_buffers_per_shader_stage.into(),
        "maxUniformBufferBindingSize" => l.max_uniform_buffer_binding_size,
        "maxStorageBufferBindingSize" => l.max_storage_buffer_binding_size,
        "minUniformBufferOffsetAlignment" => l.min_uniform_buffer_offset_alignment.into(),
        "minStorageBufferOffsetAlignment" => l.min_storage_buffer_offset_alignment.into(),
        "maxVertexBuffers" => l.max_vertex_buffers.into(),
        "maxBufferSize" => l.max_buffer_size,
        "maxVertexAttributes" => l.max_vertex_attributes.into(),
        "maxVertexBufferArrayStride" => l.max_vertex_buffer_array_stride.into(),
        "maxInterStageShaderVariables" => l.max_inter_stage_shader_variables.into(),
        "maxColorAttachments" => l.max_color_attachments.into(),
        "maxColorAttachmentBytesPerSample" => l.max_color_attachment_bytes_per_sample.into(),
        "maxComputeWorkgroupStorageSize" => l.max_compute_workgroup_storage_size.into(),
        "maxComputeInvocationsPerWorkgroup" => l.max_compute_invocations_per_workgroup.into(),
        "maxComputeWorkgroupSizeX" => l.max_compute_workgroup_size_x.into(),
        "maxComputeWorkgroupSizeY" => l.max_compute_workgroup_size_y.into(),
        "maxComputeWorkgroupSizeZ" => l.max_compute_workgroup_size_z.into(),
        "maxComputeWorkgroupsPerDimension" => l.max_compute_workgroups_per_dimension.into(),
        _ => 0,
    };
    v.min(u64::from(u32::MAX)) as u32
}

fn feature(name: &str) -> Option<wgpu::Features> {
    Some(match name {
        "depth-clip-control" => wgpu::Features::DEPTH_CLIP_CONTROL,
        "depth32float-stencil8" => wgpu::Features::DEPTH32FLOAT_STENCIL8,
        "texture-compression-bc" => wgpu::Features::TEXTURE_COMPRESSION_BC,
        "texture-compression-bc-sliced-3d" => wgpu::Features::TEXTURE_COMPRESSION_BC_SLICED_3D,
        "texture-compression-etc2" => wgpu::Features::TEXTURE_COMPRESSION_ETC2,
        "texture-compression-astc" => wgpu::Features::TEXTURE_COMPRESSION_ASTC,
        "texture-compression-astc-sliced-3d" => wgpu::Features::TEXTURE_COMPRESSION_ASTC_SLICED_3D,
        "timestamp-query" => wgpu::Features::TIMESTAMP_QUERY,
        "indirect-first-instance" => wgpu::Features::INDIRECT_FIRST_INSTANCE,
        "shader-f16" => wgpu::Features::SHADER_F16,
        "rg11b10ufloat-renderable" => wgpu::Features::RG11B10UFLOAT_RENDERABLE,
        "bgra8unorm-storage" => wgpu::Features::BGRA8UNORM_STORAGE,
        "float32-filterable" => wgpu::Features::FLOAT32_FILTERABLE,
        "float32-blendable" => wgpu::Features::FLOAT32_BLENDABLE,
        "clip-distances" => wgpu::Features::CLIP_DISTANCES,
        "dual-source-blending" => wgpu::Features::DUAL_SOURCE_BLENDING,
        "subgroups" => wgpu::Features::SUBGROUP,
        _ => return None,
    })
}
