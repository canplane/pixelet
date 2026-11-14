// @pxl/client/ui/feature/world/dock/store.test.ts

import { expect, test, vi } from "vitest";

import { CUBES } from "@/ui/shared/asset/catalog.ts";
import * as edit from "@/service/core/edit.ts";
import { RECENT_PICKS, recent_pick, select_asset } from "./store.ts";

vi.mock("@/service/core/edit.ts", () => ({ tool: vi.fn(async () => {}) }));

/** The hotbar, newest pick first. */
function hotbar(): number[] {
  return Array.from({ length: RECENT_PICKS }, (_, k) => recent_pick(k));
}

/** Picks `id`, and waits until the core has answered the tool. */
async function pick(id: number): Promise<void> {
  select_asset(id);
  await new Promise((r) => setTimeout(r, 0));
}

// The hotbar keeps the newest pick first, full: a new pick pushes the last
// one out, a pick already on it moves nothing, and a pick the core refuses is
// not added.
test("a pick goes first on the hotbar once the core holds it", async () => {
  const seed = hotbar();
  const fresh = CUBES.filter((c) => !seed.includes(c));
  await pick(seed[2]);
  expect(hotbar()).toEqual(seed);
  await pick(fresh[0]);
  expect(hotbar()).toEqual([fresh[0], ...seed.slice(0, -1)]);
  vi.mocked(edit.tool).mockRejectedValueOnce(new Error("refused"));
  vi.spyOn(console, "error").mockImplementation(() => {});
  await pick(fresh[1]);
  expect(hotbar()).toEqual([fresh[0], ...seed.slice(0, -1)]);
});
