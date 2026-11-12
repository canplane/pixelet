// @pxl/client/abi/gpu/device.spec.ts
//
// GENERATED from spec/client/render/api.json

/** The frames drawn and not yet finished on the GPU at once: a turn draws only
 * while fewer are (xpute-gpu runtime `GpuRuntime::admit`), and a surface queues as
 * many. Two, as wgpu configures a surface by default
 * (`Surface::get_default_config`): one frame's lag behind the input, with the
 * GPU kept busy while the next is drawn. */
export const FRAME_LATENCY = 2 as number;

/** The chunk the runtime stages buffer writes in, each mapped once and reused: a
 * mesh, the largest write a frame makes, is about a MiB, and a larger write
 * takes a chunk of its own. */
export const UPLOAD_CHUNK_BYTES = 1048576 as number;
