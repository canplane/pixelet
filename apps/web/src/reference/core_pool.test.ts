// @pxl/web/reference/core_pool.test.ts

import { assert, assertEquals } from "@std/assert";

import { CORE_MEMORY, MODULE_SECTION, REGION_BYTES } from "@/wasm/core_memory.ts";
import { CORE_SECTIONS } from "@/reference/core_pool.ts";

Deno.test("core pool - the module is over the memory and every section's blocks come from it", () => {
  for (const name of ["fixed", "page", "mesh"] as const) {
    const s = CORE_SECTIONS[name];
    const block = s.pool.acquire(1 << 12);
    assert(block.buf === CORE_MEMORY.buffer, `${name}: in the core memory`);
    assert(block.byte_offset >= s.offset && block.byte_offset < s.offset + s.bytes, `${name}: inside its section`);
    assert(block.byte_offset >= MODULE_SECTION.bytes, `${name}: past the module's section`);
    assertEquals(s.pool.stat.live_blocks >= 1, true);
    s.pool.release(block);
  }
});

Deno.test("core pool - the kernel's map: sections in address order past the module's, to the end of the memory", () => {
  let end = MODULE_SECTION.bytes;
  for (const name of ["fixed", "page", "mesh"] as const) {
    const s = CORE_SECTIONS[name];
    assertEquals(s.offset, end, `${name}: starts where the one before ends`);
    end = s.offset + s.bytes;
  }
  assertEquals(end % REGION_BYTES, 0, "the map ends on a region");
  assertEquals(end, CORE_MEMORY.buffer.byteLength, "the memory is the map");
  assertEquals([CORE_SECTIONS.fixed.kernel_id, CORE_SECTIONS.page.kernel_id, CORE_SECTIONS.mesh.kernel_id], [0, 1, 2]);
});
