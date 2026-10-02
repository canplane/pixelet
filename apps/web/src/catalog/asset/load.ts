// @pxl/web/catalog/asset/load.ts

/** A load number: an asset's index in the ingest's table, or one past it for the texture sheet. */

import type { u32 } from "@xpute/core/abi/word.ts";
import { INGESTED, SHEET } from "./ingested.spec.ts";

const KEYS: readonly string[] = Object.values(INGESTED).map((a) => a.key);

export function loadKey(number: u32): string | null {
  if (number === KEYS.length) return SHEET.key;
  return KEYS[number] ?? null;
}
