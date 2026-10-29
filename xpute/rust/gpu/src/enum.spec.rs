// xpute-gpu/enum.spec.rs
//
// GENERATED from spec/gpu/enum.json — do not edit.
//
// WebGPU's own enumerations, each its spec's strings in order: a number is its
// place in the list, so a host hands the string to the device as it is and a guest
// names a member by its generated constant. Nothing here is any one program's; a
// value WebGPU leaves optional is NONE (api.json) where unset.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    non_camel_case_types,
    dead_code
)]

/// GPUTextureFormat, the core spec's and its texture-compression features'.
pub const TEXTURE_FORMAT: &[&str] = &[
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
];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TextureFormat {
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
}

impl TextureFormat {
    /// Every member, in the JSON's own order.
    pub const ALL: [TextureFormat; 95] = [
        TextureFormat::r8unorm,
        TextureFormat::r8snorm,
        TextureFormat::r8uint,
        TextureFormat::r8sint,
        TextureFormat::r16uint,
        TextureFormat::r16sint,
        TextureFormat::r16float,
        TextureFormat::rg8unorm,
        TextureFormat::rg8snorm,
        TextureFormat::rg8uint,
        TextureFormat::rg8sint,
        TextureFormat::r32uint,
        TextureFormat::r32sint,
        TextureFormat::r32float,
        TextureFormat::rg16uint,
        TextureFormat::rg16sint,
        TextureFormat::rg16float,
        TextureFormat::rgba8unorm,
        TextureFormat::rgba8unorm_srgb,
        TextureFormat::rgba8snorm,
        TextureFormat::rgba8uint,
        TextureFormat::rgba8sint,
        TextureFormat::bgra8unorm,
        TextureFormat::bgra8unorm_srgb,
        TextureFormat::rgb9e5ufloat,
        TextureFormat::rgb10a2uint,
        TextureFormat::rgb10a2unorm,
        TextureFormat::rg11b10ufloat,
        TextureFormat::rg32uint,
        TextureFormat::rg32sint,
        TextureFormat::rg32float,
        TextureFormat::rgba16uint,
        TextureFormat::rgba16sint,
        TextureFormat::rgba16float,
        TextureFormat::rgba32uint,
        TextureFormat::rgba32sint,
        TextureFormat::rgba32float,
        TextureFormat::stencil8,
        TextureFormat::depth16unorm,
        TextureFormat::depth24plus,
        TextureFormat::depth24plus_stencil8,
        TextureFormat::depth32float,
        TextureFormat::depth32float_stencil8,
        TextureFormat::bc1_rgba_unorm,
        TextureFormat::bc1_rgba_unorm_srgb,
        TextureFormat::bc2_rgba_unorm,
        TextureFormat::bc2_rgba_unorm_srgb,
        TextureFormat::bc3_rgba_unorm,
        TextureFormat::bc3_rgba_unorm_srgb,
        TextureFormat::bc4_r_unorm,
        TextureFormat::bc4_r_snorm,
        TextureFormat::bc5_rg_unorm,
        TextureFormat::bc5_rg_snorm,
        TextureFormat::bc6h_rgb_ufloat,
        TextureFormat::bc6h_rgb_float,
        TextureFormat::bc7_rgba_unorm,
        TextureFormat::bc7_rgba_unorm_srgb,
        TextureFormat::etc2_rgb8unorm,
        TextureFormat::etc2_rgb8unorm_srgb,
        TextureFormat::etc2_rgb8a1unorm,
        TextureFormat::etc2_rgb8a1unorm_srgb,
        TextureFormat::etc2_rgba8unorm,
        TextureFormat::etc2_rgba8unorm_srgb,
        TextureFormat::eac_r11unorm,
        TextureFormat::eac_r11snorm,
        TextureFormat::eac_rg11unorm,
        TextureFormat::eac_rg11snorm,
        TextureFormat::astc_4x4_unorm,
        TextureFormat::astc_4x4_unorm_srgb,
        TextureFormat::astc_5x4_unorm,
        TextureFormat::astc_5x4_unorm_srgb,
        TextureFormat::astc_5x5_unorm,
        TextureFormat::astc_5x5_unorm_srgb,
        TextureFormat::astc_6x5_unorm,
        TextureFormat::astc_6x5_unorm_srgb,
        TextureFormat::astc_6x6_unorm,
        TextureFormat::astc_6x6_unorm_srgb,
        TextureFormat::astc_8x5_unorm,
        TextureFormat::astc_8x5_unorm_srgb,
        TextureFormat::astc_8x6_unorm,
        TextureFormat::astc_8x6_unorm_srgb,
        TextureFormat::astc_8x8_unorm,
        TextureFormat::astc_8x8_unorm_srgb,
        TextureFormat::astc_10x5_unorm,
        TextureFormat::astc_10x5_unorm_srgb,
        TextureFormat::astc_10x6_unorm,
        TextureFormat::astc_10x6_unorm_srgb,
        TextureFormat::astc_10x8_unorm,
        TextureFormat::astc_10x8_unorm_srgb,
        TextureFormat::astc_10x10_unorm,
        TextureFormat::astc_10x10_unorm_srgb,
        TextureFormat::astc_12x10_unorm,
        TextureFormat::astc_12x10_unorm_srgb,
        TextureFormat::astc_12x12_unorm,
        TextureFormat::astc_12x12_unorm_srgb,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<TextureFormat> {
        TextureFormat::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

/// GPUVertexFormat.
pub const VERTEX_FORMAT: &[&str] = &[
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
];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum VertexFormat {
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
}

impl VertexFormat {
    /// Every member, in the JSON's own order.
    pub const ALL: [VertexFormat; 41] = [
        VertexFormat::uint8,
        VertexFormat::uint8x2,
        VertexFormat::uint8x4,
        VertexFormat::sint8,
        VertexFormat::sint8x2,
        VertexFormat::sint8x4,
        VertexFormat::unorm8,
        VertexFormat::unorm8x2,
        VertexFormat::unorm8x4,
        VertexFormat::snorm8,
        VertexFormat::snorm8x2,
        VertexFormat::snorm8x4,
        VertexFormat::uint16,
        VertexFormat::uint16x2,
        VertexFormat::uint16x4,
        VertexFormat::sint16,
        VertexFormat::sint16x2,
        VertexFormat::sint16x4,
        VertexFormat::unorm16,
        VertexFormat::unorm16x2,
        VertexFormat::unorm16x4,
        VertexFormat::snorm16,
        VertexFormat::snorm16x2,
        VertexFormat::snorm16x4,
        VertexFormat::float16,
        VertexFormat::float16x2,
        VertexFormat::float16x4,
        VertexFormat::float32,
        VertexFormat::float32x2,
        VertexFormat::float32x3,
        VertexFormat::float32x4,
        VertexFormat::uint32,
        VertexFormat::uint32x2,
        VertexFormat::uint32x3,
        VertexFormat::uint32x4,
        VertexFormat::sint32,
        VertexFormat::sint32x2,
        VertexFormat::sint32x3,
        VertexFormat::sint32x4,
        VertexFormat::unorm10_10_10_2,
        VertexFormat::unorm8x4_bgra,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<VertexFormat> {
        VertexFormat::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

pub const PRIMITIVE_TOPOLOGY: &[&str] = &[
    "point-list",
    "line-list",
    "line-strip",
    "triangle-list",
    "triangle-strip",
];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PrimitiveTopology {
    point_list = 0,
    line_list = 1,
    line_strip = 2,
    triangle_list = 3,
    triangle_strip = 4,
}

impl PrimitiveTopology {
    /// Every member, in the JSON's own order.
    pub const ALL: [PrimitiveTopology; 5] = [
        PrimitiveTopology::point_list,
        PrimitiveTopology::line_list,
        PrimitiveTopology::line_strip,
        PrimitiveTopology::triangle_list,
        PrimitiveTopology::triangle_strip,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<PrimitiveTopology> {
        PrimitiveTopology::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

pub const INDEX_FORMAT: &[&str] = &["uint16", "uint32"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum IndexFormat {
    uint16 = 0,
    uint32 = 1,
}

impl IndexFormat {
    /// Every member, in the JSON's own order.
    pub const ALL: [IndexFormat; 2] = [IndexFormat::uint16, IndexFormat::uint32];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<IndexFormat> {
        IndexFormat::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

pub const FRONT_FACE: &[&str] = &["ccw", "cw"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FrontFace {
    ccw = 0,
    cw = 1,
}

impl FrontFace {
    /// Every member, in the JSON's own order.
    pub const ALL: [FrontFace; 2] = [FrontFace::ccw, FrontFace::cw];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<FrontFace> {
        FrontFace::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

pub const CULL_MODE: &[&str] = &["none", "front", "back"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CullMode {
    none = 0,
    front = 1,
    back = 2,
}

impl CullMode {
    /// Every member, in the JSON's own order.
    pub const ALL: [CullMode; 3] = [CullMode::none, CullMode::front, CullMode::back];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<CullMode> {
        CullMode::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

pub const COMPARE_FUNCTION: &[&str] = &[
    "never",
    "less",
    "equal",
    "less-equal",
    "greater",
    "not-equal",
    "greater-equal",
    "always",
];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CompareFunction {
    never = 0,
    less = 1,
    equal = 2,
    less_equal = 3,
    greater = 4,
    not_equal = 5,
    greater_equal = 6,
    always = 7,
}

impl CompareFunction {
    /// Every member, in the JSON's own order.
    pub const ALL: [CompareFunction; 8] = [
        CompareFunction::never,
        CompareFunction::less,
        CompareFunction::equal,
        CompareFunction::less_equal,
        CompareFunction::greater,
        CompareFunction::not_equal,
        CompareFunction::greater_equal,
        CompareFunction::always,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<CompareFunction> {
        CompareFunction::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

pub const STENCIL_OPERATION: &[&str] = &[
    "keep",
    "zero",
    "replace",
    "invert",
    "increment-clamp",
    "decrement-clamp",
    "increment-wrap",
    "decrement-wrap",
];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StencilOperation {
    keep = 0,
    zero = 1,
    replace = 2,
    invert = 3,
    increment_clamp = 4,
    decrement_clamp = 5,
    increment_wrap = 6,
    decrement_wrap = 7,
}

impl StencilOperation {
    /// Every member, in the JSON's own order.
    pub const ALL: [StencilOperation; 8] = [
        StencilOperation::keep,
        StencilOperation::zero,
        StencilOperation::replace,
        StencilOperation::invert,
        StencilOperation::increment_clamp,
        StencilOperation::decrement_clamp,
        StencilOperation::increment_wrap,
        StencilOperation::decrement_wrap,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<StencilOperation> {
        StencilOperation::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

pub const BLEND_OPERATION: &[&str] = &["add", "subtract", "reverse-subtract", "min", "max"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BlendOperation {
    add = 0,
    subtract = 1,
    reverse_subtract = 2,
    min = 3,
    max = 4,
}

impl BlendOperation {
    /// Every member, in the JSON's own order.
    pub const ALL: [BlendOperation; 5] = [
        BlendOperation::add,
        BlendOperation::subtract,
        BlendOperation::reverse_subtract,
        BlendOperation::min,
        BlendOperation::max,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<BlendOperation> {
        BlendOperation::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

pub const BLEND_FACTOR: &[&str] = &[
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
];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BlendFactor {
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
}

impl BlendFactor {
    /// Every member, in the JSON's own order.
    pub const ALL: [BlendFactor; 17] = [
        BlendFactor::zero,
        BlendFactor::one,
        BlendFactor::src,
        BlendFactor::one_minus_src,
        BlendFactor::src_alpha,
        BlendFactor::one_minus_src_alpha,
        BlendFactor::dst,
        BlendFactor::one_minus_dst,
        BlendFactor::dst_alpha,
        BlendFactor::one_minus_dst_alpha,
        BlendFactor::src_alpha_saturated,
        BlendFactor::constant,
        BlendFactor::one_minus_constant,
        BlendFactor::src1,
        BlendFactor::one_minus_src1,
        BlendFactor::src1_alpha,
        BlendFactor::one_minus_src1_alpha,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<BlendFactor> {
        BlendFactor::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

pub const VERTEX_STEP_MODE: &[&str] = &["vertex", "instance"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum VertexStepMode {
    vertex = 0,
    instance = 1,
}

impl VertexStepMode {
    /// Every member, in the JSON's own order.
    pub const ALL: [VertexStepMode; 2] = [VertexStepMode::vertex, VertexStepMode::instance];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<VertexStepMode> {
        VertexStepMode::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

pub const ADDRESS_MODE: &[&str] = &["clamp-to-edge", "repeat", "mirror-repeat"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AddressMode {
    clamp_to_edge = 0,
    repeat = 1,
    mirror_repeat = 2,
}

impl AddressMode {
    /// Every member, in the JSON's own order.
    pub const ALL: [AddressMode; 3] = [
        AddressMode::clamp_to_edge,
        AddressMode::repeat,
        AddressMode::mirror_repeat,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<AddressMode> {
        AddressMode::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

pub const FILTER_MODE: &[&str] = &["nearest", "linear"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FilterMode {
    nearest = 0,
    linear = 1,
}

impl FilterMode {
    /// Every member, in the JSON's own order.
    pub const ALL: [FilterMode; 2] = [FilterMode::nearest, FilterMode::linear];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<FilterMode> {
        FilterMode::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

pub const MIPMAP_FILTER_MODE: &[&str] = &["nearest", "linear"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MipmapFilterMode {
    nearest = 0,
    linear = 1,
}

impl MipmapFilterMode {
    /// Every member, in the JSON's own order.
    pub const ALL: [MipmapFilterMode; 2] = [MipmapFilterMode::nearest, MipmapFilterMode::linear];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<MipmapFilterMode> {
        MipmapFilterMode::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

pub const TEXTURE_DIMENSION: &[&str] = &["1d", "2d", "3d"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TextureDimension {
    _1d = 0,
    _2d = 1,
    _3d = 2,
}

impl TextureDimension {
    /// Every member, in the JSON's own order.
    pub const ALL: [TextureDimension; 3] = [
        TextureDimension::_1d,
        TextureDimension::_2d,
        TextureDimension::_3d,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<TextureDimension> {
        TextureDimension::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

pub const TEXTURE_VIEW_DIMENSION: &[&str] = &["1d", "2d", "2d-array", "cube", "cube-array", "3d"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TextureViewDimension {
    _1d = 0,
    _2d = 1,
    _2d_array = 2,
    cube = 3,
    cube_array = 4,
    _3d = 5,
}

impl TextureViewDimension {
    /// Every member, in the JSON's own order.
    pub const ALL: [TextureViewDimension; 6] = [
        TextureViewDimension::_1d,
        TextureViewDimension::_2d,
        TextureViewDimension::_2d_array,
        TextureViewDimension::cube,
        TextureViewDimension::cube_array,
        TextureViewDimension::_3d,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<TextureViewDimension> {
        TextureViewDimension::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

pub const TEXTURE_ASPECT: &[&str] = &["all", "stencil-only", "depth-only"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TextureAspect {
    all = 0,
    stencil_only = 1,
    depth_only = 2,
}

impl TextureAspect {
    /// Every member, in the JSON's own order.
    pub const ALL: [TextureAspect; 3] = [
        TextureAspect::all,
        TextureAspect::stencil_only,
        TextureAspect::depth_only,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<TextureAspect> {
        TextureAspect::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

pub const BUFFER_BINDING_TYPE: &[&str] = &["uniform", "storage", "read-only-storage"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BufferBindingType {
    uniform = 0,
    storage = 1,
    read_only_storage = 2,
}

impl BufferBindingType {
    /// Every member, in the JSON's own order.
    pub const ALL: [BufferBindingType; 3] = [
        BufferBindingType::uniform,
        BufferBindingType::storage,
        BufferBindingType::read_only_storage,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<BufferBindingType> {
        BufferBindingType::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

pub const SAMPLER_BINDING_TYPE: &[&str] = &["filtering", "non-filtering", "comparison"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SamplerBindingType {
    filtering = 0,
    non_filtering = 1,
    comparison = 2,
}

impl SamplerBindingType {
    /// Every member, in the JSON's own order.
    pub const ALL: [SamplerBindingType; 3] = [
        SamplerBindingType::filtering,
        SamplerBindingType::non_filtering,
        SamplerBindingType::comparison,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<SamplerBindingType> {
        SamplerBindingType::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

pub const TEXTURE_SAMPLE_TYPE: &[&str] = &["float", "unfilterable-float", "depth", "sint", "uint"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TextureSampleType {
    float = 0,
    unfilterable_float = 1,
    depth = 2,
    sint = 3,
    uint = 4,
}

impl TextureSampleType {
    /// Every member, in the JSON's own order.
    pub const ALL: [TextureSampleType; 5] = [
        TextureSampleType::float,
        TextureSampleType::unfilterable_float,
        TextureSampleType::depth,
        TextureSampleType::sint,
        TextureSampleType::uint,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<TextureSampleType> {
        TextureSampleType::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

pub const STORAGE_TEXTURE_ACCESS: &[&str] = &["write-only", "read-only", "read-write"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StorageTextureAccess {
    write_only = 0,
    read_only = 1,
    read_write = 2,
}

impl StorageTextureAccess {
    /// Every member, in the JSON's own order.
    pub const ALL: [StorageTextureAccess; 3] = [
        StorageTextureAccess::write_only,
        StorageTextureAccess::read_only,
        StorageTextureAccess::read_write,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<StorageTextureAccess> {
        StorageTextureAccess::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

pub const LOAD_OP: &[&str] = &["load", "clear"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LoadOp {
    load = 0,
    clear = 1,
}

impl LoadOp {
    /// Every member, in the JSON's own order.
    pub const ALL: [LoadOp; 2] = [LoadOp::load, LoadOp::clear];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<LoadOp> {
        LoadOp::ALL.into_iter().find(|member| *member as u8 == val)
    }
}

pub const STORE_OP: &[&str] = &["store", "discard"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StoreOp {
    store = 0,
    discard = 1,
}

impl StoreOp {
    /// Every member, in the JSON's own order.
    pub const ALL: [StoreOp; 2] = [StoreOp::store, StoreOp::discard];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<StoreOp> {
        StoreOp::ALL.into_iter().find(|member| *member as u8 == val)
    }
}

pub const QUERY_TYPE: &[&str] = &["occlusion", "timestamp"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum QueryType {
    occlusion = 0,
    timestamp = 1,
}

impl QueryType {
    /// Every member, in the JSON's own order.
    pub const ALL: [QueryType; 2] = [QueryType::occlusion, QueryType::timestamp];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<QueryType> {
        QueryType::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

pub const CANVAS_ALPHA_MODE: &[&str] = &["opaque", "premultiplied"];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CanvasAlphaMode {
    opaque = 0,
    premultiplied = 1,
}

impl CanvasAlphaMode {
    /// Every member, in the JSON's own order.
    pub const ALL: [CanvasAlphaMode; 2] = [CanvasAlphaMode::opaque, CanvasAlphaMode::premultiplied];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<CanvasAlphaMode> {
        CanvasAlphaMode::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

/// GPUFeatureName: a device's features are a bit each, by place (DEVICE in api.json).
pub const FEATURE_NAME: &[&str] = &[
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
];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FeatureName {
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
}

impl FeatureName {
    /// Every member, in the JSON's own order.
    pub const ALL: [FeatureName; 17] = [
        FeatureName::depth_clip_control,
        FeatureName::depth32float_stencil8,
        FeatureName::texture_compression_bc,
        FeatureName::texture_compression_bc_sliced_3d,
        FeatureName::texture_compression_etc2,
        FeatureName::texture_compression_astc,
        FeatureName::texture_compression_astc_sliced_3d,
        FeatureName::timestamp_query,
        FeatureName::indirect_first_instance,
        FeatureName::shader_f16,
        FeatureName::rg11b10ufloat_renderable,
        FeatureName::bgra8unorm_storage,
        FeatureName::float32_filterable,
        FeatureName::float32_blendable,
        FeatureName::clip_distances,
        FeatureName::dual_source_blending,
        FeatureName::subgroups,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<FeatureName> {
        FeatureName::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}

/// GPUSupportedLimits' members: a device's limits are a word each, by place (DEVICE in api.json).
pub const LIMIT_NAME: &[&str] = &[
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
];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LimitName {
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
}

impl LimitName {
    /// Every member, in the JSON's own order.
    pub const ALL: [LimitName; 31] = [
        LimitName::maxTextureDimension1D,
        LimitName::maxTextureDimension2D,
        LimitName::maxTextureDimension3D,
        LimitName::maxTextureArrayLayers,
        LimitName::maxBindGroups,
        LimitName::maxBindGroupsPlusVertexBuffers,
        LimitName::maxBindingsPerBindGroup,
        LimitName::maxDynamicUniformBuffersPerPipelineLayout,
        LimitName::maxDynamicStorageBuffersPerPipelineLayout,
        LimitName::maxSampledTexturesPerShaderStage,
        LimitName::maxSamplersPerShaderStage,
        LimitName::maxStorageBuffersPerShaderStage,
        LimitName::maxStorageTexturesPerShaderStage,
        LimitName::maxUniformBuffersPerShaderStage,
        LimitName::maxUniformBufferBindingSize,
        LimitName::maxStorageBufferBindingSize,
        LimitName::minUniformBufferOffsetAlignment,
        LimitName::minStorageBufferOffsetAlignment,
        LimitName::maxVertexBuffers,
        LimitName::maxBufferSize,
        LimitName::maxVertexAttributes,
        LimitName::maxVertexBufferArrayStride,
        LimitName::maxInterStageShaderVariables,
        LimitName::maxColorAttachments,
        LimitName::maxColorAttachmentBytesPerSample,
        LimitName::maxComputeWorkgroupStorageSize,
        LimitName::maxComputeInvocationsPerWorkgroup,
        LimitName::maxComputeWorkgroupSizeX,
        LimitName::maxComputeWorkgroupSizeY,
        LimitName::maxComputeWorkgroupSizeZ,
        LimitName::maxComputeWorkgroupsPerDimension,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<LimitName> {
        LimitName::ALL
            .into_iter()
            .find(|member| *member as u8 == val)
    }
}
