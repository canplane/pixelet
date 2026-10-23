// @xpute/gpu/enum.spec.ts
//
// GENERATED from spec/gpu/enum.json — do not edit.
//
// WebGPU's own enumerations, each its spec's strings in order: a number is its
// place in the list, so a host hands the string to the device as it is and a guest
// names a member by its generated constant. Nothing here is any one program's; a
// value WebGPU leaves optional is NONE (api.json) where unset.

/** GPUTextureFormat, the core spec's and its texture-compression features'. */
export const TEXTURE_FORMAT = [
  "r8unorm",
  "r8snorm",
  "r8uint",
  "r8sint",
  "r16uint",
  "r16sint",
  "r16float",
  "rg8unorm",
  "rg8snorm",
  "rg8uint",
  "rg8sint",
  "r32uint",
  "r32sint",
  "r32float",
  "rg16uint",
  "rg16sint",
  "rg16float",
  "rgba8unorm",
  "rgba8unorm-srgb",
  "rgba8snorm",
  "rgba8uint",
  "rgba8sint",
  "bgra8unorm",
  "bgra8unorm-srgb",
  "rgb9e5ufloat",
  "rgb10a2uint",
  "rgb10a2unorm",
  "rg11b10ufloat",
  "rg32uint",
  "rg32sint",
  "rg32float",
  "rgba16uint",
  "rgba16sint",
  "rgba16float",
  "rgba32uint",
  "rgba32sint",
  "rgba32float",
  "stencil8",
  "depth16unorm",
  "depth24plus",
  "depth24plus-stencil8",
  "depth32float",
  "depth32float-stencil8",
  "bc1-rgba-unorm",
  "bc1-rgba-unorm-srgb",
  "bc2-rgba-unorm",
  "bc2-rgba-unorm-srgb",
  "bc3-rgba-unorm",
  "bc3-rgba-unorm-srgb",
  "bc4-r-unorm",
  "bc4-r-snorm",
  "bc5-rg-unorm",
  "bc5-rg-snorm",
  "bc6h-rgb-ufloat",
  "bc6h-rgb-float",
  "bc7-rgba-unorm",
  "bc7-rgba-unorm-srgb",
  "etc2-rgb8unorm",
  "etc2-rgb8unorm-srgb",
  "etc2-rgb8a1unorm",
  "etc2-rgb8a1unorm-srgb",
  "etc2-rgba8unorm",
  "etc2-rgba8unorm-srgb",
  "eac-r11unorm",
  "eac-r11snorm",
  "eac-rg11unorm",
  "eac-rg11snorm",
  "astc-4x4-unorm",
  "astc-4x4-unorm-srgb",
  "astc-5x4-unorm",
  "astc-5x4-unorm-srgb",
  "astc-5x5-unorm",
  "astc-5x5-unorm-srgb",
  "astc-6x5-unorm",
  "astc-6x5-unorm-srgb",
  "astc-6x6-unorm",
  "astc-6x6-unorm-srgb",
  "astc-8x5-unorm",
  "astc-8x5-unorm-srgb",
  "astc-8x6-unorm",
  "astc-8x6-unorm-srgb",
  "astc-8x8-unorm",
  "astc-8x8-unorm-srgb",
  "astc-10x5-unorm",
  "astc-10x5-unorm-srgb",
  "astc-10x6-unorm",
  "astc-10x6-unorm-srgb",
  "astc-10x8-unorm",
  "astc-10x8-unorm-srgb",
  "astc-10x10-unorm",
  "astc-10x10-unorm-srgb",
  "astc-12x10-unorm",
  "astc-12x10-unorm-srgb",
  "astc-12x12-unorm",
  "astc-12x12-unorm-srgb",
] as const;

/** GPUVertexFormat. */
export const VERTEX_FORMAT = [
  "uint8",
  "uint8x2",
  "uint8x4",
  "sint8",
  "sint8x2",
  "sint8x4",
  "unorm8",
  "unorm8x2",
  "unorm8x4",
  "snorm8",
  "snorm8x2",
  "snorm8x4",
  "uint16",
  "uint16x2",
  "uint16x4",
  "sint16",
  "sint16x2",
  "sint16x4",
  "unorm16",
  "unorm16x2",
  "unorm16x4",
  "snorm16",
  "snorm16x2",
  "snorm16x4",
  "float16",
  "float16x2",
  "float16x4",
  "float32",
  "float32x2",
  "float32x3",
  "float32x4",
  "uint32",
  "uint32x2",
  "uint32x3",
  "uint32x4",
  "sint32",
  "sint32x2",
  "sint32x3",
  "sint32x4",
  "unorm10-10-10-2",
  "unorm8x4-bgra",
] as const;

export const PRIMITIVE_TOPOLOGY = ["point-list", "line-list", "line-strip", "triangle-list", "triangle-strip"] as const;

export const INDEX_FORMAT = ["uint16", "uint32"] as const;

export const FRONT_FACE = ["ccw", "cw"] as const;

export const CULL_MODE = ["none", "front", "back"] as const;

export const COMPARE_FUNCTION = ["never", "less", "equal", "less-equal", "greater", "not-equal", "greater-equal", "always"] as const;

export const STENCIL_OPERATION = ["keep", "zero", "replace", "invert", "increment-clamp", "decrement-clamp", "increment-wrap", "decrement-wrap"] as const;

export const BLEND_OPERATION = ["add", "subtract", "reverse-subtract", "min", "max"] as const;

export const BLEND_FACTOR = [
  "zero",
  "one",
  "src",
  "one-minus-src",
  "src-alpha",
  "one-minus-src-alpha",
  "dst",
  "one-minus-dst",
  "dst-alpha",
  "one-minus-dst-alpha",
  "src-alpha-saturated",
  "constant",
  "one-minus-constant",
  "src1",
  "one-minus-src1",
  "src1-alpha",
  "one-minus-src1-alpha",
] as const;

export const VERTEX_STEP_MODE = ["vertex", "instance"] as const;

export const ADDRESS_MODE = ["clamp-to-edge", "repeat", "mirror-repeat"] as const;

export const FILTER_MODE = ["nearest", "linear"] as const;

export const MIPMAP_FILTER_MODE = ["nearest", "linear"] as const;

export const TEXTURE_DIMENSION = ["1d", "2d", "3d"] as const;

export const TEXTURE_VIEW_DIMENSION = ["1d", "2d", "2d-array", "cube", "cube-array", "3d"] as const;

export const TEXTURE_ASPECT = ["all", "stencil-only", "depth-only"] as const;

export const BUFFER_BINDING_TYPE = ["uniform", "storage", "read-only-storage"] as const;

export const SAMPLER_BINDING_TYPE = ["filtering", "non-filtering", "comparison"] as const;

export const TEXTURE_SAMPLE_TYPE = ["float", "unfilterable-float", "depth", "sint", "uint"] as const;

export const STORAGE_TEXTURE_ACCESS = ["write-only", "read-only", "read-write"] as const;

export const LOAD_OP = ["load", "clear"] as const;

export const STORE_OP = ["store", "discard"] as const;

export const QUERY_TYPE = ["occlusion", "timestamp"] as const;

export const CANVAS_ALPHA_MODE = ["opaque", "premultiplied"] as const;

/** GPUFeatureName: a device's features are a bit each, by place (DEVICE in api.json). */
export const FEATURE_NAME = [
  "depth-clip-control",
  "depth32float-stencil8",
  "texture-compression-bc",
  "texture-compression-bc-sliced-3d",
  "texture-compression-etc2",
  "texture-compression-astc",
  "texture-compression-astc-sliced-3d",
  "timestamp-query",
  "indirect-first-instance",
  "shader-f16",
  "rg11b10ufloat-renderable",
  "bgra8unorm-storage",
  "float32-filterable",
  "float32-blendable",
  "clip-distances",
  "dual-source-blending",
  "subgroups",
] as const;

/** GPUSupportedLimits' members: a device's limits are a word each, by place (DEVICE in api.json). */
export const LIMIT_NAME = [
  "maxTextureDimension1D",
  "maxTextureDimension2D",
  "maxTextureDimension3D",
  "maxTextureArrayLayers",
  "maxBindGroups",
  "maxBindGroupsPlusVertexBuffers",
  "maxBindingsPerBindGroup",
  "maxDynamicUniformBuffersPerPipelineLayout",
  "maxDynamicStorageBuffersPerPipelineLayout",
  "maxSampledTexturesPerShaderStage",
  "maxSamplersPerShaderStage",
  "maxStorageBuffersPerShaderStage",
  "maxStorageTexturesPerShaderStage",
  "maxUniformBuffersPerShaderStage",
  "maxUniformBufferBindingSize",
  "maxStorageBufferBindingSize",
  "minUniformBufferOffsetAlignment",
  "minStorageBufferOffsetAlignment",
  "maxVertexBuffers",
  "maxBufferSize",
  "maxVertexAttributes",
  "maxVertexBufferArrayStride",
  "maxInterStageShaderVariables",
  "maxColorAttachments",
  "maxColorAttachmentBytesPerSample",
  "maxComputeWorkgroupStorageSize",
  "maxComputeInvocationsPerWorkgroup",
  "maxComputeWorkgroupSizeX",
  "maxComputeWorkgroupSizeY",
  "maxComputeWorkgroupSizeZ",
  "maxComputeWorkgroupsPerDimension",
] as const;
