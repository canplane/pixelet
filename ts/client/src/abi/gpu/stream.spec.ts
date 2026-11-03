// @pxl/client/abi/gpu/stream.spec.ts
//
// GENERATED from spec/client/render/gpu/api.json

export const HEAD = 423 as number;

/** The range's size: the stream, what one turn may send the device, then at
 * its end the encoder's handle table (gpu/api.rs), which zeroed bytes are an
 * empty one of. A product number, said rather than derived: a turn's uploads
 * wait (`room`) only when a burst of them comes at once. */
export const BYTES = 4194304 as number;

/** What an upload leaves of the range. The records a turn cannot put off — a
 * frame's passes and draws, the renderer's pipelines — are never refused, so
 * an upload, or a shader module made before the renderer (`MODULE_BYTES`),
 * which can wait a turn, is taken only while this much is left after it. */
export const RESERVE = 1048576 as number;

/** The WGSL a turn hands the device as shader modules before the renderer is
 * made (gpu/pipeline.rs `Pipelines::make_modules`): one module a turn, then
 * more while their sources stay within this and `RESERVE` is left. A page
 * makes a module on its main thread, so this bounds the task that makes
 * them, a module larger than it alone in its turn. */
export const MODULE_BYTES = 131072 as number;
