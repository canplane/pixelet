// @pxl/web/feature/setting/store.test.ts

import { assertEquals } from "@std/assert";

import { storage, STORAGE_KEYS } from "@/kernel/abi/host.spec.ts";
import type { Setting } from "./schema.ts";

// The bundler's define, which a test has no bundler to supply: a production
// build, whose table has no dev row.
(globalThis as { __IS_DEV__?: boolean }).__IS_DEV__ = false;
const { SETTINGS } = await import("./schema.ts");
const { keepSettings, storedSettings } = await import("./store.ts");

/** What differs from the defaults is kept by name and read back by number; a
 * value set back to its default leaves storage. */
Deno.test("settings - what is kept is what differs, by name, and it reads back", () => {
  const values = storedSettings().map((_, key) => {
    const row = Object.values(SETTINGS).find((s) => s.key === key) as (Setting & { default: boolean | string | number }) | undefined;
    // A dev setting a production build has no row for.
    if (row === undefined) return 0xffffffff;
    return row.kind === "toggle" ? Number(row.default) : row.kind === "choice" ? row.options.indexOf(row.default as string) : row.default as number;
  });
  values[SETTINGS.ao.key] = (SETTINGS.ao.options as readonly string[]).indexOf("screen");
  values[SETTINGS.person.key] = 3;
  keepSettings(values);
  assertEquals(localStorage.getItem(STORAGE_KEYS[storage.SETTINGS]), '{"ao":"screen","person":3}');
  const back = storedSettings();
  assertEquals([back[SETTINGS.ao.key], back[SETTINGS.person.key]], [values[SETTINGS.ao.key], 3]);
  values[SETTINGS.ao.key] = (SETTINGS.ao.options as readonly string[]).indexOf(SETTINGS.ao.default);
  keepSettings(values);
  assertEquals(localStorage.getItem(STORAGE_KEYS[storage.SETTINGS]), '{"person":3}');
  localStorage.clear();
});
