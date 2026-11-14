// cpp/xpute/gpu/enum.spec.hpp
//
// GENERATED from gpu/link/spec/enum.json (xpute-gen data.rs)
//
// WebGPU's own enumerations. Each one lists its WebGPU spec's strings in order, and a
// member's number is its place in that list. A host hands the device the string as it
// is, and a guest names a member by its generated constant. Nothing here belongs to any one
// program. A value that WebGPU leaves optional is NONE (api.json) when it is unset.

#pragma once

#include <array>
#include <cstdint>
#include <optional>
#include <string_view>

namespace xpute::gpu {

// GPUTextureFormat: the formats of the core WebGPU spec and of its texture-compression
// features.
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

// The member `val` is, or none for a number no member has.
constexpr std::optional<TextureFormat> texture_format_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return TextureFormat::r8unorm;
    case 1: return TextureFormat::r8snorm;
    case 2: return TextureFormat::r8uint;
    case 3: return TextureFormat::r8sint;
    case 4: return TextureFormat::r16uint;
    case 5: return TextureFormat::r16sint;
    case 6: return TextureFormat::r16float;
    case 7: return TextureFormat::rg8unorm;
    case 8: return TextureFormat::rg8snorm;
    case 9: return TextureFormat::rg8uint;
    case 10: return TextureFormat::rg8sint;
    case 11: return TextureFormat::r32uint;
    case 12: return TextureFormat::r32sint;
    case 13: return TextureFormat::r32float;
    case 14: return TextureFormat::rg16uint;
    case 15: return TextureFormat::rg16sint;
    case 16: return TextureFormat::rg16float;
    case 17: return TextureFormat::rgba8unorm;
    case 18: return TextureFormat::rgba8unorm_srgb;
    case 19: return TextureFormat::rgba8snorm;
    case 20: return TextureFormat::rgba8uint;
    case 21: return TextureFormat::rgba8sint;
    case 22: return TextureFormat::bgra8unorm;
    case 23: return TextureFormat::bgra8unorm_srgb;
    case 24: return TextureFormat::rgb9e5ufloat;
    case 25: return TextureFormat::rgb10a2uint;
    case 26: return TextureFormat::rgb10a2unorm;
    case 27: return TextureFormat::rg11b10ufloat;
    case 28: return TextureFormat::rg32uint;
    case 29: return TextureFormat::rg32sint;
    case 30: return TextureFormat::rg32float;
    case 31: return TextureFormat::rgba16uint;
    case 32: return TextureFormat::rgba16sint;
    case 33: return TextureFormat::rgba16float;
    case 34: return TextureFormat::rgba32uint;
    case 35: return TextureFormat::rgba32sint;
    case 36: return TextureFormat::rgba32float;
    case 37: return TextureFormat::stencil8;
    case 38: return TextureFormat::depth16unorm;
    case 39: return TextureFormat::depth24plus;
    case 40: return TextureFormat::depth24plus_stencil8;
    case 41: return TextureFormat::depth32float;
    case 42: return TextureFormat::depth32float_stencil8;
    case 43: return TextureFormat::bc1_rgba_unorm;
    case 44: return TextureFormat::bc1_rgba_unorm_srgb;
    case 45: return TextureFormat::bc2_rgba_unorm;
    case 46: return TextureFormat::bc2_rgba_unorm_srgb;
    case 47: return TextureFormat::bc3_rgba_unorm;
    case 48: return TextureFormat::bc3_rgba_unorm_srgb;
    case 49: return TextureFormat::bc4_r_unorm;
    case 50: return TextureFormat::bc4_r_snorm;
    case 51: return TextureFormat::bc5_rg_unorm;
    case 52: return TextureFormat::bc5_rg_snorm;
    case 53: return TextureFormat::bc6h_rgb_ufloat;
    case 54: return TextureFormat::bc6h_rgb_float;
    case 55: return TextureFormat::bc7_rgba_unorm;
    case 56: return TextureFormat::bc7_rgba_unorm_srgb;
    case 57: return TextureFormat::etc2_rgb8unorm;
    case 58: return TextureFormat::etc2_rgb8unorm_srgb;
    case 59: return TextureFormat::etc2_rgb8a1unorm;
    case 60: return TextureFormat::etc2_rgb8a1unorm_srgb;
    case 61: return TextureFormat::etc2_rgba8unorm;
    case 62: return TextureFormat::etc2_rgba8unorm_srgb;
    case 63: return TextureFormat::eac_r11unorm;
    case 64: return TextureFormat::eac_r11snorm;
    case 65: return TextureFormat::eac_rg11unorm;
    case 66: return TextureFormat::eac_rg11snorm;
    case 67: return TextureFormat::astc_4x4_unorm;
    case 68: return TextureFormat::astc_4x4_unorm_srgb;
    case 69: return TextureFormat::astc_5x4_unorm;
    case 70: return TextureFormat::astc_5x4_unorm_srgb;
    case 71: return TextureFormat::astc_5x5_unorm;
    case 72: return TextureFormat::astc_5x5_unorm_srgb;
    case 73: return TextureFormat::astc_6x5_unorm;
    case 74: return TextureFormat::astc_6x5_unorm_srgb;
    case 75: return TextureFormat::astc_6x6_unorm;
    case 76: return TextureFormat::astc_6x6_unorm_srgb;
    case 77: return TextureFormat::astc_8x5_unorm;
    case 78: return TextureFormat::astc_8x5_unorm_srgb;
    case 79: return TextureFormat::astc_8x6_unorm;
    case 80: return TextureFormat::astc_8x6_unorm_srgb;
    case 81: return TextureFormat::astc_8x8_unorm;
    case 82: return TextureFormat::astc_8x8_unorm_srgb;
    case 83: return TextureFormat::astc_10x5_unorm;
    case 84: return TextureFormat::astc_10x5_unorm_srgb;
    case 85: return TextureFormat::astc_10x6_unorm;
    case 86: return TextureFormat::astc_10x6_unorm_srgb;
    case 87: return TextureFormat::astc_10x8_unorm;
    case 88: return TextureFormat::astc_10x8_unorm_srgb;
    case 89: return TextureFormat::astc_10x10_unorm;
    case 90: return TextureFormat::astc_10x10_unorm_srgb;
    case 91: return TextureFormat::astc_12x10_unorm;
    case 92: return TextureFormat::astc_12x10_unorm_srgb;
    case 93: return TextureFormat::astc_12x12_unorm;
    case 94: return TextureFormat::astc_12x12_unorm_srgb;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 95> TEXTURE_FORMATS{"r8unorm", "r8snorm", "r8uint", "r8sint", "r16uint", "r16sint", "r16float", "rg8unorm", "rg8snorm", "rg8uint", "rg8sint", "r32uint", "r32sint", "r32float", "rg16uint", "rg16sint", "rg16float", "rgba8unorm", "rgba8unorm-srgb", "rgba8snorm", "rgba8uint", "rgba8sint", "bgra8unorm", "bgra8unorm-srgb", "rgb9e5ufloat", "rgb10a2uint", "rgb10a2unorm", "rg11b10ufloat", "rg32uint", "rg32sint", "rg32float", "rgba16uint", "rgba16sint", "rgba16float", "rgba32uint", "rgba32sint", "rgba32float", "stencil8", "depth16unorm", "depth24plus", "depth24plus-stencil8", "depth32float", "depth32float-stencil8", "bc1-rgba-unorm", "bc1-rgba-unorm-srgb", "bc2-rgba-unorm", "bc2-rgba-unorm-srgb", "bc3-rgba-unorm", "bc3-rgba-unorm-srgb", "bc4-r-unorm", "bc4-r-snorm", "bc5-rg-unorm", "bc5-rg-snorm", "bc6h-rgb-ufloat", "bc6h-rgb-float", "bc7-rgba-unorm", "bc7-rgba-unorm-srgb", "etc2-rgb8unorm", "etc2-rgb8unorm-srgb", "etc2-rgb8a1unorm", "etc2-rgb8a1unorm-srgb", "etc2-rgba8unorm", "etc2-rgba8unorm-srgb", "eac-r11unorm", "eac-r11snorm", "eac-rg11unorm", "eac-rg11snorm", "astc-4x4-unorm", "astc-4x4-unorm-srgb", "astc-5x4-unorm", "astc-5x4-unorm-srgb", "astc-5x5-unorm", "astc-5x5-unorm-srgb", "astc-6x5-unorm", "astc-6x5-unorm-srgb", "astc-6x6-unorm", "astc-6x6-unorm-srgb", "astc-8x5-unorm", "astc-8x5-unorm-srgb", "astc-8x6-unorm", "astc-8x6-unorm-srgb", "astc-8x8-unorm", "astc-8x8-unorm-srgb", "astc-10x5-unorm", "astc-10x5-unorm-srgb", "astc-10x6-unorm", "astc-10x6-unorm-srgb", "astc-10x8-unorm", "astc-10x8-unorm-srgb", "astc-10x10-unorm", "astc-10x10-unorm-srgb", "astc-12x10-unorm", "astc-12x10-unorm-srgb", "astc-12x12-unorm", "astc-12x12-unorm-srgb"};

// These are the members of GPUVertexFormat.
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

// The member `val` is, or none for a number no member has.
constexpr std::optional<VertexFormat> vertex_format_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return VertexFormat::uint8;
    case 1: return VertexFormat::uint8x2;
    case 2: return VertexFormat::uint8x4;
    case 3: return VertexFormat::sint8;
    case 4: return VertexFormat::sint8x2;
    case 5: return VertexFormat::sint8x4;
    case 6: return VertexFormat::unorm8;
    case 7: return VertexFormat::unorm8x2;
    case 8: return VertexFormat::unorm8x4;
    case 9: return VertexFormat::snorm8;
    case 10: return VertexFormat::snorm8x2;
    case 11: return VertexFormat::snorm8x4;
    case 12: return VertexFormat::uint16;
    case 13: return VertexFormat::uint16x2;
    case 14: return VertexFormat::uint16x4;
    case 15: return VertexFormat::sint16;
    case 16: return VertexFormat::sint16x2;
    case 17: return VertexFormat::sint16x4;
    case 18: return VertexFormat::unorm16;
    case 19: return VertexFormat::unorm16x2;
    case 20: return VertexFormat::unorm16x4;
    case 21: return VertexFormat::snorm16;
    case 22: return VertexFormat::snorm16x2;
    case 23: return VertexFormat::snorm16x4;
    case 24: return VertexFormat::float16;
    case 25: return VertexFormat::float16x2;
    case 26: return VertexFormat::float16x4;
    case 27: return VertexFormat::float32;
    case 28: return VertexFormat::float32x2;
    case 29: return VertexFormat::float32x3;
    case 30: return VertexFormat::float32x4;
    case 31: return VertexFormat::uint32;
    case 32: return VertexFormat::uint32x2;
    case 33: return VertexFormat::uint32x3;
    case 34: return VertexFormat::uint32x4;
    case 35: return VertexFormat::sint32;
    case 36: return VertexFormat::sint32x2;
    case 37: return VertexFormat::sint32x3;
    case 38: return VertexFormat::sint32x4;
    case 39: return VertexFormat::unorm10_10_10_2;
    case 40: return VertexFormat::unorm8x4_bgra;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 41> VERTEX_FORMATS{"uint8", "uint8x2", "uint8x4", "sint8", "sint8x2", "sint8x4", "unorm8", "unorm8x2", "unorm8x4", "snorm8", "snorm8x2", "snorm8x4", "uint16", "uint16x2", "uint16x4", "sint16", "sint16x2", "sint16x4", "unorm16", "unorm16x2", "unorm16x4", "snorm16", "snorm16x2", "snorm16x4", "float16", "float16x2", "float16x4", "float32", "float32x2", "float32x3", "float32x4", "uint32", "uint32x2", "uint32x3", "uint32x4", "sint32", "sint32x2", "sint32x3", "sint32x4", "unorm10-10-10-2", "unorm8x4-bgra"};

enum class PrimitiveTopology : std::uint8_t {
    point_list = 0,
    line_list = 1,
    line_strip = 2,
    triangle_list = 3,
    triangle_strip = 4,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<PrimitiveTopology> primitive_topology_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return PrimitiveTopology::point_list;
    case 1: return PrimitiveTopology::line_list;
    case 2: return PrimitiveTopology::line_strip;
    case 3: return PrimitiveTopology::triangle_list;
    case 4: return PrimitiveTopology::triangle_strip;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 5> PRIMITIVE_TOPOLOGIES{"point-list", "line-list", "line-strip", "triangle-list", "triangle-strip"};

enum class IndexFormat : std::uint8_t {
    uint16 = 0,
    uint32 = 1,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<IndexFormat> index_format_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return IndexFormat::uint16;
    case 1: return IndexFormat::uint32;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 2> INDEX_FORMATS{"uint16", "uint32"};

enum class FrontFace : std::uint8_t {
    ccw = 0,
    cw = 1,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<FrontFace> front_face_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return FrontFace::ccw;
    case 1: return FrontFace::cw;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 2> FRONT_FACES{"ccw", "cw"};

enum class CullMode : std::uint8_t {
    none = 0,
    front = 1,
    back = 2,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<CullMode> cull_mode_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return CullMode::none;
    case 1: return CullMode::front;
    case 2: return CullMode::back;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 3> CULL_MODES{"none", "front", "back"};

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

// The member `val` is, or none for a number no member has.
constexpr std::optional<CompareFunction> compare_function_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return CompareFunction::never;
    case 1: return CompareFunction::less;
    case 2: return CompareFunction::equal;
    case 3: return CompareFunction::less_equal;
    case 4: return CompareFunction::greater;
    case 5: return CompareFunction::not_equal;
    case 6: return CompareFunction::greater_equal;
    case 7: return CompareFunction::always;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 8> COMPARE_FUNCTIONS{"never", "less", "equal", "less-equal", "greater", "not-equal", "greater-equal", "always"};

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

// The member `val` is, or none for a number no member has.
constexpr std::optional<StencilOperation> stencil_operation_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return StencilOperation::keep;
    case 1: return StencilOperation::zero;
    case 2: return StencilOperation::replace;
    case 3: return StencilOperation::invert;
    case 4: return StencilOperation::increment_clamp;
    case 5: return StencilOperation::decrement_clamp;
    case 6: return StencilOperation::increment_wrap;
    case 7: return StencilOperation::decrement_wrap;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 8> STENCIL_OPERATIONS{"keep", "zero", "replace", "invert", "increment-clamp", "decrement-clamp", "increment-wrap", "decrement-wrap"};

enum class BlendOperation : std::uint8_t {
    add = 0,
    subtract = 1,
    reverse_subtract = 2,
    min = 3,
    max = 4,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<BlendOperation> blend_operation_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return BlendOperation::add;
    case 1: return BlendOperation::subtract;
    case 2: return BlendOperation::reverse_subtract;
    case 3: return BlendOperation::min;
    case 4: return BlendOperation::max;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 5> BLEND_OPERATIONS{"add", "subtract", "reverse-subtract", "min", "max"};

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

// The member `val` is, or none for a number no member has.
constexpr std::optional<BlendFactor> blend_factor_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return BlendFactor::zero;
    case 1: return BlendFactor::one;
    case 2: return BlendFactor::src;
    case 3: return BlendFactor::one_minus_src;
    case 4: return BlendFactor::src_alpha;
    case 5: return BlendFactor::one_minus_src_alpha;
    case 6: return BlendFactor::dst;
    case 7: return BlendFactor::one_minus_dst;
    case 8: return BlendFactor::dst_alpha;
    case 9: return BlendFactor::one_minus_dst_alpha;
    case 10: return BlendFactor::src_alpha_saturated;
    case 11: return BlendFactor::constant;
    case 12: return BlendFactor::one_minus_constant;
    case 13: return BlendFactor::src1;
    case 14: return BlendFactor::one_minus_src1;
    case 15: return BlendFactor::src1_alpha;
    case 16: return BlendFactor::one_minus_src1_alpha;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 17> BLEND_FACTORS{"zero", "one", "src", "one-minus-src", "src-alpha", "one-minus-src-alpha", "dst", "one-minus-dst", "dst-alpha", "one-minus-dst-alpha", "src-alpha-saturated", "constant", "one-minus-constant", "src1", "one-minus-src1", "src1-alpha", "one-minus-src1-alpha"};

enum class VertexStepMode : std::uint8_t {
    vertex = 0,
    instance = 1,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<VertexStepMode> vertex_step_mode_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return VertexStepMode::vertex;
    case 1: return VertexStepMode::instance;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 2> VERTEX_STEP_MODES{"vertex", "instance"};

enum class AddressMode : std::uint8_t {
    clamp_to_edge = 0,
    repeat = 1,
    mirror_repeat = 2,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<AddressMode> address_mode_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return AddressMode::clamp_to_edge;
    case 1: return AddressMode::repeat;
    case 2: return AddressMode::mirror_repeat;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 3> ADDRESS_MODES{"clamp-to-edge", "repeat", "mirror-repeat"};

enum class FilterMode : std::uint8_t {
    nearest = 0,
    linear = 1,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<FilterMode> filter_mode_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return FilterMode::nearest;
    case 1: return FilterMode::linear;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 2> FILTER_MODES{"nearest", "linear"};

enum class MipmapFilterMode : std::uint8_t {
    nearest = 0,
    linear = 1,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<MipmapFilterMode> mipmap_filter_mode_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return MipmapFilterMode::nearest;
    case 1: return MipmapFilterMode::linear;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 2> MIPMAP_FILTER_MODES{"nearest", "linear"};

enum class TextureDimension : std::uint8_t {
    _1d = 0,
    _2d = 1,
    _3d = 2,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<TextureDimension> texture_dimension_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return TextureDimension::_1d;
    case 1: return TextureDimension::_2d;
    case 2: return TextureDimension::_3d;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 3> TEXTURE_DIMENSIONS{"1d", "2d", "3d"};

enum class TextureViewDimension : std::uint8_t {
    _1d = 0,
    _2d = 1,
    _2d_array = 2,
    cube = 3,
    cube_array = 4,
    _3d = 5,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<TextureViewDimension> texture_view_dimension_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return TextureViewDimension::_1d;
    case 1: return TextureViewDimension::_2d;
    case 2: return TextureViewDimension::_2d_array;
    case 3: return TextureViewDimension::cube;
    case 4: return TextureViewDimension::cube_array;
    case 5: return TextureViewDimension::_3d;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 6> TEXTURE_VIEW_DIMENSIONS{"1d", "2d", "2d-array", "cube", "cube-array", "3d"};

enum class TextureAspect : std::uint8_t {
    all = 0,
    stencil_only = 1,
    depth_only = 2,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<TextureAspect> texture_aspect_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return TextureAspect::all;
    case 1: return TextureAspect::stencil_only;
    case 2: return TextureAspect::depth_only;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 3> TEXTURE_ASPECTS{"all", "stencil-only", "depth-only"};

enum class BufferBindingType : std::uint8_t {
    uniform = 0,
    storage = 1,
    read_only_storage = 2,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<BufferBindingType> buffer_binding_type_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return BufferBindingType::uniform;
    case 1: return BufferBindingType::storage;
    case 2: return BufferBindingType::read_only_storage;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 3> BUFFER_BINDING_TYPES{"uniform", "storage", "read-only-storage"};

enum class SamplerBindingType : std::uint8_t {
    filtering = 0,
    non_filtering = 1,
    comparison = 2,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<SamplerBindingType> sampler_binding_type_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return SamplerBindingType::filtering;
    case 1: return SamplerBindingType::non_filtering;
    case 2: return SamplerBindingType::comparison;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 3> SAMPLER_BINDING_TYPES{"filtering", "non-filtering", "comparison"};

enum class TextureSampleType : std::uint8_t {
    float_ = 0,
    unfilterable_float = 1,
    depth = 2,
    sint = 3,
    uint = 4,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<TextureSampleType> texture_sample_type_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return TextureSampleType::float_;
    case 1: return TextureSampleType::unfilterable_float;
    case 2: return TextureSampleType::depth;
    case 3: return TextureSampleType::sint;
    case 4: return TextureSampleType::uint;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 5> TEXTURE_SAMPLE_TYPES{"float", "unfilterable-float", "depth", "sint", "uint"};

enum class StorageTextureAccess : std::uint8_t {
    write_only = 0,
    read_only = 1,
    read_write = 2,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<StorageTextureAccess> storage_texture_access_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return StorageTextureAccess::write_only;
    case 1: return StorageTextureAccess::read_only;
    case 2: return StorageTextureAccess::read_write;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 3> STORAGE_TEXTURE_ACCESSES{"write-only", "read-only", "read-write"};

enum class LoadOp : std::uint8_t {
    load = 0,
    clear = 1,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<LoadOp> load_op_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return LoadOp::load;
    case 1: return LoadOp::clear;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 2> LOAD_OPS{"load", "clear"};

enum class StoreOp : std::uint8_t {
    store = 0,
    discard = 1,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<StoreOp> store_op_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return StoreOp::store;
    case 1: return StoreOp::discard;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 2> STORE_OPS{"store", "discard"};

enum class QueryType : std::uint8_t {
    occlusion = 0,
    timestamp = 1,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<QueryType> query_type_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return QueryType::occlusion;
    case 1: return QueryType::timestamp;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 2> QUERY_TYPES{"occlusion", "timestamp"};

enum class CanvasAlphaMode : std::uint8_t {
    opaque = 0,
    premultiplied = 1,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<CanvasAlphaMode> canvas_alpha_mode_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return CanvasAlphaMode::opaque;
    case 1: return CanvasAlphaMode::premultiplied;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 2> CANVAS_ALPHA_MODES{"opaque", "premultiplied"};

// GPUFeatureName. The DEVICE record (api.json) reports a device's features as one bit each,
// at the member's place.
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

// The member `val` is, or none for a number no member has.
constexpr std::optional<FeatureName> feature_name_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return FeatureName::depth_clip_control;
    case 1: return FeatureName::depth32float_stencil8;
    case 2: return FeatureName::texture_compression_bc;
    case 3: return FeatureName::texture_compression_bc_sliced_3d;
    case 4: return FeatureName::texture_compression_etc2;
    case 5: return FeatureName::texture_compression_astc;
    case 6: return FeatureName::texture_compression_astc_sliced_3d;
    case 7: return FeatureName::timestamp_query;
    case 8: return FeatureName::indirect_first_instance;
    case 9: return FeatureName::shader_f16;
    case 10: return FeatureName::rg11b10ufloat_renderable;
    case 11: return FeatureName::bgra8unorm_storage;
    case 12: return FeatureName::float32_filterable;
    case 13: return FeatureName::float32_blendable;
    case 14: return FeatureName::clip_distances;
    case 15: return FeatureName::dual_source_blending;
    case 16: return FeatureName::subgroups;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 17> FEATURE_NAMES{"depth-clip-control", "depth32float-stencil8", "texture-compression-bc", "texture-compression-bc-sliced-3d", "texture-compression-etc2", "texture-compression-astc", "texture-compression-astc-sliced-3d", "timestamp-query", "indirect-first-instance", "shader-f16", "rg11b10ufloat-renderable", "bgra8unorm-storage", "float32-filterable", "float32-blendable", "clip-distances", "dual-source-blending", "subgroups"};

// The members of GPUSupportedLimits. The DEVICE record (api.json) reports a device's limits
// as one word each, at the member's place.
enum class LimitName : std::uint8_t {
    max_texture_dimension_1d = 0,
    max_texture_dimension_2d = 1,
    max_texture_dimension_3d = 2,
    max_texture_array_layers = 3,
    max_bind_groups = 4,
    max_bind_groups_plus_vertex_buffers = 5,
    max_bindings_per_bind_group = 6,
    max_dynamic_uniform_buffers_per_pipeline_layout = 7,
    max_dynamic_storage_buffers_per_pipeline_layout = 8,
    max_sampled_textures_per_shader_stage = 9,
    max_samplers_per_shader_stage = 10,
    max_storage_buffers_per_shader_stage = 11,
    max_storage_textures_per_shader_stage = 12,
    max_uniform_buffers_per_shader_stage = 13,
    max_uniform_buffer_binding_size = 14,
    max_storage_buffer_binding_size = 15,
    min_uniform_buffer_offset_alignment = 16,
    min_storage_buffer_offset_alignment = 17,
    max_vertex_buffers = 18,
    max_buffer_size = 19,
    max_vertex_attributes = 20,
    max_vertex_buffer_array_stride = 21,
    max_inter_stage_shader_variables = 22,
    max_color_attachments = 23,
    max_color_attachment_bytes_per_sample = 24,
    max_compute_workgroup_storage_size = 25,
    max_compute_invocations_per_workgroup = 26,
    max_compute_workgroup_size_x = 27,
    max_compute_workgroup_size_y = 28,
    max_compute_workgroup_size_z = 29,
    max_compute_workgroups_per_dimension = 30,
};

// The member `val` is, or none for a number no member has.
constexpr std::optional<LimitName> limit_name_of(std::uint8_t val) noexcept {
    switch (val) {
    case 0: return LimitName::max_texture_dimension_1d;
    case 1: return LimitName::max_texture_dimension_2d;
    case 2: return LimitName::max_texture_dimension_3d;
    case 3: return LimitName::max_texture_array_layers;
    case 4: return LimitName::max_bind_groups;
    case 5: return LimitName::max_bind_groups_plus_vertex_buffers;
    case 6: return LimitName::max_bindings_per_bind_group;
    case 7: return LimitName::max_dynamic_uniform_buffers_per_pipeline_layout;
    case 8: return LimitName::max_dynamic_storage_buffers_per_pipeline_layout;
    case 9: return LimitName::max_sampled_textures_per_shader_stage;
    case 10: return LimitName::max_samplers_per_shader_stage;
    case 11: return LimitName::max_storage_buffers_per_shader_stage;
    case 12: return LimitName::max_storage_textures_per_shader_stage;
    case 13: return LimitName::max_uniform_buffers_per_shader_stage;
    case 14: return LimitName::max_uniform_buffer_binding_size;
    case 15: return LimitName::max_storage_buffer_binding_size;
    case 16: return LimitName::min_uniform_buffer_offset_alignment;
    case 17: return LimitName::min_storage_buffer_offset_alignment;
    case 18: return LimitName::max_vertex_buffers;
    case 19: return LimitName::max_buffer_size;
    case 20: return LimitName::max_vertex_attributes;
    case 21: return LimitName::max_vertex_buffer_array_stride;
    case 22: return LimitName::max_inter_stage_shader_variables;
    case 23: return LimitName::max_color_attachments;
    case 24: return LimitName::max_color_attachment_bytes_per_sample;
    case 25: return LimitName::max_compute_workgroup_storage_size;
    case 26: return LimitName::max_compute_invocations_per_workgroup;
    case 27: return LimitName::max_compute_workgroup_size_x;
    case 28: return LimitName::max_compute_workgroup_size_y;
    case 29: return LimitName::max_compute_workgroup_size_z;
    case 30: return LimitName::max_compute_workgroups_per_dimension;
    }
    return std::nullopt;
}

inline constexpr std::array<std::string_view, 31> LIMIT_NAMES{"maxTextureDimension1D", "maxTextureDimension2D", "maxTextureDimension3D", "maxTextureArrayLayers", "maxBindGroups", "maxBindGroupsPlusVertexBuffers", "maxBindingsPerBindGroup", "maxDynamicUniformBuffersPerPipelineLayout", "maxDynamicStorageBuffersPerPipelineLayout", "maxSampledTexturesPerShaderStage", "maxSamplersPerShaderStage", "maxStorageBuffersPerShaderStage", "maxStorageTexturesPerShaderStage", "maxUniformBuffersPerShaderStage", "maxUniformBufferBindingSize", "maxStorageBufferBindingSize", "minUniformBufferOffsetAlignment", "minStorageBufferOffsetAlignment", "maxVertexBuffers", "maxBufferSize", "maxVertexAttributes", "maxVertexBufferArrayStride", "maxInterStageShaderVariables", "maxColorAttachments", "maxColorAttachmentBytesPerSample", "maxComputeWorkgroupStorageSize", "maxComputeInvocationsPerWorkgroup", "maxComputeWorkgroupSizeX", "maxComputeWorkgroupSizeY", "maxComputeWorkgroupSizeZ", "maxComputeWorkgroupsPerDimension"};

} // namespace xpute::gpu
