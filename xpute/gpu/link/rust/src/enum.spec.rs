// xpute-gpu-link/enum.spec.rs
//
// GENERATED from gpu/link/spec/enum.json (xpute-gen data.rs)
//
// WebGPU's own enumerations. Each one lists its WebGPU spec's strings in order, and a
// member's number is its place in that list. A host hands the device the string as it
// is, and a guest names a member by its generated constant. Nothing here belongs to any one
// program. A value that WebGPU leaves optional is NONE (api.json) when it is unset.

/// GPUTextureFormat: the formats of the core WebGPU spec and of its texture-compression
/// features.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TextureFormat {
    R8unorm = 0,
    R8snorm = 1,
    R8uint = 2,
    R8sint = 3,
    R16uint = 4,
    R16sint = 5,
    R16float = 6,
    Rg8unorm = 7,
    Rg8snorm = 8,
    Rg8uint = 9,
    Rg8sint = 10,
    R32uint = 11,
    R32sint = 12,
    R32float = 13,
    Rg16uint = 14,
    Rg16sint = 15,
    Rg16float = 16,
    Rgba8unorm = 17,
    Rgba8unormSrgb = 18,
    Rgba8snorm = 19,
    Rgba8uint = 20,
    Rgba8sint = 21,
    Bgra8unorm = 22,
    Bgra8unormSrgb = 23,
    Rgb9e5ufloat = 24,
    Rgb10a2uint = 25,
    Rgb10a2unorm = 26,
    Rg11b10ufloat = 27,
    Rg32uint = 28,
    Rg32sint = 29,
    Rg32float = 30,
    Rgba16uint = 31,
    Rgba16sint = 32,
    Rgba16float = 33,
    Rgba32uint = 34,
    Rgba32sint = 35,
    Rgba32float = 36,
    Stencil8 = 37,
    Depth16unorm = 38,
    Depth24plus = 39,
    Depth24plusStencil8 = 40,
    Depth32float = 41,
    Depth32floatStencil8 = 42,
    Bc1RgbaUnorm = 43,
    Bc1RgbaUnormSrgb = 44,
    Bc2RgbaUnorm = 45,
    Bc2RgbaUnormSrgb = 46,
    Bc3RgbaUnorm = 47,
    Bc3RgbaUnormSrgb = 48,
    Bc4RUnorm = 49,
    Bc4RSnorm = 50,
    Bc5RgUnorm = 51,
    Bc5RgSnorm = 52,
    Bc6hRgbUfloat = 53,
    Bc6hRgbFloat = 54,
    Bc7RgbaUnorm = 55,
    Bc7RgbaUnormSrgb = 56,
    Etc2Rgb8unorm = 57,
    Etc2Rgb8unormSrgb = 58,
    Etc2Rgb8a1unorm = 59,
    Etc2Rgb8a1unormSrgb = 60,
    Etc2Rgba8unorm = 61,
    Etc2Rgba8unormSrgb = 62,
    EacR11unorm = 63,
    EacR11snorm = 64,
    EacRg11unorm = 65,
    EacRg11snorm = 66,
    Astc4x4Unorm = 67,
    Astc4x4UnormSrgb = 68,
    Astc5x4Unorm = 69,
    Astc5x4UnormSrgb = 70,
    Astc5x5Unorm = 71,
    Astc5x5UnormSrgb = 72,
    Astc6x5Unorm = 73,
    Astc6x5UnormSrgb = 74,
    Astc6x6Unorm = 75,
    Astc6x6UnormSrgb = 76,
    Astc8x5Unorm = 77,
    Astc8x5UnormSrgb = 78,
    Astc8x6Unorm = 79,
    Astc8x6UnormSrgb = 80,
    Astc8x8Unorm = 81,
    Astc8x8UnormSrgb = 82,
    Astc10x5Unorm = 83,
    Astc10x5UnormSrgb = 84,
    Astc10x6Unorm = 85,
    Astc10x6UnormSrgb = 86,
    Astc10x8Unorm = 87,
    Astc10x8UnormSrgb = 88,
    Astc10x10Unorm = 89,
    Astc10x10UnormSrgb = 90,
    Astc12x10Unorm = 91,
    Astc12x10UnormSrgb = 92,
    Astc12x12Unorm = 93,
    Astc12x12UnormSrgb = 94,
}

impl TextureFormat {
    /// Every member, in its source's own order.
    pub const ALL: [TextureFormat; 95] = [
        TextureFormat::R8unorm,
        TextureFormat::R8snorm,
        TextureFormat::R8uint,
        TextureFormat::R8sint,
        TextureFormat::R16uint,
        TextureFormat::R16sint,
        TextureFormat::R16float,
        TextureFormat::Rg8unorm,
        TextureFormat::Rg8snorm,
        TextureFormat::Rg8uint,
        TextureFormat::Rg8sint,
        TextureFormat::R32uint,
        TextureFormat::R32sint,
        TextureFormat::R32float,
        TextureFormat::Rg16uint,
        TextureFormat::Rg16sint,
        TextureFormat::Rg16float,
        TextureFormat::Rgba8unorm,
        TextureFormat::Rgba8unormSrgb,
        TextureFormat::Rgba8snorm,
        TextureFormat::Rgba8uint,
        TextureFormat::Rgba8sint,
        TextureFormat::Bgra8unorm,
        TextureFormat::Bgra8unormSrgb,
        TextureFormat::Rgb9e5ufloat,
        TextureFormat::Rgb10a2uint,
        TextureFormat::Rgb10a2unorm,
        TextureFormat::Rg11b10ufloat,
        TextureFormat::Rg32uint,
        TextureFormat::Rg32sint,
        TextureFormat::Rg32float,
        TextureFormat::Rgba16uint,
        TextureFormat::Rgba16sint,
        TextureFormat::Rgba16float,
        TextureFormat::Rgba32uint,
        TextureFormat::Rgba32sint,
        TextureFormat::Rgba32float,
        TextureFormat::Stencil8,
        TextureFormat::Depth16unorm,
        TextureFormat::Depth24plus,
        TextureFormat::Depth24plusStencil8,
        TextureFormat::Depth32float,
        TextureFormat::Depth32floatStencil8,
        TextureFormat::Bc1RgbaUnorm,
        TextureFormat::Bc1RgbaUnormSrgb,
        TextureFormat::Bc2RgbaUnorm,
        TextureFormat::Bc2RgbaUnormSrgb,
        TextureFormat::Bc3RgbaUnorm,
        TextureFormat::Bc3RgbaUnormSrgb,
        TextureFormat::Bc4RUnorm,
        TextureFormat::Bc4RSnorm,
        TextureFormat::Bc5RgUnorm,
        TextureFormat::Bc5RgSnorm,
        TextureFormat::Bc6hRgbUfloat,
        TextureFormat::Bc6hRgbFloat,
        TextureFormat::Bc7RgbaUnorm,
        TextureFormat::Bc7RgbaUnormSrgb,
        TextureFormat::Etc2Rgb8unorm,
        TextureFormat::Etc2Rgb8unormSrgb,
        TextureFormat::Etc2Rgb8a1unorm,
        TextureFormat::Etc2Rgb8a1unormSrgb,
        TextureFormat::Etc2Rgba8unorm,
        TextureFormat::Etc2Rgba8unormSrgb,
        TextureFormat::EacR11unorm,
        TextureFormat::EacR11snorm,
        TextureFormat::EacRg11unorm,
        TextureFormat::EacRg11snorm,
        TextureFormat::Astc4x4Unorm,
        TextureFormat::Astc4x4UnormSrgb,
        TextureFormat::Astc5x4Unorm,
        TextureFormat::Astc5x4UnormSrgb,
        TextureFormat::Astc5x5Unorm,
        TextureFormat::Astc5x5UnormSrgb,
        TextureFormat::Astc6x5Unorm,
        TextureFormat::Astc6x5UnormSrgb,
        TextureFormat::Astc6x6Unorm,
        TextureFormat::Astc6x6UnormSrgb,
        TextureFormat::Astc8x5Unorm,
        TextureFormat::Astc8x5UnormSrgb,
        TextureFormat::Astc8x6Unorm,
        TextureFormat::Astc8x6UnormSrgb,
        TextureFormat::Astc8x8Unorm,
        TextureFormat::Astc8x8UnormSrgb,
        TextureFormat::Astc10x5Unorm,
        TextureFormat::Astc10x5UnormSrgb,
        TextureFormat::Astc10x6Unorm,
        TextureFormat::Astc10x6UnormSrgb,
        TextureFormat::Astc10x8Unorm,
        TextureFormat::Astc10x8UnormSrgb,
        TextureFormat::Astc10x10Unorm,
        TextureFormat::Astc10x10UnormSrgb,
        TextureFormat::Astc12x10Unorm,
        TextureFormat::Astc12x10UnormSrgb,
        TextureFormat::Astc12x12Unorm,
        TextureFormat::Astc12x12UnormSrgb,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<TextureFormat> {
        match val {
            0 => Some(TextureFormat::R8unorm),
            1 => Some(TextureFormat::R8snorm),
            2 => Some(TextureFormat::R8uint),
            3 => Some(TextureFormat::R8sint),
            4 => Some(TextureFormat::R16uint),
            5 => Some(TextureFormat::R16sint),
            6 => Some(TextureFormat::R16float),
            7 => Some(TextureFormat::Rg8unorm),
            8 => Some(TextureFormat::Rg8snorm),
            9 => Some(TextureFormat::Rg8uint),
            10 => Some(TextureFormat::Rg8sint),
            11 => Some(TextureFormat::R32uint),
            12 => Some(TextureFormat::R32sint),
            13 => Some(TextureFormat::R32float),
            14 => Some(TextureFormat::Rg16uint),
            15 => Some(TextureFormat::Rg16sint),
            16 => Some(TextureFormat::Rg16float),
            17 => Some(TextureFormat::Rgba8unorm),
            18 => Some(TextureFormat::Rgba8unormSrgb),
            19 => Some(TextureFormat::Rgba8snorm),
            20 => Some(TextureFormat::Rgba8uint),
            21 => Some(TextureFormat::Rgba8sint),
            22 => Some(TextureFormat::Bgra8unorm),
            23 => Some(TextureFormat::Bgra8unormSrgb),
            24 => Some(TextureFormat::Rgb9e5ufloat),
            25 => Some(TextureFormat::Rgb10a2uint),
            26 => Some(TextureFormat::Rgb10a2unorm),
            27 => Some(TextureFormat::Rg11b10ufloat),
            28 => Some(TextureFormat::Rg32uint),
            29 => Some(TextureFormat::Rg32sint),
            30 => Some(TextureFormat::Rg32float),
            31 => Some(TextureFormat::Rgba16uint),
            32 => Some(TextureFormat::Rgba16sint),
            33 => Some(TextureFormat::Rgba16float),
            34 => Some(TextureFormat::Rgba32uint),
            35 => Some(TextureFormat::Rgba32sint),
            36 => Some(TextureFormat::Rgba32float),
            37 => Some(TextureFormat::Stencil8),
            38 => Some(TextureFormat::Depth16unorm),
            39 => Some(TextureFormat::Depth24plus),
            40 => Some(TextureFormat::Depth24plusStencil8),
            41 => Some(TextureFormat::Depth32float),
            42 => Some(TextureFormat::Depth32floatStencil8),
            43 => Some(TextureFormat::Bc1RgbaUnorm),
            44 => Some(TextureFormat::Bc1RgbaUnormSrgb),
            45 => Some(TextureFormat::Bc2RgbaUnorm),
            46 => Some(TextureFormat::Bc2RgbaUnormSrgb),
            47 => Some(TextureFormat::Bc3RgbaUnorm),
            48 => Some(TextureFormat::Bc3RgbaUnormSrgb),
            49 => Some(TextureFormat::Bc4RUnorm),
            50 => Some(TextureFormat::Bc4RSnorm),
            51 => Some(TextureFormat::Bc5RgUnorm),
            52 => Some(TextureFormat::Bc5RgSnorm),
            53 => Some(TextureFormat::Bc6hRgbUfloat),
            54 => Some(TextureFormat::Bc6hRgbFloat),
            55 => Some(TextureFormat::Bc7RgbaUnorm),
            56 => Some(TextureFormat::Bc7RgbaUnormSrgb),
            57 => Some(TextureFormat::Etc2Rgb8unorm),
            58 => Some(TextureFormat::Etc2Rgb8unormSrgb),
            59 => Some(TextureFormat::Etc2Rgb8a1unorm),
            60 => Some(TextureFormat::Etc2Rgb8a1unormSrgb),
            61 => Some(TextureFormat::Etc2Rgba8unorm),
            62 => Some(TextureFormat::Etc2Rgba8unormSrgb),
            63 => Some(TextureFormat::EacR11unorm),
            64 => Some(TextureFormat::EacR11snorm),
            65 => Some(TextureFormat::EacRg11unorm),
            66 => Some(TextureFormat::EacRg11snorm),
            67 => Some(TextureFormat::Astc4x4Unorm),
            68 => Some(TextureFormat::Astc4x4UnormSrgb),
            69 => Some(TextureFormat::Astc5x4Unorm),
            70 => Some(TextureFormat::Astc5x4UnormSrgb),
            71 => Some(TextureFormat::Astc5x5Unorm),
            72 => Some(TextureFormat::Astc5x5UnormSrgb),
            73 => Some(TextureFormat::Astc6x5Unorm),
            74 => Some(TextureFormat::Astc6x5UnormSrgb),
            75 => Some(TextureFormat::Astc6x6Unorm),
            76 => Some(TextureFormat::Astc6x6UnormSrgb),
            77 => Some(TextureFormat::Astc8x5Unorm),
            78 => Some(TextureFormat::Astc8x5UnormSrgb),
            79 => Some(TextureFormat::Astc8x6Unorm),
            80 => Some(TextureFormat::Astc8x6UnormSrgb),
            81 => Some(TextureFormat::Astc8x8Unorm),
            82 => Some(TextureFormat::Astc8x8UnormSrgb),
            83 => Some(TextureFormat::Astc10x5Unorm),
            84 => Some(TextureFormat::Astc10x5UnormSrgb),
            85 => Some(TextureFormat::Astc10x6Unorm),
            86 => Some(TextureFormat::Astc10x6UnormSrgb),
            87 => Some(TextureFormat::Astc10x8Unorm),
            88 => Some(TextureFormat::Astc10x8UnormSrgb),
            89 => Some(TextureFormat::Astc10x10Unorm),
            90 => Some(TextureFormat::Astc10x10UnormSrgb),
            91 => Some(TextureFormat::Astc12x10Unorm),
            92 => Some(TextureFormat::Astc12x10UnormSrgb),
            93 => Some(TextureFormat::Astc12x12Unorm),
            94 => Some(TextureFormat::Astc12x12UnormSrgb),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            TextureFormat::R8unorm => "r8unorm",
            TextureFormat::R8snorm => "r8snorm",
            TextureFormat::R8uint => "r8uint",
            TextureFormat::R8sint => "r8sint",
            TextureFormat::R16uint => "r16uint",
            TextureFormat::R16sint => "r16sint",
            TextureFormat::R16float => "r16float",
            TextureFormat::Rg8unorm => "rg8unorm",
            TextureFormat::Rg8snorm => "rg8snorm",
            TextureFormat::Rg8uint => "rg8uint",
            TextureFormat::Rg8sint => "rg8sint",
            TextureFormat::R32uint => "r32uint",
            TextureFormat::R32sint => "r32sint",
            TextureFormat::R32float => "r32float",
            TextureFormat::Rg16uint => "rg16uint",
            TextureFormat::Rg16sint => "rg16sint",
            TextureFormat::Rg16float => "rg16float",
            TextureFormat::Rgba8unorm => "rgba8unorm",
            TextureFormat::Rgba8unormSrgb => "rgba8unorm-srgb",
            TextureFormat::Rgba8snorm => "rgba8snorm",
            TextureFormat::Rgba8uint => "rgba8uint",
            TextureFormat::Rgba8sint => "rgba8sint",
            TextureFormat::Bgra8unorm => "bgra8unorm",
            TextureFormat::Bgra8unormSrgb => "bgra8unorm-srgb",
            TextureFormat::Rgb9e5ufloat => "rgb9e5ufloat",
            TextureFormat::Rgb10a2uint => "rgb10a2uint",
            TextureFormat::Rgb10a2unorm => "rgb10a2unorm",
            TextureFormat::Rg11b10ufloat => "rg11b10ufloat",
            TextureFormat::Rg32uint => "rg32uint",
            TextureFormat::Rg32sint => "rg32sint",
            TextureFormat::Rg32float => "rg32float",
            TextureFormat::Rgba16uint => "rgba16uint",
            TextureFormat::Rgba16sint => "rgba16sint",
            TextureFormat::Rgba16float => "rgba16float",
            TextureFormat::Rgba32uint => "rgba32uint",
            TextureFormat::Rgba32sint => "rgba32sint",
            TextureFormat::Rgba32float => "rgba32float",
            TextureFormat::Stencil8 => "stencil8",
            TextureFormat::Depth16unorm => "depth16unorm",
            TextureFormat::Depth24plus => "depth24plus",
            TextureFormat::Depth24plusStencil8 => "depth24plus-stencil8",
            TextureFormat::Depth32float => "depth32float",
            TextureFormat::Depth32floatStencil8 => "depth32float-stencil8",
            TextureFormat::Bc1RgbaUnorm => "bc1-rgba-unorm",
            TextureFormat::Bc1RgbaUnormSrgb => "bc1-rgba-unorm-srgb",
            TextureFormat::Bc2RgbaUnorm => "bc2-rgba-unorm",
            TextureFormat::Bc2RgbaUnormSrgb => "bc2-rgba-unorm-srgb",
            TextureFormat::Bc3RgbaUnorm => "bc3-rgba-unorm",
            TextureFormat::Bc3RgbaUnormSrgb => "bc3-rgba-unorm-srgb",
            TextureFormat::Bc4RUnorm => "bc4-r-unorm",
            TextureFormat::Bc4RSnorm => "bc4-r-snorm",
            TextureFormat::Bc5RgUnorm => "bc5-rg-unorm",
            TextureFormat::Bc5RgSnorm => "bc5-rg-snorm",
            TextureFormat::Bc6hRgbUfloat => "bc6h-rgb-ufloat",
            TextureFormat::Bc6hRgbFloat => "bc6h-rgb-float",
            TextureFormat::Bc7RgbaUnorm => "bc7-rgba-unorm",
            TextureFormat::Bc7RgbaUnormSrgb => "bc7-rgba-unorm-srgb",
            TextureFormat::Etc2Rgb8unorm => "etc2-rgb8unorm",
            TextureFormat::Etc2Rgb8unormSrgb => "etc2-rgb8unorm-srgb",
            TextureFormat::Etc2Rgb8a1unorm => "etc2-rgb8a1unorm",
            TextureFormat::Etc2Rgb8a1unormSrgb => "etc2-rgb8a1unorm-srgb",
            TextureFormat::Etc2Rgba8unorm => "etc2-rgba8unorm",
            TextureFormat::Etc2Rgba8unormSrgb => "etc2-rgba8unorm-srgb",
            TextureFormat::EacR11unorm => "eac-r11unorm",
            TextureFormat::EacR11snorm => "eac-r11snorm",
            TextureFormat::EacRg11unorm => "eac-rg11unorm",
            TextureFormat::EacRg11snorm => "eac-rg11snorm",
            TextureFormat::Astc4x4Unorm => "astc-4x4-unorm",
            TextureFormat::Astc4x4UnormSrgb => "astc-4x4-unorm-srgb",
            TextureFormat::Astc5x4Unorm => "astc-5x4-unorm",
            TextureFormat::Astc5x4UnormSrgb => "astc-5x4-unorm-srgb",
            TextureFormat::Astc5x5Unorm => "astc-5x5-unorm",
            TextureFormat::Astc5x5UnormSrgb => "astc-5x5-unorm-srgb",
            TextureFormat::Astc6x5Unorm => "astc-6x5-unorm",
            TextureFormat::Astc6x5UnormSrgb => "astc-6x5-unorm-srgb",
            TextureFormat::Astc6x6Unorm => "astc-6x6-unorm",
            TextureFormat::Astc6x6UnormSrgb => "astc-6x6-unorm-srgb",
            TextureFormat::Astc8x5Unorm => "astc-8x5-unorm",
            TextureFormat::Astc8x5UnormSrgb => "astc-8x5-unorm-srgb",
            TextureFormat::Astc8x6Unorm => "astc-8x6-unorm",
            TextureFormat::Astc8x6UnormSrgb => "astc-8x6-unorm-srgb",
            TextureFormat::Astc8x8Unorm => "astc-8x8-unorm",
            TextureFormat::Astc8x8UnormSrgb => "astc-8x8-unorm-srgb",
            TextureFormat::Astc10x5Unorm => "astc-10x5-unorm",
            TextureFormat::Astc10x5UnormSrgb => "astc-10x5-unorm-srgb",
            TextureFormat::Astc10x6Unorm => "astc-10x6-unorm",
            TextureFormat::Astc10x6UnormSrgb => "astc-10x6-unorm-srgb",
            TextureFormat::Astc10x8Unorm => "astc-10x8-unorm",
            TextureFormat::Astc10x8UnormSrgb => "astc-10x8-unorm-srgb",
            TextureFormat::Astc10x10Unorm => "astc-10x10-unorm",
            TextureFormat::Astc10x10UnormSrgb => "astc-10x10-unorm-srgb",
            TextureFormat::Astc12x10Unorm => "astc-12x10-unorm",
            TextureFormat::Astc12x10UnormSrgb => "astc-12x10-unorm-srgb",
            TextureFormat::Astc12x12Unorm => "astc-12x12-unorm",
            TextureFormat::Astc12x12UnormSrgb => "astc-12x12-unorm-srgb",
        }
    }
}

/// These are the members of GPUVertexFormat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum VertexFormat {
    Uint8 = 0,
    Uint8x2 = 1,
    Uint8x4 = 2,
    Sint8 = 3,
    Sint8x2 = 4,
    Sint8x4 = 5,
    Unorm8 = 6,
    Unorm8x2 = 7,
    Unorm8x4 = 8,
    Snorm8 = 9,
    Snorm8x2 = 10,
    Snorm8x4 = 11,
    Uint16 = 12,
    Uint16x2 = 13,
    Uint16x4 = 14,
    Sint16 = 15,
    Sint16x2 = 16,
    Sint16x4 = 17,
    Unorm16 = 18,
    Unorm16x2 = 19,
    Unorm16x4 = 20,
    Snorm16 = 21,
    Snorm16x2 = 22,
    Snorm16x4 = 23,
    Float16 = 24,
    Float16x2 = 25,
    Float16x4 = 26,
    Float32 = 27,
    Float32x2 = 28,
    Float32x3 = 29,
    Float32x4 = 30,
    Uint32 = 31,
    Uint32x2 = 32,
    Uint32x3 = 33,
    Uint32x4 = 34,
    Sint32 = 35,
    Sint32x2 = 36,
    Sint32x3 = 37,
    Sint32x4 = 38,
    Unorm1010102 = 39,
    Unorm8x4Bgra = 40,
}

impl VertexFormat {
    /// Every member, in its source's own order.
    pub const ALL: [VertexFormat; 41] = [
        VertexFormat::Uint8,
        VertexFormat::Uint8x2,
        VertexFormat::Uint8x4,
        VertexFormat::Sint8,
        VertexFormat::Sint8x2,
        VertexFormat::Sint8x4,
        VertexFormat::Unorm8,
        VertexFormat::Unorm8x2,
        VertexFormat::Unorm8x4,
        VertexFormat::Snorm8,
        VertexFormat::Snorm8x2,
        VertexFormat::Snorm8x4,
        VertexFormat::Uint16,
        VertexFormat::Uint16x2,
        VertexFormat::Uint16x4,
        VertexFormat::Sint16,
        VertexFormat::Sint16x2,
        VertexFormat::Sint16x4,
        VertexFormat::Unorm16,
        VertexFormat::Unorm16x2,
        VertexFormat::Unorm16x4,
        VertexFormat::Snorm16,
        VertexFormat::Snorm16x2,
        VertexFormat::Snorm16x4,
        VertexFormat::Float16,
        VertexFormat::Float16x2,
        VertexFormat::Float16x4,
        VertexFormat::Float32,
        VertexFormat::Float32x2,
        VertexFormat::Float32x3,
        VertexFormat::Float32x4,
        VertexFormat::Uint32,
        VertexFormat::Uint32x2,
        VertexFormat::Uint32x3,
        VertexFormat::Uint32x4,
        VertexFormat::Sint32,
        VertexFormat::Sint32x2,
        VertexFormat::Sint32x3,
        VertexFormat::Sint32x4,
        VertexFormat::Unorm1010102,
        VertexFormat::Unorm8x4Bgra,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<VertexFormat> {
        match val {
            0 => Some(VertexFormat::Uint8),
            1 => Some(VertexFormat::Uint8x2),
            2 => Some(VertexFormat::Uint8x4),
            3 => Some(VertexFormat::Sint8),
            4 => Some(VertexFormat::Sint8x2),
            5 => Some(VertexFormat::Sint8x4),
            6 => Some(VertexFormat::Unorm8),
            7 => Some(VertexFormat::Unorm8x2),
            8 => Some(VertexFormat::Unorm8x4),
            9 => Some(VertexFormat::Snorm8),
            10 => Some(VertexFormat::Snorm8x2),
            11 => Some(VertexFormat::Snorm8x4),
            12 => Some(VertexFormat::Uint16),
            13 => Some(VertexFormat::Uint16x2),
            14 => Some(VertexFormat::Uint16x4),
            15 => Some(VertexFormat::Sint16),
            16 => Some(VertexFormat::Sint16x2),
            17 => Some(VertexFormat::Sint16x4),
            18 => Some(VertexFormat::Unorm16),
            19 => Some(VertexFormat::Unorm16x2),
            20 => Some(VertexFormat::Unorm16x4),
            21 => Some(VertexFormat::Snorm16),
            22 => Some(VertexFormat::Snorm16x2),
            23 => Some(VertexFormat::Snorm16x4),
            24 => Some(VertexFormat::Float16),
            25 => Some(VertexFormat::Float16x2),
            26 => Some(VertexFormat::Float16x4),
            27 => Some(VertexFormat::Float32),
            28 => Some(VertexFormat::Float32x2),
            29 => Some(VertexFormat::Float32x3),
            30 => Some(VertexFormat::Float32x4),
            31 => Some(VertexFormat::Uint32),
            32 => Some(VertexFormat::Uint32x2),
            33 => Some(VertexFormat::Uint32x3),
            34 => Some(VertexFormat::Uint32x4),
            35 => Some(VertexFormat::Sint32),
            36 => Some(VertexFormat::Sint32x2),
            37 => Some(VertexFormat::Sint32x3),
            38 => Some(VertexFormat::Sint32x4),
            39 => Some(VertexFormat::Unorm1010102),
            40 => Some(VertexFormat::Unorm8x4Bgra),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            VertexFormat::Uint8 => "uint8",
            VertexFormat::Uint8x2 => "uint8x2",
            VertexFormat::Uint8x4 => "uint8x4",
            VertexFormat::Sint8 => "sint8",
            VertexFormat::Sint8x2 => "sint8x2",
            VertexFormat::Sint8x4 => "sint8x4",
            VertexFormat::Unorm8 => "unorm8",
            VertexFormat::Unorm8x2 => "unorm8x2",
            VertexFormat::Unorm8x4 => "unorm8x4",
            VertexFormat::Snorm8 => "snorm8",
            VertexFormat::Snorm8x2 => "snorm8x2",
            VertexFormat::Snorm8x4 => "snorm8x4",
            VertexFormat::Uint16 => "uint16",
            VertexFormat::Uint16x2 => "uint16x2",
            VertexFormat::Uint16x4 => "uint16x4",
            VertexFormat::Sint16 => "sint16",
            VertexFormat::Sint16x2 => "sint16x2",
            VertexFormat::Sint16x4 => "sint16x4",
            VertexFormat::Unorm16 => "unorm16",
            VertexFormat::Unorm16x2 => "unorm16x2",
            VertexFormat::Unorm16x4 => "unorm16x4",
            VertexFormat::Snorm16 => "snorm16",
            VertexFormat::Snorm16x2 => "snorm16x2",
            VertexFormat::Snorm16x4 => "snorm16x4",
            VertexFormat::Float16 => "float16",
            VertexFormat::Float16x2 => "float16x2",
            VertexFormat::Float16x4 => "float16x4",
            VertexFormat::Float32 => "float32",
            VertexFormat::Float32x2 => "float32x2",
            VertexFormat::Float32x3 => "float32x3",
            VertexFormat::Float32x4 => "float32x4",
            VertexFormat::Uint32 => "uint32",
            VertexFormat::Uint32x2 => "uint32x2",
            VertexFormat::Uint32x3 => "uint32x3",
            VertexFormat::Uint32x4 => "uint32x4",
            VertexFormat::Sint32 => "sint32",
            VertexFormat::Sint32x2 => "sint32x2",
            VertexFormat::Sint32x3 => "sint32x3",
            VertexFormat::Sint32x4 => "sint32x4",
            VertexFormat::Unorm1010102 => "unorm10-10-10-2",
            VertexFormat::Unorm8x4Bgra => "unorm8x4-bgra",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PrimitiveTopology {
    PointList = 0,
    LineList = 1,
    LineStrip = 2,
    TriangleList = 3,
    TriangleStrip = 4,
}

impl PrimitiveTopology {
    /// Every member, in its source's own order.
    pub const ALL: [PrimitiveTopology; 5] = [
        PrimitiveTopology::PointList,
        PrimitiveTopology::LineList,
        PrimitiveTopology::LineStrip,
        PrimitiveTopology::TriangleList,
        PrimitiveTopology::TriangleStrip,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<PrimitiveTopology> {
        match val {
            0 => Some(PrimitiveTopology::PointList),
            1 => Some(PrimitiveTopology::LineList),
            2 => Some(PrimitiveTopology::LineStrip),
            3 => Some(PrimitiveTopology::TriangleList),
            4 => Some(PrimitiveTopology::TriangleStrip),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            PrimitiveTopology::PointList => "point-list",
            PrimitiveTopology::LineList => "line-list",
            PrimitiveTopology::LineStrip => "line-strip",
            PrimitiveTopology::TriangleList => "triangle-list",
            PrimitiveTopology::TriangleStrip => "triangle-strip",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum IndexFormat {
    Uint16 = 0,
    Uint32 = 1,
}

impl IndexFormat {
    /// Every member, in its source's own order.
    pub const ALL: [IndexFormat; 2] = [IndexFormat::Uint16, IndexFormat::Uint32];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<IndexFormat> {
        match val {
            0 => Some(IndexFormat::Uint16),
            1 => Some(IndexFormat::Uint32),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            IndexFormat::Uint16 => "uint16",
            IndexFormat::Uint32 => "uint32",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FrontFace {
    Ccw = 0,
    Cw = 1,
}

impl FrontFace {
    /// Every member, in its source's own order.
    pub const ALL: [FrontFace; 2] = [FrontFace::Ccw, FrontFace::Cw];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<FrontFace> {
        match val {
            0 => Some(FrontFace::Ccw),
            1 => Some(FrontFace::Cw),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            FrontFace::Ccw => "ccw",
            FrontFace::Cw => "cw",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CullMode {
    None = 0,
    Front = 1,
    Back = 2,
}

impl CullMode {
    /// Every member, in its source's own order.
    pub const ALL: [CullMode; 3] = [CullMode::None, CullMode::Front, CullMode::Back];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<CullMode> {
        match val {
            0 => Some(CullMode::None),
            1 => Some(CullMode::Front),
            2 => Some(CullMode::Back),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            CullMode::None => "none",
            CullMode::Front => "front",
            CullMode::Back => "back",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CompareFunction {
    Never = 0,
    Less = 1,
    Equal = 2,
    LessEqual = 3,
    Greater = 4,
    NotEqual = 5,
    GreaterEqual = 6,
    Always = 7,
}

impl CompareFunction {
    /// Every member, in its source's own order.
    pub const ALL: [CompareFunction; 8] = [
        CompareFunction::Never,
        CompareFunction::Less,
        CompareFunction::Equal,
        CompareFunction::LessEqual,
        CompareFunction::Greater,
        CompareFunction::NotEqual,
        CompareFunction::GreaterEqual,
        CompareFunction::Always,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<CompareFunction> {
        match val {
            0 => Some(CompareFunction::Never),
            1 => Some(CompareFunction::Less),
            2 => Some(CompareFunction::Equal),
            3 => Some(CompareFunction::LessEqual),
            4 => Some(CompareFunction::Greater),
            5 => Some(CompareFunction::NotEqual),
            6 => Some(CompareFunction::GreaterEqual),
            7 => Some(CompareFunction::Always),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            CompareFunction::Never => "never",
            CompareFunction::Less => "less",
            CompareFunction::Equal => "equal",
            CompareFunction::LessEqual => "less-equal",
            CompareFunction::Greater => "greater",
            CompareFunction::NotEqual => "not-equal",
            CompareFunction::GreaterEqual => "greater-equal",
            CompareFunction::Always => "always",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StencilOperation {
    Keep = 0,
    Zero = 1,
    Replace = 2,
    Invert = 3,
    IncrementClamp = 4,
    DecrementClamp = 5,
    IncrementWrap = 6,
    DecrementWrap = 7,
}

impl StencilOperation {
    /// Every member, in its source's own order.
    pub const ALL: [StencilOperation; 8] = [
        StencilOperation::Keep,
        StencilOperation::Zero,
        StencilOperation::Replace,
        StencilOperation::Invert,
        StencilOperation::IncrementClamp,
        StencilOperation::DecrementClamp,
        StencilOperation::IncrementWrap,
        StencilOperation::DecrementWrap,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<StencilOperation> {
        match val {
            0 => Some(StencilOperation::Keep),
            1 => Some(StencilOperation::Zero),
            2 => Some(StencilOperation::Replace),
            3 => Some(StencilOperation::Invert),
            4 => Some(StencilOperation::IncrementClamp),
            5 => Some(StencilOperation::DecrementClamp),
            6 => Some(StencilOperation::IncrementWrap),
            7 => Some(StencilOperation::DecrementWrap),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            StencilOperation::Keep => "keep",
            StencilOperation::Zero => "zero",
            StencilOperation::Replace => "replace",
            StencilOperation::Invert => "invert",
            StencilOperation::IncrementClamp => "increment-clamp",
            StencilOperation::DecrementClamp => "decrement-clamp",
            StencilOperation::IncrementWrap => "increment-wrap",
            StencilOperation::DecrementWrap => "decrement-wrap",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BlendOperation {
    Add = 0,
    Subtract = 1,
    ReverseSubtract = 2,
    Min = 3,
    Max = 4,
}

impl BlendOperation {
    /// Every member, in its source's own order.
    pub const ALL: [BlendOperation; 5] = [
        BlendOperation::Add,
        BlendOperation::Subtract,
        BlendOperation::ReverseSubtract,
        BlendOperation::Min,
        BlendOperation::Max,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<BlendOperation> {
        match val {
            0 => Some(BlendOperation::Add),
            1 => Some(BlendOperation::Subtract),
            2 => Some(BlendOperation::ReverseSubtract),
            3 => Some(BlendOperation::Min),
            4 => Some(BlendOperation::Max),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            BlendOperation::Add => "add",
            BlendOperation::Subtract => "subtract",
            BlendOperation::ReverseSubtract => "reverse-subtract",
            BlendOperation::Min => "min",
            BlendOperation::Max => "max",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BlendFactor {
    Zero = 0,
    One = 1,
    Src = 2,
    OneMinusSrc = 3,
    SrcAlpha = 4,
    OneMinusSrcAlpha = 5,
    Dst = 6,
    OneMinusDst = 7,
    DstAlpha = 8,
    OneMinusDstAlpha = 9,
    SrcAlphaSaturated = 10,
    Constant = 11,
    OneMinusConstant = 12,
    Src1 = 13,
    OneMinusSrc1 = 14,
    Src1Alpha = 15,
    OneMinusSrc1Alpha = 16,
}

impl BlendFactor {
    /// Every member, in its source's own order.
    pub const ALL: [BlendFactor; 17] = [
        BlendFactor::Zero,
        BlendFactor::One,
        BlendFactor::Src,
        BlendFactor::OneMinusSrc,
        BlendFactor::SrcAlpha,
        BlendFactor::OneMinusSrcAlpha,
        BlendFactor::Dst,
        BlendFactor::OneMinusDst,
        BlendFactor::DstAlpha,
        BlendFactor::OneMinusDstAlpha,
        BlendFactor::SrcAlphaSaturated,
        BlendFactor::Constant,
        BlendFactor::OneMinusConstant,
        BlendFactor::Src1,
        BlendFactor::OneMinusSrc1,
        BlendFactor::Src1Alpha,
        BlendFactor::OneMinusSrc1Alpha,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<BlendFactor> {
        match val {
            0 => Some(BlendFactor::Zero),
            1 => Some(BlendFactor::One),
            2 => Some(BlendFactor::Src),
            3 => Some(BlendFactor::OneMinusSrc),
            4 => Some(BlendFactor::SrcAlpha),
            5 => Some(BlendFactor::OneMinusSrcAlpha),
            6 => Some(BlendFactor::Dst),
            7 => Some(BlendFactor::OneMinusDst),
            8 => Some(BlendFactor::DstAlpha),
            9 => Some(BlendFactor::OneMinusDstAlpha),
            10 => Some(BlendFactor::SrcAlphaSaturated),
            11 => Some(BlendFactor::Constant),
            12 => Some(BlendFactor::OneMinusConstant),
            13 => Some(BlendFactor::Src1),
            14 => Some(BlendFactor::OneMinusSrc1),
            15 => Some(BlendFactor::Src1Alpha),
            16 => Some(BlendFactor::OneMinusSrc1Alpha),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            BlendFactor::Zero => "zero",
            BlendFactor::One => "one",
            BlendFactor::Src => "src",
            BlendFactor::OneMinusSrc => "one-minus-src",
            BlendFactor::SrcAlpha => "src-alpha",
            BlendFactor::OneMinusSrcAlpha => "one-minus-src-alpha",
            BlendFactor::Dst => "dst",
            BlendFactor::OneMinusDst => "one-minus-dst",
            BlendFactor::DstAlpha => "dst-alpha",
            BlendFactor::OneMinusDstAlpha => "one-minus-dst-alpha",
            BlendFactor::SrcAlphaSaturated => "src-alpha-saturated",
            BlendFactor::Constant => "constant",
            BlendFactor::OneMinusConstant => "one-minus-constant",
            BlendFactor::Src1 => "src1",
            BlendFactor::OneMinusSrc1 => "one-minus-src1",
            BlendFactor::Src1Alpha => "src1-alpha",
            BlendFactor::OneMinusSrc1Alpha => "one-minus-src1-alpha",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum VertexStepMode {
    Vertex = 0,
    Instance = 1,
}

impl VertexStepMode {
    /// Every member, in its source's own order.
    pub const ALL: [VertexStepMode; 2] = [VertexStepMode::Vertex, VertexStepMode::Instance];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<VertexStepMode> {
        match val {
            0 => Some(VertexStepMode::Vertex),
            1 => Some(VertexStepMode::Instance),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            VertexStepMode::Vertex => "vertex",
            VertexStepMode::Instance => "instance",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AddressMode {
    ClampToEdge = 0,
    Repeat = 1,
    MirrorRepeat = 2,
}

impl AddressMode {
    /// Every member, in its source's own order.
    pub const ALL: [AddressMode; 3] = [
        AddressMode::ClampToEdge,
        AddressMode::Repeat,
        AddressMode::MirrorRepeat,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<AddressMode> {
        match val {
            0 => Some(AddressMode::ClampToEdge),
            1 => Some(AddressMode::Repeat),
            2 => Some(AddressMode::MirrorRepeat),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            AddressMode::ClampToEdge => "clamp-to-edge",
            AddressMode::Repeat => "repeat",
            AddressMode::MirrorRepeat => "mirror-repeat",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FilterMode {
    Nearest = 0,
    Linear = 1,
}

impl FilterMode {
    /// Every member, in its source's own order.
    pub const ALL: [FilterMode; 2] = [FilterMode::Nearest, FilterMode::Linear];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<FilterMode> {
        match val {
            0 => Some(FilterMode::Nearest),
            1 => Some(FilterMode::Linear),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            FilterMode::Nearest => "nearest",
            FilterMode::Linear => "linear",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MipmapFilterMode {
    Nearest = 0,
    Linear = 1,
}

impl MipmapFilterMode {
    /// Every member, in its source's own order.
    pub const ALL: [MipmapFilterMode; 2] = [MipmapFilterMode::Nearest, MipmapFilterMode::Linear];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<MipmapFilterMode> {
        match val {
            0 => Some(MipmapFilterMode::Nearest),
            1 => Some(MipmapFilterMode::Linear),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            MipmapFilterMode::Nearest => "nearest",
            MipmapFilterMode::Linear => "linear",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TextureDimension {
    _1d = 0,
    _2d = 1,
    _3d = 2,
}

impl TextureDimension {
    /// Every member, in its source's own order.
    pub const ALL: [TextureDimension; 3] = [
        TextureDimension::_1d,
        TextureDimension::_2d,
        TextureDimension::_3d,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<TextureDimension> {
        match val {
            0 => Some(TextureDimension::_1d),
            1 => Some(TextureDimension::_2d),
            2 => Some(TextureDimension::_3d),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            TextureDimension::_1d => "1d",
            TextureDimension::_2d => "2d",
            TextureDimension::_3d => "3d",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TextureViewDimension {
    _1d = 0,
    _2d = 1,
    _2dArray = 2,
    Cube = 3,
    CubeArray = 4,
    _3d = 5,
}

impl TextureViewDimension {
    /// Every member, in its source's own order.
    pub const ALL: [TextureViewDimension; 6] = [
        TextureViewDimension::_1d,
        TextureViewDimension::_2d,
        TextureViewDimension::_2dArray,
        TextureViewDimension::Cube,
        TextureViewDimension::CubeArray,
        TextureViewDimension::_3d,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<TextureViewDimension> {
        match val {
            0 => Some(TextureViewDimension::_1d),
            1 => Some(TextureViewDimension::_2d),
            2 => Some(TextureViewDimension::_2dArray),
            3 => Some(TextureViewDimension::Cube),
            4 => Some(TextureViewDimension::CubeArray),
            5 => Some(TextureViewDimension::_3d),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            TextureViewDimension::_1d => "1d",
            TextureViewDimension::_2d => "2d",
            TextureViewDimension::_2dArray => "2d-array",
            TextureViewDimension::Cube => "cube",
            TextureViewDimension::CubeArray => "cube-array",
            TextureViewDimension::_3d => "3d",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TextureAspect {
    All = 0,
    StencilOnly = 1,
    DepthOnly = 2,
}

impl TextureAspect {
    /// Every member, in its source's own order.
    pub const ALL: [TextureAspect; 3] = [
        TextureAspect::All,
        TextureAspect::StencilOnly,
        TextureAspect::DepthOnly,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<TextureAspect> {
        match val {
            0 => Some(TextureAspect::All),
            1 => Some(TextureAspect::StencilOnly),
            2 => Some(TextureAspect::DepthOnly),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            TextureAspect::All => "all",
            TextureAspect::StencilOnly => "stencil-only",
            TextureAspect::DepthOnly => "depth-only",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BufferBindingType {
    Uniform = 0,
    Storage = 1,
    ReadOnlyStorage = 2,
}

impl BufferBindingType {
    /// Every member, in its source's own order.
    pub const ALL: [BufferBindingType; 3] = [
        BufferBindingType::Uniform,
        BufferBindingType::Storage,
        BufferBindingType::ReadOnlyStorage,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<BufferBindingType> {
        match val {
            0 => Some(BufferBindingType::Uniform),
            1 => Some(BufferBindingType::Storage),
            2 => Some(BufferBindingType::ReadOnlyStorage),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            BufferBindingType::Uniform => "uniform",
            BufferBindingType::Storage => "storage",
            BufferBindingType::ReadOnlyStorage => "read-only-storage",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SamplerBindingType {
    Filtering = 0,
    NonFiltering = 1,
    Comparison = 2,
}

impl SamplerBindingType {
    /// Every member, in its source's own order.
    pub const ALL: [SamplerBindingType; 3] = [
        SamplerBindingType::Filtering,
        SamplerBindingType::NonFiltering,
        SamplerBindingType::Comparison,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<SamplerBindingType> {
        match val {
            0 => Some(SamplerBindingType::Filtering),
            1 => Some(SamplerBindingType::NonFiltering),
            2 => Some(SamplerBindingType::Comparison),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            SamplerBindingType::Filtering => "filtering",
            SamplerBindingType::NonFiltering => "non-filtering",
            SamplerBindingType::Comparison => "comparison",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TextureSampleType {
    Float = 0,
    UnfilterableFloat = 1,
    Depth = 2,
    Sint = 3,
    Uint = 4,
}

impl TextureSampleType {
    /// Every member, in its source's own order.
    pub const ALL: [TextureSampleType; 5] = [
        TextureSampleType::Float,
        TextureSampleType::UnfilterableFloat,
        TextureSampleType::Depth,
        TextureSampleType::Sint,
        TextureSampleType::Uint,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<TextureSampleType> {
        match val {
            0 => Some(TextureSampleType::Float),
            1 => Some(TextureSampleType::UnfilterableFloat),
            2 => Some(TextureSampleType::Depth),
            3 => Some(TextureSampleType::Sint),
            4 => Some(TextureSampleType::Uint),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            TextureSampleType::Float => "float",
            TextureSampleType::UnfilterableFloat => "unfilterable-float",
            TextureSampleType::Depth => "depth",
            TextureSampleType::Sint => "sint",
            TextureSampleType::Uint => "uint",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StorageTextureAccess {
    WriteOnly = 0,
    ReadOnly = 1,
    ReadWrite = 2,
}

impl StorageTextureAccess {
    /// Every member, in its source's own order.
    pub const ALL: [StorageTextureAccess; 3] = [
        StorageTextureAccess::WriteOnly,
        StorageTextureAccess::ReadOnly,
        StorageTextureAccess::ReadWrite,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<StorageTextureAccess> {
        match val {
            0 => Some(StorageTextureAccess::WriteOnly),
            1 => Some(StorageTextureAccess::ReadOnly),
            2 => Some(StorageTextureAccess::ReadWrite),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            StorageTextureAccess::WriteOnly => "write-only",
            StorageTextureAccess::ReadOnly => "read-only",
            StorageTextureAccess::ReadWrite => "read-write",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LoadOp {
    Load = 0,
    Clear = 1,
}

impl LoadOp {
    /// Every member, in its source's own order.
    pub const ALL: [LoadOp; 2] = [LoadOp::Load, LoadOp::Clear];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<LoadOp> {
        match val {
            0 => Some(LoadOp::Load),
            1 => Some(LoadOp::Clear),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            LoadOp::Load => "load",
            LoadOp::Clear => "clear",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StoreOp {
    Store = 0,
    Discard = 1,
}

impl StoreOp {
    /// Every member, in its source's own order.
    pub const ALL: [StoreOp; 2] = [StoreOp::Store, StoreOp::Discard];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<StoreOp> {
        match val {
            0 => Some(StoreOp::Store),
            1 => Some(StoreOp::Discard),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            StoreOp::Store => "store",
            StoreOp::Discard => "discard",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum QueryType {
    Occlusion = 0,
    Timestamp = 1,
}

impl QueryType {
    /// Every member, in its source's own order.
    pub const ALL: [QueryType; 2] = [QueryType::Occlusion, QueryType::Timestamp];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<QueryType> {
        match val {
            0 => Some(QueryType::Occlusion),
            1 => Some(QueryType::Timestamp),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            QueryType::Occlusion => "occlusion",
            QueryType::Timestamp => "timestamp",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CanvasAlphaMode {
    Opaque = 0,
    Premultiplied = 1,
}

impl CanvasAlphaMode {
    /// Every member, in its source's own order.
    pub const ALL: [CanvasAlphaMode; 2] = [CanvasAlphaMode::Opaque, CanvasAlphaMode::Premultiplied];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<CanvasAlphaMode> {
        match val {
            0 => Some(CanvasAlphaMode::Opaque),
            1 => Some(CanvasAlphaMode::Premultiplied),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            CanvasAlphaMode::Opaque => "opaque",
            CanvasAlphaMode::Premultiplied => "premultiplied",
        }
    }
}

/// GPUFeatureName. The DEVICE record (api.json) reports a device's features as one bit each,
/// at the member's place.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FeatureName {
    DepthClipControl = 0,
    Depth32floatStencil8 = 1,
    TextureCompressionBc = 2,
    TextureCompressionBcSliced3d = 3,
    TextureCompressionEtc2 = 4,
    TextureCompressionAstc = 5,
    TextureCompressionAstcSliced3d = 6,
    TimestampQuery = 7,
    IndirectFirstInstance = 8,
    ShaderF16 = 9,
    Rg11b10ufloatRenderable = 10,
    Bgra8unormStorage = 11,
    Float32Filterable = 12,
    Float32Blendable = 13,
    ClipDistances = 14,
    DualSourceBlending = 15,
    Subgroups = 16,
}

impl FeatureName {
    /// Every member, in its source's own order.
    pub const ALL: [FeatureName; 17] = [
        FeatureName::DepthClipControl,
        FeatureName::Depth32floatStencil8,
        FeatureName::TextureCompressionBc,
        FeatureName::TextureCompressionBcSliced3d,
        FeatureName::TextureCompressionEtc2,
        FeatureName::TextureCompressionAstc,
        FeatureName::TextureCompressionAstcSliced3d,
        FeatureName::TimestampQuery,
        FeatureName::IndirectFirstInstance,
        FeatureName::ShaderF16,
        FeatureName::Rg11b10ufloatRenderable,
        FeatureName::Bgra8unormStorage,
        FeatureName::Float32Filterable,
        FeatureName::Float32Blendable,
        FeatureName::ClipDistances,
        FeatureName::DualSourceBlending,
        FeatureName::Subgroups,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<FeatureName> {
        match val {
            0 => Some(FeatureName::DepthClipControl),
            1 => Some(FeatureName::Depth32floatStencil8),
            2 => Some(FeatureName::TextureCompressionBc),
            3 => Some(FeatureName::TextureCompressionBcSliced3d),
            4 => Some(FeatureName::TextureCompressionEtc2),
            5 => Some(FeatureName::TextureCompressionAstc),
            6 => Some(FeatureName::TextureCompressionAstcSliced3d),
            7 => Some(FeatureName::TimestampQuery),
            8 => Some(FeatureName::IndirectFirstInstance),
            9 => Some(FeatureName::ShaderF16),
            10 => Some(FeatureName::Rg11b10ufloatRenderable),
            11 => Some(FeatureName::Bgra8unormStorage),
            12 => Some(FeatureName::Float32Filterable),
            13 => Some(FeatureName::Float32Blendable),
            14 => Some(FeatureName::ClipDistances),
            15 => Some(FeatureName::DualSourceBlending),
            16 => Some(FeatureName::Subgroups),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            FeatureName::DepthClipControl => "depth-clip-control",
            FeatureName::Depth32floatStencil8 => "depth32float-stencil8",
            FeatureName::TextureCompressionBc => "texture-compression-bc",
            FeatureName::TextureCompressionBcSliced3d => "texture-compression-bc-sliced-3d",
            FeatureName::TextureCompressionEtc2 => "texture-compression-etc2",
            FeatureName::TextureCompressionAstc => "texture-compression-astc",
            FeatureName::TextureCompressionAstcSliced3d => "texture-compression-astc-sliced-3d",
            FeatureName::TimestampQuery => "timestamp-query",
            FeatureName::IndirectFirstInstance => "indirect-first-instance",
            FeatureName::ShaderF16 => "shader-f16",
            FeatureName::Rg11b10ufloatRenderable => "rg11b10ufloat-renderable",
            FeatureName::Bgra8unormStorage => "bgra8unorm-storage",
            FeatureName::Float32Filterable => "float32-filterable",
            FeatureName::Float32Blendable => "float32-blendable",
            FeatureName::ClipDistances => "clip-distances",
            FeatureName::DualSourceBlending => "dual-source-blending",
            FeatureName::Subgroups => "subgroups",
        }
    }
}

/// The members of GPUSupportedLimits. The DEVICE record (api.json) reports a device's limits
/// as one word each, at the member's place.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LimitName {
    MaxTextureDimension1d = 0,
    MaxTextureDimension2d = 1,
    MaxTextureDimension3d = 2,
    MaxTextureArrayLayers = 3,
    MaxBindGroups = 4,
    MaxBindGroupsPlusVertexBuffers = 5,
    MaxBindingsPerBindGroup = 6,
    MaxDynamicUniformBuffersPerPipelineLayout = 7,
    MaxDynamicStorageBuffersPerPipelineLayout = 8,
    MaxSampledTexturesPerShaderStage = 9,
    MaxSamplersPerShaderStage = 10,
    MaxStorageBuffersPerShaderStage = 11,
    MaxStorageTexturesPerShaderStage = 12,
    MaxUniformBuffersPerShaderStage = 13,
    MaxUniformBufferBindingSize = 14,
    MaxStorageBufferBindingSize = 15,
    MinUniformBufferOffsetAlignment = 16,
    MinStorageBufferOffsetAlignment = 17,
    MaxVertexBuffers = 18,
    MaxBufferSize = 19,
    MaxVertexAttributes = 20,
    MaxVertexBufferArrayStride = 21,
    MaxInterStageShaderVariables = 22,
    MaxColorAttachments = 23,
    MaxColorAttachmentBytesPerSample = 24,
    MaxComputeWorkgroupStorageSize = 25,
    MaxComputeInvocationsPerWorkgroup = 26,
    MaxComputeWorkgroupSizeX = 27,
    MaxComputeWorkgroupSizeY = 28,
    MaxComputeWorkgroupSizeZ = 29,
    MaxComputeWorkgroupsPerDimension = 30,
}

impl LimitName {
    /// Every member, in its source's own order.
    pub const ALL: [LimitName; 31] = [
        LimitName::MaxTextureDimension1d,
        LimitName::MaxTextureDimension2d,
        LimitName::MaxTextureDimension3d,
        LimitName::MaxTextureArrayLayers,
        LimitName::MaxBindGroups,
        LimitName::MaxBindGroupsPlusVertexBuffers,
        LimitName::MaxBindingsPerBindGroup,
        LimitName::MaxDynamicUniformBuffersPerPipelineLayout,
        LimitName::MaxDynamicStorageBuffersPerPipelineLayout,
        LimitName::MaxSampledTexturesPerShaderStage,
        LimitName::MaxSamplersPerShaderStage,
        LimitName::MaxStorageBuffersPerShaderStage,
        LimitName::MaxStorageTexturesPerShaderStage,
        LimitName::MaxUniformBuffersPerShaderStage,
        LimitName::MaxUniformBufferBindingSize,
        LimitName::MaxStorageBufferBindingSize,
        LimitName::MinUniformBufferOffsetAlignment,
        LimitName::MinStorageBufferOffsetAlignment,
        LimitName::MaxVertexBuffers,
        LimitName::MaxBufferSize,
        LimitName::MaxVertexAttributes,
        LimitName::MaxVertexBufferArrayStride,
        LimitName::MaxInterStageShaderVariables,
        LimitName::MaxColorAttachments,
        LimitName::MaxColorAttachmentBytesPerSample,
        LimitName::MaxComputeWorkgroupStorageSize,
        LimitName::MaxComputeInvocationsPerWorkgroup,
        LimitName::MaxComputeWorkgroupSizeX,
        LimitName::MaxComputeWorkgroupSizeY,
        LimitName::MaxComputeWorkgroupSizeZ,
        LimitName::MaxComputeWorkgroupsPerDimension,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<LimitName> {
        match val {
            0 => Some(LimitName::MaxTextureDimension1d),
            1 => Some(LimitName::MaxTextureDimension2d),
            2 => Some(LimitName::MaxTextureDimension3d),
            3 => Some(LimitName::MaxTextureArrayLayers),
            4 => Some(LimitName::MaxBindGroups),
            5 => Some(LimitName::MaxBindGroupsPlusVertexBuffers),
            6 => Some(LimitName::MaxBindingsPerBindGroup),
            7 => Some(LimitName::MaxDynamicUniformBuffersPerPipelineLayout),
            8 => Some(LimitName::MaxDynamicStorageBuffersPerPipelineLayout),
            9 => Some(LimitName::MaxSampledTexturesPerShaderStage),
            10 => Some(LimitName::MaxSamplersPerShaderStage),
            11 => Some(LimitName::MaxStorageBuffersPerShaderStage),
            12 => Some(LimitName::MaxStorageTexturesPerShaderStage),
            13 => Some(LimitName::MaxUniformBuffersPerShaderStage),
            14 => Some(LimitName::MaxUniformBufferBindingSize),
            15 => Some(LimitName::MaxStorageBufferBindingSize),
            16 => Some(LimitName::MinUniformBufferOffsetAlignment),
            17 => Some(LimitName::MinStorageBufferOffsetAlignment),
            18 => Some(LimitName::MaxVertexBuffers),
            19 => Some(LimitName::MaxBufferSize),
            20 => Some(LimitName::MaxVertexAttributes),
            21 => Some(LimitName::MaxVertexBufferArrayStride),
            22 => Some(LimitName::MaxInterStageShaderVariables),
            23 => Some(LimitName::MaxColorAttachments),
            24 => Some(LimitName::MaxColorAttachmentBytesPerSample),
            25 => Some(LimitName::MaxComputeWorkgroupStorageSize),
            26 => Some(LimitName::MaxComputeInvocationsPerWorkgroup),
            27 => Some(LimitName::MaxComputeWorkgroupSizeX),
            28 => Some(LimitName::MaxComputeWorkgroupSizeY),
            29 => Some(LimitName::MaxComputeWorkgroupSizeZ),
            30 => Some(LimitName::MaxComputeWorkgroupsPerDimension),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            LimitName::MaxTextureDimension1d => "maxTextureDimension1D",
            LimitName::MaxTextureDimension2d => "maxTextureDimension2D",
            LimitName::MaxTextureDimension3d => "maxTextureDimension3D",
            LimitName::MaxTextureArrayLayers => "maxTextureArrayLayers",
            LimitName::MaxBindGroups => "maxBindGroups",
            LimitName::MaxBindGroupsPlusVertexBuffers => "maxBindGroupsPlusVertexBuffers",
            LimitName::MaxBindingsPerBindGroup => "maxBindingsPerBindGroup",
            LimitName::MaxDynamicUniformBuffersPerPipelineLayout => {
                "maxDynamicUniformBuffersPerPipelineLayout"
            }
            LimitName::MaxDynamicStorageBuffersPerPipelineLayout => {
                "maxDynamicStorageBuffersPerPipelineLayout"
            }
            LimitName::MaxSampledTexturesPerShaderStage => "maxSampledTexturesPerShaderStage",
            LimitName::MaxSamplersPerShaderStage => "maxSamplersPerShaderStage",
            LimitName::MaxStorageBuffersPerShaderStage => "maxStorageBuffersPerShaderStage",
            LimitName::MaxStorageTexturesPerShaderStage => "maxStorageTexturesPerShaderStage",
            LimitName::MaxUniformBuffersPerShaderStage => "maxUniformBuffersPerShaderStage",
            LimitName::MaxUniformBufferBindingSize => "maxUniformBufferBindingSize",
            LimitName::MaxStorageBufferBindingSize => "maxStorageBufferBindingSize",
            LimitName::MinUniformBufferOffsetAlignment => "minUniformBufferOffsetAlignment",
            LimitName::MinStorageBufferOffsetAlignment => "minStorageBufferOffsetAlignment",
            LimitName::MaxVertexBuffers => "maxVertexBuffers",
            LimitName::MaxBufferSize => "maxBufferSize",
            LimitName::MaxVertexAttributes => "maxVertexAttributes",
            LimitName::MaxVertexBufferArrayStride => "maxVertexBufferArrayStride",
            LimitName::MaxInterStageShaderVariables => "maxInterStageShaderVariables",
            LimitName::MaxColorAttachments => "maxColorAttachments",
            LimitName::MaxColorAttachmentBytesPerSample => "maxColorAttachmentBytesPerSample",
            LimitName::MaxComputeWorkgroupStorageSize => "maxComputeWorkgroupStorageSize",
            LimitName::MaxComputeInvocationsPerWorkgroup => "maxComputeInvocationsPerWorkgroup",
            LimitName::MaxComputeWorkgroupSizeX => "maxComputeWorkgroupSizeX",
            LimitName::MaxComputeWorkgroupSizeY => "maxComputeWorkgroupSizeY",
            LimitName::MaxComputeWorkgroupSizeZ => "maxComputeWorkgroupSizeZ",
            LimitName::MaxComputeWorkgroupsPerDimension => "maxComputeWorkgroupsPerDimension",
        }
    }
}
