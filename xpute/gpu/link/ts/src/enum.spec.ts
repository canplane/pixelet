// @xpute/gpu-link/enum.spec.ts
//
// GENERATED from gpu/link/spec/enum.json (xpute-gen data.rs)
//
// WebGPU's own enumerations. Each one lists its WebGPU spec's strings in order, and a
// member's number is its place in that list. A host hands the device the string as it
// is, and a guest names a member by its generated constant. Nothing here belongs to any one
// program. A value that WebGPU leaves optional is NONE (api.json) when it is unset.

/** GPUTextureFormat: the formats of the core WebGPU spec and of its texture-compression
 * features. */
export const TEXTURE_FORMATS = [
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

export type TextureFormat =
  | typeof TEXTURE_FORMAT_R8UNORM
  | typeof TEXTURE_FORMAT_R8SNORM
  | typeof TEXTURE_FORMAT_R8UINT
  | typeof TEXTURE_FORMAT_R8SINT
  | typeof TEXTURE_FORMAT_R16UINT
  | typeof TEXTURE_FORMAT_R16SINT
  | typeof TEXTURE_FORMAT_R16FLOAT
  | typeof TEXTURE_FORMAT_RG8UNORM
  | typeof TEXTURE_FORMAT_RG8SNORM
  | typeof TEXTURE_FORMAT_RG8UINT
  | typeof TEXTURE_FORMAT_RG8SINT
  | typeof TEXTURE_FORMAT_R32UINT
  | typeof TEXTURE_FORMAT_R32SINT
  | typeof TEXTURE_FORMAT_R32FLOAT
  | typeof TEXTURE_FORMAT_RG16UINT
  | typeof TEXTURE_FORMAT_RG16SINT
  | typeof TEXTURE_FORMAT_RG16FLOAT
  | typeof TEXTURE_FORMAT_RGBA8UNORM
  | typeof TEXTURE_FORMAT_RGBA8UNORM_SRGB
  | typeof TEXTURE_FORMAT_RGBA8SNORM
  | typeof TEXTURE_FORMAT_RGBA8UINT
  | typeof TEXTURE_FORMAT_RGBA8SINT
  | typeof TEXTURE_FORMAT_BGRA8UNORM
  | typeof TEXTURE_FORMAT_BGRA8UNORM_SRGB
  | typeof TEXTURE_FORMAT_RGB9E5UFLOAT
  | typeof TEXTURE_FORMAT_RGB10A2UINT
  | typeof TEXTURE_FORMAT_RGB10A2UNORM
  | typeof TEXTURE_FORMAT_RG11B10UFLOAT
  | typeof TEXTURE_FORMAT_RG32UINT
  | typeof TEXTURE_FORMAT_RG32SINT
  | typeof TEXTURE_FORMAT_RG32FLOAT
  | typeof TEXTURE_FORMAT_RGBA16UINT
  | typeof TEXTURE_FORMAT_RGBA16SINT
  | typeof TEXTURE_FORMAT_RGBA16FLOAT
  | typeof TEXTURE_FORMAT_RGBA32UINT
  | typeof TEXTURE_FORMAT_RGBA32SINT
  | typeof TEXTURE_FORMAT_RGBA32FLOAT
  | typeof TEXTURE_FORMAT_STENCIL8
  | typeof TEXTURE_FORMAT_DEPTH16UNORM
  | typeof TEXTURE_FORMAT_DEPTH24PLUS
  | typeof TEXTURE_FORMAT_DEPTH24PLUS_STENCIL8
  | typeof TEXTURE_FORMAT_DEPTH32FLOAT
  | typeof TEXTURE_FORMAT_DEPTH32FLOAT_STENCIL8
  | typeof TEXTURE_FORMAT_BC1_RGBA_UNORM
  | typeof TEXTURE_FORMAT_BC1_RGBA_UNORM_SRGB
  | typeof TEXTURE_FORMAT_BC2_RGBA_UNORM
  | typeof TEXTURE_FORMAT_BC2_RGBA_UNORM_SRGB
  | typeof TEXTURE_FORMAT_BC3_RGBA_UNORM
  | typeof TEXTURE_FORMAT_BC3_RGBA_UNORM_SRGB
  | typeof TEXTURE_FORMAT_BC4_R_UNORM
  | typeof TEXTURE_FORMAT_BC4_R_SNORM
  | typeof TEXTURE_FORMAT_BC5_RG_UNORM
  | typeof TEXTURE_FORMAT_BC5_RG_SNORM
  | typeof TEXTURE_FORMAT_BC6H_RGB_UFLOAT
  | typeof TEXTURE_FORMAT_BC6H_RGB_FLOAT
  | typeof TEXTURE_FORMAT_BC7_RGBA_UNORM
  | typeof TEXTURE_FORMAT_BC7_RGBA_UNORM_SRGB
  | typeof TEXTURE_FORMAT_ETC2_RGB8UNORM
  | typeof TEXTURE_FORMAT_ETC2_RGB8UNORM_SRGB
  | typeof TEXTURE_FORMAT_ETC2_RGB8A1UNORM
  | typeof TEXTURE_FORMAT_ETC2_RGB8A1UNORM_SRGB
  | typeof TEXTURE_FORMAT_ETC2_RGBA8UNORM
  | typeof TEXTURE_FORMAT_ETC2_RGBA8UNORM_SRGB
  | typeof TEXTURE_FORMAT_EAC_R11UNORM
  | typeof TEXTURE_FORMAT_EAC_R11SNORM
  | typeof TEXTURE_FORMAT_EAC_RG11UNORM
  | typeof TEXTURE_FORMAT_EAC_RG11SNORM
  | typeof TEXTURE_FORMAT_ASTC_4X4_UNORM
  | typeof TEXTURE_FORMAT_ASTC_4X4_UNORM_SRGB
  | typeof TEXTURE_FORMAT_ASTC_5X4_UNORM
  | typeof TEXTURE_FORMAT_ASTC_5X4_UNORM_SRGB
  | typeof TEXTURE_FORMAT_ASTC_5X5_UNORM
  | typeof TEXTURE_FORMAT_ASTC_5X5_UNORM_SRGB
  | typeof TEXTURE_FORMAT_ASTC_6X5_UNORM
  | typeof TEXTURE_FORMAT_ASTC_6X5_UNORM_SRGB
  | typeof TEXTURE_FORMAT_ASTC_6X6_UNORM
  | typeof TEXTURE_FORMAT_ASTC_6X6_UNORM_SRGB
  | typeof TEXTURE_FORMAT_ASTC_8X5_UNORM
  | typeof TEXTURE_FORMAT_ASTC_8X5_UNORM_SRGB
  | typeof TEXTURE_FORMAT_ASTC_8X6_UNORM
  | typeof TEXTURE_FORMAT_ASTC_8X6_UNORM_SRGB
  | typeof TEXTURE_FORMAT_ASTC_8X8_UNORM
  | typeof TEXTURE_FORMAT_ASTC_8X8_UNORM_SRGB
  | typeof TEXTURE_FORMAT_ASTC_10X5_UNORM
  | typeof TEXTURE_FORMAT_ASTC_10X5_UNORM_SRGB
  | typeof TEXTURE_FORMAT_ASTC_10X6_UNORM
  | typeof TEXTURE_FORMAT_ASTC_10X6_UNORM_SRGB
  | typeof TEXTURE_FORMAT_ASTC_10X8_UNORM
  | typeof TEXTURE_FORMAT_ASTC_10X8_UNORM_SRGB
  | typeof TEXTURE_FORMAT_ASTC_10X10_UNORM
  | typeof TEXTURE_FORMAT_ASTC_10X10_UNORM_SRGB
  | typeof TEXTURE_FORMAT_ASTC_12X10_UNORM
  | typeof TEXTURE_FORMAT_ASTC_12X10_UNORM_SRGB
  | typeof TEXTURE_FORMAT_ASTC_12X12_UNORM
  | typeof TEXTURE_FORMAT_ASTC_12X12_UNORM_SRGB;
export const TEXTURE_FORMAT_R8UNORM = 0;
export const TEXTURE_FORMAT_R8SNORM = 1;
export const TEXTURE_FORMAT_R8UINT = 2;
export const TEXTURE_FORMAT_R8SINT = 3;
export const TEXTURE_FORMAT_R16UINT = 4;
export const TEXTURE_FORMAT_R16SINT = 5;
export const TEXTURE_FORMAT_R16FLOAT = 6;
export const TEXTURE_FORMAT_RG8UNORM = 7;
export const TEXTURE_FORMAT_RG8SNORM = 8;
export const TEXTURE_FORMAT_RG8UINT = 9;
export const TEXTURE_FORMAT_RG8SINT = 10;
export const TEXTURE_FORMAT_R32UINT = 11;
export const TEXTURE_FORMAT_R32SINT = 12;
export const TEXTURE_FORMAT_R32FLOAT = 13;
export const TEXTURE_FORMAT_RG16UINT = 14;
export const TEXTURE_FORMAT_RG16SINT = 15;
export const TEXTURE_FORMAT_RG16FLOAT = 16;
export const TEXTURE_FORMAT_RGBA8UNORM = 17;
export const TEXTURE_FORMAT_RGBA8UNORM_SRGB = 18;
export const TEXTURE_FORMAT_RGBA8SNORM = 19;
export const TEXTURE_FORMAT_RGBA8UINT = 20;
export const TEXTURE_FORMAT_RGBA8SINT = 21;
export const TEXTURE_FORMAT_BGRA8UNORM = 22;
export const TEXTURE_FORMAT_BGRA8UNORM_SRGB = 23;
export const TEXTURE_FORMAT_RGB9E5UFLOAT = 24;
export const TEXTURE_FORMAT_RGB10A2UINT = 25;
export const TEXTURE_FORMAT_RGB10A2UNORM = 26;
export const TEXTURE_FORMAT_RG11B10UFLOAT = 27;
export const TEXTURE_FORMAT_RG32UINT = 28;
export const TEXTURE_FORMAT_RG32SINT = 29;
export const TEXTURE_FORMAT_RG32FLOAT = 30;
export const TEXTURE_FORMAT_RGBA16UINT = 31;
export const TEXTURE_FORMAT_RGBA16SINT = 32;
export const TEXTURE_FORMAT_RGBA16FLOAT = 33;
export const TEXTURE_FORMAT_RGBA32UINT = 34;
export const TEXTURE_FORMAT_RGBA32SINT = 35;
export const TEXTURE_FORMAT_RGBA32FLOAT = 36;
export const TEXTURE_FORMAT_STENCIL8 = 37;
export const TEXTURE_FORMAT_DEPTH16UNORM = 38;
export const TEXTURE_FORMAT_DEPTH24PLUS = 39;
export const TEXTURE_FORMAT_DEPTH24PLUS_STENCIL8 = 40;
export const TEXTURE_FORMAT_DEPTH32FLOAT = 41;
export const TEXTURE_FORMAT_DEPTH32FLOAT_STENCIL8 = 42;
export const TEXTURE_FORMAT_BC1_RGBA_UNORM = 43;
export const TEXTURE_FORMAT_BC1_RGBA_UNORM_SRGB = 44;
export const TEXTURE_FORMAT_BC2_RGBA_UNORM = 45;
export const TEXTURE_FORMAT_BC2_RGBA_UNORM_SRGB = 46;
export const TEXTURE_FORMAT_BC3_RGBA_UNORM = 47;
export const TEXTURE_FORMAT_BC3_RGBA_UNORM_SRGB = 48;
export const TEXTURE_FORMAT_BC4_R_UNORM = 49;
export const TEXTURE_FORMAT_BC4_R_SNORM = 50;
export const TEXTURE_FORMAT_BC5_RG_UNORM = 51;
export const TEXTURE_FORMAT_BC5_RG_SNORM = 52;
export const TEXTURE_FORMAT_BC6H_RGB_UFLOAT = 53;
export const TEXTURE_FORMAT_BC6H_RGB_FLOAT = 54;
export const TEXTURE_FORMAT_BC7_RGBA_UNORM = 55;
export const TEXTURE_FORMAT_BC7_RGBA_UNORM_SRGB = 56;
export const TEXTURE_FORMAT_ETC2_RGB8UNORM = 57;
export const TEXTURE_FORMAT_ETC2_RGB8UNORM_SRGB = 58;
export const TEXTURE_FORMAT_ETC2_RGB8A1UNORM = 59;
export const TEXTURE_FORMAT_ETC2_RGB8A1UNORM_SRGB = 60;
export const TEXTURE_FORMAT_ETC2_RGBA8UNORM = 61;
export const TEXTURE_FORMAT_ETC2_RGBA8UNORM_SRGB = 62;
export const TEXTURE_FORMAT_EAC_R11UNORM = 63;
export const TEXTURE_FORMAT_EAC_R11SNORM = 64;
export const TEXTURE_FORMAT_EAC_RG11UNORM = 65;
export const TEXTURE_FORMAT_EAC_RG11SNORM = 66;
export const TEXTURE_FORMAT_ASTC_4X4_UNORM = 67;
export const TEXTURE_FORMAT_ASTC_4X4_UNORM_SRGB = 68;
export const TEXTURE_FORMAT_ASTC_5X4_UNORM = 69;
export const TEXTURE_FORMAT_ASTC_5X4_UNORM_SRGB = 70;
export const TEXTURE_FORMAT_ASTC_5X5_UNORM = 71;
export const TEXTURE_FORMAT_ASTC_5X5_UNORM_SRGB = 72;
export const TEXTURE_FORMAT_ASTC_6X5_UNORM = 73;
export const TEXTURE_FORMAT_ASTC_6X5_UNORM_SRGB = 74;
export const TEXTURE_FORMAT_ASTC_6X6_UNORM = 75;
export const TEXTURE_FORMAT_ASTC_6X6_UNORM_SRGB = 76;
export const TEXTURE_FORMAT_ASTC_8X5_UNORM = 77;
export const TEXTURE_FORMAT_ASTC_8X5_UNORM_SRGB = 78;
export const TEXTURE_FORMAT_ASTC_8X6_UNORM = 79;
export const TEXTURE_FORMAT_ASTC_8X6_UNORM_SRGB = 80;
export const TEXTURE_FORMAT_ASTC_8X8_UNORM = 81;
export const TEXTURE_FORMAT_ASTC_8X8_UNORM_SRGB = 82;
export const TEXTURE_FORMAT_ASTC_10X5_UNORM = 83;
export const TEXTURE_FORMAT_ASTC_10X5_UNORM_SRGB = 84;
export const TEXTURE_FORMAT_ASTC_10X6_UNORM = 85;
export const TEXTURE_FORMAT_ASTC_10X6_UNORM_SRGB = 86;
export const TEXTURE_FORMAT_ASTC_10X8_UNORM = 87;
export const TEXTURE_FORMAT_ASTC_10X8_UNORM_SRGB = 88;
export const TEXTURE_FORMAT_ASTC_10X10_UNORM = 89;
export const TEXTURE_FORMAT_ASTC_10X10_UNORM_SRGB = 90;
export const TEXTURE_FORMAT_ASTC_12X10_UNORM = 91;
export const TEXTURE_FORMAT_ASTC_12X10_UNORM_SRGB = 92;
export const TEXTURE_FORMAT_ASTC_12X12_UNORM = 93;
export const TEXTURE_FORMAT_ASTC_12X12_UNORM_SRGB = 94;

/** The member `val` is, or null for a number no member has. */
export function of_TextureFormat(val: number): TextureFormat | null {
  return val === TEXTURE_FORMAT_R8UNORM ||
    val === TEXTURE_FORMAT_R8SNORM ||
    val === TEXTURE_FORMAT_R8UINT ||
    val === TEXTURE_FORMAT_R8SINT ||
    val === TEXTURE_FORMAT_R16UINT ||
    val === TEXTURE_FORMAT_R16SINT ||
    val === TEXTURE_FORMAT_R16FLOAT ||
    val === TEXTURE_FORMAT_RG8UNORM ||
    val === TEXTURE_FORMAT_RG8SNORM ||
    val === TEXTURE_FORMAT_RG8UINT ||
    val === TEXTURE_FORMAT_RG8SINT ||
    val === TEXTURE_FORMAT_R32UINT ||
    val === TEXTURE_FORMAT_R32SINT ||
    val === TEXTURE_FORMAT_R32FLOAT ||
    val === TEXTURE_FORMAT_RG16UINT ||
    val === TEXTURE_FORMAT_RG16SINT ||
    val === TEXTURE_FORMAT_RG16FLOAT ||
    val === TEXTURE_FORMAT_RGBA8UNORM ||
    val === TEXTURE_FORMAT_RGBA8UNORM_SRGB ||
    val === TEXTURE_FORMAT_RGBA8SNORM ||
    val === TEXTURE_FORMAT_RGBA8UINT ||
    val === TEXTURE_FORMAT_RGBA8SINT ||
    val === TEXTURE_FORMAT_BGRA8UNORM ||
    val === TEXTURE_FORMAT_BGRA8UNORM_SRGB ||
    val === TEXTURE_FORMAT_RGB9E5UFLOAT ||
    val === TEXTURE_FORMAT_RGB10A2UINT ||
    val === TEXTURE_FORMAT_RGB10A2UNORM ||
    val === TEXTURE_FORMAT_RG11B10UFLOAT ||
    val === TEXTURE_FORMAT_RG32UINT ||
    val === TEXTURE_FORMAT_RG32SINT ||
    val === TEXTURE_FORMAT_RG32FLOAT ||
    val === TEXTURE_FORMAT_RGBA16UINT ||
    val === TEXTURE_FORMAT_RGBA16SINT ||
    val === TEXTURE_FORMAT_RGBA16FLOAT ||
    val === TEXTURE_FORMAT_RGBA32UINT ||
    val === TEXTURE_FORMAT_RGBA32SINT ||
    val === TEXTURE_FORMAT_RGBA32FLOAT ||
    val === TEXTURE_FORMAT_STENCIL8 ||
    val === TEXTURE_FORMAT_DEPTH16UNORM ||
    val === TEXTURE_FORMAT_DEPTH24PLUS ||
    val === TEXTURE_FORMAT_DEPTH24PLUS_STENCIL8 ||
    val === TEXTURE_FORMAT_DEPTH32FLOAT ||
    val === TEXTURE_FORMAT_DEPTH32FLOAT_STENCIL8 ||
    val === TEXTURE_FORMAT_BC1_RGBA_UNORM ||
    val === TEXTURE_FORMAT_BC1_RGBA_UNORM_SRGB ||
    val === TEXTURE_FORMAT_BC2_RGBA_UNORM ||
    val === TEXTURE_FORMAT_BC2_RGBA_UNORM_SRGB ||
    val === TEXTURE_FORMAT_BC3_RGBA_UNORM ||
    val === TEXTURE_FORMAT_BC3_RGBA_UNORM_SRGB ||
    val === TEXTURE_FORMAT_BC4_R_UNORM ||
    val === TEXTURE_FORMAT_BC4_R_SNORM ||
    val === TEXTURE_FORMAT_BC5_RG_UNORM ||
    val === TEXTURE_FORMAT_BC5_RG_SNORM ||
    val === TEXTURE_FORMAT_BC6H_RGB_UFLOAT ||
    val === TEXTURE_FORMAT_BC6H_RGB_FLOAT ||
    val === TEXTURE_FORMAT_BC7_RGBA_UNORM ||
    val === TEXTURE_FORMAT_BC7_RGBA_UNORM_SRGB ||
    val === TEXTURE_FORMAT_ETC2_RGB8UNORM ||
    val === TEXTURE_FORMAT_ETC2_RGB8UNORM_SRGB ||
    val === TEXTURE_FORMAT_ETC2_RGB8A1UNORM ||
    val === TEXTURE_FORMAT_ETC2_RGB8A1UNORM_SRGB ||
    val === TEXTURE_FORMAT_ETC2_RGBA8UNORM ||
    val === TEXTURE_FORMAT_ETC2_RGBA8UNORM_SRGB ||
    val === TEXTURE_FORMAT_EAC_R11UNORM ||
    val === TEXTURE_FORMAT_EAC_R11SNORM ||
    val === TEXTURE_FORMAT_EAC_RG11UNORM ||
    val === TEXTURE_FORMAT_EAC_RG11SNORM ||
    val === TEXTURE_FORMAT_ASTC_4X4_UNORM ||
    val === TEXTURE_FORMAT_ASTC_4X4_UNORM_SRGB ||
    val === TEXTURE_FORMAT_ASTC_5X4_UNORM ||
    val === TEXTURE_FORMAT_ASTC_5X4_UNORM_SRGB ||
    val === TEXTURE_FORMAT_ASTC_5X5_UNORM ||
    val === TEXTURE_FORMAT_ASTC_5X5_UNORM_SRGB ||
    val === TEXTURE_FORMAT_ASTC_6X5_UNORM ||
    val === TEXTURE_FORMAT_ASTC_6X5_UNORM_SRGB ||
    val === TEXTURE_FORMAT_ASTC_6X6_UNORM ||
    val === TEXTURE_FORMAT_ASTC_6X6_UNORM_SRGB ||
    val === TEXTURE_FORMAT_ASTC_8X5_UNORM ||
    val === TEXTURE_FORMAT_ASTC_8X5_UNORM_SRGB ||
    val === TEXTURE_FORMAT_ASTC_8X6_UNORM ||
    val === TEXTURE_FORMAT_ASTC_8X6_UNORM_SRGB ||
    val === TEXTURE_FORMAT_ASTC_8X8_UNORM ||
    val === TEXTURE_FORMAT_ASTC_8X8_UNORM_SRGB ||
    val === TEXTURE_FORMAT_ASTC_10X5_UNORM ||
    val === TEXTURE_FORMAT_ASTC_10X5_UNORM_SRGB ||
    val === TEXTURE_FORMAT_ASTC_10X6_UNORM ||
    val === TEXTURE_FORMAT_ASTC_10X6_UNORM_SRGB ||
    val === TEXTURE_FORMAT_ASTC_10X8_UNORM ||
    val === TEXTURE_FORMAT_ASTC_10X8_UNORM_SRGB ||
    val === TEXTURE_FORMAT_ASTC_10X10_UNORM ||
    val === TEXTURE_FORMAT_ASTC_10X10_UNORM_SRGB ||
    val === TEXTURE_FORMAT_ASTC_12X10_UNORM ||
    val === TEXTURE_FORMAT_ASTC_12X10_UNORM_SRGB ||
    val === TEXTURE_FORMAT_ASTC_12X12_UNORM ||
    val === TEXTURE_FORMAT_ASTC_12X12_UNORM_SRGB
    ? val
    : null;
}

/** These are the members of GPUVertexFormat. */
export const VERTEX_FORMATS = [
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

export type VertexFormat =
  | typeof VERTEX_FORMAT_UINT8
  | typeof VERTEX_FORMAT_UINT8X2
  | typeof VERTEX_FORMAT_UINT8X4
  | typeof VERTEX_FORMAT_SINT8
  | typeof VERTEX_FORMAT_SINT8X2
  | typeof VERTEX_FORMAT_SINT8X4
  | typeof VERTEX_FORMAT_UNORM8
  | typeof VERTEX_FORMAT_UNORM8X2
  | typeof VERTEX_FORMAT_UNORM8X4
  | typeof VERTEX_FORMAT_SNORM8
  | typeof VERTEX_FORMAT_SNORM8X2
  | typeof VERTEX_FORMAT_SNORM8X4
  | typeof VERTEX_FORMAT_UINT16
  | typeof VERTEX_FORMAT_UINT16X2
  | typeof VERTEX_FORMAT_UINT16X4
  | typeof VERTEX_FORMAT_SINT16
  | typeof VERTEX_FORMAT_SINT16X2
  | typeof VERTEX_FORMAT_SINT16X4
  | typeof VERTEX_FORMAT_UNORM16
  | typeof VERTEX_FORMAT_UNORM16X2
  | typeof VERTEX_FORMAT_UNORM16X4
  | typeof VERTEX_FORMAT_SNORM16
  | typeof VERTEX_FORMAT_SNORM16X2
  | typeof VERTEX_FORMAT_SNORM16X4
  | typeof VERTEX_FORMAT_FLOAT16
  | typeof VERTEX_FORMAT_FLOAT16X2
  | typeof VERTEX_FORMAT_FLOAT16X4
  | typeof VERTEX_FORMAT_FLOAT32
  | typeof VERTEX_FORMAT_FLOAT32X2
  | typeof VERTEX_FORMAT_FLOAT32X3
  | typeof VERTEX_FORMAT_FLOAT32X4
  | typeof VERTEX_FORMAT_UINT32
  | typeof VERTEX_FORMAT_UINT32X2
  | typeof VERTEX_FORMAT_UINT32X3
  | typeof VERTEX_FORMAT_UINT32X4
  | typeof VERTEX_FORMAT_SINT32
  | typeof VERTEX_FORMAT_SINT32X2
  | typeof VERTEX_FORMAT_SINT32X3
  | typeof VERTEX_FORMAT_SINT32X4
  | typeof VERTEX_FORMAT_UNORM10_10_10_2
  | typeof VERTEX_FORMAT_UNORM8X4_BGRA;
export const VERTEX_FORMAT_UINT8 = 0;
export const VERTEX_FORMAT_UINT8X2 = 1;
export const VERTEX_FORMAT_UINT8X4 = 2;
export const VERTEX_FORMAT_SINT8 = 3;
export const VERTEX_FORMAT_SINT8X2 = 4;
export const VERTEX_FORMAT_SINT8X4 = 5;
export const VERTEX_FORMAT_UNORM8 = 6;
export const VERTEX_FORMAT_UNORM8X2 = 7;
export const VERTEX_FORMAT_UNORM8X4 = 8;
export const VERTEX_FORMAT_SNORM8 = 9;
export const VERTEX_FORMAT_SNORM8X2 = 10;
export const VERTEX_FORMAT_SNORM8X4 = 11;
export const VERTEX_FORMAT_UINT16 = 12;
export const VERTEX_FORMAT_UINT16X2 = 13;
export const VERTEX_FORMAT_UINT16X4 = 14;
export const VERTEX_FORMAT_SINT16 = 15;
export const VERTEX_FORMAT_SINT16X2 = 16;
export const VERTEX_FORMAT_SINT16X4 = 17;
export const VERTEX_FORMAT_UNORM16 = 18;
export const VERTEX_FORMAT_UNORM16X2 = 19;
export const VERTEX_FORMAT_UNORM16X4 = 20;
export const VERTEX_FORMAT_SNORM16 = 21;
export const VERTEX_FORMAT_SNORM16X2 = 22;
export const VERTEX_FORMAT_SNORM16X4 = 23;
export const VERTEX_FORMAT_FLOAT16 = 24;
export const VERTEX_FORMAT_FLOAT16X2 = 25;
export const VERTEX_FORMAT_FLOAT16X4 = 26;
export const VERTEX_FORMAT_FLOAT32 = 27;
export const VERTEX_FORMAT_FLOAT32X2 = 28;
export const VERTEX_FORMAT_FLOAT32X3 = 29;
export const VERTEX_FORMAT_FLOAT32X4 = 30;
export const VERTEX_FORMAT_UINT32 = 31;
export const VERTEX_FORMAT_UINT32X2 = 32;
export const VERTEX_FORMAT_UINT32X3 = 33;
export const VERTEX_FORMAT_UINT32X4 = 34;
export const VERTEX_FORMAT_SINT32 = 35;
export const VERTEX_FORMAT_SINT32X2 = 36;
export const VERTEX_FORMAT_SINT32X3 = 37;
export const VERTEX_FORMAT_SINT32X4 = 38;
export const VERTEX_FORMAT_UNORM10_10_10_2 = 39;
export const VERTEX_FORMAT_UNORM8X4_BGRA = 40;

/** The member `val` is, or null for a number no member has. */
export function of_VertexFormat(val: number): VertexFormat | null {
  return val === VERTEX_FORMAT_UINT8 ||
    val === VERTEX_FORMAT_UINT8X2 ||
    val === VERTEX_FORMAT_UINT8X4 ||
    val === VERTEX_FORMAT_SINT8 ||
    val === VERTEX_FORMAT_SINT8X2 ||
    val === VERTEX_FORMAT_SINT8X4 ||
    val === VERTEX_FORMAT_UNORM8 ||
    val === VERTEX_FORMAT_UNORM8X2 ||
    val === VERTEX_FORMAT_UNORM8X4 ||
    val === VERTEX_FORMAT_SNORM8 ||
    val === VERTEX_FORMAT_SNORM8X2 ||
    val === VERTEX_FORMAT_SNORM8X4 ||
    val === VERTEX_FORMAT_UINT16 ||
    val === VERTEX_FORMAT_UINT16X2 ||
    val === VERTEX_FORMAT_UINT16X4 ||
    val === VERTEX_FORMAT_SINT16 ||
    val === VERTEX_FORMAT_SINT16X2 ||
    val === VERTEX_FORMAT_SINT16X4 ||
    val === VERTEX_FORMAT_UNORM16 ||
    val === VERTEX_FORMAT_UNORM16X2 ||
    val === VERTEX_FORMAT_UNORM16X4 ||
    val === VERTEX_FORMAT_SNORM16 ||
    val === VERTEX_FORMAT_SNORM16X2 ||
    val === VERTEX_FORMAT_SNORM16X4 ||
    val === VERTEX_FORMAT_FLOAT16 ||
    val === VERTEX_FORMAT_FLOAT16X2 ||
    val === VERTEX_FORMAT_FLOAT16X4 ||
    val === VERTEX_FORMAT_FLOAT32 ||
    val === VERTEX_FORMAT_FLOAT32X2 ||
    val === VERTEX_FORMAT_FLOAT32X3 ||
    val === VERTEX_FORMAT_FLOAT32X4 ||
    val === VERTEX_FORMAT_UINT32 ||
    val === VERTEX_FORMAT_UINT32X2 ||
    val === VERTEX_FORMAT_UINT32X3 ||
    val === VERTEX_FORMAT_UINT32X4 ||
    val === VERTEX_FORMAT_SINT32 ||
    val === VERTEX_FORMAT_SINT32X2 ||
    val === VERTEX_FORMAT_SINT32X3 ||
    val === VERTEX_FORMAT_SINT32X4 ||
    val === VERTEX_FORMAT_UNORM10_10_10_2 ||
    val === VERTEX_FORMAT_UNORM8X4_BGRA
    ? val
    : null;
}

export const PRIMITIVE_TOPOLOGIES = [
  "point-list",
  "line-list",
  "line-strip",
  "triangle-list",
  "triangle-strip",
] as const;

export type PrimitiveTopology =
  | typeof PRIMITIVE_TOPOLOGY_POINT_LIST
  | typeof PRIMITIVE_TOPOLOGY_LINE_LIST
  | typeof PRIMITIVE_TOPOLOGY_LINE_STRIP
  | typeof PRIMITIVE_TOPOLOGY_TRIANGLE_LIST
  | typeof PRIMITIVE_TOPOLOGY_TRIANGLE_STRIP;
export const PRIMITIVE_TOPOLOGY_POINT_LIST = 0;
export const PRIMITIVE_TOPOLOGY_LINE_LIST = 1;
export const PRIMITIVE_TOPOLOGY_LINE_STRIP = 2;
export const PRIMITIVE_TOPOLOGY_TRIANGLE_LIST = 3;
export const PRIMITIVE_TOPOLOGY_TRIANGLE_STRIP = 4;

/** The member `val` is, or null for a number no member has. */
export function of_PrimitiveTopology(val: number): PrimitiveTopology | null {
  return val === PRIMITIVE_TOPOLOGY_POINT_LIST ||
    val === PRIMITIVE_TOPOLOGY_LINE_LIST ||
    val === PRIMITIVE_TOPOLOGY_LINE_STRIP ||
    val === PRIMITIVE_TOPOLOGY_TRIANGLE_LIST ||
    val === PRIMITIVE_TOPOLOGY_TRIANGLE_STRIP
    ? val
    : null;
}

export const INDEX_FORMATS = ["uint16", "uint32"] as const;

export type IndexFormat =
  typeof INDEX_FORMAT_UINT16 | typeof INDEX_FORMAT_UINT32;
export const INDEX_FORMAT_UINT16 = 0;
export const INDEX_FORMAT_UINT32 = 1;

/** The member `val` is, or null for a number no member has. */
export function of_IndexFormat(val: number): IndexFormat | null {
  return val === INDEX_FORMAT_UINT16 || val === INDEX_FORMAT_UINT32
    ? val
    : null;
}

export const FRONT_FACES = ["ccw", "cw"] as const;

export type FrontFace = typeof FRONT_FACE_CCW | typeof FRONT_FACE_CW;
export const FRONT_FACE_CCW = 0;
export const FRONT_FACE_CW = 1;

/** The member `val` is, or null for a number no member has. */
export function of_FrontFace(val: number): FrontFace | null {
  return val === FRONT_FACE_CCW || val === FRONT_FACE_CW ? val : null;
}

export const CULL_MODES = ["none", "front", "back"] as const;

export type CullMode =
  typeof CULL_MODE_NONE | typeof CULL_MODE_FRONT | typeof CULL_MODE_BACK;
export const CULL_MODE_NONE = 0;
export const CULL_MODE_FRONT = 1;
export const CULL_MODE_BACK = 2;

/** The member `val` is, or null for a number no member has. */
export function of_CullMode(val: number): CullMode | null {
  return val === CULL_MODE_NONE ||
    val === CULL_MODE_FRONT ||
    val === CULL_MODE_BACK
    ? val
    : null;
}

export const COMPARE_FUNCTIONS = [
  "never",
  "less",
  "equal",
  "less-equal",
  "greater",
  "not-equal",
  "greater-equal",
  "always",
] as const;

export type CompareFunction =
  | typeof COMPARE_FUNCTION_NEVER
  | typeof COMPARE_FUNCTION_LESS
  | typeof COMPARE_FUNCTION_EQUAL
  | typeof COMPARE_FUNCTION_LESS_EQUAL
  | typeof COMPARE_FUNCTION_GREATER
  | typeof COMPARE_FUNCTION_NOT_EQUAL
  | typeof COMPARE_FUNCTION_GREATER_EQUAL
  | typeof COMPARE_FUNCTION_ALWAYS;
export const COMPARE_FUNCTION_NEVER = 0;
export const COMPARE_FUNCTION_LESS = 1;
export const COMPARE_FUNCTION_EQUAL = 2;
export const COMPARE_FUNCTION_LESS_EQUAL = 3;
export const COMPARE_FUNCTION_GREATER = 4;
export const COMPARE_FUNCTION_NOT_EQUAL = 5;
export const COMPARE_FUNCTION_GREATER_EQUAL = 6;
export const COMPARE_FUNCTION_ALWAYS = 7;

/** The member `val` is, or null for a number no member has. */
export function of_CompareFunction(val: number): CompareFunction | null {
  return val === COMPARE_FUNCTION_NEVER ||
    val === COMPARE_FUNCTION_LESS ||
    val === COMPARE_FUNCTION_EQUAL ||
    val === COMPARE_FUNCTION_LESS_EQUAL ||
    val === COMPARE_FUNCTION_GREATER ||
    val === COMPARE_FUNCTION_NOT_EQUAL ||
    val === COMPARE_FUNCTION_GREATER_EQUAL ||
    val === COMPARE_FUNCTION_ALWAYS
    ? val
    : null;
}

export const STENCIL_OPERATIONS = [
  "keep",
  "zero",
  "replace",
  "invert",
  "increment-clamp",
  "decrement-clamp",
  "increment-wrap",
  "decrement-wrap",
] as const;

export type StencilOperation =
  | typeof STENCIL_OPERATION_KEEP
  | typeof STENCIL_OPERATION_ZERO
  | typeof STENCIL_OPERATION_REPLACE
  | typeof STENCIL_OPERATION_INVERT
  | typeof STENCIL_OPERATION_INCREMENT_CLAMP
  | typeof STENCIL_OPERATION_DECREMENT_CLAMP
  | typeof STENCIL_OPERATION_INCREMENT_WRAP
  | typeof STENCIL_OPERATION_DECREMENT_WRAP;
export const STENCIL_OPERATION_KEEP = 0;
export const STENCIL_OPERATION_ZERO = 1;
export const STENCIL_OPERATION_REPLACE = 2;
export const STENCIL_OPERATION_INVERT = 3;
export const STENCIL_OPERATION_INCREMENT_CLAMP = 4;
export const STENCIL_OPERATION_DECREMENT_CLAMP = 5;
export const STENCIL_OPERATION_INCREMENT_WRAP = 6;
export const STENCIL_OPERATION_DECREMENT_WRAP = 7;

/** The member `val` is, or null for a number no member has. */
export function of_StencilOperation(val: number): StencilOperation | null {
  return val === STENCIL_OPERATION_KEEP ||
    val === STENCIL_OPERATION_ZERO ||
    val === STENCIL_OPERATION_REPLACE ||
    val === STENCIL_OPERATION_INVERT ||
    val === STENCIL_OPERATION_INCREMENT_CLAMP ||
    val === STENCIL_OPERATION_DECREMENT_CLAMP ||
    val === STENCIL_OPERATION_INCREMENT_WRAP ||
    val === STENCIL_OPERATION_DECREMENT_WRAP
    ? val
    : null;
}

export const BLEND_OPERATIONS = [
  "add",
  "subtract",
  "reverse-subtract",
  "min",
  "max",
] as const;

export type BlendOperation =
  | typeof BLEND_OPERATION_ADD
  | typeof BLEND_OPERATION_SUBTRACT
  | typeof BLEND_OPERATION_REVERSE_SUBTRACT
  | typeof BLEND_OPERATION_MIN
  | typeof BLEND_OPERATION_MAX;
export const BLEND_OPERATION_ADD = 0;
export const BLEND_OPERATION_SUBTRACT = 1;
export const BLEND_OPERATION_REVERSE_SUBTRACT = 2;
export const BLEND_OPERATION_MIN = 3;
export const BLEND_OPERATION_MAX = 4;

/** The member `val` is, or null for a number no member has. */
export function of_BlendOperation(val: number): BlendOperation | null {
  return val === BLEND_OPERATION_ADD ||
    val === BLEND_OPERATION_SUBTRACT ||
    val === BLEND_OPERATION_REVERSE_SUBTRACT ||
    val === BLEND_OPERATION_MIN ||
    val === BLEND_OPERATION_MAX
    ? val
    : null;
}

export const BLEND_FACTORS = [
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

export type BlendFactor =
  | typeof BLEND_FACTOR_ZERO
  | typeof BLEND_FACTOR_ONE
  | typeof BLEND_FACTOR_SRC
  | typeof BLEND_FACTOR_ONE_MINUS_SRC
  | typeof BLEND_FACTOR_SRC_ALPHA
  | typeof BLEND_FACTOR_ONE_MINUS_SRC_ALPHA
  | typeof BLEND_FACTOR_DST
  | typeof BLEND_FACTOR_ONE_MINUS_DST
  | typeof BLEND_FACTOR_DST_ALPHA
  | typeof BLEND_FACTOR_ONE_MINUS_DST_ALPHA
  | typeof BLEND_FACTOR_SRC_ALPHA_SATURATED
  | typeof BLEND_FACTOR_CONSTANT
  | typeof BLEND_FACTOR_ONE_MINUS_CONSTANT
  | typeof BLEND_FACTOR_SRC1
  | typeof BLEND_FACTOR_ONE_MINUS_SRC1
  | typeof BLEND_FACTOR_SRC1_ALPHA
  | typeof BLEND_FACTOR_ONE_MINUS_SRC1_ALPHA;
export const BLEND_FACTOR_ZERO = 0;
export const BLEND_FACTOR_ONE = 1;
export const BLEND_FACTOR_SRC = 2;
export const BLEND_FACTOR_ONE_MINUS_SRC = 3;
export const BLEND_FACTOR_SRC_ALPHA = 4;
export const BLEND_FACTOR_ONE_MINUS_SRC_ALPHA = 5;
export const BLEND_FACTOR_DST = 6;
export const BLEND_FACTOR_ONE_MINUS_DST = 7;
export const BLEND_FACTOR_DST_ALPHA = 8;
export const BLEND_FACTOR_ONE_MINUS_DST_ALPHA = 9;
export const BLEND_FACTOR_SRC_ALPHA_SATURATED = 10;
export const BLEND_FACTOR_CONSTANT = 11;
export const BLEND_FACTOR_ONE_MINUS_CONSTANT = 12;
export const BLEND_FACTOR_SRC1 = 13;
export const BLEND_FACTOR_ONE_MINUS_SRC1 = 14;
export const BLEND_FACTOR_SRC1_ALPHA = 15;
export const BLEND_FACTOR_ONE_MINUS_SRC1_ALPHA = 16;

/** The member `val` is, or null for a number no member has. */
export function of_BlendFactor(val: number): BlendFactor | null {
  return val === BLEND_FACTOR_ZERO ||
    val === BLEND_FACTOR_ONE ||
    val === BLEND_FACTOR_SRC ||
    val === BLEND_FACTOR_ONE_MINUS_SRC ||
    val === BLEND_FACTOR_SRC_ALPHA ||
    val === BLEND_FACTOR_ONE_MINUS_SRC_ALPHA ||
    val === BLEND_FACTOR_DST ||
    val === BLEND_FACTOR_ONE_MINUS_DST ||
    val === BLEND_FACTOR_DST_ALPHA ||
    val === BLEND_FACTOR_ONE_MINUS_DST_ALPHA ||
    val === BLEND_FACTOR_SRC_ALPHA_SATURATED ||
    val === BLEND_FACTOR_CONSTANT ||
    val === BLEND_FACTOR_ONE_MINUS_CONSTANT ||
    val === BLEND_FACTOR_SRC1 ||
    val === BLEND_FACTOR_ONE_MINUS_SRC1 ||
    val === BLEND_FACTOR_SRC1_ALPHA ||
    val === BLEND_FACTOR_ONE_MINUS_SRC1_ALPHA
    ? val
    : null;
}

export const VERTEX_STEP_MODES = ["vertex", "instance"] as const;

export type VertexStepMode =
  typeof VERTEX_STEP_MODE_VERTEX | typeof VERTEX_STEP_MODE_INSTANCE;
export const VERTEX_STEP_MODE_VERTEX = 0;
export const VERTEX_STEP_MODE_INSTANCE = 1;

/** The member `val` is, or null for a number no member has. */
export function of_VertexStepMode(val: number): VertexStepMode | null {
  return val === VERTEX_STEP_MODE_VERTEX || val === VERTEX_STEP_MODE_INSTANCE
    ? val
    : null;
}

export const ADDRESS_MODES = [
  "clamp-to-edge",
  "repeat",
  "mirror-repeat",
] as const;

export type AddressMode =
  | typeof ADDRESS_MODE_CLAMP_TO_EDGE
  | typeof ADDRESS_MODE_REPEAT
  | typeof ADDRESS_MODE_MIRROR_REPEAT;
export const ADDRESS_MODE_CLAMP_TO_EDGE = 0;
export const ADDRESS_MODE_REPEAT = 1;
export const ADDRESS_MODE_MIRROR_REPEAT = 2;

/** The member `val` is, or null for a number no member has. */
export function of_AddressMode(val: number): AddressMode | null {
  return val === ADDRESS_MODE_CLAMP_TO_EDGE ||
    val === ADDRESS_MODE_REPEAT ||
    val === ADDRESS_MODE_MIRROR_REPEAT
    ? val
    : null;
}

export const FILTER_MODES = ["nearest", "linear"] as const;

export type FilterMode = typeof FILTER_MODE_NEAREST | typeof FILTER_MODE_LINEAR;
export const FILTER_MODE_NEAREST = 0;
export const FILTER_MODE_LINEAR = 1;

/** The member `val` is, or null for a number no member has. */
export function of_FilterMode(val: number): FilterMode | null {
  return val === FILTER_MODE_NEAREST || val === FILTER_MODE_LINEAR ? val : null;
}

export const MIPMAP_FILTER_MODES = ["nearest", "linear"] as const;

export type MipmapFilterMode =
  typeof MIPMAP_FILTER_MODE_NEAREST | typeof MIPMAP_FILTER_MODE_LINEAR;
export const MIPMAP_FILTER_MODE_NEAREST = 0;
export const MIPMAP_FILTER_MODE_LINEAR = 1;

/** The member `val` is, or null for a number no member has. */
export function of_MipmapFilterMode(val: number): MipmapFilterMode | null {
  return val === MIPMAP_FILTER_MODE_NEAREST || val === MIPMAP_FILTER_MODE_LINEAR
    ? val
    : null;
}

export const TEXTURE_DIMENSIONS = ["1d", "2d", "3d"] as const;

export type TextureDimension =
  | typeof TEXTURE_DIMENSION_1D
  | typeof TEXTURE_DIMENSION_2D
  | typeof TEXTURE_DIMENSION_3D;
export const TEXTURE_DIMENSION_1D = 0;
export const TEXTURE_DIMENSION_2D = 1;
export const TEXTURE_DIMENSION_3D = 2;

/** The member `val` is, or null for a number no member has. */
export function of_TextureDimension(val: number): TextureDimension | null {
  return val === TEXTURE_DIMENSION_1D ||
    val === TEXTURE_DIMENSION_2D ||
    val === TEXTURE_DIMENSION_3D
    ? val
    : null;
}

export const TEXTURE_VIEW_DIMENSIONS = [
  "1d",
  "2d",
  "2d-array",
  "cube",
  "cube-array",
  "3d",
] as const;

export type TextureViewDimension =
  | typeof TEXTURE_VIEW_DIMENSION_1D
  | typeof TEXTURE_VIEW_DIMENSION_2D
  | typeof TEXTURE_VIEW_DIMENSION_2D_ARRAY
  | typeof TEXTURE_VIEW_DIMENSION_CUBE
  | typeof TEXTURE_VIEW_DIMENSION_CUBE_ARRAY
  | typeof TEXTURE_VIEW_DIMENSION_3D;
export const TEXTURE_VIEW_DIMENSION_1D = 0;
export const TEXTURE_VIEW_DIMENSION_2D = 1;
export const TEXTURE_VIEW_DIMENSION_2D_ARRAY = 2;
export const TEXTURE_VIEW_DIMENSION_CUBE = 3;
export const TEXTURE_VIEW_DIMENSION_CUBE_ARRAY = 4;
export const TEXTURE_VIEW_DIMENSION_3D = 5;

/** The member `val` is, or null for a number no member has. */
export function of_TextureViewDimension(
  val: number,
): TextureViewDimension | null {
  return val === TEXTURE_VIEW_DIMENSION_1D ||
    val === TEXTURE_VIEW_DIMENSION_2D ||
    val === TEXTURE_VIEW_DIMENSION_2D_ARRAY ||
    val === TEXTURE_VIEW_DIMENSION_CUBE ||
    val === TEXTURE_VIEW_DIMENSION_CUBE_ARRAY ||
    val === TEXTURE_VIEW_DIMENSION_3D
    ? val
    : null;
}

export const TEXTURE_ASPECTS = ["all", "stencil-only", "depth-only"] as const;

export type TextureAspect =
  | typeof TEXTURE_ASPECT_ALL
  | typeof TEXTURE_ASPECT_STENCIL_ONLY
  | typeof TEXTURE_ASPECT_DEPTH_ONLY;
export const TEXTURE_ASPECT_ALL = 0;
export const TEXTURE_ASPECT_STENCIL_ONLY = 1;
export const TEXTURE_ASPECT_DEPTH_ONLY = 2;

/** The member `val` is, or null for a number no member has. */
export function of_TextureAspect(val: number): TextureAspect | null {
  return val === TEXTURE_ASPECT_ALL ||
    val === TEXTURE_ASPECT_STENCIL_ONLY ||
    val === TEXTURE_ASPECT_DEPTH_ONLY
    ? val
    : null;
}

export const BUFFER_BINDING_TYPES = [
  "uniform",
  "storage",
  "read-only-storage",
] as const;

export type BufferBindingType =
  | typeof BUFFER_BINDING_TYPE_UNIFORM
  | typeof BUFFER_BINDING_TYPE_STORAGE
  | typeof BUFFER_BINDING_TYPE_READ_ONLY_STORAGE;
export const BUFFER_BINDING_TYPE_UNIFORM = 0;
export const BUFFER_BINDING_TYPE_STORAGE = 1;
export const BUFFER_BINDING_TYPE_READ_ONLY_STORAGE = 2;

/** The member `val` is, or null for a number no member has. */
export function of_BufferBindingType(val: number): BufferBindingType | null {
  return val === BUFFER_BINDING_TYPE_UNIFORM ||
    val === BUFFER_BINDING_TYPE_STORAGE ||
    val === BUFFER_BINDING_TYPE_READ_ONLY_STORAGE
    ? val
    : null;
}

export const SAMPLER_BINDING_TYPES = [
  "filtering",
  "non-filtering",
  "comparison",
] as const;

export type SamplerBindingType =
  | typeof SAMPLER_BINDING_TYPE_FILTERING
  | typeof SAMPLER_BINDING_TYPE_NON_FILTERING
  | typeof SAMPLER_BINDING_TYPE_COMPARISON;
export const SAMPLER_BINDING_TYPE_FILTERING = 0;
export const SAMPLER_BINDING_TYPE_NON_FILTERING = 1;
export const SAMPLER_BINDING_TYPE_COMPARISON = 2;

/** The member `val` is, or null for a number no member has. */
export function of_SamplerBindingType(val: number): SamplerBindingType | null {
  return val === SAMPLER_BINDING_TYPE_FILTERING ||
    val === SAMPLER_BINDING_TYPE_NON_FILTERING ||
    val === SAMPLER_BINDING_TYPE_COMPARISON
    ? val
    : null;
}

export const TEXTURE_SAMPLE_TYPES = [
  "float",
  "unfilterable-float",
  "depth",
  "sint",
  "uint",
] as const;

export type TextureSampleType =
  | typeof TEXTURE_SAMPLE_TYPE_FLOAT
  | typeof TEXTURE_SAMPLE_TYPE_UNFILTERABLE_FLOAT
  | typeof TEXTURE_SAMPLE_TYPE_DEPTH
  | typeof TEXTURE_SAMPLE_TYPE_SINT
  | typeof TEXTURE_SAMPLE_TYPE_UINT;
export const TEXTURE_SAMPLE_TYPE_FLOAT = 0;
export const TEXTURE_SAMPLE_TYPE_UNFILTERABLE_FLOAT = 1;
export const TEXTURE_SAMPLE_TYPE_DEPTH = 2;
export const TEXTURE_SAMPLE_TYPE_SINT = 3;
export const TEXTURE_SAMPLE_TYPE_UINT = 4;

/** The member `val` is, or null for a number no member has. */
export function of_TextureSampleType(val: number): TextureSampleType | null {
  return val === TEXTURE_SAMPLE_TYPE_FLOAT ||
    val === TEXTURE_SAMPLE_TYPE_UNFILTERABLE_FLOAT ||
    val === TEXTURE_SAMPLE_TYPE_DEPTH ||
    val === TEXTURE_SAMPLE_TYPE_SINT ||
    val === TEXTURE_SAMPLE_TYPE_UINT
    ? val
    : null;
}

export const STORAGE_TEXTURE_ACCESSES = [
  "write-only",
  "read-only",
  "read-write",
] as const;

export type StorageTextureAccess =
  | typeof STORAGE_TEXTURE_ACCESS_WRITE_ONLY
  | typeof STORAGE_TEXTURE_ACCESS_READ_ONLY
  | typeof STORAGE_TEXTURE_ACCESS_READ_WRITE;
export const STORAGE_TEXTURE_ACCESS_WRITE_ONLY = 0;
export const STORAGE_TEXTURE_ACCESS_READ_ONLY = 1;
export const STORAGE_TEXTURE_ACCESS_READ_WRITE = 2;

/** The member `val` is, or null for a number no member has. */
export function of_StorageTextureAccess(
  val: number,
): StorageTextureAccess | null {
  return val === STORAGE_TEXTURE_ACCESS_WRITE_ONLY ||
    val === STORAGE_TEXTURE_ACCESS_READ_ONLY ||
    val === STORAGE_TEXTURE_ACCESS_READ_WRITE
    ? val
    : null;
}

export const LOAD_OPS = ["load", "clear"] as const;

export type LoadOp = typeof LOAD_OP_LOAD | typeof LOAD_OP_CLEAR;
export const LOAD_OP_LOAD = 0;
export const LOAD_OP_CLEAR = 1;

/** The member `val` is, or null for a number no member has. */
export function of_LoadOp(val: number): LoadOp | null {
  return val === LOAD_OP_LOAD || val === LOAD_OP_CLEAR ? val : null;
}

export const STORE_OPS = ["store", "discard"] as const;

export type StoreOp = typeof STORE_OP_STORE | typeof STORE_OP_DISCARD;
export const STORE_OP_STORE = 0;
export const STORE_OP_DISCARD = 1;

/** The member `val` is, or null for a number no member has. */
export function of_StoreOp(val: number): StoreOp | null {
  return val === STORE_OP_STORE || val === STORE_OP_DISCARD ? val : null;
}

export const QUERY_TYPES = ["occlusion", "timestamp"] as const;

export type QueryType =
  typeof QUERY_TYPE_OCCLUSION | typeof QUERY_TYPE_TIMESTAMP;
export const QUERY_TYPE_OCCLUSION = 0;
export const QUERY_TYPE_TIMESTAMP = 1;

/** The member `val` is, or null for a number no member has. */
export function of_QueryType(val: number): QueryType | null {
  return val === QUERY_TYPE_OCCLUSION || val === QUERY_TYPE_TIMESTAMP
    ? val
    : null;
}

export const CANVAS_ALPHA_MODES = ["opaque", "premultiplied"] as const;

export type CanvasAlphaMode =
  typeof CANVAS_ALPHA_MODE_OPAQUE | typeof CANVAS_ALPHA_MODE_PREMULTIPLIED;
export const CANVAS_ALPHA_MODE_OPAQUE = 0;
export const CANVAS_ALPHA_MODE_PREMULTIPLIED = 1;

/** The member `val` is, or null for a number no member has. */
export function of_CanvasAlphaMode(val: number): CanvasAlphaMode | null {
  return val === CANVAS_ALPHA_MODE_OPAQUE ||
    val === CANVAS_ALPHA_MODE_PREMULTIPLIED
    ? val
    : null;
}

/** GPUFeatureName. The DEVICE block (api.json) holds a device's features as one bit each,
 * at the member's place. */
export const FEATURE_NAMES = [
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

export type FeatureName =
  | typeof FEATURE_NAME_DEPTH_CLIP_CONTROL
  | typeof FEATURE_NAME_DEPTH32FLOAT_STENCIL8
  | typeof FEATURE_NAME_TEXTURE_COMPRESSION_BC
  | typeof FEATURE_NAME_TEXTURE_COMPRESSION_BC_SLICED_3D
  | typeof FEATURE_NAME_TEXTURE_COMPRESSION_ETC2
  | typeof FEATURE_NAME_TEXTURE_COMPRESSION_ASTC
  | typeof FEATURE_NAME_TEXTURE_COMPRESSION_ASTC_SLICED_3D
  | typeof FEATURE_NAME_TIMESTAMP_QUERY
  | typeof FEATURE_NAME_INDIRECT_FIRST_INSTANCE
  | typeof FEATURE_NAME_SHADER_F16
  | typeof FEATURE_NAME_RG11B10UFLOAT_RENDERABLE
  | typeof FEATURE_NAME_BGRA8UNORM_STORAGE
  | typeof FEATURE_NAME_FLOAT32_FILTERABLE
  | typeof FEATURE_NAME_FLOAT32_BLENDABLE
  | typeof FEATURE_NAME_CLIP_DISTANCES
  | typeof FEATURE_NAME_DUAL_SOURCE_BLENDING
  | typeof FEATURE_NAME_SUBGROUPS;
export const FEATURE_NAME_DEPTH_CLIP_CONTROL = 0;
export const FEATURE_NAME_DEPTH32FLOAT_STENCIL8 = 1;
export const FEATURE_NAME_TEXTURE_COMPRESSION_BC = 2;
export const FEATURE_NAME_TEXTURE_COMPRESSION_BC_SLICED_3D = 3;
export const FEATURE_NAME_TEXTURE_COMPRESSION_ETC2 = 4;
export const FEATURE_NAME_TEXTURE_COMPRESSION_ASTC = 5;
export const FEATURE_NAME_TEXTURE_COMPRESSION_ASTC_SLICED_3D = 6;
export const FEATURE_NAME_TIMESTAMP_QUERY = 7;
export const FEATURE_NAME_INDIRECT_FIRST_INSTANCE = 8;
export const FEATURE_NAME_SHADER_F16 = 9;
export const FEATURE_NAME_RG11B10UFLOAT_RENDERABLE = 10;
export const FEATURE_NAME_BGRA8UNORM_STORAGE = 11;
export const FEATURE_NAME_FLOAT32_FILTERABLE = 12;
export const FEATURE_NAME_FLOAT32_BLENDABLE = 13;
export const FEATURE_NAME_CLIP_DISTANCES = 14;
export const FEATURE_NAME_DUAL_SOURCE_BLENDING = 15;
export const FEATURE_NAME_SUBGROUPS = 16;

/** The member `val` is, or null for a number no member has. */
export function of_FeatureName(val: number): FeatureName | null {
  return val === FEATURE_NAME_DEPTH_CLIP_CONTROL ||
    val === FEATURE_NAME_DEPTH32FLOAT_STENCIL8 ||
    val === FEATURE_NAME_TEXTURE_COMPRESSION_BC ||
    val === FEATURE_NAME_TEXTURE_COMPRESSION_BC_SLICED_3D ||
    val === FEATURE_NAME_TEXTURE_COMPRESSION_ETC2 ||
    val === FEATURE_NAME_TEXTURE_COMPRESSION_ASTC ||
    val === FEATURE_NAME_TEXTURE_COMPRESSION_ASTC_SLICED_3D ||
    val === FEATURE_NAME_TIMESTAMP_QUERY ||
    val === FEATURE_NAME_INDIRECT_FIRST_INSTANCE ||
    val === FEATURE_NAME_SHADER_F16 ||
    val === FEATURE_NAME_RG11B10UFLOAT_RENDERABLE ||
    val === FEATURE_NAME_BGRA8UNORM_STORAGE ||
    val === FEATURE_NAME_FLOAT32_FILTERABLE ||
    val === FEATURE_NAME_FLOAT32_BLENDABLE ||
    val === FEATURE_NAME_CLIP_DISTANCES ||
    val === FEATURE_NAME_DUAL_SOURCE_BLENDING ||
    val === FEATURE_NAME_SUBGROUPS
    ? val
    : null;
}

/** The members of GPUSupportedLimits. The DEVICE block (api.json) holds a device's limits
 * as one word each, at the member's place. */
export const LIMIT_NAMES = [
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

export type LimitName =
  | typeof LIMIT_NAME_MAX_TEXTURE_DIMENSION_1D
  | typeof LIMIT_NAME_MAX_TEXTURE_DIMENSION_2D
  | typeof LIMIT_NAME_MAX_TEXTURE_DIMENSION_3D
  | typeof LIMIT_NAME_MAX_TEXTURE_ARRAY_LAYERS
  | typeof LIMIT_NAME_MAX_BIND_GROUPS
  | typeof LIMIT_NAME_MAX_BIND_GROUPS_PLUS_VERTEX_BUFFERS
  | typeof LIMIT_NAME_MAX_BINDINGS_PER_BIND_GROUP
  | typeof LIMIT_NAME_MAX_DYNAMIC_UNIFORM_BUFFERS_PER_PIPELINE_LAYOUT
  | typeof LIMIT_NAME_MAX_DYNAMIC_STORAGE_BUFFERS_PER_PIPELINE_LAYOUT
  | typeof LIMIT_NAME_MAX_SAMPLED_TEXTURES_PER_SHADER_STAGE
  | typeof LIMIT_NAME_MAX_SAMPLERS_PER_SHADER_STAGE
  | typeof LIMIT_NAME_MAX_STORAGE_BUFFERS_PER_SHADER_STAGE
  | typeof LIMIT_NAME_MAX_STORAGE_TEXTURES_PER_SHADER_STAGE
  | typeof LIMIT_NAME_MAX_UNIFORM_BUFFERS_PER_SHADER_STAGE
  | typeof LIMIT_NAME_MAX_UNIFORM_BUFFER_BINDING_SIZE
  | typeof LIMIT_NAME_MAX_STORAGE_BUFFER_BINDING_SIZE
  | typeof LIMIT_NAME_MIN_UNIFORM_BUFFER_OFFSET_ALIGNMENT
  | typeof LIMIT_NAME_MIN_STORAGE_BUFFER_OFFSET_ALIGNMENT
  | typeof LIMIT_NAME_MAX_VERTEX_BUFFERS
  | typeof LIMIT_NAME_MAX_BUFFER_SIZE
  | typeof LIMIT_NAME_MAX_VERTEX_ATTRIBUTES
  | typeof LIMIT_NAME_MAX_VERTEX_BUFFER_ARRAY_STRIDE
  | typeof LIMIT_NAME_MAX_INTER_STAGE_SHADER_VARIABLES
  | typeof LIMIT_NAME_MAX_COLOR_ATTACHMENTS
  | typeof LIMIT_NAME_MAX_COLOR_ATTACHMENT_BYTES_PER_SAMPLE
  | typeof LIMIT_NAME_MAX_COMPUTE_WORKGROUP_STORAGE_SIZE
  | typeof LIMIT_NAME_MAX_COMPUTE_INVOCATIONS_PER_WORKGROUP
  | typeof LIMIT_NAME_MAX_COMPUTE_WORKGROUP_SIZE_X
  | typeof LIMIT_NAME_MAX_COMPUTE_WORKGROUP_SIZE_Y
  | typeof LIMIT_NAME_MAX_COMPUTE_WORKGROUP_SIZE_Z
  | typeof LIMIT_NAME_MAX_COMPUTE_WORKGROUPS_PER_DIMENSION;
export const LIMIT_NAME_MAX_TEXTURE_DIMENSION_1D = 0;
export const LIMIT_NAME_MAX_TEXTURE_DIMENSION_2D = 1;
export const LIMIT_NAME_MAX_TEXTURE_DIMENSION_3D = 2;
export const LIMIT_NAME_MAX_TEXTURE_ARRAY_LAYERS = 3;
export const LIMIT_NAME_MAX_BIND_GROUPS = 4;
export const LIMIT_NAME_MAX_BIND_GROUPS_PLUS_VERTEX_BUFFERS = 5;
export const LIMIT_NAME_MAX_BINDINGS_PER_BIND_GROUP = 6;
export const LIMIT_NAME_MAX_DYNAMIC_UNIFORM_BUFFERS_PER_PIPELINE_LAYOUT = 7;
export const LIMIT_NAME_MAX_DYNAMIC_STORAGE_BUFFERS_PER_PIPELINE_LAYOUT = 8;
export const LIMIT_NAME_MAX_SAMPLED_TEXTURES_PER_SHADER_STAGE = 9;
export const LIMIT_NAME_MAX_SAMPLERS_PER_SHADER_STAGE = 10;
export const LIMIT_NAME_MAX_STORAGE_BUFFERS_PER_SHADER_STAGE = 11;
export const LIMIT_NAME_MAX_STORAGE_TEXTURES_PER_SHADER_STAGE = 12;
export const LIMIT_NAME_MAX_UNIFORM_BUFFERS_PER_SHADER_STAGE = 13;
export const LIMIT_NAME_MAX_UNIFORM_BUFFER_BINDING_SIZE = 14;
export const LIMIT_NAME_MAX_STORAGE_BUFFER_BINDING_SIZE = 15;
export const LIMIT_NAME_MIN_UNIFORM_BUFFER_OFFSET_ALIGNMENT = 16;
export const LIMIT_NAME_MIN_STORAGE_BUFFER_OFFSET_ALIGNMENT = 17;
export const LIMIT_NAME_MAX_VERTEX_BUFFERS = 18;
export const LIMIT_NAME_MAX_BUFFER_SIZE = 19;
export const LIMIT_NAME_MAX_VERTEX_ATTRIBUTES = 20;
export const LIMIT_NAME_MAX_VERTEX_BUFFER_ARRAY_STRIDE = 21;
export const LIMIT_NAME_MAX_INTER_STAGE_SHADER_VARIABLES = 22;
export const LIMIT_NAME_MAX_COLOR_ATTACHMENTS = 23;
export const LIMIT_NAME_MAX_COLOR_ATTACHMENT_BYTES_PER_SAMPLE = 24;
export const LIMIT_NAME_MAX_COMPUTE_WORKGROUP_STORAGE_SIZE = 25;
export const LIMIT_NAME_MAX_COMPUTE_INVOCATIONS_PER_WORKGROUP = 26;
export const LIMIT_NAME_MAX_COMPUTE_WORKGROUP_SIZE_X = 27;
export const LIMIT_NAME_MAX_COMPUTE_WORKGROUP_SIZE_Y = 28;
export const LIMIT_NAME_MAX_COMPUTE_WORKGROUP_SIZE_Z = 29;
export const LIMIT_NAME_MAX_COMPUTE_WORKGROUPS_PER_DIMENSION = 30;

/** The member `val` is, or null for a number no member has. */
export function of_LimitName(val: number): LimitName | null {
  return val === LIMIT_NAME_MAX_TEXTURE_DIMENSION_1D ||
    val === LIMIT_NAME_MAX_TEXTURE_DIMENSION_2D ||
    val === LIMIT_NAME_MAX_TEXTURE_DIMENSION_3D ||
    val === LIMIT_NAME_MAX_TEXTURE_ARRAY_LAYERS ||
    val === LIMIT_NAME_MAX_BIND_GROUPS ||
    val === LIMIT_NAME_MAX_BIND_GROUPS_PLUS_VERTEX_BUFFERS ||
    val === LIMIT_NAME_MAX_BINDINGS_PER_BIND_GROUP ||
    val === LIMIT_NAME_MAX_DYNAMIC_UNIFORM_BUFFERS_PER_PIPELINE_LAYOUT ||
    val === LIMIT_NAME_MAX_DYNAMIC_STORAGE_BUFFERS_PER_PIPELINE_LAYOUT ||
    val === LIMIT_NAME_MAX_SAMPLED_TEXTURES_PER_SHADER_STAGE ||
    val === LIMIT_NAME_MAX_SAMPLERS_PER_SHADER_STAGE ||
    val === LIMIT_NAME_MAX_STORAGE_BUFFERS_PER_SHADER_STAGE ||
    val === LIMIT_NAME_MAX_STORAGE_TEXTURES_PER_SHADER_STAGE ||
    val === LIMIT_NAME_MAX_UNIFORM_BUFFERS_PER_SHADER_STAGE ||
    val === LIMIT_NAME_MAX_UNIFORM_BUFFER_BINDING_SIZE ||
    val === LIMIT_NAME_MAX_STORAGE_BUFFER_BINDING_SIZE ||
    val === LIMIT_NAME_MIN_UNIFORM_BUFFER_OFFSET_ALIGNMENT ||
    val === LIMIT_NAME_MIN_STORAGE_BUFFER_OFFSET_ALIGNMENT ||
    val === LIMIT_NAME_MAX_VERTEX_BUFFERS ||
    val === LIMIT_NAME_MAX_BUFFER_SIZE ||
    val === LIMIT_NAME_MAX_VERTEX_ATTRIBUTES ||
    val === LIMIT_NAME_MAX_VERTEX_BUFFER_ARRAY_STRIDE ||
    val === LIMIT_NAME_MAX_INTER_STAGE_SHADER_VARIABLES ||
    val === LIMIT_NAME_MAX_COLOR_ATTACHMENTS ||
    val === LIMIT_NAME_MAX_COLOR_ATTACHMENT_BYTES_PER_SAMPLE ||
    val === LIMIT_NAME_MAX_COMPUTE_WORKGROUP_STORAGE_SIZE ||
    val === LIMIT_NAME_MAX_COMPUTE_INVOCATIONS_PER_WORKGROUP ||
    val === LIMIT_NAME_MAX_COMPUTE_WORKGROUP_SIZE_X ||
    val === LIMIT_NAME_MAX_COMPUTE_WORKGROUP_SIZE_Y ||
    val === LIMIT_NAME_MAX_COMPUTE_WORKGROUP_SIZE_Z ||
    val === LIMIT_NAME_MAX_COMPUTE_WORKGROUPS_PER_DIMENSION
    ? val
    : null;
}
