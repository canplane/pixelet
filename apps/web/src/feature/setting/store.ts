// @pxl/web/feature/setting/store.ts

/**
 * What the browser keeps of the settings, which is the shell's to keep: the
 * kernel holds each by its number (crates/pxl/web/kernel/src/shell/setting/
 * store.rs) and names none, so the names stay here, where the sheet has
 * them anyway (schema.ts).
 *
 * Stored as one object by name, `{"poi":true,"ao":"screen"}`: a setting
 * and a choice's option are numbered by their place in schema.json, and a
 * name outlives the order they are listed in. Only what differs from the
 * default is stored, and a value equal to it is not: a default written to
 * storage is not a default — it is a decision nobody made, and changing the
 * value in the source would then reach only people with empty storage.
 *
 * What this build does not offer is neither read nor written back: a
 * dev-only setting has no row outside a development build, so nothing on
 * screen could undo it, and the kernel refuses it too. A stored value the
 * schema does not have, or a choice without that option, goes the same way,
 * and what is kept is rewritten from the kernel's snapshot, so storage holds
 * only what the page can reach. The cost is a development build finding its
 * dev toggles at their defaults after the production one has been opened on
 * the same origin, which is a toggle to flip and not a state to recover.
 */

import type { u32 } from "@xpute/core/abi/word.ts";

import { storage, STORAGE_KEYS } from "@/kernel/abi/host.spec.ts";
import { storageGet, storageSet } from "@platform/storage.ts";
import { type Setting, SETTINGS } from "./schema.ts";

/** A setting storage kept nothing for, as WORLD_BOOT carries it (the
 * kernel's `NONE`). */
const NONE: u32 = 0xffffffff;

const KEY = STORAGE_KEYS[storage.SETTINGS];

const ROWS = Object.entries(SETTINGS) as [string, Setting & { default: boolean | string | number }][];

/** A setting's value as the kernel numbers it. */
function number(row: Setting, value: unknown): u32 {
  switch (row.kind) {
    case "toggle":
      return typeof value === "boolean" ? Number(value) : NONE;
    case "choice":
      return typeof value === "string" && row.options.includes(value) ? row.options.indexOf(value) : NONE;
    case "index":
      return typeof value === "number" && Number.isInteger(value) && value >= 0 ? value : NONE;
  }
}

/** A setting's value as storage keeps it. */
function stored(row: Setting, value: u32): boolean | string | number {
  return row.kind === "toggle" ? value === 1 : row.kind === "choice" ? row.options[value] : value;
}

/** What storage kept, as WORLD_BOOT carries it: a value a setting, by its
 * key, NONE where it kept nothing this build can read. */
export function storedSettings(): u32[] {
  const values: u32[] = new Array(Math.max(...ROWS.map(([, row]) => row.key)) + 1).fill(NONE);
  let kept: Record<string, unknown> = {};
  try {
    const raw = storageGet(KEY);
    if (raw) kept = JSON.parse(raw);
  } catch {
    // Unreadable storage is none: every setting at its default, and the
    // first snapshot writes back what the kernel holds.
  }
  for (const [name, row] of ROWS) if (name in kept) values[row.key] = number(row, kept[name]);
  return values;
}

let written: string | null = null;

/** Keeps the settings the kernel's snapshot says it holds, `values` a value a
 * setting by its key: what differs from the defaults, by name. */
export function keepSettings(values: readonly u32[]): void {
  const out: Record<string, boolean | string | number> = {};
  for (const [name, row] of ROWS) {
    const value = values[row.key];
    if (value === undefined || value === number(row, row.default)) continue;
    out[name] = stored(row, value);
  }
  const json = JSON.stringify(out);
  if (json === written) return;
  written = json;
  storageSet(KEY, json);
}
