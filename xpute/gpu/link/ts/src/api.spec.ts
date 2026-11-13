// @xpute/gpu-link/api.spec.ts
//
// GENERATED from gpu/link/spec/api.json (xpute-gen data.rs)
//
// The calls a guest makes to a GPU it cannot reach, written as records of a stream
// (xpute-gpu-guest stream.rs). The vocabulary is WebGPU's, with its descriptors written out as words, and
// it serves any program. A record is an op, a count, and the op's words in the order the
// op's doc lists them. Bytes go as a count followed by padded words, as the stream lays
// them. Objects are named by the guest's handles (abi/handle). An optional field holds NONE
// when it is unset. Render and compute ops sit side by side.
//
// A transaction (xpute-gpu-guest stream.rs) that does not commit drops its records and gives back the
// handles they took, so the host never hears of those objects. The encoder's begin and
// commit open and close the transaction on both the stream and the handle table.

/** The value of an optional field that is unset, such as an absent index or handle. */
export const NONE = 4294967295 as number;

/** The low bits of a handle, which hold its slot; the high 16 hold the slot's generation.
 * Both sides split a handle the same way. */
export const SLOT_BITS = 16 as number;

/** A value for CONFIGURE's format word that stands for the format the host's surface
 * prefers. */
export const SURFACE_PREFERRED = 255 as number;

/** The length in words of one entry of a bind group layout (BIND_GROUP_LAYOUT). */
export const BINDING_ENTRY_WORDS = 6 as number;

/** The length in words of one entry of a bind group (BIND_GROUP). */
export const RESOURCE_ENTRY_WORDS = 5 as number;

/** The ops, numbered from 1; an op of 0 ends a stream. */
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
  /** id, which may name any object an op above made. */
  DESTROY: 12 as number,
  /** buffer, offset, the bytes. */
  WRITE_BUFFER: 13 as number,
  /** buffer, offset, address, bytes. The host reads the bytes where they lie in the guest's
   * memory when it runs the record, and makes no copy of them first. From the stream's
   * flush until `gpu.run` answers, the guest neither rewrites nor frees them (stream.rs). */
  WRITE_BUFFER_AT: 14 as number,
  /** texture, mip, origin x, y, z, aspect, bytes per row, rows per image, width, height, depth, the bytes. */
  WRITE_TEXTURE: 15 as number,
  /** The words of WRITE_TEXTURE, except that the bytes are given as an address and a count in
   * the guest's memory, which the host reads as it reads WRITE_BUFFER_AT's. */
  WRITE_TEXTURE_AT: 16 as number,
  /** texture, mip, origin x, y, z, aspect (TEXTURE_ASPECT), flip y (0 or 1), premultiplied (0
   * or 1), address, bytes, word, ticket. The bytes are an encoded image (PNG, JPEG, WebP),
   * which the host decodes into the texture. The word and the one after it are the
   * request's fence and result (RESULT): OK once the image is in the texture, or REFUSED
   * when the host cannot decode the image. A request whose texture has been destroyed
   * gets no answer. */
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
  /** pass, pipeline; the pipeline is a render or a compute pipeline, as the pass is. */
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
  /** pass. The record ends the pass and lets go of it, so no DESTROY follows. */
  END: 41 as number,
  /** encoders' count, that many encoders. The record finishes each encoder and lets go of it,
   * so no DESTROY follows. */
  SUBMIT: 42 as number,
  /** buffer, offset, size, address, word, ticket. WebGPU's mapAsync for reading, on a buffer
   * the guest made with MAP_READ | COPY_DST and copied into on an encoder it submitted
   * earlier. Once the buffer is mapped, the host copies the range to the address and
   * unmaps the buffer. The word and the one after it are the request's fence and result
   * (RESULT): OK, or REFUSED when the device refused the mapping and nothing was copied. */
  MAP_ASYNC: 43 as number,
  /** word, ticket. The word is the request's fence (RESULT), answered once the work submitted
   * before this record is done. */
  QUEUE_DONE: 44 as number,
  /** surface, format (TEXTURE_FORMAT, or SURFACE_PREFERRED), usage (TEXTURE_USAGE), alpha mode (CANVAS_ALPHA_MODE). */
  CONFIGURE: 45 as number,
  /** surface, width, height. */
  RESIZE: 46 as number,
  /** id, surface. The id names a view of the surface's texture for this entry. */
  CURRENT: 47 as number,
  /** address, words. The host writes the device block there: the device's limits
   * (LIMIT_NAME, one word each), then its features (FEATURE_NAME, one bit each, packed in
   * words), then its timestamp period (f32 bits: how many nanoseconds one unit of a resolved
   * timestamp is, 1 when the units are nanoseconds as WebGPU's are), then the surface format
   * it prefers (TEXTURE_FORMAT: the format SURFACE_PREFERRED stands for, as the web's
   * getPreferredCanvasFormat gives it), then the count of uncaptured errors, then the count
   * of unfinished frames, then STATUS. The uncaptured errors are the errors the device raised
   * that no error scope caught, each logged where the host logs. The unfinished frames are
   * the frames the guest has drawn whose work the queue has not yet done. Only the host
   * writes the two counts: first before STATUS, as it writes the rest of the block, and
   * afterwards with release each time a count changes. The guest loads them with acquire once
   * STATUS reads READY, and draws a frame only while the unfinished frames are below its
   * frame latency; both hosts hold a frame to that rule (xpute-gpu-link `admits`). */
  DEVICE: 48 as number,
  /** RENDER_PIPELINE's words, then word, ticket. WebGPU's createRenderPipelineAsync; the word
   * is the request's fence (RESULT), answered once the pipeline is made, and until then the
   * guest names the pipeline in no other record. When the device refuses the pipeline, the
   * host makes it as RENDER_PIPELINE would, reports the error as that op would, and still
   * writes the word. A host with no asynchronous call makes the pipeline at once and
   * writes the word the same way, before the next entry reads it. */
  RENDER_PIPELINE_ASYNC: 49 as number,
  /** COMPUTE_PIPELINE's words, then word, ticket. WebGPU's createComputePipelineAsync, whose
   * word is answered as RENDER_PIPELINE_ASYNC's is. */
  COMPUTE_PIPELINE_ASYNC: 50 as number,
  /** id, the label as bytes. The label is the name the device gives the object in its
   * messages, as WebGPU's label is. The id may name any object, encoder or pass that an
   * earlier record made. A host that cannot name an object after it is made ignores the
   * record. */
  LABEL: 51 as number,
} as const;

export const BINDING_KIND = [
  "buffer",
  "sampler",
  "texture",
  "storage-texture",
] as const;

export const BindingKind = {
  buffer: 0,
  sampler: 1,
  texture: 2,
  "storage-texture": 3,
} as const;
export type BindingKind = (typeof BindingKind)[keyof typeof BindingKind];

/** The name of the `BindingKind` member numbered `n`, or undefined for a number
 * no member has. */
export function binding_kind_name(
  n: number,
): keyof typeof BindingKind | undefined {
  return (Object.keys(BindingKind) as (keyof typeof BindingKind)[]).find(
    (k) => BindingKind[k] === n,
  );
}

/** The `BindingKind` member `n` is, or null for a number no member has. */
export function binding_kind_of(n: number): BindingKind | null {
  return binding_kind_name(n) === undefined ? null : (n as BindingKind);
}

export const RESOURCE_KIND = ["buffer", "sampler", "texture-view"] as const;

export const ResourceKind = {
  buffer: 0,
  sampler: 1,
  "texture-view": 2,
} as const;
export type ResourceKind = (typeof ResourceKind)[keyof typeof ResourceKind];

/** The name of the `ResourceKind` member numbered `n`, or undefined for a number
 * no member has. */
export function resource_kind_name(
  n: number,
): keyof typeof ResourceKind | undefined {
  return (Object.keys(ResourceKind) as (keyof typeof ResourceKind)[]).find(
    (k) => ResourceKind[k] === n,
  );
}

/** The `ResourceKind` member `n` is, or null for a number no member has. */
export function resource_kind_of(n: number): ResourceKind | null {
  return resource_kind_name(n) === undefined ? null : (n as ResourceKind);
}

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

/** The words of the device block, in order (op DEVICE): one a limit (LIMIT_NAME), the words
 * that hold a bit a feature (FEATURE_NAME), the timestamp period, the surface format, the
 * uncaptured errors, the unfinished frames and STATUS, the last, which the host stores
 * with release after the rest. */
export const DEVICE = {
  LIMITS_AT: 0 as number,
  FEATS_AT: 31 as number,
  PERIOD_AT: 32 as number,
  FORMAT_AT: 33 as number,
  ERRORS_AT: 34 as number,
  UNFINISHED_AT: 35 as number,
  STATUS_AT: 36 as number,
  WORDS: 37 as number,
} as const;

/** The last word of the DEVICE block: what the host knows of the device. Only the
 * host writes it, and it stores it with release after the rest of the block. The guest
 * loads it with acquire before it reads the block. */
export const STATUS = {
  READY: 1 as number,
  LOST: 2 as number,
} as const;

/** How a completion is answered. A completion (COPY_EXTERNAL_IMAGE, MAP_ASYNC, QUEUE_DONE,
 * RENDER_PIPELINE_ASYNC, COMPUTE_PIPELINE_ASYNC) names a fence word and a ticket. Only the
 * guest picks tickets: for each fence word it advances a sequence, which wraps, and it
 * keeps one request out on that word at a time. Only the host writes the fence. Once
 * what the request guards is written, and after the result in the next word where the
 * request can fail (COPY_EXTERNAL_IMAGE, MAP_ASYNC), the host stores the ticket it
 * finished into the fence with release. The guest loads the fence with acquire and holds
 * the request done when the fence equals the ticket the guest asked with, a test for
 * equality that the sequence's wrap does not break. Only then does the guest read the
 * result and what the request guards. A late answer to a request that was dropped or
 * superseded carries an older ticket and never equals the current one. A lost device
 * answers none of its requests. */
export const RESULT = {
  OK: 1 as number,
  REFUSED: 2 as number,
} as const;
