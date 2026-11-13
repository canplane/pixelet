// xpute-gpu-native-host/open.rs

use xpute_gpu_link::enums::{FEATURE_NAME, LIMIT_NAME};
use xpute_gpu_link::GpuAsk;

/// This says what a device is asked to be as it opens.
#[derive(Clone)]
pub struct Ask {
    pub backends: wgpu::Backends,
    pub power_preference: wgpu::PowerPreference,
    /// The WebGPU names of the features the device must have. One the
    /// adapter lacks makes the device refuse to open (`OpenError::Device`);
    /// one wgpu does not know refuses it before it is asked
    /// (`OpenError::Feature`). The guest reads the features the device has
    /// from the device's block (spec DEVICE).
    pub required: &'static [&'static str],
    /// The device takes these features where the adapter has them.
    pub optional: &'static [&'static str],
    /// These are the limits asked of the device, given the adapter it is asked
    /// of, since an adapter refuses a device that asks past its own limits.
    pub limits: fn(&wgpu::Adapter) -> wgpu::Limits,
    pub memory_hints: wgpu::MemoryHints,
}

impl Ask {
    /// This is the device the guest asks for (`GpuAsk`), with the adapter's
    /// own limits and the platform's primary backends.
    pub fn of(GpuAsk { power, required_feats, optional_feats }: GpuAsk) -> Ask {
        Ask {
            backends: wgpu::Backends::PRIMARY,
            power_preference: match power {
                "low-power" => wgpu::PowerPreference::LowPower,
                _ => wgpu::PowerPreference::HighPerformance,
            },
            required: required_feats,
            optional: optional_feats,
            limits: wgpu::Adapter::limits,
            memory_hints: wgpu::MemoryHints::Performance,
        }
    }

    fn features(&self, adapter: &wgpu::Adapter) -> Result<wgpu::Features, OpenError> {
        let mut out = wgpu::Features::empty();
        for name in self.required {
            out |= feature(name).ok_or(OpenError::Feature(name))?;
        }
        for name in self.optional {
            out |= feature(name).unwrap_or(wgpu::Features::empty()) & adapter.features();
        }
        Ok(out)
    }
}

#[derive(Debug)]
pub enum OpenError {
    Adapter(wgpu::RequestAdapterError),
    Device(wgpu::RequestDeviceError),
    /// A required feature has a WebGPU name wgpu does not know.
    Feature(&'static str),
}

/// The wgpu objects of a device opened as asked, on an instance (`instance`).
pub struct Wgpu {
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

/// This is the instance devices are opened on as `ask` says (`open`), and a
/// program makes its surfaces with (`GpuHost::bind`). It is made without a
/// display handle, since each surface brings its own (`SurfaceTargetUnsafe`).
/// It outlives a device lost; a surface goes with its device's host, and
/// the next device's host makes each again of the window it was made of
/// (`native::open`).
pub fn instance(ask: &Ask) -> wgpu::Instance {
    // The guest's indirect calls are its own shaders' output, so they are not
    // validated, which would also put a pass of wgpu's ahead of each pass that
    // draws indirectly.
    let defaults = wgpu::InstanceDescriptor::new_without_display_handle();
    wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: ask.backends,
        flags: defaults.flags - wgpu::InstanceFlags::VALIDATION_INDIRECT_CALL,
        ..defaults
    })
}

/// Opens a device on `instance` as `ask` says, for `GpuHost::new`. It
/// blocks on wgpu's futures, which a native backend resolves at once.
pub fn open(instance: &wgpu::Instance, ask: &Ask) -> Result<Wgpu, OpenError> {
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: ask.power_preference,
        ..Default::default()
    }))
    .map_err(OpenError::Adapter)?;
    // The host keeps the pipelines it compiles across launches where the
    // adapter can (`Cache`), which no program asks for by a WebGPU name.
    let keeps = adapter.features() & wgpu::Features::PIPELINE_CACHE;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: ask.features(&adapter)? | keeps,
        required_limits: (ask.limits)(&adapter),
        memory_hints: ask.memory_hints.clone(),
        ..Default::default()
    }))
    .map_err(OpenError::Device)?;
    Ok(Wgpu {
        adapter,
        device,
        queue,
    })
}

/// These are the device's limits and features as the device's block lays them
/// out (spec DEVICE): a word for each limit LIMIT_NAME names, then a bit for
/// each feature FEATURE_NAME names.
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

/// This is the value of the limit WebGPU calls `name`, clamped to a word. A
/// name wgpu has no limit for is 0.
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
