// @pxl/web/catalog/asset/load.ts

/**
 * A load as the kernel names it on the wire (LOAD_CREATE): an asset's number
 * in the ingest's table, or one past the last for the texture pack's sheet.
 * The kernel holds no text, so the number is the whole of what crosses; the
 * key it names is read here, from the same rows the Rust's `Asset` was
 * numbered by (pxl-tool/asset/table.rs renders both).
 */

import type { u32 } from "@xpute/core/abi/word.ts";
import { INGESTED, SHEET } from "./ingested.spec.ts";

const KEYS: readonly string[] = Object.values(INGESTED).map((a) => a.key);

/** The bucket key a load number names, or null for a number the table does
 * not give out. */
export function load_key(number: u32): string | null {
  if (number === KEYS.length) return SHEET.key;
  return KEYS[number] ?? null;
}
