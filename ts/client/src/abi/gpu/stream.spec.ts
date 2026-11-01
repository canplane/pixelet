// @pxl/client/abi/gpu/stream.spec.ts
//
// GENERATED from spec/client/render/gpu/api.json

export const HEAD = 141 as number;

/** The range's size: the stream, what one turn may send the device, then at
 * its end the encoder's handle table (gpu/api.rs), which zeroed bytes are an
 * empty one of. A product number, said rather than derived: a turn's uploads
 * wait (`room`) only when a burst of them comes at once. */
export const BYTES = 4194304 as number;

/** What an upload leaves of the range. The records a turn cannot put off — a
 * frame's passes and draws, the shaders the first frame makes — are never
 * refused, so an upload, which can wait a turn, is taken only while this much
 * is left after it. */
export const RESERVE = 1048576 as number;
