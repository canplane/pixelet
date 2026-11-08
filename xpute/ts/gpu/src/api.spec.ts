// @xpute/gpu/api.spec.ts
//
// GENERATED from spec/gpu/api.json
//
// The calls a core makes to a GPU it cannot reach, written as records of a stream
// (ipc/stream). The vocabulary is WebGPU's, with its descriptors written out as words, and
// it serves any program. A record is an op, a count, and the op's words in the order the
// op's doc lists them. Bytes go as a count followed by padded words, as the stream lays
// them. Objects are named by the core's handles (abi/handle). An optional field holds NONE
// when it is unset. Render and compute ops sit side by side.
//
// A transaction (ipc/stream) that does not commit drops its records and gives back the
// handles they took, so the runtime never hears of those objects. The encoder's begin and
// commit open and close the transaction on both the stream and the handle table.

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as op from "./api/op.spec.ts";
import * as BUFFER_USAGE from "./api/buffer_usage.spec.ts";
import * as TEXTURE_USAGE from "./api/texture_usage.spec.ts";
import * as SHADER_STAGE from "./api/shader_stage.spec.ts";
import * as COLOR_WRITE from "./api/color_write.spec.ts";
import * as STATUS from "./api/status.spec.ts";
import * as RESULT from "./api/result.spec.ts";

/** The value of an optional field that is unset, such as an absent index or handle. */
export const NONE: u32 = 4294967295 as number;

/** A value for CONFIGURE's format word that stands for the format the runtime's surface
 * prefers. */
export const SURFACE_PREFERRED: u32 = 255 as number;

/** The length in words of one entry of a bind group layout (BIND_GROUP_LAYOUT). */
export const BINDING_ENTRY_WORDS: u32 = 6 as number;

/** The length in words of one entry of a bind group (BIND_GROUP). */
export const RESOURCE_ENTRY_WORDS: u32 = 5 as number;

/** The ops, numbered from 1; an op of 0 ends a stream. */
export { op };

export const BINDING_KIND = [
  "buffer",
  "sampler",
  "texture",
  "storage-texture",
] as const;

export const RESOURCE_KIND = ["buffer", "sampler", "texture-view"] as const;

/** GPUBufferUsage, its bits as WebGPU numbers them. */
export { BUFFER_USAGE };

/** GPUTextureUsage, its bits as WebGPU numbers them. */
export { TEXTURE_USAGE };

/** GPUShaderStage, its bits as WebGPU numbers them. */
export { SHADER_STAGE };

/** GPUColorWrite, its bits as WebGPU numbers them. */
export { COLOR_WRITE };

/** The last word of the DEVICE block: what the runtime knows of the device. Only the
 * runtime writes it, and it stores it with release after the rest of the block. The core
 * loads it with acquire before it reads the block. */
export { STATUS };

/** How a completion is answered. A completion (COPY_EXTERNAL_IMAGE, MAP_ASYNC, QUEUE_DONE,
 * RENDER_PIPELINE_ASYNC, COMPUTE_PIPELINE_ASYNC) names a fence word and a ticket. Only the
 * core picks tickets: for each fence word it advances a sequence, which wraps, and it
 * keeps one request out on that word at a time. Only the runtime writes the fence. Once
 * what the request guards is written, and after the result in the next word where the
 * request can fail (COPY_EXTERNAL_IMAGE, MAP_ASYNC), the runtime stores the ticket it
 * finished into the fence with release. The core loads the fence with acquire and holds
 * the request done when the fence equals the ticket the core asked with, a test for
 * equality that the sequence's wrap does not break. Only then does the core read the
 * result and what the request guards. A late answer to a request that was dropped or
 * superseded carries an older ticket and never equals the current one. A lost device
 * answers none of its requests. */
export { RESULT };
