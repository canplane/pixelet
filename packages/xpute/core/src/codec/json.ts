// @xpute/core/codec/json.ts

/**
 * json codec
 * - encode: strict (throws on invalid JSON shape / cycles / bigint)
 * - decode: best-effort (logs and returns undefined on failure)
 */

import type { Bytes } from "@xpute/core/abi/array.ts";
import * as encoding from "@xpute/core/codec/encoding.ts";

/**
 * [Encode] Converts an object to UTF-8 binary.
 * @throws {Error} on circular references or non-serializable data
 */
export function encode<T>(data: T): Bytes {
  // no data -> empty binary
  if (data === undefined || data === null) return new Uint8Array(0);

  try {
    // [Strict Serialization]
    // JSON.stringify is the strongest checker available at runtime.
    // - throws on a circular reference
    // - throws on a bigint
    const json = JSON.stringify(data);

    // [Validation]
    // undefined result means the input was only a function/symbol — reject it.
    if (json === undefined) {
      throw new Error("json codec: result is undefined. input is not a valid json shape.");
    }

    return encoding.te.encode(json);
  } catch (err) {
    // wrap and report as a system-level error
    throw new Error(`json codec: encode failed: ${err instanceof Error ? err.message : "unknown error"}`);
  }
}

/**
 * [Decode] Restores an object from binary.
 */
export function decode<T>(buffer: ArrayBuffer | ArrayBufferView): T | undefined {
  if (!buffer || buffer.byteLength === 0) return undefined;

  try {
    const json = encoding.td.decode(buffer);
    const result = JSON.parse(json);

    // minimal shape check after restore, if needed
    if (result === undefined) return undefined;

    return result as T;
  } catch (err) {
    console.error("[codec/json] decode failed:", err);
    return undefined;
  }
}
