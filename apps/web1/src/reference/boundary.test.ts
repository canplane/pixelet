// @pxl/web/reference/boundary.test.ts

import { assertEquals } from "@std/assert";

/** No live module imports a reference: the app runs the module, and the
 * references exist only for the tests to hold it to. */
Deno.test("reference - nothing outside reference/ and the tests imports from it", async () => {
  const root = new URL("../", import.meta.url);
  const offenders: string[] = [];
  async function walk(dir: URL): Promise<void> {
    for await (const entry of Deno.readDir(dir)) {
      const path = new URL(entry.name + (entry.isDirectory ? "/" : ""), dir);
      if (entry.isDirectory) {
        if (entry.name !== "reference") await walk(path);
        continue;
      }
      if (!/\.tsx?$/.test(entry.name) || /\.test\.tsx?$/.test(entry.name)) continue;
      const text = await Deno.readTextFile(path);
      if (/from\s+"@\/reference\//.test(text) || /import\s+"@\/reference\//.test(text)) offenders.push(path.pathname.slice(root.pathname.length));
    }
  }
  await walk(root);
  assertEquals(offenders, []);
});
