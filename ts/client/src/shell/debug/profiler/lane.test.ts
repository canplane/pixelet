// @pxl/client/shell/debug/profiler/lane.test.ts

import { assertEquals } from "@std/assert";

import { FRAME_GROUP } from "@/abi/profiler.spec.ts";
import { is_list, LANES } from "@/abi/profiler.ts";
import { GROUPS, LIST_NAMES } from "./lane.ts";

Deno.test("FRAME_GROUP - every lane is printed, in one group", () => {
  const grouped = GROUPS.flatMap((g) => FRAME_GROUP[g] as readonly string[]);
  assertEquals([...grouped].sort(), LANES.map((l) => l.name as string).sort());
});

Deno.test("LANES - a list lane spans a word a name, a census entry two", () => {
  for (const { name, width } of LANES) {
    if (!is_list(name)) assertEquals(width, 1, name);
    else {assertEquals(
        width,
        LIST_NAMES[name].length * (name === "census" ? 2 : 1),
        name,
      );}
  }
});
