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

export const TextureFormat = {
  r8unorm: 0,
  r8snorm: 1,
  r8uint: 2,
  r8sint: 3,
  r16uint: 4,
  r16sint: 5,
  r16float: 6,
  rg8unorm: 7,
  rg8snorm: 8,
  rg8uint: 9,
  rg8sint: 10,
  r32uint: 11,
  r32sint: 12,
  r32float: 13,
  rg16uint: 14,
  rg16sint: 15,
  rg16float: 16,
  rgba8unorm: 17,
  "rgba8unorm-srgb": 18,
  rgba8snorm: 19,
  rgba8uint: 20,
  rgba8sint: 21,
  bgra8unorm: 22,
  "bgra8unorm-srgb": 23,
  rgb9e5ufloat: 24,
  rgb10a2uint: 25,
  rgb10a2unorm: 26,
  rg11b10ufloat: 27,
  rg32uint: 28,
  rg32sint: 29,
  rg32float: 30,
  rgba16uint: 31,
  rgba16sint: 32,
  rgba16float: 33,
  rgba32uint: 34,
  rgba32sint: 35,
  rgba32float: 36,
  stencil8: 37,
  depth16unorm: 38,
  depth24plus: 39,
  "depth24plus-stencil8": 40,
  depth32float: 41,
  "depth32float-stencil8": 42,
  "bc1-rgba-unorm": 43,
  "bc1-rgba-unorm-srgb": 44,
  "bc2-rgba-unorm": 45,
  "bc2-rgba-unorm-srgb": 46,
  "bc3-rgba-unorm": 47,
  "bc3-rgba-unorm-srgb": 48,
  "bc4-r-unorm": 49,
  "bc4-r-snorm": 50,
  "bc5-rg-unorm": 51,
  "bc5-rg-snorm": 52,
  "bc6h-rgb-ufloat": 53,
  "bc6h-rgb-float": 54,
  "bc7-rgba-unorm": 55,
  "bc7-rgba-unorm-srgb": 56,
  "etc2-rgb8unorm": 57,
  "etc2-rgb8unorm-srgb": 58,
  "etc2-rgb8a1unorm": 59,
  "etc2-rgb8a1unorm-srgb": 60,
  "etc2-rgba8unorm": 61,
  "etc2-rgba8unorm-srgb": 62,
  "eac-r11unorm": 63,
  "eac-r11snorm": 64,
  "eac-rg11unorm": 65,
  "eac-rg11snorm": 66,
  "astc-4x4-unorm": 67,
  "astc-4x4-unorm-srgb": 68,
  "astc-5x4-unorm": 69,
  "astc-5x4-unorm-srgb": 70,
  "astc-5x5-unorm": 71,
  "astc-5x5-unorm-srgb": 72,
  "astc-6x5-unorm": 73,
  "astc-6x5-unorm-srgb": 74,
  "astc-6x6-unorm": 75,
  "astc-6x6-unorm-srgb": 76,
  "astc-8x5-unorm": 77,
  "astc-8x5-unorm-srgb": 78,
  "astc-8x6-unorm": 79,
  "astc-8x6-unorm-srgb": 80,
  "astc-8x8-unorm": 81,
  "astc-8x8-unorm-srgb": 82,
  "astc-10x5-unorm": 83,
  "astc-10x5-unorm-srgb": 84,
  "astc-10x6-unorm": 85,
  "astc-10x6-unorm-srgb": 86,
  "astc-10x8-unorm": 87,
  "astc-10x8-unorm-srgb": 88,
  "astc-10x10-unorm": 89,
  "astc-10x10-unorm-srgb": 90,
  "astc-12x10-unorm": 91,
  "astc-12x10-unorm-srgb": 92,
  "astc-12x12-unorm": 93,
  "astc-12x12-unorm-srgb": 94,
} as const;
export type TextureFormat = (typeof TextureFormat)[keyof typeof TextureFormat];

/** The name of the `TextureFormat` member numbered `n`, or undefined for a number
 * no member has. */
export function texture_format_name(
  n: number,
): keyof typeof TextureFormat | undefined {
  return (Object.keys(TextureFormat) as (keyof typeof TextureFormat)[]).find(
    (k) => TextureFormat[k] === n,
  );
}

/** The `TextureFormat` member `n` is, or null for a number no member has. */
export function texture_format_of(n: number): TextureFormat | null {
  return texture_format_name(n) === undefined ? null : (n as TextureFormat);
}

/** These are the members of GPUVertexFormat. */
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

export const VertexFormat = {
  uint8: 0,
  uint8x2: 1,
  uint8x4: 2,
  sint8: 3,
  sint8x2: 4,
  sint8x4: 5,
  unorm8: 6,
  unorm8x2: 7,
  unorm8x4: 8,
  snorm8: 9,
  snorm8x2: 10,
  snorm8x4: 11,
  uint16: 12,
  uint16x2: 13,
  uint16x4: 14,
  sint16: 15,
  sint16x2: 16,
  sint16x4: 17,
  unorm16: 18,
  unorm16x2: 19,
  unorm16x4: 20,
  snorm16: 21,
  snorm16x2: 22,
  snorm16x4: 23,
  float16: 24,
  float16x2: 25,
  float16x4: 26,
  float32: 27,
  float32x2: 28,
  float32x3: 29,
  float32x4: 30,
  uint32: 31,
  uint32x2: 32,
  uint32x3: 33,
  uint32x4: 34,
  sint32: 35,
  sint32x2: 36,
  sint32x3: 37,
  sint32x4: 38,
  "unorm10-10-10-2": 39,
  "unorm8x4-bgra": 40,
} as const;
export type VertexFormat = (typeof VertexFormat)[keyof typeof VertexFormat];

/** The name of the `VertexFormat` member numbered `n`, or undefined for a number
 * no member has. */
export function vertex_format_name(
  n: number,
): keyof typeof VertexFormat | undefined {
  return (Object.keys(VertexFormat) as (keyof typeof VertexFormat)[]).find(
    (k) => VertexFormat[k] === n,
  );
}

/** The `VertexFormat` member `n` is, or null for a number no member has. */
export function vertex_format_of(n: number): VertexFormat | null {
  return vertex_format_name(n) === undefined ? null : (n as VertexFormat);
}

export const PRIMITIVE_TOPOLOGY = [
  "point-list",
  "line-list",
  "line-strip",
  "triangle-list",
  "triangle-strip",
] as const;

export const PrimitiveTopology = {
  "point-list": 0,
  "line-list": 1,
  "line-strip": 2,
  "triangle-list": 3,
  "triangle-strip": 4,
} as const;
export type PrimitiveTopology =
  (typeof PrimitiveTopology)[keyof typeof PrimitiveTopology];

/** The name of the `PrimitiveTopology` member numbered `n`, or undefined for a number
 * no member has. */
export function primitive_topology_name(
  n: number,
): keyof typeof PrimitiveTopology | undefined {
  return (
    Object.keys(PrimitiveTopology) as (keyof typeof PrimitiveTopology)[]
  ).find((k) => PrimitiveTopology[k] === n);
}

/** The `PrimitiveTopology` member `n` is, or null for a number no member has. */
export function primitive_topology_of(n: number): PrimitiveTopology | null {
  return primitive_topology_name(n) === undefined
    ? null
    : (n as PrimitiveTopology);
}

export const INDEX_FORMAT = ["uint16", "uint32"] as const;

export const IndexFormat = {
  uint16: 0,
  uint32: 1,
} as const;
export type IndexFormat = (typeof IndexFormat)[keyof typeof IndexFormat];

/** The name of the `IndexFormat` member numbered `n`, or undefined for a number
 * no member has. */
export function index_format_name(
  n: number,
): keyof typeof IndexFormat | undefined {
  return (Object.keys(IndexFormat) as (keyof typeof IndexFormat)[]).find(
    (k) => IndexFormat[k] === n,
  );
}

/** The `IndexFormat` member `n` is, or null for a number no member has. */
export function index_format_of(n: number): IndexFormat | null {
  return index_format_name(n) === undefined ? null : (n as IndexFormat);
}

export const FRONT_FACE = ["ccw", "cw"] as const;

export const FrontFace = {
  ccw: 0,
  cw: 1,
} as const;
export type FrontFace = (typeof FrontFace)[keyof typeof FrontFace];

/** The name of the `FrontFace` member numbered `n`, or undefined for a number
 * no member has. */
export function front_face_name(n: number): keyof typeof FrontFace | undefined {
  return (Object.keys(FrontFace) as (keyof typeof FrontFace)[]).find(
    (k) => FrontFace[k] === n,
  );
}

/** The `FrontFace` member `n` is, or null for a number no member has. */
export function front_face_of(n: number): FrontFace | null {
  return front_face_name(n) === undefined ? null : (n as FrontFace);
}

export const CULL_MODE = ["none", "front", "back"] as const;

export const CullMode = {
  none: 0,
  front: 1,
  back: 2,
} as const;
export type CullMode = (typeof CullMode)[keyof typeof CullMode];

/** The name of the `CullMode` member numbered `n`, or undefined for a number
 * no member has. */
export function cull_mode_name(n: number): keyof typeof CullMode | undefined {
  return (Object.keys(CullMode) as (keyof typeof CullMode)[]).find(
    (k) => CullMode[k] === n,
  );
}

/** The `CullMode` member `n` is, or null for a number no member has. */
export function cull_mode_of(n: number): CullMode | null {
  return cull_mode_name(n) === undefined ? null : (n as CullMode);
}

export const COMPARE_FUNCTION = [
  "never",
  "less",
  "equal",
  "less-equal",
  "greater",
  "not-equal",
  "greater-equal",
  "always",
] as const;

export const CompareFunction = {
  never: 0,
  less: 1,
  equal: 2,
  "less-equal": 3,
  greater: 4,
  "not-equal": 5,
  "greater-equal": 6,
  always: 7,
} as const;
export type CompareFunction =
  (typeof CompareFunction)[keyof typeof CompareFunction];

/** The name of the `CompareFunction` member numbered `n`, or undefined for a number
 * no member has. */
export function compare_function_name(
  n: number,
): keyof typeof CompareFunction | undefined {
  return (
    Object.keys(CompareFunction) as (keyof typeof CompareFunction)[]
  ).find((k) => CompareFunction[k] === n);
}

/** The `CompareFunction` member `n` is, or null for a number no member has. */
export function compare_function_of(n: number): CompareFunction | null {
  return compare_function_name(n) === undefined ? null : (n as CompareFunction);
}

export const STENCIL_OPERATION = [
  "keep",
  "zero",
  "replace",
  "invert",
  "increment-clamp",
  "decrement-clamp",
  "increment-wrap",
  "decrement-wrap",
] as const;

export const StencilOperation = {
  keep: 0,
  zero: 1,
  replace: 2,
  invert: 3,
  "increment-clamp": 4,
  "decrement-clamp": 5,
  "increment-wrap": 6,
  "decrement-wrap": 7,
} as const;
export type StencilOperation =
  (typeof StencilOperation)[keyof typeof StencilOperation];

/** The name of the `StencilOperation` member numbered `n`, or undefined for a number
 * no member has. */
export function stencil_operation_name(
  n: number,
): keyof typeof StencilOperation | undefined {
  return (
    Object.keys(StencilOperation) as (keyof typeof StencilOperation)[]
  ).find((k) => StencilOperation[k] === n);
}

/** The `StencilOperation` member `n` is, or null for a number no member has. */
export function stencil_operation_of(n: number): StencilOperation | null {
  return stencil_operation_name(n) === undefined
    ? null
    : (n as StencilOperation);
}

export const BLEND_OPERATION = [
  "add",
  "subtract",
  "reverse-subtract",
  "min",
  "max",
] as const;

export const BlendOperation = {
  add: 0,
  subtract: 1,
  "reverse-subtract": 2,
  min: 3,
  max: 4,
} as const;
export type BlendOperation =
  (typeof BlendOperation)[keyof typeof BlendOperation];

/** The name of the `BlendOperation` member numbered `n`, or undefined for a number
 * no member has. */
export function blend_operation_name(
  n: number,
): keyof typeof BlendOperation | undefined {
  return (Object.keys(BlendOperation) as (keyof typeof BlendOperation)[]).find(
    (k) => BlendOperation[k] === n,
  );
}

/** The `BlendOperation` member `n` is, or null for a number no member has. */
export function blend_operation_of(n: number): BlendOperation | null {
  return blend_operation_name(n) === undefined ? null : (n as BlendOperation);
}

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

export const BlendFactor = {
  zero: 0,
  one: 1,
  src: 2,
  "one-minus-src": 3,
  "src-alpha": 4,
  "one-minus-src-alpha": 5,
  dst: 6,
  "one-minus-dst": 7,
  "dst-alpha": 8,
  "one-minus-dst-alpha": 9,
  "src-alpha-saturated": 10,
  constant: 11,
  "one-minus-constant": 12,
  src1: 13,
  "one-minus-src1": 14,
  "src1-alpha": 15,
  "one-minus-src1-alpha": 16,
} as const;
export type BlendFactor = (typeof BlendFactor)[keyof typeof BlendFactor];

/** The name of the `BlendFactor` member numbered `n`, or undefined for a number
 * no member has. */
export function blend_factor_name(
  n: number,
): keyof typeof BlendFactor | undefined {
  return (Object.keys(BlendFactor) as (keyof typeof BlendFactor)[]).find(
    (k) => BlendFactor[k] === n,
  );
}

/** The `BlendFactor` member `n` is, or null for a number no member has. */
export function blend_factor_of(n: number): BlendFactor | null {
  return blend_factor_name(n) === undefined ? null : (n as BlendFactor);
}

export const VERTEX_STEP_MODE = ["vertex", "instance"] as const;

export const VertexStepMode = {
  vertex: 0,
  instance: 1,
} as const;
export type VertexStepMode =
  (typeof VertexStepMode)[keyof typeof VertexStepMode];

/** The name of the `VertexStepMode` member numbered `n`, or undefined for a number
 * no member has. */
export function vertex_step_mode_name(
  n: number,
): keyof typeof VertexStepMode | undefined {
  return (Object.keys(VertexStepMode) as (keyof typeof VertexStepMode)[]).find(
    (k) => VertexStepMode[k] === n,
  );
}

/** The `VertexStepMode` member `n` is, or null for a number no member has. */
export function vertex_step_mode_of(n: number): VertexStepMode | null {
  return vertex_step_mode_name(n) === undefined ? null : (n as VertexStepMode);
}

export const ADDRESS_MODE = [
  "clamp-to-edge",
  "repeat",
  "mirror-repeat",
] as const;

export const AddressMode = {
  "clamp-to-edge": 0,
  repeat: 1,
  "mirror-repeat": 2,
} as const;
export type AddressMode = (typeof AddressMode)[keyof typeof AddressMode];

/** The name of the `AddressMode` member numbered `n`, or undefined for a number
 * no member has. */
export function address_mode_name(
  n: number,
): keyof typeof AddressMode | undefined {
  return (Object.keys(AddressMode) as (keyof typeof AddressMode)[]).find(
    (k) => AddressMode[k] === n,
  );
}

/** The `AddressMode` member `n` is, or null for a number no member has. */
export function address_mode_of(n: number): AddressMode | null {
  return address_mode_name(n) === undefined ? null : (n as AddressMode);
}

export const FILTER_MODE = ["nearest", "linear"] as const;

export const FilterMode = {
  nearest: 0,
  linear: 1,
} as const;
export type FilterMode = (typeof FilterMode)[keyof typeof FilterMode];

/** The name of the `FilterMode` member numbered `n`, or undefined for a number
 * no member has. */
export function filter_mode_name(
  n: number,
): keyof typeof FilterMode | undefined {
  return (Object.keys(FilterMode) as (keyof typeof FilterMode)[]).find(
    (k) => FilterMode[k] === n,
  );
}

/** The `FilterMode` member `n` is, or null for a number no member has. */
export function filter_mode_of(n: number): FilterMode | null {
  return filter_mode_name(n) === undefined ? null : (n as FilterMode);
}

export const MIPMAP_FILTER_MODE = ["nearest", "linear"] as const;

export const MipmapFilterMode = {
  nearest: 0,
  linear: 1,
} as const;
export type MipmapFilterMode =
  (typeof MipmapFilterMode)[keyof typeof MipmapFilterMode];

/** The name of the `MipmapFilterMode` member numbered `n`, or undefined for a number
 * no member has. */
export function mipmap_filter_mode_name(
  n: number,
): keyof typeof MipmapFilterMode | undefined {
  return (
    Object.keys(MipmapFilterMode) as (keyof typeof MipmapFilterMode)[]
  ).find((k) => MipmapFilterMode[k] === n);
}

/** The `MipmapFilterMode` member `n` is, or null for a number no member has. */
export function mipmap_filter_mode_of(n: number): MipmapFilterMode | null {
  return mipmap_filter_mode_name(n) === undefined
    ? null
    : (n as MipmapFilterMode);
}

export const TEXTURE_DIMENSION = ["1d", "2d", "3d"] as const;

export const TextureDimension = {
  "1d": 0,
  "2d": 1,
  "3d": 2,
} as const;
export type TextureDimension =
  (typeof TextureDimension)[keyof typeof TextureDimension];

/** The name of the `TextureDimension` member numbered `n`, or undefined for a number
 * no member has. */
export function texture_dimension_name(
  n: number,
): keyof typeof TextureDimension | undefined {
  return (
    Object.keys(TextureDimension) as (keyof typeof TextureDimension)[]
  ).find((k) => TextureDimension[k] === n);
}

/** The `TextureDimension` member `n` is, or null for a number no member has. */
export function texture_dimension_of(n: number): TextureDimension | null {
  return texture_dimension_name(n) === undefined
    ? null
    : (n as TextureDimension);
}

export const TEXTURE_VIEW_DIMENSION = [
  "1d",
  "2d",
  "2d-array",
  "cube",
  "cube-array",
  "3d",
] as const;

export const TextureViewDimension = {
  "1d": 0,
  "2d": 1,
  "2d-array": 2,
  cube: 3,
  "cube-array": 4,
  "3d": 5,
} as const;
export type TextureViewDimension =
  (typeof TextureViewDimension)[keyof typeof TextureViewDimension];

/** The name of the `TextureViewDimension` member numbered `n`, or undefined for a number
 * no member has. */
export function texture_view_dimension_name(
  n: number,
): keyof typeof TextureViewDimension | undefined {
  return (
    Object.keys(TextureViewDimension) as (keyof typeof TextureViewDimension)[]
  ).find((k) => TextureViewDimension[k] === n);
}

/** The `TextureViewDimension` member `n` is, or null for a number no member has. */
export function texture_view_dimension_of(
  n: number,
): TextureViewDimension | null {
  return texture_view_dimension_name(n) === undefined
    ? null
    : (n as TextureViewDimension);
}

export const TEXTURE_ASPECT = ["all", "stencil-only", "depth-only"] as const;

export const TextureAspect = {
  all: 0,
  "stencil-only": 1,
  "depth-only": 2,
} as const;
export type TextureAspect = (typeof TextureAspect)[keyof typeof TextureAspect];

/** The name of the `TextureAspect` member numbered `n`, or undefined for a number
 * no member has. */
export function texture_aspect_name(
  n: number,
): keyof typeof TextureAspect | undefined {
  return (Object.keys(TextureAspect) as (keyof typeof TextureAspect)[]).find(
    (k) => TextureAspect[k] === n,
  );
}

/** The `TextureAspect` member `n` is, or null for a number no member has. */
export function texture_aspect_of(n: number): TextureAspect | null {
  return texture_aspect_name(n) === undefined ? null : (n as TextureAspect);
}

export const BUFFER_BINDING_TYPE = [
  "uniform",
  "storage",
  "read-only-storage",
] as const;

export const BufferBindingType = {
  uniform: 0,
  storage: 1,
  "read-only-storage": 2,
} as const;
export type BufferBindingType =
  (typeof BufferBindingType)[keyof typeof BufferBindingType];

/** The name of the `BufferBindingType` member numbered `n`, or undefined for a number
 * no member has. */
export function buffer_binding_type_name(
  n: number,
): keyof typeof BufferBindingType | undefined {
  return (
    Object.keys(BufferBindingType) as (keyof typeof BufferBindingType)[]
  ).find((k) => BufferBindingType[k] === n);
}

/** The `BufferBindingType` member `n` is, or null for a number no member has. */
export function buffer_binding_type_of(n: number): BufferBindingType | null {
  return buffer_binding_type_name(n) === undefined
    ? null
    : (n as BufferBindingType);
}

export const SAMPLER_BINDING_TYPE = [
  "filtering",
  "non-filtering",
  "comparison",
] as const;

export const SamplerBindingType = {
  filtering: 0,
  "non-filtering": 1,
  comparison: 2,
} as const;
export type SamplerBindingType =
  (typeof SamplerBindingType)[keyof typeof SamplerBindingType];

/** The name of the `SamplerBindingType` member numbered `n`, or undefined for a number
 * no member has. */
export function sampler_binding_type_name(
  n: number,
): keyof typeof SamplerBindingType | undefined {
  return (
    Object.keys(SamplerBindingType) as (keyof typeof SamplerBindingType)[]
  ).find((k) => SamplerBindingType[k] === n);
}

/** The `SamplerBindingType` member `n` is, or null for a number no member has. */
export function sampler_binding_type_of(n: number): SamplerBindingType | null {
  return sampler_binding_type_name(n) === undefined
    ? null
    : (n as SamplerBindingType);
}

export const TEXTURE_SAMPLE_TYPE = [
  "float",
  "unfilterable-float",
  "depth",
  "sint",
  "uint",
] as const;

export const TextureSampleType = {
  float: 0,
  "unfilterable-float": 1,
  depth: 2,
  sint: 3,
  uint: 4,
} as const;
export type TextureSampleType =
  (typeof TextureSampleType)[keyof typeof TextureSampleType];

/** The name of the `TextureSampleType` member numbered `n`, or undefined for a number
 * no member has. */
export function texture_sample_type_name(
  n: number,
): keyof typeof TextureSampleType | undefined {
  return (
    Object.keys(TextureSampleType) as (keyof typeof TextureSampleType)[]
  ).find((k) => TextureSampleType[k] === n);
}

/** The `TextureSampleType` member `n` is, or null for a number no member has. */
export function texture_sample_type_of(n: number): TextureSampleType | null {
  return texture_sample_type_name(n) === undefined
    ? null
    : (n as TextureSampleType);
}

export const STORAGE_TEXTURE_ACCESS = [
  "write-only",
  "read-only",
  "read-write",
] as const;

export const StorageTextureAccess = {
  "write-only": 0,
  "read-only": 1,
  "read-write": 2,
} as const;
export type StorageTextureAccess =
  (typeof StorageTextureAccess)[keyof typeof StorageTextureAccess];

/** The name of the `StorageTextureAccess` member numbered `n`, or undefined for a number
 * no member has. */
export function storage_texture_access_name(
  n: number,
): keyof typeof StorageTextureAccess | undefined {
  return (
    Object.keys(StorageTextureAccess) as (keyof typeof StorageTextureAccess)[]
  ).find((k) => StorageTextureAccess[k] === n);
}

/** The `StorageTextureAccess` member `n` is, or null for a number no member has. */
export function storage_texture_access_of(
  n: number,
): StorageTextureAccess | null {
  return storage_texture_access_name(n) === undefined
    ? null
    : (n as StorageTextureAccess);
}

export const LOAD_OP = ["load", "clear"] as const;

export const LoadOp = {
  load: 0,
  clear: 1,
} as const;
export type LoadOp = (typeof LoadOp)[keyof typeof LoadOp];

/** The name of the `LoadOp` member numbered `n`, or undefined for a number
 * no member has. */
export function load_op_name(n: number): keyof typeof LoadOp | undefined {
  return (Object.keys(LoadOp) as (keyof typeof LoadOp)[]).find(
    (k) => LoadOp[k] === n,
  );
}

/** The `LoadOp` member `n` is, or null for a number no member has. */
export function load_op_of(n: number): LoadOp | null {
  return load_op_name(n) === undefined ? null : (n as LoadOp);
}

export const STORE_OP = ["store", "discard"] as const;

export const StoreOp = {
  store: 0,
  discard: 1,
} as const;
export type StoreOp = (typeof StoreOp)[keyof typeof StoreOp];

/** The name of the `StoreOp` member numbered `n`, or undefined for a number
 * no member has. */
export function store_op_name(n: number): keyof typeof StoreOp | undefined {
  return (Object.keys(StoreOp) as (keyof typeof StoreOp)[]).find(
    (k) => StoreOp[k] === n,
  );
}

/** The `StoreOp` member `n` is, or null for a number no member has. */
export function store_op_of(n: number): StoreOp | null {
  return store_op_name(n) === undefined ? null : (n as StoreOp);
}

export const QUERY_TYPE = ["occlusion", "timestamp"] as const;

export const QueryType = {
  occlusion: 0,
  timestamp: 1,
} as const;
export type QueryType = (typeof QueryType)[keyof typeof QueryType];

/** The name of the `QueryType` member numbered `n`, or undefined for a number
 * no member has. */
export function query_type_name(n: number): keyof typeof QueryType | undefined {
  return (Object.keys(QueryType) as (keyof typeof QueryType)[]).find(
    (k) => QueryType[k] === n,
  );
}

/** The `QueryType` member `n` is, or null for a number no member has. */
export function query_type_of(n: number): QueryType | null {
  return query_type_name(n) === undefined ? null : (n as QueryType);
}

export const CANVAS_ALPHA_MODE = ["opaque", "premultiplied"] as const;

export const CanvasAlphaMode = {
  opaque: 0,
  premultiplied: 1,
} as const;
export type CanvasAlphaMode =
  (typeof CanvasAlphaMode)[keyof typeof CanvasAlphaMode];

/** The name of the `CanvasAlphaMode` member numbered `n`, or undefined for a number
 * no member has. */
export function canvas_alpha_mode_name(
  n: number,
): keyof typeof CanvasAlphaMode | undefined {
  return (
    Object.keys(CanvasAlphaMode) as (keyof typeof CanvasAlphaMode)[]
  ).find((k) => CanvasAlphaMode[k] === n);
}

/** The `CanvasAlphaMode` member `n` is, or null for a number no member has. */
export function canvas_alpha_mode_of(n: number): CanvasAlphaMode | null {
  return canvas_alpha_mode_name(n) === undefined
    ? null
    : (n as CanvasAlphaMode);
}

/** GPUFeatureName. The DEVICE record (api.json) reports a device's features as one bit each,
 * at the member's place. */
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

export const FeatureName = {
  "depth-clip-control": 0,
  "depth32float-stencil8": 1,
  "texture-compression-bc": 2,
  "texture-compression-bc-sliced-3d": 3,
  "texture-compression-etc2": 4,
  "texture-compression-astc": 5,
  "texture-compression-astc-sliced-3d": 6,
  "timestamp-query": 7,
  "indirect-first-instance": 8,
  "shader-f16": 9,
  "rg11b10ufloat-renderable": 10,
  "bgra8unorm-storage": 11,
  "float32-filterable": 12,
  "float32-blendable": 13,
  "clip-distances": 14,
  "dual-source-blending": 15,
  subgroups: 16,
} as const;
export type FeatureName = (typeof FeatureName)[keyof typeof FeatureName];

/** The name of the `FeatureName` member numbered `n`, or undefined for a number
 * no member has. */
export function feature_name_name(
  n: number,
): keyof typeof FeatureName | undefined {
  return (Object.keys(FeatureName) as (keyof typeof FeatureName)[]).find(
    (k) => FeatureName[k] === n,
  );
}

/** The `FeatureName` member `n` is, or null for a number no member has. */
export function feature_name_of(n: number): FeatureName | null {
  return feature_name_name(n) === undefined ? null : (n as FeatureName);
}

/** The members of GPUSupportedLimits. The DEVICE record (api.json) reports a device's limits
 * as one word each, at the member's place. */
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

export const LimitName = {
  maxTextureDimension1D: 0,
  maxTextureDimension2D: 1,
  maxTextureDimension3D: 2,
  maxTextureArrayLayers: 3,
  maxBindGroups: 4,
  maxBindGroupsPlusVertexBuffers: 5,
  maxBindingsPerBindGroup: 6,
  maxDynamicUniformBuffersPerPipelineLayout: 7,
  maxDynamicStorageBuffersPerPipelineLayout: 8,
  maxSampledTexturesPerShaderStage: 9,
  maxSamplersPerShaderStage: 10,
  maxStorageBuffersPerShaderStage: 11,
  maxStorageTexturesPerShaderStage: 12,
  maxUniformBuffersPerShaderStage: 13,
  maxUniformBufferBindingSize: 14,
  maxStorageBufferBindingSize: 15,
  minUniformBufferOffsetAlignment: 16,
  minStorageBufferOffsetAlignment: 17,
  maxVertexBuffers: 18,
  maxBufferSize: 19,
  maxVertexAttributes: 20,
  maxVertexBufferArrayStride: 21,
  maxInterStageShaderVariables: 22,
  maxColorAttachments: 23,
  maxColorAttachmentBytesPerSample: 24,
  maxComputeWorkgroupStorageSize: 25,
  maxComputeInvocationsPerWorkgroup: 26,
  maxComputeWorkgroupSizeX: 27,
  maxComputeWorkgroupSizeY: 28,
  maxComputeWorkgroupSizeZ: 29,
  maxComputeWorkgroupsPerDimension: 30,
} as const;
export type LimitName = (typeof LimitName)[keyof typeof LimitName];

/** The name of the `LimitName` member numbered `n`, or undefined for a number
 * no member has. */
export function limit_name_name(n: number): keyof typeof LimitName | undefined {
  return (Object.keys(LimitName) as (keyof typeof LimitName)[]).find(
    (k) => LimitName[k] === n,
  );
}

/** The `LimitName` member `n` is, or null for a number no member has. */
export function limit_name_of(n: number): LimitName | null {
  return limit_name_name(n) === undefined ? null : (n as LimitName);
}
