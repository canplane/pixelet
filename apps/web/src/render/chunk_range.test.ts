// @pxl/web/render/chunk_range.test.ts

import { assertEquals } from "@std/assert";

import { ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";

import { render_cfg } from "@/modules/world/environment/config.ts";
import { page_dist2, page_priority, pages_in_radius, pages_to_hold, range_anchor_of } from "./chunk_range.ts";

const at = (x: number, y: number) => ivec.gather(BigInt(x), BigInt(y), 0n);
const RADIUS = 1536;

Deno.test("range_anchor_of - snaps to the chunk, not to the page", () => {
  assertEquals(range_anchor_of(at(1100, 70)), at(1088, 64));
  assertEquals(range_anchor_of(at(-1, -1)), at(-64, -64));
});

Deno.test("pages_in_radius - holds every page within the radius and nothing outside it", () => {
  for (const [x, y] of [[0, 0], [512, 512], [1023, 1000], [1024, 0], [-1, -1], [-1025, 700]] as const) {
    const anchor = range_anchor_of(at(x, y));
    const got = pages_in_radius(anchor, RADIUS);
    for (const { page_idx } of got) assertEquals(page_dist2(anchor, page_idx) <= RADIUS * RADIUS, true);

    // And nothing in range is missing — scanned over a box wider than the
    // enumeration's own, so a too-narrow span would show up here.
    const have = new Set(got.map(({ page_idx }) => page_idx));
    const center_page = TO_XY(L3_IDX(anchor));
    for (let dy = -4; dy <= 4; dy++) {
      for (let dx = -4; dx <= 4; dx++) {
        const page_idx = ivec.add(center_page, at(dx, dy));
        if (page_dist2(anchor, page_idx) <= RADIUS * RADIUS) assertEquals(have.has(page_idx), true);
      }
    }
  }
});

Deno.test("pages_in_radius - nearest first, and the anchor's own page is first", () => {
  const anchor = range_anchor_of(at(512, 512));
  const got = pages_in_radius(anchor, RADIUS);
  assertEquals(got[0].dist2, 0);
  assertEquals(got.map(({ dist2 }) => dist2), [...got.map(({ dist2 }) => dist2)].sort((a, b) => a - b));
});

Deno.test("pages_in_radius - crossing a page boundary never drops a page still in range", () => {
  // The failure this replaces: the set was keyed on which page the view
  // centre stood in, so one boundary crossing moved it by a whole page and a
  // page's worth of terrain vanished.
  let previous: Set<bigint> | null = null;
  for (let x = 1024 - 256; x <= 1024 + 256; x += 64) {
    const anchor = range_anchor_of(at(x, 0));
    const set = new Set(pages_in_radius(anchor, RADIUS).map(({ page_idx }) => page_idx));
    if (previous) {
      for (const page_idx of previous) {
        if (page_dist2(anchor, page_idx) <= RADIUS * RADIUS) assertEquals(set.has(page_idx), true);
      }
    }
    previous = set;
  }
});

Deno.test("pages_in_radius - the configured range stays inside its measured page budget", () => {
  // Read off the range itself rather than a copy of it, so moving the range
  // has to face this number: 21 pages worst case over every anchor position,
  // ~85MB decoded — see discussions/20260912-what-to-render-when.md.
  let worst = 0;
  for (let y = 0; y < 1024; y += 64) {
    for (let x = 0; x < 1024; x += 64) {
      worst = Math.max(worst, pages_in_radius(range_anchor_of(at(x, y)), render_cfg.CHUNK_RENDER_DIST).length);
    }
  }
  assertEquals(worst <= 21, true);
});

Deno.test("page_priority - counts whole pages of distance, doubled so a second subsystem can interleave", () => {
  assertEquals(page_priority(0), 0);
  assertEquals(page_priority(1024 * 1024), 2);
});

Deno.test("pages_to_hold - always offers the full count, wherever the anchor stands", () => {
  // The count is what decides; the radius only has to be wide enough to pick
  // from. If it ever came up short the fetch set would shrink at some anchor
  // positions and pages would be evicted mid-flight.
  for (let y = 0; y < 1024; y += 64) {
    for (let x = 0; x < 1024; x += 64) {
      assertEquals(pages_to_hold(range_anchor_of(at(x, y)), 3072, 32).length, 32);
    }
  }
});

Deno.test("pages_to_hold - a page being fetched is one worth keeping", () => {
  // The property the cancellation bug came down to: everything the fetch set
  // asks for is inside the set that decides what is kept, because they are
  // the same set.
  const anchor = range_anchor_of(at(512, 512));
  const held = pages_to_hold(anchor, 3072, 32);
  const keys = new Set(held.map(({ page_idx }) => page_idx));
  assertEquals(keys.size, held.length);
  for (const { page_idx, dist2 } of held) assertEquals(page_dist2(anchor, page_idx), dist2);
});
