// cpp/xpute/gpu/enum.spec.hpp
//
// GENERATED from spec/gpu/enum.json
//
// WebGPU's own enumerations, each its spec's strings in order: a number is its
// place in the list, so a host hands the string to the device as it is and a guest
// names a member by its generated constant. Nothing here is any one program's; a
// value WebGPU leaves optional is NONE (api.json) where unset.

#pragma once

#include <array>
#include <cstdint>
#include <string_view>

namespace xpute::gpu {

// GPUTextureFormat, the core spec's and its texture-compression features'.
enum class TextureFormat : std::uint8_t {
    r8unorm = 0,
    r8snorm = 1,
    r8uint = 2,
    r8sint = 3,
    r16uint = 4,
    r16sint = 5,
    r16float = 6,
    rg8unorm = 7,
    rg8snorm = 8,
    rg8uint = 9,
    rg8sint = 10,
    r32uint = 11,
    r32sint = 12,
    r32float = 13,
    rg16uint = 14,
    rg16sint = 15,
    rg16float = 16,
    rgba8unorm = 17,
    rgba8unorm_srgb = 18,
    rgba8snorm = 19,
    rgba8uint = 20,
    rgba8sint = 21,
    bgra8unorm = 22,
    bgra8unorm_srgb = 23,
    rgb9e5ufloat = 24,
    rgb10a2uint = 25,
    rgb10a2unorm = 26,
    rg11b10ufloat = 27,
    rg32uint = 28,
    rg32sint = 29,
    rg32float = 30,
    rgba16uint = 31,
    rgba16sint = 32,
    rgba16float = 33,
    rgba32uint = 34,
    rgba32sint = 35,
    rgba32float = 36,
    stencil8 = 37,
    depth16unorm = 38,
    depth24plus = 39,
    depth24plus_stencil8 = 40,
    depth32float = 41,
    depth32float_stencil8 = 42,
    bc1_rgba_unorm = 43,
    bc1_rgba_unorm_srgb = 44,
    bc2_rgba_unorm = 45,
    bc2_rgba_unorm_srgb = 46,
    bc3_rgba_unorm = 47,
    bc3_rgba_unorm_srgb = 48,
    bc4_r_unorm = 49,
    bc4_r_snorm = 50,
    bc5_rg_unorm = 51,
    bc5_rg_snorm = 52,
    bc6h_rgb_ufloat = 53,
    bc6h_rgb_float = 54,
    bc7_rgba_unorm = 55,
    bc7_rgba_unorm_srgb = 56,
    etc2_rgb8unorm = 57,
    etc2_rgb8unorm_srgb = 58,
    etc2_rgb8a1unorm = 59,
    etc2_rgb8a1unorm_srgb = 60,
    etc2_rgba8unorm = 61,
    etc2_rgba8unorm_srgb = 62,
    eac_r11unorm = 63,
    eac_r11snorm = 64,
    eac_rg11unorm = 65,
    eac_rg11snorm = 66,
    astc_4x4_unorm = 67,
    astc_4x4_unorm_srgb = 68,
    astc_5x4_unorm = 69,
    astc_5x4_unorm_srgb = 70,
    astc_5x5_unorm = 71,
    astc_5x5_unorm_srgb = 72,
    astc_6x5_unorm = 73,
    astc_6x5_unorm_srgb = 74,
    astc_6x6_unorm = 75,
    astc_6x6_unorm_srgb = 76,
    astc_8x5_unorm = 77,
    astc_8x5_unorm_srgb = 78,
    astc_8x6_unorm = 79,
    astc_8x6_unorm_srgb = 80,
    astc_8x8_unorm = 81,
    astc_8x8_unorm_srgb = 82,
    astc_10x5_unorm = 83,
    astc_10x5_unorm_srgb = 84,
    astc_10x6_unorm = 85,
    astc_10x6_unorm_srgb = 86,
    astc_10x8_unorm = 87,
    astc_10x8_unorm_srgb = 88,
    astc_10x10_unorm = 89,
    astc_10x10_unorm_srgb = 90,
    astc_12x10_unorm = 91,
    astc_12x10_unorm_srgb = 92,
    astc_12x12_unorm = 93,
    astc_12x12_unorm_srgb = 94,
};

inline constexpr std::array<std::string_view, 95> TEXTURE_FORMAT{"r8unorm", "r8snorm", "r8uint", "r8sint", "r16uint", "r16sint", "r16float", "rg8unorm", "rg8snorm", "rg8uint", "rg8sint", "r32uint", "r32sint", "r32float", "rg16uint", "rg16sint", "rg16float", "rgba8unorm", "rgba8unorm-srgb", "rgba8snorm", "rgba8uint", "rgba8sint", "bgra8unorm", "bgra8unorm-srgb", "rgb9e5ufloat", "rgb10a2uint", "rgb10a2unorm", "rg11b10ufloat", "rg32uint", "rg32sint", "rg32float", "rgba16uint", "rgba16sint", "rgba16float", "rgba32uint", "rgba32sint", "rgba32float", "stencil8", "depth16unorm", "depth24plus", "depth24plus-stencil8", "depth32float", "depth32float-stencil8", "bc1-rgba-unorm", "bc1-rgba-unorm-srgb", "bc2-rgba-unorm", "bc2-rgba-unorm-srgb", "bc3-rgba-unorm", "bc3-rgba-unorm-srgb", "bc4-r-unorm", "bc4-r-snorm", "bc5-rg-unorm", "bc5-rg-snorm", "bc6h-rgb-ufloat", "bc6h-rgb-float", "bc7-rgba-unorm", "bc7-rgba-unorm-srgb", "etc2-rgb8unorm", "etc2-rgb8unorm-srgb", "etc2-rgb8a1unorm", "etc2-rgb8a1unorm-srgb", "etc2-rgba8unorm", "etc2-rgba8unorm-srgb", "eac-r11unorm", "eac-r11snorm", "eac-rg11unorm", "eac-rg11snorm", "astc-4x4-unorm", "astc-4x4-unorm-srgb", "astc-5x4-unorm", "astc-5x4-unorm-srgb", "astc-5x5-unorm", "astc-5x5-unorm-srgb", "astc-6x5-unorm", "astc-6x5-unorm-srgb", "astc-6x6-unorm", "astc-6x6-unorm-srgb", "astc-8x5-unorm", "astc-8x5-unorm-srgb", "astc-8x6-unorm", "astc-8x6-unorm-srgb", "astc-8x8-unorm", "astc-8x8-unorm-srgb", "astc-10x5-unorm", "astc-10x5-unorm-srgb", "astc-10x6-unorm", "astc-10x6-unorm-srgb", "astc-10x8-unorm", "astc-10x8-unorm-srgb", "astc-10x10-unorm", "astc-10x10-unorm-srgb", "astc-12x10-unorm", "astc-12x10-unorm-srgb", "astc-12x12-unorm", "astc-12x12-unorm-srgb"};

// GPUVertexFormat.
enum class VertexFormat : std::uint8_t {
    uint8 = 0,
    uint8x2 = 1,
    uint8x4 = 2,
    sint8 = 3,
    sint8x2 = 4,
    sint8x4 = 5,
    unorm8 = 6,
    unorm8x2 = 7,
    unorm8x4 = 8,
    snorm8 = 9,
    snorm8x2 = 10,
    snorm8x4 = 11,
    uint16 = 12,
    uint16x2 = 13,
    uint16x4 = 14,
    sint16 = 15,
    sint16x2 = 16,
    sint16x4 = 17,
    unorm16 = 18,
    unorm16x2 = 19,
    unorm16x4 = 20,
    snorm16 = 21,
    snorm16x2 = 22,
    snorm16x4 = 23,
    float16 = 24,
    float16x2 = 25,
    float16x4 = 26,
    float32 = 27,
    float32x2 = 28,
    float32x3 = 29,
    float32x4 = 30,
    uint32 = 31,
    uint32x2 = 32,
    uint32x3 = 33,
    uint32x4 = 34,
    sint32 = 35,
    sint32x2 = 36,
    sint32x3 = 37,
    sint32x4 = 38,
    unorm10_10_10_2 = 39,
    unorm8x4_bgra = 40,
};

inline constexpr std::array<std::string_view, 41> VERTEX_FORMAT{"uint8", "uint8x2", "uint8x4", "sint8", "sint8x2", "sint8x4", "unorm8", "unorm8x2", "unorm8x4", "snorm8", "snorm8x2", "snorm8x4", "uint16", "uint16x2", "uint16x4", "sint16", "sint16x2", "sint16x4", "unorm16", "unorm16x2", "unorm16x4", "snorm16", "snorm16x2", "snorm16x4", "float16", "float16x2", "float16x4", "float32", "float32x2", "float32x3", "float32x4", "uint32", "uint32x2", "uint32x3", "uint32x4", "sint32", "sint32x2", "sint32x3", "sint32x4", "unorm10-10-10-2", "unorm8x4-bgra"};

enum class PrimitiveTopology : std::uint8_t {
    point_list = 0,
    line_list = 1,
    line_strip = 2,
    triangle_list = 3,
    triangle_strip = 4,
};

inline constexpr std::array<std::string_view, 5> PRIMITIVE_TOPOLOGY{"point-list", "line-list", "line-strip", "triangle-list", "triangle-strip"};

enum class IndexFormat : std::uint8_t {
    uint16 = 0,
    uint32 = 1,
};

inline constexpr std::array<std::string_view, 2> INDEX_FORMAT{"uint16", "uint32"};

enum class FrontFace : std::uint8_t {
    ccw = 0,
    cw = 1,
};

inline constexpr std::array<std::string_view, 2> FRONT_FACE{"ccw", "cw"};

enum class CullMode : std::uint8_t {
    none = 0,
    front = 1,
    back = 2,
};

inline constexpr std::array<std::string_view, 3> CULL_MODE{"none", "front", "back"};

enum class CompareFunction : std::uint8_t {
    never = 0,
    less = 1,
    equal = 2,
    less_equal = 3,
    greater = 4,
    not_equal = 5,
    greater_equal = 6,
    always = 7,
};

inline constexpr std::array<std::string_view, 8> COMPARE_FUNCTION{"never", "less", "equal", "less-equal", "greater", "not-equal", "greater-equal", "always"};

enum class StencilOperation : std::uint8_t {
    keep = 0,
    zero = 1,
    replace = 2,
    invert = 3,
    increment_clamp = 4,
    decrement_clamp = 5,
    increment_wrap = 6,
    decrement_wrap = 7,
};

inline constexpr std::array<std::string_view, 8> STENCIL_OPERATION{"keep", "zero", "replace", "invert", "increment-clamp", "decrement-clamp", "increment-wrap", "decrement-wrap"};

enum class BlendOperation : std::uint8_t {
    add = 0,
    subtract = 1,
    reverse_subtract = 2,
    min = 3,
    max = 4,
};

inline constexpr std::array<std::string_view, 5> BLEND_OPERATION{"add", "subtract", "reverse-subtract", "min", "max"};

enum class BlendFactor : std::uint8_t {
    zero = 0,
    one = 1,
    src = 2,
    one_minus_src = 3,
    src_alpha = 4,
    one_minus_src_alpha = 5,
    dst = 6,
    one_minus_dst = 7,
    dst_alpha = 8,
    one_minus_dst_alpha = 9,
    src_alpha_saturated = 10,
    constant = 11,
    one_minus_constant = 12,
    src1 = 13,
    one_minus_src1 = 14,
    src1_alpha = 15,
    one_minus_src1_alpha = 16,
};

inline constexpr std::array<std::string_view, 17> BLEND_FACTOR{"zero", "one", "src", "one-minus-src", "src-alpha", "one-minus-src-alpha", "dst", "one-minus-dst", "dst-alpha", "one-minus-dst-alpha", "src-alpha-saturated", "constant", "one-minus-constant", "src1", "one-minus-src1", "src1-alpha", "one-minus-src1-alpha"};

enum class VertexStepMode : std::uint8_t {
    vertex = 0,
    instance = 1,
};

inline constexpr std::array<std::string_view, 2> VERTEX_STEP_MODE{"vertex", "instance"};

enum class AddressMode : std::uint8_t {
    clamp_to_edge = 0,
    repeat = 1,
    mirror_repeat = 2,
};

inline constexpr std::array<std::string_view, 3> ADDRESS_MODE{"clamp-to-edge", "repeat", "mirror-repeat"};

enum class FilterMode : std::uint8_t {
    nearest = 0,
    linear = 1,
};

inline constexpr std::array<std::string_view, 2> FILTER_MODE{"nearest", "linear"};

enum class MipmapFilterMode : std::uint8_t {
    nearest = 0,
    linear = 1,
};

inline constexpr std::array<std::string_view, 2> MIPMAP_FILTER_MODE{"nearest", "linear"};

enum class TextureDimension : std::uint8_t {
    _1d = 0,
    _2d = 1,
    _3d = 2,
};

inline constexpr std::array<std::string_view, 3> TEXTURE_DIMENSION{"1d", "2d", "3d"};

enum class TextureViewDimension : std::uint8_t {
    _1d = 0,
    _2d = 1,
    _2d_array = 2,
    cube = 3,
    cube_array = 4,
    _3d = 5,
};

inline constexpr std::array<std::string_view, 6> TEXTURE_VIEW_DIMENSION{"1d", "2d", "2d-array", "cube", "cube-array", "3d"};

enum class TextureAspect : std::uint8_t {
    all = 0,
    stencil_only = 1,
    depth_only = 2,
};

inline constexpr std::array<std::string_view, 3> TEXTURE_ASPECT{"all", "stencil-only", "depth-only"};

enum class BufferBindingType : std::uint8_t {
    uniform = 0,
    storage = 1,
    read_only_storage = 2,
};

inline constexpr std::array<std::string_view, 3> BUFFER_BINDING_TYPE{"uniform", "storage", "read-only-storage"};

enum class SamplerBindingType : std::uint8_t {
    filtering = 0,
    non_filtering = 1,
    comparison = 2,
};

inline constexpr std::array<std::string_view, 3> SAMPLER_BINDING_TYPE{"filtering", "non-filtering", "comparison"};

enum class TextureSampleType : std::uint8_t {
    float_ = 0,
    unfilterable_float = 1,
    depth = 2,
    sint = 3,
    uint = 4,
};

inline constexpr std::array<std::string_view, 5> TEXTURE_SAMPLE_TYPE{"float", "unfilterable-float", "depth", "sint", "uint"};

enum class StorageTextureAccess : std::uint8_t {
    write_only = 0,
    read_only = 1,
    read_write = 2,
};

inline constexpr std::array<std::string_view, 3> STORAGE_TEXTURE_ACCESS{"write-only", "read-only", "read-write"};

enum class LoadOp : std::uint8_t {
    load = 0,
    clear = 1,
};

inline constexpr std::array<std::string_view, 2> LOAD_OP{"load", "clear"};

enum class StoreOp : std::uint8_t {
    store = 0,
    discard = 1,
};

inline constexpr std::array<std::string_view, 2> STORE_OP{"store", "discard"};

enum class QueryType : std::uint8_t {
    occlusion = 0,
    timestamp = 1,
};

inline constexpr std::array<std::string_view, 2> QUERY_TYPE{"occlusion", "timestamp"};

enum class CanvasAlphaMode : std::uint8_t {
    opaque = 0,
    premultiplied = 1,
};

inline constexpr std::array<std::string_view, 2> CANVAS_ALPHA_MODE{"opaque", "premultiplied"};

// GPUFeatureName: a device's features are a bit each, by place (DEVICE in api.json).
enum class FeatureName : std::uint8_t {
    depth_clip_control = 0,
    depth32float_stencil8 = 1,
    texture_compression_bc = 2,
    texture_compression_bc_sliced_3d = 3,
    texture_compression_etc2 = 4,
    texture_compression_astc = 5,
    texture_compression_astc_sliced_3d = 6,
    timestamp_query = 7,
    indirect_first_instance = 8,
    shader_f16 = 9,
    rg11b10ufloat_renderable = 10,
    bgra8unorm_storage = 11,
    float32_filterable = 12,
    float32_blendable = 13,
    clip_distances = 14,
    dual_source_blending = 15,
    subgroups = 16,
};

inline constexpr std::array<std::string_view, 17> FEATURE_NAME{"depth-clip-control", "depth32float-stencil8", "texture-compression-bc", "texture-compression-bc-sliced-3d", "texture-compression-etc2", "texture-compression-astc", "texture-compression-astc-sliced-3d", "timestamp-query", "indirect-first-instance", "shader-f16", "rg11b10ufloat-renderable", "bgra8unorm-storage", "float32-filterable", "float32-blendable", "clip-distances", "dual-source-blending", "subgroups"};

// GPUSupportedLimits' members: a device's limits are a word each, by place (DEVICE in api.json).
enum class LimitName : std::uint8_t {
    maxTextureDimension1D = 0,
    maxTextureDimension2D = 1,
    maxTextureDimension3D = 2,
    maxTextureArrayLayers = 3,
    maxBindGroups = 4,
    maxBindGroupsPlusVertexBuffers = 5,
    maxBindingsPerBindGroup = 6,
    maxDynamicUniformBuffersPerPipelineLayout = 7,
    maxDynamicStorageBuffersPerPipelineLayout = 8,
    maxSampledTexturesPerShaderStage = 9,
    maxSamplersPerShaderStage = 10,
    maxStorageBuffersPerShaderStage = 11,
    maxStorageTexturesPerShaderStage = 12,
    maxUniformBuffersPerShaderStage = 13,
    maxUniformBufferBindingSize = 14,
    maxStorageBufferBindingSize = 15,
    minUniformBufferOffsetAlignment = 16,
    minStorageBufferOffsetAlignment = 17,
    maxVertexBuffers = 18,
    maxBufferSize = 19,
    maxVertexAttributes = 20,
    maxVertexBufferArrayStride = 21,
    maxInterStageShaderVariables = 22,
    maxColorAttachments = 23,
    maxColorAttachmentBytesPerSample = 24,
    maxComputeWorkgroupStorageSize = 25,
    maxComputeInvocationsPerWorkgroup = 26,
    maxComputeWorkgroupSizeX = 27,
    maxComputeWorkgroupSizeY = 28,
    maxComputeWorkgroupSizeZ = 29,
    maxComputeWorkgroupsPerDimension = 30,
};

inline constexpr std::array<std::string_view, 31> LIMIT_NAME{"maxTextureDimension1D", "maxTextureDimension2D", "maxTextureDimension3D", "maxTextureArrayLayers", "maxBindGroups", "maxBindGroupsPlusVertexBuffers", "maxBindingsPerBindGroup", "maxDynamicUniformBuffersPerPipelineLayout", "maxDynamicStorageBuffersPerPipelineLayout", "maxSampledTexturesPerShaderStage", "maxSamplersPerShaderStage", "maxStorageBuffersPerShaderStage", "maxStorageTexturesPerShaderStage", "maxUniformBuffersPerShaderStage", "maxUniformBufferBindingSize", "maxStorageBufferBindingSize", "minUniformBufferOffsetAlignment", "minStorageBufferOffsetAlignment", "maxVertexBuffers", "maxBufferSize", "maxVertexAttributes", "maxVertexBufferArrayStride", "maxInterStageShaderVariables", "maxColorAttachments", "maxColorAttachmentBytesPerSample", "maxComputeWorkgroupStorageSize", "maxComputeInvocationsPerWorkgroup", "maxComputeWorkgroupSizeX", "maxComputeWorkgroupSizeY", "maxComputeWorkgroupSizeZ", "maxComputeWorkgroupsPerDimension"};

} // namespace xpute::gpu
