// @xpute/core/sys/memory.ts

/**
 * A linear memory cut into sections: the mechanism under a kernel's memory
 * map.
 *
 * The central scheduler reserves the memory — one fixed `WebAssembly.Memory`,
 * `initial == maximum`, never grown, since a grown memory detaches every view
 * and a resizable buffer is slower to read and refused by WebGPU — and gives
 * it to the kernel. The kernel lays out everything inside it: a section is a
 * range of whole regions, each the range one pool cuts, none borrowing from
 * another. What the sections are for, and how many regions each takes, is the
 * kernel's policy, not this file's.
 */

import type { u32 } from "../kit/word.ts";
import { MarshalError } from "./error.ts";
import { Errno } from "./errno.ts";

/** A WebAssembly page. */
export const WASM_PAGE_BYTES: u32 = 1 << 16;

/** Words a section takes in a boot block: its offset, bytes and region size. */
export const SECTION_WORDS: u32 = 3;

export type Section = { offset: u32; bytes: u32; region_bytes: u32 };

/**
 * Sections over a memory of `bytes`, in address order. The first starts past
 * `front_bytes` — what the module keeps for itself — and ends at region
 * `regions[0]`, cut in `first_region_bytes`; each next takes `regions[i]`
 * regions; one more takes every whole region left, at least one.
 */
export function lay_out(bytes: u32, front_bytes: u32, region_bytes: u32, first_region_bytes: u32, regions: readonly u32[]): Section[] {
  const total = Math.floor(bytes / region_bytes);
  const asked = regions.reduce((a, b) => a + b, 0);
  const first_end = regions[0] * region_bytes;
  if (regions.length === 0 || front_bytes >= first_end || front_bytes % first_region_bytes !== 0 || (first_end - front_bytes) % first_region_bytes !== 0) {
    throw new MarshalError(Errno.EINVAL, `memory: a first section of ${regions[0]} regions past ${front_bytes} bytes is not whole ${first_region_bytes}-byte regions`);
  }
  if (total < asked + 1) throw new MarshalError(Errno.EINVAL, `memory: ${total} regions for sections asking ${asked} and one more`);
  const out: Section[] = [{ offset: front_bytes, bytes: first_end - front_bytes, region_bytes: first_region_bytes }];
  let end = first_end;
  for (let i = 1; i < regions.length; i++) {
    out.push({ offset: end, bytes: regions[i] * region_bytes, region_bytes });
    end += regions[i] * region_bytes;
  }
  out.push({ offset: end, bytes: total * region_bytes - end, region_bytes });
  return out;
}

/** Writes a section's words at `at`. */
export function write_section(words: Uint32Array, at: u32, s: Section): void {
  words[at] = s.offset;
  words[at + 1] = s.bytes;
  words[at + 2] = s.region_bytes;
}

/** The section whose words are at `at`. */
export function read_section(words: ArrayLike<u32>, at: u32): Section {
  return { offset: words[at], bytes: words[at + 1], region_bytes: words[at + 2] };
}

/**
 * The central scheduler's reservation: a fixed memory of the first region
 * count in `candidates` the device will reserve.
 */
export function reserve_memory(candidates: readonly u32[], region_bytes: u32): WebAssembly.Memory {
  for (const regions of candidates) {
    const pages = (regions * region_bytes) / WASM_PAGE_BYTES;
    try {
      return new WebAssembly.Memory({ initial: pages, maximum: pages });
    } catch {
      // too large for this device; try the next
    }
  }
  throw new MarshalError(Errno.ENOMEM, `memory: the device reserved none of ${candidates.join(", ")} regions`);
}
