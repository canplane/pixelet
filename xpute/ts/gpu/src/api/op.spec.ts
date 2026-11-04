// @xpute/gpu/api/op.spec.ts
//
// GENERATED from spec/gpu/api.json

/** id, size, usage (BUFFER_USAGE), mapped at creation (0 or 1). */
export const BUFFER = 1 as number;

/** id, width, height, depth or layers, mip levels, samples, dimension (TEXTURE_DIMENSION), format (TEXTURE_FORMAT), usage (TEXTURE_USAGE), view formats' count, that many formats. */
export const TEXTURE = 2 as number;

/** id, texture, format or NONE, dimension (TEXTURE_VIEW_DIMENSION) or NONE, aspect (TEXTURE_ASPECT), base mip, mips or NONE, base layer, layers or NONE. */
export const VIEW = 3 as number;

/** id, address u, v, w (ADDRESS_MODE), mag and min (FILTER_MODE), mipmap (MIPMAP_FILTER_MODE), lod min and max (f32 bits), compare (COMPARE_FUNCTION) or NONE, max anisotropy. */
export const SAMPLER = 4 as number;

/** id, the WGSL source as bytes. */
export const SHADER_MODULE = 5 as number;

/** id, entries' count, then each entry's BINDING_ENTRY_WORDS: binding, visibility (SHADER_STAGE), kind (BINDING_KIND) and three words the kind reads — a buffer its type (BUFFER_BINDING_TYPE), dynamic offset (0 or 1), min binding size; a sampler its type (SAMPLER_BINDING_TYPE); a texture its sample type (TEXTURE_SAMPLE_TYPE), view dimension, multisampled (0 or 1); a storage texture its access (STORAGE_TEXTURE_ACCESS), format, view dimension. */
export const BIND_GROUP_LAYOUT = 6 as number;

/** id, bind group layouts' count, that many ids. */
export const PIPELINE_LAYOUT = 7 as number;

/** id, layout, entries' count, then each entry's RESOURCE_ENTRY_WORDS: binding, kind (RESOURCE_KIND), the object's id, offset, size or NONE (a buffer's; 0 for the others). */
export const BIND_GROUP = 8 as number;

/** id, layout or NONE for an automatic one; vertex: module, entry (bytes), buffers' count, each: stride, step (VERTEX_STEP_MODE), attributes' count, each: format (VERTEX_FORMAT), offset, location; primitive: topology (PRIMITIVE_TOPOLOGY), strip index format (INDEX_FORMAT) or NONE, front face (FRONT_FACE), cull (CULL_MODE), unclipped depth (0 or 1); depth-stencil: format or NONE for none, the rest present either way: depth write (0 or 1), compare, stencil front and back each compare, fail, depth fail and pass (STENCIL_OPERATION), read mask, write mask, bias (i32), slope scale and clamp (f32 bits); multisample: count, mask, alpha to coverage (0 or 1); fragment: module or NONE for none, the rest present either way (no bytes, no targets): entry (bytes), targets' count, each: format, blend color op, src, dst and alpha op, src, dst (BLEND_OPERATION, BLEND_FACTOR) or NONE in the color op for no blending and the five after it present either way, write mask (COLOR_WRITE). */
export const RENDER_PIPELINE = 9 as number;

/** id, layout or NONE, module, entry (bytes). */
export const COMPUTE_PIPELINE = 10 as number;

/** id, type (QUERY_TYPE), count. */
export const QUERY_SET = 11 as number;

/** id: any object, made by any op above. */
export const DESTROY = 12 as number;

/** buffer, offset, the bytes. */
export const WRITE_BUFFER = 13 as number;

/** buffer, offset, address, bytes: read from the guest's memory when the host runs it. */
export const WRITE_BUFFER_AT = 14 as number;

/** texture, mip, origin x, y, z, aspect, bytes per row, rows per image, width, height, depth, the bytes. */
export const WRITE_TEXTURE = 15 as number;

/** as WRITE_TEXTURE, the bytes an address and a count in the guest's memory. */
export const WRITE_TEXTURE_AT = 16 as number;

/** texture, mip, origin x, y, z, aspect (TEXTURE_ASPECT), flip y (0 or 1), premultiplied (0 or 1), address, bytes, word, ticket: an encoded image (PNG, JPEG, WebP) the host decodes into the texture, and once it is there writes ticket | DONE at the word, where the ticket still stands; an image the host cannot decode is left out and its word not written. */
export const COPY_EXTERNAL_IMAGE = 17 as number;

/** id. */
export const ENCODER = 18 as number;

/** encoder, source, its offset, destination, its offset, size. */
export const COPY_BUFFER_TO_BUFFER = 19 as number;

/** encoder, buffer, offset, bytes per row, rows per image, texture, mip, origin x, y, z, aspect, width, height, depth. */
export const COPY_BUFFER_TO_TEXTURE = 20 as number;

/** encoder, texture, mip, origin x, y, z, aspect, buffer, offset, bytes per row, rows per image, width, height, depth. */
export const COPY_TEXTURE_TO_BUFFER = 21 as number;

/** encoder, source, mip, origin x, y, z, aspect, destination, mip, origin x, y, z, aspect, width, height, depth. */
export const COPY_TEXTURE_TO_TEXTURE = 22 as number;

/** encoder, buffer, offset, size or NONE. */
export const CLEAR_BUFFER = 23 as number;

/** encoder, query set, first, count, buffer, offset. */
export const RESOLVE_QUERY_SET = 24 as number;

/** id, encoder, color attachments' count, each: view, resolve view or NONE, load (LOAD_OP), store (STORE_OP), clear r, g, b, a (f32 bits); depth-stencil: view or NONE for none, the rest present either way: depth load, store, clear (f32 bits), read only (0 or 1), stencil load, store, clear, read only; timestamps: query set or NONE, begin index or NONE, end index or NONE; occlusion query set or NONE. */
export const RENDER_PASS = 25 as number;

/** id, encoder; timestamps: query set or NONE, begin index or NONE, end index or NONE. */
export const COMPUTE_PASS = 26 as number;

/** pass, pipeline: render or compute, as the pass is. */
export const SET_PIPELINE = 27 as number;

/** pass, index, group, dynamic offsets' count, that many offsets. */
export const SET_BIND_GROUP = 28 as number;

/** pass, slot, buffer, offset, size or NONE. */
export const SET_VERTEX_BUFFER = 29 as number;

/** pass, buffer, format (INDEX_FORMAT), offset, size or NONE. */
export const SET_INDEX_BUFFER = 30 as number;

/** pass, x, y, width, height, min depth, max depth (f32 bits). */
export const SET_VIEWPORT = 31 as number;

/** pass, x, y, width, height. */
export const SET_SCISSOR = 32 as number;

/** pass, r, g, b, a (f32 bits). */
export const SET_BLEND_CONSTANT = 33 as number;

/** pass, reference. */
export const SET_STENCIL_REFERENCE = 34 as number;

/** pass, vertices, instances, first vertex, first instance. */
export const DRAW = 35 as number;

/** pass, indices, instances, first index, base vertex (i32), first instance. */
export const DRAW_INDEXED = 36 as number;

/** pass, buffer, offset. */
export const DRAW_INDIRECT = 37 as number;

/** pass, buffer, offset. */
export const DRAW_INDEXED_INDIRECT = 38 as number;

/** pass, workgroups x, y, z. */
export const DISPATCH = 39 as number;

/** pass, buffer, offset. */
export const DISPATCH_INDIRECT = 40 as number;

/** pass: ended, and let go with the record. */
export const END = 41 as number;

/** encoders' count, that many encoders, each finished and let go with the record. */
export const SUBMIT = 42 as number;

/** buffer, offset, size, address, word, ticket: WebGPU's mapAsync for reading, of a buffer the guest made MAP_READ | COPY_DST and copied into on an encoder it submitted before. Once mapped, the range is copied to the address, ticket | DONE written at the word and the buffer unmapped. The word is the mapping's own: the guest maps the buffer again only once it holds DONE, and judges by the ticket whose bytes landed. */
export const MAP_ASYNC = 43 as number;

/** word, ticket: ticket | DONE at the word once what was submitted before it is done, where the ticket still stands. */
export const QUEUE_DONE = 44 as number;

/** surface, format (TEXTURE_FORMAT, or SURFACE_PREFERRED), usage (TEXTURE_USAGE), alpha mode (CANVAS_ALPHA_MODE). */
export const CONFIGURE = 45 as number;

/** surface, width, height. */
export const RESIZE = 46 as number;

/** id, surface: the surface's texture for this turn, as a view. */
export const CURRENT = 47 as number;

/** address, words: the host writes there the device's limits (LIMIT_NAME, a word each), then its features (FEATURE_NAME, a bit each, in words), then its timestamp period (f32 bits: the nanoseconds a resolved timestamp's unit is, 1 where they are nanoseconds as WebGPU's are), then the surface format it prefers (TEXTURE_FORMAT: what SURFACE_PREFERRED names, as the web's getPreferredCanvasFormat), then STATUS. */
export const DEVICE = 48 as number;

/** RENDER_PIPELINE's words, then word, ticket: WebGPU's createRenderPipelineAsync. Once the pipeline is made, ticket | DONE at the word, where the ticket still stands; the guest names the pipeline in no other record until it holds DONE. One the device refuses is made as RENDER_PIPELINE makes it, its error reported as that op's, and its word written all the same. A host with no async call makes it at once and writes the word by the same path, before the next turn reads it. */
export const RENDER_PIPELINE_ASYNC = 49 as number;

/** COMPUTE_PIPELINE's words, then word, ticket: WebGPU's createComputePipelineAsync, its word as RENDER_PIPELINE_ASYNC's. */
export const COMPUTE_PIPELINE_ASYNC = 50 as number;
