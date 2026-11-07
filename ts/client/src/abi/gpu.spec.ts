// @pxl/client/abi/gpu.spec.ts
//
// GENERATED from spec/client/render/gpu/api.json
//
// pixelet's conventions over xpute's GPU calls (xpute/spec/gpu/api.json), which
// the renderer writes through xpute's encoder (gpu/api.rs): the stream it writes
// them into, its pipeline records' codes, and what a surface shows.

import * as surface_role from "./gpu/surface_role.spec.ts";
import * as stream from "./gpu/stream.spec.ts";

/** What a canvas the UI hands over shows (gpu/surface.rs): the world, a character alone, or
 * nothing yet, as a canvas opens before the UI says what it draws. */
export { surface_role };

/** The one place the renderer's calls cross (gpu/api.rs): a stream
 * (xpute-core ipc/stream.rs) in the core's machine's GPU socket, which the
 * core writes its calls into during a turn and the runtime runs once the turn
 * has returned. Its size is the socket's (rust/client/core/machine.json `gpu`).
 *
 * Its head, past the stream's own words, is what the runtime writes for the
 * core: the device's block (xpute DEVICE) and the timed spans' reads, three
 * of them (gpu/api.rs `READS`), each its word, then each span's begin and end
 * ticks. gpu/api.rs lays them out, and the stream says the head's length itself. */
export { stream };
