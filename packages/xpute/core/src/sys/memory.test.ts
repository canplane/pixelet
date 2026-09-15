// @xpute/core/sys/memory.test.ts

import { assertEquals, assertThrows } from "@std/assert";

import { lay_out, read_section, SECTION_WORDS, write_section } from "./memory.ts";

const R = 1 << 26;

Deno.test("memory - sections are whole regions in address order past the front, the last taking what is left", () => {
  assertEquals(lay_out(8 * R + 123, 32 << 20, R, 16 << 20, [1, 3]), [
    { offset: 32 << 20, bytes: 32 << 20, region_bytes: 16 << 20 },
    { offset: R, bytes: 3 * R, region_bytes: R },
    { offset: 4 * R, bytes: 4 * R, region_bytes: R },
  ]);
});

Deno.test("memory - a section's words read back as they were written", () => {
  const words = new Uint32Array(2 * SECTION_WORDS);
  const s = { offset: R, bytes: 2 * R, region_bytes: R };
  write_section(words, SECTION_WORDS, s);
  assertEquals(read_section(words, SECTION_WORDS), s);
});

Deno.test("memory - too few regions for the sections is refused", () => {
  assertThrows(() => lay_out(2 * R, 32 << 20, R, 16 << 20, [1, 1]));
});
