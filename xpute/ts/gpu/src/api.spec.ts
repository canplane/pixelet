// @xpute/gpu/api.spec.ts
//
// GENERATED from spec/gpu/api.json
//
// The calls a guest makes to a GPU it cannot reach, as records of a stream (ipc/stream):
// WebGPU's vocabulary, its descriptors written out as words, for any program. A record is
// an op, a count and the op's words in the order each op's doc gives; bytes go as a count
// and padded words, as the stream lays them. Objects are the guest's handles (abi/handle).
// An optional field holds NONE where unset. Render and compute sit side by side.
// A transaction (ipc/stream) that does not commit drops its records and gives back the handles they took, so the host never hears of the objects: the encoder's begin and commit open and close both.

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as op from "./api/op.spec.ts";
import * as BUFFER_USAGE from "./api/buffer_usage.spec.ts";
import * as TEXTURE_USAGE from "./api/texture_usage.spec.ts";
import * as SHADER_STAGE from "./api/shader_stage.spec.ts";
import * as COLOR_WRITE from "./api/color_write.spec.ts";
import * as STATUS from "./api/status.spec.ts";

/** An optional field left unset, an index or a handle absent. */
export const NONE: u32 = 4294967295 as number;

/** Or-ed into a ticket where a completion word is written (COPY_EXTERNAL_IMAGE, MAP_ASYNC, QUEUE_DONE, RENDER_PIPELINE_ASYNC, COMPUTE_PIPELINE_ASYNC). */
export const DONE: u32 = 2147483648 as number;

/** In CONFIGURE's format: the format the host's surface prefers. */
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

/** DEVICE's last word: what the host knows of the device. */
export { STATUS };
