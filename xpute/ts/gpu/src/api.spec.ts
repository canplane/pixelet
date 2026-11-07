// @xpute/gpu/api.spec.ts
//
// GENERATED from spec/gpu/api.json
//
// The calls a core makes to a GPU it cannot reach, as records of a stream (ipc/stream):
// WebGPU's vocabulary, its descriptors written out as words, for any program. A record is
// an op, a count and the op's words in the order each op's doc gives; bytes go as a count
// and padded words, as the stream lays them. Objects are the core's handles (abi/handle).
// An optional field holds NONE where unset. Render and compute sit side by side.
// A transaction (ipc/stream) that does not commit drops its records and gives back the handles they took, so the runtime never hears of the objects: the encoder's begin and commit open and close both.

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as op from "./api/op.spec.ts";
import * as BUFFER_USAGE from "./api/buffer_usage.spec.ts";
import * as TEXTURE_USAGE from "./api/texture_usage.spec.ts";
import * as SHADER_STAGE from "./api/shader_stage.spec.ts";
import * as COLOR_WRITE from "./api/color_write.spec.ts";
import * as STATUS from "./api/status.spec.ts";
import * as RESULT from "./api/result.spec.ts";

/** An optional field left unset, an index or a handle absent. */
export const NONE: u32 = 4294967295 as number;

/** In CONFIGURE's format: the format the runtime's surface prefers. */
export const SURFACE_PREFERRED: u32 = 255 as number;

/** A bind group layout's entry (BIND_GROUP_LAYOUT). */
export const BINDING_ENTRY_WORDS: u32 = 6 as number;

/** A bind group's entry (BIND_GROUP). */
export const RESOURCE_ENTRY_WORDS: u32 = 5 as number;

/** The ops, numbered from 1: 0 ends a stream. */
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

/** DEVICE's last word: what the runtime knows of the device, written by the runtime alone and stored with release once the block before it is; the core loads it with acquire before it reads the block. */
export { STATUS };

/** A completion (COPY_EXTERNAL_IMAGE, MAP_ASYNC, QUEUE_DONE, RENDER_PIPELINE_ASYNC, COMPUTE_PIPELINE_ASYNC) names a fence and a ticket. The ticket is the core's alone: a sequence it advances for that word, wrapping, with one request out on the word at a time. The fence is the runtime's alone: once what the request guards is written, the runtime stores there the ticket it finished, with release, after the result in the word after it where the request can fail (COPY_EXTERNAL_IMAGE, MAP_ASYNC). The core loads the fence with acquire and holds the request done where it equals the ticket it asked with, an equality the sequence's wrap keeps, then reads the result and what the request guards. A late answer to a request dropped or superseded carries an older ticket and never equals the current one; a device lost answers none of its requests. */
export { RESULT };
