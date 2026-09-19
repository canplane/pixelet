// @xpute/runtime/mem/section.test.ts

import { assert, assertEquals } from "@std/assert";

import { align_up, nth_at, read_section, type Section, SECTION_WORDS, span_of, write_section } from "./section.ts";

const PAGE = 1 << 12;

Deno.test("memory - a range ends at the boundary the next one begins on", () => {
  assertEquals(align_up(0, PAGE), 0);
  assertEquals(align_up(1, PAGE), PAGE);
  assertEquals(align_up(PAGE - 1, PAGE), PAGE);
  assertEquals(align_up(PAGE, PAGE), PAGE, "a boundary is already one");
  assertEquals(align_up(PAGE + 1, PAGE), 2 * PAGE);
});

Deno.test("memory - ranges laid end to end each begin on the unit and never overlap", () => {
  const sizes = [PAGE, 1, 3 * PAGE + 1, 0];
  const base = 8 * PAGE;
  const at = sizes.map((_, n) => nth_at(base, sizes, PAGE, n));
  assertEquals(at, [8 * PAGE, 9 * PAGE, 10 * PAGE, 14 * PAGE]);
  at.forEach((a, i) => {
    assert(a % PAGE === 0, `${i}`);
    assert(a + sizes[i] <= nth_at(base, sizes, PAGE, i + 1), `${i}: runs into the next`);
  });
  assertEquals(span_of(sizes, PAGE), 6 * PAGE, "the last one's rounding is in the span");
  assertEquals(nth_at(base, sizes, PAGE, sizes.length), base + span_of(sizes, PAGE), "and one past the last is where the span ends");
});

Deno.test("memory - nothing laid spans nothing", () => {
  assertEquals(span_of([], PAGE), 0);
  assertEquals(nth_at(64, [], PAGE, 0), 64);
});

Deno.test("memory - a section's words read back as they were written", () => {
  const words = new Uint32Array(2 * SECTION_WORDS);
  const s: Section = { offset: 1 << 26, bytes: 2 << 26, align: PAGE };
  write_section(words, SECTION_WORDS, s);
  assertEquals(read_section(words, SECTION_WORDS), s);
});
