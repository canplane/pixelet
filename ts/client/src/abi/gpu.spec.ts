// @pxl/client/abi/gpu.spec.ts
//
// GENERATED from spec/client/render/api.json5 (xpute-gen data.rs)
//
// pixelet's conventions over the GPU's calls (spec/gpu/api.json5), which
// the renderer writes through the GPU's encoder (gpu/api.rs): the stream it writes
// them into, its pipeline records' codes, and what a surface shows.

/** What a canvas the UI hands over shows (gpu/surface.rs): the world, a character alone, or
 * nothing yet, as a canvas opens before the UI says what it draws. */
export const surface_role = {
  WORLD: 0 as number,
  CHARACTER: 1 as number,
  NONE: 2 as number,
} as const;

/** The one place the renderer's calls cross (gpu/api.rs): a stream
 * (pxl-gpu-guest stream.rs) in the front of the core's memory, which the
 * core writes its calls into during an entry and the host runs once the entry
 * has returned. Its size is `SZ`.
 *
 * Its head, past the stream's own words, is what the host writes for the
 * core: the device's block (spec/gpu/api.json5 DEVICE) and the timed spans' reads, three
 * of them (gpu/api.rs `READ_CAP`), each its word, then each span's begin and end
 * ticks. gpu/api.rs lays them out, and the stream says the head's length itself. */
export const stream = {
  /** The stream's room: what one entry may send the device, then at its end the
   * encoder's handle table, a product number said rather than derived. The
   * stream says its own head. */
  SZ: 4194304 as number,
  /** What an upload leaves of the range. The records a frame cannot put off — a
   * frame's passes and draws, the renderer's pipelines — are never refused, so
   * an upload, or a shader module made before the renderer (`MODULE_SZ`),
   * which can wait a frame, is taken only while this much is left after it. */
  RESERVE: 1048576 as number,
  /** The WGSL a frame hands the device as shader modules before the renderer is
   * made (gpu/pipeline.rs `PipelineCache::make_modules`): one module a frame, then
   * more while their sources stay within this and `RESERVE` is left. A page
   * makes a module on its main thread, so this bounds the task that makes
   * them, a module larger than it alone in its frame. */
  MODULE_SZ: 131072 as number,
} as const;

/** What the GPU's host runs the renderer's stream by (pxl-gpu-link and @pxl/gpu/link
 * `GpuConfig`), the same on a device, where the runtime hands it (pxl-runtime lib.rs `GPU`),
 * and on a page, where the page's worker hands it (ts/client runtime/worker.ts). */
export const device = {
  /** The frames drawn and not yet finished on the GPU at once: a frame draws only
   * while fewer are (pxl-gpu-guest `admit::frame`), and a surface queues as
   * many. Two, as wgpu configures a surface by default
   * (`Surface::get_default_config`): one frame's lag behind the input, with the
   * GPU kept busy while the next is drawn. */
  FRAME_LATENCY: 2 as number,
  /** The chunk the host stages buffer writes in, each mapped once and reused: a
   * mesh, the largest write a frame makes, is about a MiB, and a larger write
   * takes a chunk of its own. */
  UPLOAD_CHUNK_SZ: 1048576 as number,
  /** WebGPU's power preference, which the device is asked with (pxl-gpu-link and
   * @pxl/gpu/link `GpuAsk`, pxl-gpu-host `Ask::of`). */
  POWER: "high-performance",
  /** The WebGPU features the device must have: a depth-stencil format the
   * renderer draws with. */
  REQUIRED_FEATURES: ["depth32float-stencil8"],
  /** The WebGPU features the device takes where the adapter has them: timestamps,
   * for the timed spans. */
  OPTIONAL_FEATURES: ["timestamp-query"],
} as const;

/** A pipeline record's codes (frame/shader/record.rs, expanded into xpute's
 * descriptors by gpu/api.rs), and HDR, the words it opens with before its
 * vertex buffers. Then: the topology, the cull mode, the depth compare under
 * reversed Z (LESS is plain depth, for a depth-only pass; NO_DEPTH, no
 * depth attachment), the blend, which vertex entry point of
 * its shader the pipeline enters, a vertex step and format, and a binding's
 * kind (its stages are xpute's SHADER_STAGE) — TEXTURE_UNFILTERED a texture in a
 * format no sampler
 * filters (RGBA32F), read by textureLoad alone; TEXTURE_UINT_ARRAY a u32
 * texture's layers, read by textureLoad; STORAGE_TEXTURE_RW an r32uint
 * texture's layers read and written in place, the one format family WebGPU
 * lets a shader do both to; TEXTURE_3D a filtered volume (the air's,
 * pass/atmosphere.rs); STORAGE_TEXTURE_3D an rgba16float volume a compute
 * pass writes, STORAGE_TEXTURE_2D an rgba16float texture one writes (the
 * exposure's texel, pass/atmosphere.rs); SAMPLER_COMPARISON a sampler that
 * compares a depth texture's texels against a reference (the light's shadow
 * map, pass/shadow.rs). */
export const record = {
  HDR: 10 as number,
  TRIANGLES: 0 as number,
  STRIP: 1 as number,
  CULL_NONE: 0 as number,
  CULL_BACK: 1 as number,
  ALWAYS: 0 as number,
  GREATER: 1 as number,
  GREATER_EQUAL: 2 as number,
  LESS: 3 as number,
  NO_DEPTH: 4 as number,
  OPAQUE: 0 as number,
  ALPHA: 1 as number,
  ADD: 2 as number,
  VS: 0 as number,
  VS_PLACE: 1 as number,
  PER_VERTEX: 0 as number,
  PER_INSTANCE: 1 as number,
  U32: 5 as number,
  U32X2: 6 as number,
  F32X2: 1 as number,
  F32X3: 2 as number,
  F32X4: 3 as number,
  U32X4: 4 as number,
  UNIFORM: 0 as number,
  UNIFORM_DYNAMIC: 1 as number,
  STORAGE: 2 as number,
  TEXTURE: 3 as number,
  TEXTURE_ARRAY: 4 as number,
  DEPTH: 5 as number,
  SAMPLER: 6 as number,
  SAMPLER_COMPARISON: 7 as number,
  TEXTURE_UNFILTERED: 8 as number,
  STORAGE_RW: 9 as number,
  TEXTURE_UINT_ARRAY: 10 as number,
  STORAGE_TEXTURE_RW: 11 as number,
  TEXTURE_3D: 12 as number,
  STORAGE_TEXTURE_3D: 13 as number,
  STORAGE_TEXTURE_2D: 14 as number,
} as const;
