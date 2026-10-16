// @xpute/gpu/api.spec.ts
//
// GENERATED from spec/gpu/api.json — do not edit.
//
// The calls a guest makes to a GPU it cannot reach, as records of a stream (ipc/stream):
// WebGPU's vocabulary, its descriptors written out as words, for any program. A record is
// an op, a count and the op's words in the order each op's doc gives; bytes go as a count
// and padded words, as the stream lays them. Objects are the guest's handles (abi/handle).
// An optional field holds NONE where unset. Render and compute sit side by side.
// A transaction (ipc/stream) that does not commit drops its records and gives back the handles they took, so the host never hears of the objects: the encoder's begin and commit open and close both.

import type { u32 } from "@xpute/kit/abi/word.ts";

/** An optional field left unset, an index or a handle absent. */
export const NONE: u32 = 4294967295 as number;

/** Or-ed into a ticket where a completion word is written (COPY_EXTERNAL_IMAGE, READ_BUFFER, QUEUE_DONE). */
export const DONE: u32 = 2147483648 as number;

/** In CONFIGURE's format: the format the host's surface prefers. */
export const SURFACE_PREFERRED: u32 = 255 as number;

/** A bind group layout's entry (BIND_GROUP_LAYOUT). */
export const BINDING_ENTRY_WORDS: u32 = 6 as number;

/** A bind group's entry (BIND_GROUP). */
export const RESOURCE_ENTRY_WORDS: u32 = 5 as number;

/** The ops, numbered from 1: 0 ends a stream. */
export const op = {
  /** id, size, usage (BUFFER_USAGE), mapped at creation (0 or 1). */
  BUFFER: 1 as number,
  /** id, width, height, depth or layers, mip levels, samples, dimension (TEXTURE_DIMENSION), format (TEXTURE_FORMAT), usage (TEXTURE_USAGE), view formats' count, that many formats. */
  TEXTURE: 2 as number,
  /** id, texture, format or NONE, dimension (TEXTURE_VIEW_DIMENSION) or NONE, aspect (TEXTURE_ASPECT), base mip, mips or NONE, base layer, layers or NONE. */
  VIEW: 3 as number,
  /** id, address u, v, w (ADDRESS_MODE), mag and min (FILTER_MODE), mipmap (MIPMAP_FILTER_MODE), lod min and max (f32 bits), compare (COMPARE_FUNCTION) or NONE, max anisotropy. */
  SAMPLER: 4 as number,
  /** id, the WGSL source as bytes. */
  SHADER_MODULE: 5 as number,
  /** id, entries' count, then each entry's BINDING_ENTRY_WORDS: binding, visibility (SHADER_STAGE), kind (BINDING_KIND) and three words the kind reads — a buffer its type (BUFFER_BINDING_TYPE), dynamic offset (0 or 1), min binding size; a sampler its type (SAMPLER_BINDING_TYPE); a texture its sample type (TEXTURE_SAMPLE_TYPE), view dimension, multisampled (0 or 1); a storage texture its access (STORAGE_TEXTURE_ACCESS), format, view dimension. */
  BIND_GROUP_LAYOUT: 6 as number,
  /** id, bind group layouts' count, that many ids. */
  PIPELINE_LAYOUT: 7 as number,
  /** id, layout, entries' count, then each entry's RESOURCE_ENTRY_WORDS: binding, kind (RESOURCE_KIND), the object's id, offset, size or NONE (a buffer's; 0 for the others). */
  BIND_GROUP: 8 as number,
  /** id, layout or NONE for an automatic one; vertex: module, entry (bytes), buffers' count, each: stride, step (VERTEX_STEP_MODE), attributes' count, each: format (VERTEX_FORMAT), offset, location; primitive: topology (PRIMITIVE_TOPOLOGY), strip index format (INDEX_FORMAT) or NONE, front face (FRONT_FACE), cull (CULL_MODE), unclipped depth (0 or 1); depth-stencil: format or NONE for none, the rest present either way: depth write (0 or 1), compare, stencil front and back each compare, fail, depth fail and pass (STENCIL_OPERATION), read mask, write mask, bias (i32), slope scale and clamp (f32 bits); multisample: count, mask, alpha to coverage (0 or 1); fragment: module or NONE for none, the rest present either way (no bytes, no targets): entry (bytes), targets' count, each: format, blend color op, src, dst and alpha op, src, dst (BLEND_OPERATION, BLEND_FACTOR) or NONE in the color op for no blending and the five after it present either way, write mask (COLOR_WRITE). */
  RENDER_PIPELINE: 9 as number,
  /** id, layout or NONE, module, entry (bytes). */
  COMPUTE_PIPELINE: 10 as number,
  /** id, type (QUERY_TYPE), count. */
  QUERY_SET: 11 as number,
  /** id: any object, made by any op above. */
  DESTROY: 12 as number,
  /** buffer, offset, the bytes. */
  WRITE_BUFFER: 13 as number,
  /** buffer, offset, address, bytes: read from the guest's memory when the host runs it. */
  WRITE_BUFFER_AT: 14 as number,
  /** texture, mip, origin x, y, z, aspect, bytes per row, rows per image, width, height, depth, the bytes. */
  WRITE_TEXTURE: 15 as number,
  /** as WRITE_TEXTURE, the bytes an address and a count in the guest's memory. */
  WRITE_TEXTURE_AT: 16 as number,
  /** texture, mip, origin x, y, z, aspect (TEXTURE_ASPECT), flip y (0 or 1), premultiplied (0 or 1), address, bytes, word, ticket: an encoded image (PNG, JPEG, WebP) the host decodes into the texture, and once it is there writes ticket | DONE at the word, where the ticket still stands; an image the host cannot decode is left out and its word not written. */
  COPY_EXTERNAL_IMAGE: 17 as number,
  /** id. */
  ENCODER: 18 as number,
  /** encoder, source, its offset, destination, its offset, size. */
  COPY_BUFFER_TO_BUFFER: 19 as number,
  /** encoder, buffer, offset, bytes per row, rows per image, texture, mip, origin x, y, z, aspect, width, height, depth. */
  COPY_BUFFER_TO_TEXTURE: 20 as number,
  /** encoder, texture, mip, origin x, y, z, aspect, buffer, offset, bytes per row, rows per image, width, height, depth. */
  COPY_TEXTURE_TO_BUFFER: 21 as number,
  /** encoder, source, mip, origin x, y, z, aspect, destination, mip, origin x, y, z, aspect, width, height, depth. */
  COPY_TEXTURE_TO_TEXTURE: 22 as number,
  /** encoder, buffer, offset, size or NONE. */
  CLEAR_BUFFER: 23 as number,
  /** encoder, query set, first, count, buffer, offset. */
  RESOLVE_QUERY_SET: 24 as number,
  /** id, encoder, color attachments' count, each: view, resolve view or NONE, load (LOAD_OP), store (STORE_OP), clear r, g, b, a (f32 bits); depth-stencil: view or NONE for none, the rest present either way: depth load, store, clear (f32 bits), read only (0 or 1), stencil load, store, clear, read only; timestamps: query set or NONE, begin index or NONE, end index or NONE; occlusion query set or NONE. */
  RENDER_PASS: 25 as number,
  /** id, encoder; timestamps: query set or NONE, begin index or NONE, end index or NONE. */
  COMPUTE_PASS: 26 as number,
  /** pass, pipeline: render or compute, as the pass is. */
  SET_PIPELINE: 27 as number,
  /** pass, index, group, dynamic offsets' count, that many offsets. */
  SET_BIND_GROUP: 28 as number,
  /** pass, slot, buffer, offset, size or NONE. */
  SET_VERTEX_BUFFER: 29 as number,
  /** pass, buffer, format (INDEX_FORMAT), offset, size or NONE. */
  SET_INDEX_BUFFER: 30 as number,
  /** pass, x, y, width, height, min depth, max depth (f32 bits). */
  SET_VIEWPORT: 31 as number,
  /** pass, x, y, width, height. */
  SET_SCISSOR: 32 as number,
  /** pass, r, g, b, a (f32 bits). */
  SET_BLEND_CONSTANT: 33 as number,
  /** pass, reference. */
  SET_STENCIL_REFERENCE: 34 as number,
  /** pass, vertices, instances, first vertex, first instance. */
  DRAW: 35 as number,
  /** pass, indices, instances, first index, base vertex (i32), first instance. */
  DRAW_INDEXED: 36 as number,
  /** pass, buffer, offset. */
  DRAW_INDIRECT: 37 as number,
  /** pass, buffer, offset. */
  DRAW_INDEXED_INDIRECT: 38 as number,
  /** pass, workgroups x, y, z. */
  DISPATCH: 39 as number,
  /** pass, buffer, offset. */
  DISPATCH_INDIRECT: 40 as number,
  /** pass: ended, and let go with the record. */
  END: 41 as number,
  /** encoders' count, that many encoders, each finished and let go with the record. */
  SUBMIT: 42 as number,
  /** buffer, offset, size, address, word, ticket: once the bytes are copied to the address the host writes ticket | DONE at the word, where the ticket still stands. */
  READ_BUFFER: 43 as number,
  /** word, ticket: ticket | DONE at the word once what was submitted before it is done, where the ticket still stands. */
  QUEUE_DONE: 44 as number,
  /** surface, format (TEXTURE_FORMAT, or SURFACE_PREFERRED), usage (TEXTURE_USAGE), alpha mode (CANVAS_ALPHA_MODE). */
  CONFIGURE: 45 as number,
  /** surface, width, height. */
  RESIZE: 46 as number,
  /** id, surface: the surface's texture for this turn, as a view. */
  CURRENT: 47 as number,
  /** address, words: the host writes there the device's limits (LIMIT_NAME, a word each), then its features (FEATURE_NAME, a bit each, in words), then its timestamp period (f32 bits: the nanoseconds a resolved timestamp's unit is, 1 where they are nanoseconds as WebGPU's are), then the surface format it prefers (TEXTURE_FORMAT: what SURFACE_PREFERRED names, as the web's getPreferredCanvasFormat), then STATUS. */
  DEVICE: 48 as number,
} as const;

export const BINDING_KIND = ["buffer", "sampler", "texture", "storage-texture"] as const;

export const RESOURCE_KIND = ["buffer", "sampler", "texture-view"] as const;

/** GPUBufferUsage, its bits as WebGPU numbers them. */
export const BUFFER_USAGE = {
  MAP_READ: 1 as number,
  MAP_WRITE: 2 as number,
  COPY_SRC: 4 as number,
  COPY_DST: 8 as number,
  INDEX: 16 as number,
  VERTEX: 32 as number,
  UNIFORM: 64 as number,
  STORAGE: 128 as number,
  INDIRECT: 256 as number,
  QUERY_RESOLVE: 512 as number,
} as const;

/** GPUTextureUsage, its bits as WebGPU numbers them. */
export const TEXTURE_USAGE = {
  COPY_SRC: 1 as number,
  COPY_DST: 2 as number,
  TEXTURE_BINDING: 4 as number,
  STORAGE_BINDING: 8 as number,
  RENDER_ATTACHMENT: 16 as number,
} as const;

/** GPUShaderStage, its bits as WebGPU numbers them. */
export const SHADER_STAGE = {
  VERTEX: 1 as number,
  FRAGMENT: 2 as number,
  COMPUTE: 4 as number,
} as const;

/** GPUColorWrite, its bits as WebGPU numbers them. */
export const COLOR_WRITE = {
  RED: 1 as number,
  GREEN: 2 as number,
  BLUE: 4 as number,
  ALPHA: 8 as number,
  ALL: 15 as number,
} as const;

/** DEVICE's last word: what the host knows of the device. */
export const STATUS = {
  READY: 1 as number,
  LOST: 2 as number,
} as const;
