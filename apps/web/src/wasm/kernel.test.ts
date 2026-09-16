// @pxl/web/wasm/kernel.test.ts

/**
 * The kernel driven the way wasm/kernel.ts drives it, without a page: the
 * boot, a map centre, strobes of the world's turn, the fetches it starts
 * answered from fixtures through the room it gives their bodies, and what
 * comes back — chunks mounted and drawn, the map's status, the UI's snapshot
 * and its actions.
 */

import { assert, assertEquals } from "@std/assert";

import type { u32 } from "@xpute/core/kit/word.ts";
import { fnv1a64_str } from "@xpute/core/kit/hash.ts";
import { Cmd } from "@pxl/shared/spec/cmd.ts";
import { ELE_PADDED_SPAN, PAGE_CELL_SPAN } from "@pxl/shared/bake/constant.ts";
import { encode_poi_page, encode_terrain } from "@pxl/shared/bake/format.ts";
import { cube_types } from "@pxl/shared/spec/cell/cube.ts";

import { CORE_BUILD, packet } from "@/wasm/core_build.ts";
import { CORE_KERNEL } from "@/wasm/core_mem.ts";
import { BOOT, boot_block } from "@/wasm/module.ts";

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

type Fetched = { key: string; url: string };

type Turn = { snapshot: Snapshot; fetched: Fetched[]; draws: number };

let snapshot: Snapshot | null = null;
let fetched: Fetched[] = [];
let loads: string[] = [];
let draws = 0;

CORE_BUILD.shell = (_tag, cmd, f) => {
  if (cmd === Cmd.WORLD_SNAPSHOT) snapshot = JSON.parse(f!.at(0).get<string>()) as Snapshot;
  else if (cmd === Cmd.WORLD_UPDATE) draws = f!.at(3).get<number>();
  else if (cmd === Cmd.FETCH_CREATE) {
    const [key, url] = f!.get<[string, string]>();
    fetched.push({ key, url });
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

/** A response as the driver reports one: the body written into the room the
 * kernel gives it. */
const heap_row = new Uint32Array(CORE_KERNEL.memory.buffer, boot_block(CORE_KERNEL)[BOOT.heap_row], 2);
let strobe_requests = 0;
const answer_requests: Record<string, number> = {};

function answer(key: string, status: number, body: Uint8Array | null): void {
  const requests = heap_row[1];
  try {
    answer_once(key, status, body);
  } finally {
    const category = key.slice(0, key.indexOf(":"));
    answer_requests[category] = (answer_requests[category] ?? 0) + heap_row[1] - requests;
  }
}

function answer_once(key: string, status: number, body: Uint8Array | null): void {
  let at = 0;
  const bytes = body?.byteLength ?? 0;
  if (body !== null && bytes > 0) {
    at = CORE_BUILD.call(Cmd.FETCH_BODY, packet((b) => b.str(key).u32(bytes)), (r) => r >>> 0);
    assert(at !== 0, `room for ${key}'s body`);
    new Uint8Array(CORE_KERNEL.memory.buffer, at, bytes).set(body);
  }
  CORE_BUILD.send(0, Cmd.FETCH_UPDATE, packet((b) => b.str(key).u32(0).u32(status).u32(at).u32(at === 0 ? 0 : bytes).str(null).f64(0)));
  CORE_BUILD.settle();
}

function fail(key: string): void {
  CORE_BUILD.send(0, Cmd.FETCH_UPDATE, packet((b) => b.str(key).u32(2).u32(0).u32(0).u32(0).str("test").f64(0)));
  CORE_BUILD.settle();
}

/** What a search is answered with; with none, a search fails. */
let search_body: Uint8Array | null = null;

/** One frame as the driver runs it, with every fetch answered at once. */
function turn(delta = 1 / 60): Turn {
  fetched = [];
  loads = [];
  const requests = heap_row[1];
  CORE_BUILD.strobe(Cmd.WORLD_UPDATE, packet((b) => b.f64(1280).f64(720).f64(1).f64(delta).bool(false).bool(false).bool(false).f64(8).f64(12).f64(1000 / 60).u32(6)));
  strobe_requests += heap_row[1] - requests;
  const started = fetched;
  for (const f of started) {
    const category = f.key.slice(0, f.key.indexOf(":"));
    const subject = f.key.slice(f.key.indexOf(":") + 1);
    if (category === "terrain") answer(f.key, 200, terrain_body(Number(BigInt(subject) & 0xffffn)));
    else if (category === "poi") answer(f.key, 200, encode_poi_page([]));
    else if (category === "geocode") answer(f.key, 404, null);
    else if (category === "search" && search_body !== null) answer(f.key, 200, search_body);
    else fail(f.key);
  }
  for (const url of loads) CORE_BUILD.send(0, Cmd.LOAD_DELETE, packet((b) => b.str(url)));
  assert(snapshot !== null, "the kernel posted its snapshot");
  return { snapshot, fetched: started, draws };
}

function action(a: Record<string, unknown>): boolean {
  return CORE_BUILD.call(Cmd.WORLD_ACTION, packet((b) => b.str(JSON.stringify(a))), (r) => r === 1);
}

CORE_BUILD.call(
  Cmd.WORLD_CREATE,
  packet((b) => b.str("test").str("2026-09-15T00:00:00Z").str("r0-test").str("b?").bool(false).bool(false)),
  (r) => r,
);
console.log(`heap: boot and world create ${heap_row[1]} requests, top ${heap_row[0]}`);
// deno-lint-ignore no-explicit-any
(CORE_KERNEL as any).probe_reset();
turn();
console.log(`heap: through the first turn ${heap_row[1]} requests, top ${heap_row[0]}`);
console.log(`fixed pool: ${CORE_BUILD.call(Cmd.POOL_READ, packet((p) => p.u32(0)), (at) => Array.from(new Uint32Array(CORE_KERNEL.memory.buffer, at >>> 0, 12)))}`);
{
  // deno-lint-ignore no-explicit-any
  const at = (CORE_KERNEL as any).probe_dump() >>> 0;
  const len = new Uint32Array(CORE_KERNEL.memory.buffer, at, 1)[0];
  console.log(new TextDecoder().decode(new Uint8Array(CORE_KERNEL.memory.buffer, at + 4, len)));
}

Deno.test("kernel - a map centre fetches its terrain, mounts its chunks and draws them, and the map settles to success", () => {
  assertEquals(action({ type: "go_to", lon: 126.978, lat: 37.5665 }), true);
  let first = turn();
  assert(first.snapshot.map.has_center, "the centre is set");
  const statuses = new Set<string>([first.snapshot.map.status]);
  const categories = new Set(first.fetched.map((f) => f.key.slice(0, f.key.indexOf(":"))));
  let terrain_urls = first.fetched.filter((f) => f.key.startsWith("terrain:")).map((f) => f.url);
  let max_draws = first.draws;
  for (let i = 0; i < 400; i++) {
    const t = turn();
    statuses.add(t.snapshot.map.status);
    for (const f of t.fetched) categories.add(f.key.slice(0, f.key.indexOf(":")));
    terrain_urls = terrain_urls.concat(t.fetched.filter((f) => f.key.startsWith("terrain:")).map((f) => f.url));
    max_draws = Math.max(max_draws, t.draws);
    first = t;
  }
  assert(categories.has("terrain"), `terrain was asked for: ${[...categories]}`);
  assert(terrain_urls.every((u) => /\/\d+$/.test(u)), `terrain URLs end in a page: ${terrain_urls.slice(0, 3)}`);
  const chunks = CORE_BUILD.call(Cmd.CHUNK_READ, null, (_, reply) => new Float64Array(CORE_KERNEL.memory.buffer, reply!.at(0).get<number>(), 1)[0]);
  assert(chunks > 0, "chunks mounted");
  assert(max_draws > 0, "something was drawn");
  assert(statuses.has("loading"), `the map loaded: ${[...statuses]}`);
  assertEquals(first.snapshot.map.status, "success");
});

Deno.test("kernel - the settings actions reach the snapshot, and an action that does not read is refused", () => {
  const before = turn().snapshot.settings.show_poi;
  assertEquals(action({ type: "toggle_setting", key: "show_poi" }), true);
  assertEquals(turn().snapshot.settings.show_poi, !before);
  assertEquals(action({ type: "set_setting", key: "show_poi", value: before }), true);
  assertEquals(turn().snapshot.settings.show_poi, before);
  assertEquals(action({ type: "no_such_action" }), false);
  assertEquals(action({ type: "set_object_size" }), false);
});

Deno.test("kernel - a search query becomes one search demand after its debounce, with the query and the centre in its URL", () => {
  assertEquals(action({ type: "search_query", query: "city hall" }), true);
  assert(turn().snapshot.search.loading);
  const start = performance.now();
  let demand: Fetched | undefined;
  while (!demand && performance.now() - start < 2000) {
    demand = turn().fetched.find((f) => f.key.startsWith("search:"));
  }
  assert(demand, "the search was asked for");
  assertEquals(demand.key, `search:${fnv1a64_str("city hall").toString(16).padStart(16, "0")}`, "the query stands in the key as its hash");
  assert(demand.url.includes("q=city+hall"), demand.url);
  assert(demand.url.includes("near="), demand.url);
  assertEquals(action({ type: "search_close" }), true);
});

Deno.test("kernel - a fetched page's body is the kernel's: its room comes back to the page section once the page is resident", () => {
  const stat = () => CORE_BUILD.call(Cmd.POOL_READ, packet((p) => p.u32(1)), (at) => new Uint32Array(CORE_KERNEL.memory.buffer, at >>> 0, 12).slice());
  const body = terrain_body(7);
  const before = stat();
  const at = CORE_BUILD.call(Cmd.FETCH_BODY, packet((b) => b.str("terrain:1").u32(body.byteLength)), (r) => r >>> 0);
  assert(at !== 0, "room for a body");
  assert(stat().some((v, i) => v !== before[i]), "the room is the page section's");
  CORE_BUILD.send(0, Cmd.FETCH_UPDATE, packet((b) => b.str("terrain:1").u32(1).u32(0).u32(at).u32(body.byteLength).str(null).f64(0)));
  CORE_BUILD.settle();
  assertEquals(CORE_BUILD.call(Cmd.FETCH_BODY, packet((b) => b.str("terrain:1").u32(body.byteLength)), (r) => r >>> 0), at, "an aborted body's room is given back, and given again");
  CORE_BUILD.send(0, Cmd.FETCH_UPDATE, packet((b) => b.str("terrain:1").u32(1).u32(0).u32(at).u32(body.byteLength).str(null).f64(0)));
  CORE_BUILD.settle();
});

Deno.test("kernel - a settled turn asks the module heap for nothing", () => {
  const row = new Uint32Array(CORE_KERNEL.memory.buffer, boot_block(CORE_KERNEL)[BOOT.heap_row], 2);
  for (let i = 0; i < 200; i++) turn();
  const [top, requests] = [row[0], row[1]];
  for (let i = 0; i < 100; i++) turn();
  assertEquals([row[1] - requests, row[0] - top], [0, 0], "a hundred settled turns: no request, and the heap's top where it was");
});

Deno.test("kernel - heap requests along the event paths", () => {
  const row = new Uint32Array(CORE_KERNEL.memory.buffer, boot_block(CORE_KERNEL)[BOOT.heap_row], 2);
  for (let i = 0; i < 50; i++) turn();
  let mark = row[1];
  const since = () => {
    const n = row[1] - mark;
    mark = row[1];
    return n;
  };
  assertEquals(action({ type: "search_query", query: "gangnam station" }), true);
  const typed = since();
  search_body = new TextEncoder().encode(JSON.stringify([
    { gers_id: "g1", kind: "poi", name: "Gangnam Station", category: "train_station", context: [{ name: "서울" }, { name: "강남구", common: { en: "Gangnam-gu" } }], lon: 127.02, lat: 37.49 },
  ]));
  const start = performance.now();
  let asked = false;
  while (!asked && performance.now() - start < 2000) asked = turn().fetched.some((f) => f.key.startsWith("search:"));
  assert(asked, "the search was asked for");
  const answered = since();
  const shown = turn().snapshot.search.results.length;
  const after = since();
  assertEquals(action({ type: "search_close" }), true);
  search_body = null;
  turn();
  since();
  strobe_requests = 0;
  for (const k of Object.keys(answer_requests)) delete answer_requests[k];
  // deno-lint-ignore no-explicit-any
  const k = CORE_KERNEL as any;
  k.probe_reset();
  assertEquals(action({ type: "go_to", lon: 127.3, lat: 36.35 }), true);
  let load = 0;
  for (let i = 0; i < 200; i++) turn();
  load = since();
  console.log(`heap: typed ${typed}, fetched and answered ${answered}, the turn showing ${shown} ${after}, a far jump and its load ${load}`);
  console.log(`heap: the load's strobes ${strobe_requests}, its answers ${JSON.stringify(answer_requests)}`);
  const at = k.probe_dump() >>> 0;
  const len = new Uint32Array(CORE_KERNEL.memory.buffer, at, 1)[0];
  console.log(new TextDecoder().decode(new Uint8Array(CORE_KERNEL.memory.buffer, at + 4, len)));
});
