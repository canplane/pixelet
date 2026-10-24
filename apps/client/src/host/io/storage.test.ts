// @pxl/client/host/io/storage.test.ts

import { assertEquals } from "@std/assert";
import { b64url_to_bytes, bytes_to_b64url } from "@xpute/kit/codec/encoding.ts";

import { format, kept } from "@/host/abi/host.spec.ts";
import { follow_refused, formatted, kept_key, migrate, set_aside, type Storage } from "./storage.ts";

function storage(entries: Record<string, string> = {}, refuses = false): Storage & { map: Map<string, string> } {
  const map = new Map(Object.entries(entries));
  return { map, get: (k) => map.get(k) ?? null, set: (k, v) => void (refuses || map.set(k, v)) };
}

const PACKET = bytes_to_b64url(new Uint8Array([0x58, 0x54, 0x50, 0, 1, 2, 3]));

Deno.test("migrate - the objects carry over with their format before them, and stay where they were", () => {
  const s = storage({ "pxl:object:v15": PACKET });
  migrate(s);
  const slot = b64url_to_bytes(s.map.get(kept_key(kept.OBJECTS)!)!);
  assertEquals([...slot], [15, 0x58, 0x54, 0x50, 0, 1, 2, 3]);
  assertEquals(format.OBJECTS, 15);
  assertEquals(s.map.get("pxl:object:v15"), PACKET);
});

Deno.test("migrate - once: a slot that holds anything is not written", () => {
  const s = storage({ "pxl:object:v15": PACKET, "pxl:kept:objects": "", "pxl:settings": "AQ", "pxl:kept:settings": "AQIB" });
  migrate(s);
  assertEquals(s.map.get("pxl:kept:objects"), "");
  assertEquals(s.map.get("pxl:kept:settings"), "AQIB");
});

Deno.test("migrate - the settings carry over as they are, for the kernel to read or refuse", () => {
  const s = storage({ "pxl:settings": '{"ao":"screen"}' });
  migrate(s);
  assertEquals(s.map.get("pxl:kept:settings"), '{"ao":"screen"}');
  const none = storage();
  migrate(none);
  assertEquals(none.map.size, 0);
});

Deno.test("formatted - what is not base64url stays as it is", () => {
  assertEquals(formatted(15, "{not base64}"), "{not base64}");
  assertEquals(formatted(15, ""), "");
});

Deno.test("set_aside - a refused file is copied, read back, then its slot emptied", () => {
  const s = storage({ "pxl:kept:objects": "AAAA", "pxl:kept:objects:aside:7": "older" });
  assertEquals(set_aside(s, kept.OBJECTS, 7), true);
  assertEquals(s.map.get("pxl:kept:objects"), "");
  assertEquals(s.map.get("pxl:kept:objects:aside:7"), "older");
  assertEquals(s.map.get("pxl:kept:objects:aside:7-1"), "AAAA");
});

Deno.test("set_aside - storage that refuses the copy keeps the slot as it was", () => {
  const s = storage({ "pxl:kept:objects": "AAAA" }, true);
  assertEquals(set_aside(s, kept.OBJECTS, 7), false);
  assertEquals(s.map.get("pxl:kept:objects"), "AAAA");
  assertEquals(set_aside(storage(), 99, 7), false);
});

Deno.test("follow_refused - each refused slot is set aside and emptied once", () => {
  const s = storage({ "pxl:kept:settings": '{"ao":"screen"}' });
  const told: number[] = [];
  follow_refused([kept.SETTINGS], s, (slot) => told.push(slot));
  follow_refused([kept.SETTINGS], s, (slot) => told.push(slot));
  assertEquals(told, [kept.SETTINGS]);
  assertEquals(s.map.get("pxl:kept:settings"), "");
  follow_refused([], s, (slot) => told.push(slot));
  assertEquals(told, [kept.SETTINGS]);
});
