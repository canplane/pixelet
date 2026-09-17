// @pxl/web/wasm/kernel.test.ts

/**
 * The kernel driven the way wasm/kernel.ts drives it, without a page: the
 * boot, a map centre, the world's turns rung, the fetches it starts
 * answered from fixtures through the bodies the host holds, and what
 * comes back — what was drawn, the map's status, the UI's snapshot and its
 * actions.
 */

import { assert, assertEquals } from "@std/assert";

import type { u32 } from "@xpute/core/kit/word.ts";
import { fnv1a64_bytes, fnv1a64_str } from "@xpute/core/kit/hash.ts";
import { Cmd } from "@pxl/shared/spec/cmd.ts";
import { ELE_PADDED_SPAN, PAGE_CELL_SPAN } from "@pxl/shared/bake/constant.ts";
import { encode_poi_page, encode_terrain } from "@pxl/shared/bake/format.ts";
import { cube_types } from "@pxl/shared/spec/cell/cube.ts";

import { CORE_BUILD, packet } from "@/wasm/core_build.ts";
import { CORE_KERNEL } from "@/wasm/core_mem.ts";
import { BOOT, boot_block, hold_body, text_at } from "@/wasm/module.ts";

function rng(seed: u32): () => u32 {
  let s = seed >>> 0 || 1;
  return () => {
    s ^= s << 13;
    s ^= s >>> 17;
    s ^= s << 5;
    return s >>> 0;
  };
}

type Snapshot = {
  map: { has_center: boolean; center_lonlat: [number, number] | null; status: string };
  settings: Record<string, boolean | string>;
  search: { loading: boolean; results: unknown[] };
};

/** A started fetch: the key the kernel hands out and takes back, its URL,
 * and the stream the URL says it belongs to. */
type Fetched = { key: bigint; url: string; category: string };

/** The stream a fetch's URL is for (crates/pxl/core/src/lib/env/route.rs). */
function category_of(url: string): string {
  if (url.startsWith("/api/search")) return "search";
  if (url.endsWith("/_ocean")) return "ocean";
  return url.split("/")[3];
}

/** crates/pxl/core/src/module/world/sched/demand.rs's Category, in order. */
const SEARCH_CATEGORY = 3;

/** fetch_scheduler.rs's hash_of: the category as a u32 and the subject as a
 * u64, both little-endian, under FNV-1a. */
function key_of(category: number, subject: bigint): bigint {
  const bytes = new Uint8Array(12);
  const view = new DataView(bytes.buffer);
  view.setUint32(0, category, true);
  view.setBigUint64(4, subject, true);
  return fnv1a64_bytes(bytes);
}

type Turn = { snapshot: Snapshot; fetched: Fetched[]; draws: number };

let snapshot: Snapshot | null = null;
let fetched: Fetched[] = [];
let loads: string[] = [];
let draws = 0;

CORE_BUILD.signal = (_tag, cmd, f) => {
  if (cmd === Cmd.WORLD_SNAPSHOT) {
    const [at, len] = f!.get<[number, number]>();
    snapshot = JSON.parse(text_at(CORE_KERNEL.memory, at >>> 0, len >>> 0)) as Snapshot;
  } else if (cmd === Cmd.WORLD_UPDATE) draws = f!.at(0).get<number>();
  else if (cmd === Cmd.FETCH_CREATE) {
    const [key, url] = f!.get<[bigint, string]>();
    fetched.push({ key, url, category: category_of(url) });
  } else if (cmd === Cmd.LOAD_CREATE) loads.push(f!.at(1).get<string>());
};

/** A page's body as the bake encodes it: a rolling elevation raster and
 * ground that is mostly land, with water, structures and roads sprinkled. */
function terrain_body(seed: u32): Uint8Array {
  const next = rng(seed);
  const cells = PAGE_CELL_SPAN * PAGE_CELL_SPAN;
  const ele = new Int16Array(ELE_PADDED_SPAN * ELE_PADDED_SPAN);
  for (let i = 0; i < ele.length; i++) ele[i] = 10 + (next() % 30);
  const land = new Uint8Array(cells), water = new Uint8Array(cells), structure = new Uint8Array(cells), transport = new Uint8Array(cells);
  for (let i = 0; i < cells; i++) {
    land[i] = next() % 3 === 0 ? cube_types.land.bare : cube_types.land.green_light;
    const r = next() % 50;
    water[i] = r === 0 ? cube_types.water.river : 0;
    structure[i] = r === 1 ? cube_types.structure.building : 0;
    transport[i] = r === 2 ? cube_types.transport.road_motorway : 0;
  }
  return encode_terrain(ele, { land, water, structure, transport });
}

/** The heap's row (xpute's BuddyMalloc::stat): base, top reached, end and
 * live as words, then releases and requests. */
const HEAP_TOP = 1;
const HEAP_REQUESTS = 5;
const heap_row = new Uint32Array(CORE_KERNEL.memory.buffer, boot_block(CORE_KERNEL)[BOOT.heap_row], 6);
let strobe_requests = 0;
const answer_requests: Record<string, number> = {};

/** A response as the driver reports one: the body held for the kernel. */
function answer(f: Fetched, status: number, body: Uint8Array | null): void {
  const requests = heap_row[HEAP_REQUESTS];
  try {
    const bytes = body?.byteLength ?? 0;
    const held = body !== null && bytes > 0 ? hold_body(body) : 0;
    CORE_BUILD.send(0, Cmd.FETCH_UPDATE, packet((b) => b.u64(f.key).u32(0).u32(status).u32(held).u32(bytes).u32(0).f64(0)));
    CORE_BUILD.ring(0);
  } finally {
    answer_requests[f.category] = (answer_requests[f.category] ?? 0) + heap_row[HEAP_REQUESTS] - requests;
  }
}

function fail(f: Fetched): void {
  CORE_BUILD.send(0, Cmd.FETCH_UPDATE, packet((b) => b.u64(f.key).u32(2).u32(0).u32(0).u32(0).u32(0).f64(0)));
  CORE_BUILD.ring(0);
}

/** What a search is answered with; with none, a search fails. */
let search_body: Uint8Array | null = null;

/** One frame as the driver runs it, with every fetch answered at once. */
function turn(delta = 1 / 60): Turn {
  fetched = [];
  loads = [];
  const requests = heap_row[HEAP_REQUESTS];
  CORE_BUILD.send(0, Cmd.WORLD_UPDATE, packet((b) => b.f64(1280).f64(720).f64(1).f64(delta).bool(false).bool(false).bool(false).f64(1000 / 60).bool(true).u32(6)));
  CORE_BUILD.ring(8);
  strobe_requests += heap_row[HEAP_REQUESTS] - requests;
  const started = fetched;
  for (const f of started) {
    if (f.category === "terrain") answer(f, 200, terrain_body(Number(BigInt(f.url.slice(f.url.lastIndexOf("/") + 1)) & 0xffffn)));
    else if (f.category === "poi") answer(f, 200, encode_poi_page([]));
    else if (f.category === "geocode") answer(f, 404, null);
    else if (f.category === "search" && search_body !== null) answer(f, 200, search_body);
    else fail(f);
  }
  for (const url of loads) CORE_BUILD.send(0, Cmd.LOAD_DELETE, packet((b) => b.str(url)));
  assert(snapshot !== null, "the kernel posted its snapshot");
  return { snapshot, fetched: started, draws };
}

function action(a: Record<string, unknown>): void {
  CORE_BUILD.send(0, Cmd.WORLD_ACTION, packet((b) => b.str(JSON.stringify(a))));
  CORE_BUILD.ring(0);
}

CORE_BUILD.call(
  Cmd.WORLD_CREATE,
  packet((b) => b.str("test").str("2026-09-15T00:00:00Z").str("r0-test").str("b?").bool(false).bool(false)),
  (r) => r,
);
console.log(`heap: boot and world create ${heap_row[HEAP_REQUESTS]} requests, top ${heap_row[HEAP_TOP]}`);
turn();
console.log(`heap: through the first turn ${heap_row[HEAP_REQUESTS]} requests, top ${heap_row[HEAP_TOP]}`);

Deno.test("kernel - a map centre fetches its terrain and draws it, and the map settles to success", () => {
  action({ type: "go_to", lon: 126.978, lat: 37.5665 });
  let first = turn();
  assert(first.snapshot.map.has_center, "the centre is set");
  const statuses = new Set<string>([first.snapshot.map.status]);
  const categories = new Set(first.fetched.map((f) => f.category));
  let terrain_urls = first.fetched.filter((f) => f.category === "terrain").map((f) => f.url);
  let max_draws = first.draws;
  for (let i = 0; i < 400; i++) {
    const t = turn();
    statuses.add(t.snapshot.map.status);
    for (const f of t.fetched) categories.add(f.category);
    terrain_urls = terrain_urls.concat(t.fetched.filter((f) => f.category === "terrain").map((f) => f.url));
    max_draws = Math.max(max_draws, t.draws);
    first = t;
  }
  assert(categories.has("terrain"), `terrain was asked for: ${[...categories]}`);
  assert(terrain_urls.every((u) => /\/\d+$/.test(u)), `terrain URLs end in a page: ${terrain_urls.slice(0, 3)}`);
  assert(max_draws > 0, "something was drawn");
  assert(statuses.has("loading"), `the map loaded: ${[...statuses]}`);
  assertEquals(first.snapshot.map.status, "success");
});

Deno.test("kernel - the settings actions reach the snapshot", () => {
  const before = turn().snapshot.settings.show_poi;
  action({ type: "toggle_setting", key: "show_poi" });
  assertEquals(turn().snapshot.settings.show_poi, !before);
  action({ type: "set_setting", key: "show_poi", value: before });
  assertEquals(turn().snapshot.settings.show_poi, before);
});

Deno.test("kernel - a search query becomes one search demand after its debounce, with the query and the centre in its URL", () => {
  action({ type: "search_query", query: "city hall" });
  assert(turn().snapshot.search.loading);
  const start = performance.now();
  let demand: Fetched | undefined;
  while (!demand && performance.now() - start < 2000) {
    demand = turn().fetched.find((f) => f.category === "search");
  }
  assert(demand, "the search was asked for");
  assertEquals(demand.key, key_of(SEARCH_CATEGORY, fnv1a64_str("city hall")), "the query stands in the key as its hash");
  assert(demand.url.includes("q=city+hall"), demand.url);
  assert(demand.url.includes("near="), demand.url);
  action({ type: "search_close" });
});

Deno.test("kernel - a settled turn asks the module heap for nothing", () => {
  for (let i = 0; i < 200; i++) turn();
  const [top, requests] = [heap_row[HEAP_TOP], heap_row[HEAP_REQUESTS]];
  for (let i = 0; i < 100; i++) turn();
  assertEquals([heap_row[HEAP_REQUESTS] - requests, heap_row[HEAP_TOP] - top], [0, 0], "a hundred settled turns: no request, and the heap's top where it was");
});

Deno.test("kernel - heap requests along the event paths", () => {
  for (let i = 0; i < 50; i++) turn();
  let mark = heap_row[HEAP_REQUESTS];
  const since = () => {
    const n = heap_row[HEAP_REQUESTS] - mark;
    mark = heap_row[HEAP_REQUESTS];
    return n;
  };
  action({ type: "search_query", query: "gangnam station" });
  const typed = since();
  search_body = new TextEncoder().encode(JSON.stringify([
    { gers_id: "g1", kind: "poi", name: "Gangnam Station", category: "train_station", context: [{ name: "서울" }, { name: "강남구", common: { en: "Gangnam-gu" } }], lon: 127.02, lat: 37.49 },
  ]));
  const start = performance.now();
  let asked = false;
  while (!asked && performance.now() - start < 2000) asked = turn().fetched.some((f) => f.category === "search");
  assert(asked, "the search was asked for");
  const answered = since();
  const shown = turn().snapshot.search.results.length;
  const after = since();
  action({ type: "search_close" });
  search_body = null;
  turn();
  since();
  strobe_requests = 0;
  for (const k of Object.keys(answer_requests)) delete answer_requests[k];
  action({ type: "go_to", lon: 127.3, lat: 36.35 });
  let load = 0;
  for (let i = 0; i < 200; i++) turn();
  load = since();
  console.log(`heap: typed ${typed}, fetched and answered ${answered}, the turn showing ${shown} ${after}, a far jump and its load ${load}`);
  console.log(`heap: the load's strobes ${strobe_requests}, its answers ${JSON.stringify(answer_requests)}`);
});
