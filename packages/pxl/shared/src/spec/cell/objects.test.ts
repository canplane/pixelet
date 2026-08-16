// @pxl/shared/spec/cell/objects.test.ts

import { assertEquals } from "@std/assert";
import { OBJECT_LAYOUT } from "@pxl/shared/bake/constants.ts";
import { pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import * as sparse_tree from "@pxl/shared/spec/space/sparse_tree/tree.ts";

import { object_color_id_from_num, object_color_ids, object_color_num_id, OBJECT_PRESENT_MASK, object_types, OBJECT_UNDEF, object_value_is_air, object_value_is_present } from "./objects.ts";

Deno.test("object cell values - zero is sparse absence and explicit values own the high bit", () => {
  assertEquals(OBJECT_UNDEF, 0);
  assertEquals(object_value_is_present(OBJECT_UNDEF), false);
  assertEquals(object_types.block.AIR, 0x81);
  assertEquals(object_value_is_air(object_types.block.AIR), true);
  assertEquals(object_types.block.UNKNOWN, 0xff);

  const red = object_color_num_id("red");
  assertEquals((red & OBJECT_PRESENT_MASK) !== 0, true);
  assertEquals(red >= 0x82 && red < 0xff, true);
  assertEquals(object_color_id_from_num(red), "red");
  assertEquals(object_color_ids.every((id) => (object_color_num_id(id) & OBJECT_PRESENT_MASK) !== 0), true);
});

Deno.test("object cell values - explicit air survives sparse-tree compaction", () => {
  const tree = sparse_tree.alloc(OBJECT_LAYOUT, 8, Uint8Array);
  const coord = pvf3.gather({ i: 0n, f: 0n }, 0n, 0n, 0n);
  sparse_tree.write(tree, coord, object_types.block.AIR);
  sparse_tree.compact(tree);
  assertEquals(sparse_tree.read(tree, coord), 0x81);
});
