// @pxl/web/wasm/kernel.ts

/**
 * The shell's driver of the kernel (crates/pxl/core behind crates/pxl/wasm):
 * the page's interrupts in, the kernel's asks out, and nothing decided here.
 *
 * Each frame is the frame's turn (WORLD_UPDATE) — the effects, then the
 * tick's phases — and then what the turn asks of the shell: toasts, the
 * sensors' watches, console lines, the UI's snapshot and the models' skins
 * (WORLD_READ); the fetches to start and cancel and the page blocks to give
 * back (FETCH_READ); the models and images to load (LOAD_READ); and the
 * renderer's frame, laid out from the kernel's lanes. What arrives later — a
 * response, a sensor fix, a pointer — is sent in and asks for the next frame.
 *
 * The UI is a display and an interrupt source: it renders `useKernel`'s
 * snapshot and applies `kernel_action`s.
 */

import { useLayoutEffect, useSyncExternalStore } from "react";
import i18next from "i18next";

import type { BranchCursor } from "@xpute/core/io/marshal/tree/mod.ts";
import type { Block } from "@xpute/core/ds/buddy.ts";
import { Cmd } from "@pxl/shared/spec/cmd.ts";
import { decode_terrain, type Terrain } from "@pxl/shared/bake/format.ts";

import { APP_ENV, BUILD_ID, BUILD_NUMBER, BUILD_TIME, PUBLISHED_TILES, TILE_BASE_URL } from "@/lib/env/config.ts";
import { HANDHELD } from "@/shared/lib/browser/handheld.ts";
import { CORE_BUILD, packet } from "@/wasm/core_build.ts";
import { CORE_SECTIONS, PAGE_POOL, PAGE_SECTION_BYTES } from "@/wasm/core_pool.ts";
import { type DecorModelRow, gpu_frame, type ModelSkinRow } from "@/render/gpu/stage.tsx";
import { KERNEL_SHEET } from "@/render/gpu/badge_sheet.ts";

// ---- the snapshot the UI renders (crates/pxl/core/src/app/shell.rs) ----

export type PoseStatus = "idle" | "loading" | "ready" | "error";
export type ActiveTool = "pointer" | "object" | "decor" | "eraser";
export type RecentPick = { kind: "object" | "decor"; id: string };
export type ChatLine = { id: number; text: string; ts: number };

export type UiSnapshot = {
  map: { has_center: boolean; center_lonlat: [number, number] | null; status: "idle" | "loading" | "success" | "error"; is_off_center: boolean };
  location: { status: PoseStatus; error_key: string | null; has_location: boolean };
  orientation: { status: PoseStatus; heading: number | null };
  view: { dist: number; azimuth: number; polar: number };
  is_follow_mode: boolean;
  camera_busy: boolean;
  fog: string;
  ui: { active_tool: ActiveTool; object_color_id: string; object_size: number; decor_id: string; recent_picks: RecentPick[]; chat_lines: ChatLine[] };
  settings: Record<string, boolean | string>;
  companion: { person_ready: boolean; pet_ready: boolean };
  address: { label: string | null; loading: boolean };
  scale_bar: { total_meters: number; bar_width_px: number; is_default_zoom: boolean; label: string };
  compass: { show: boolean };
  previews: { id: number; model: number; skin: number }[];
  object_storage_version: number;
  report: string | null;
};

const INITIAL: UiSnapshot = {
  map: { has_center: false, center_lonlat: null, status: "idle", is_off_center: false },
  location: { status: "idle", error_key: null, has_location: false },
  orientation: { status: "idle", heading: null },
  view: { dist: 0, azimuth: 0, polar: 0 },
  is_follow_mode: false,
  camera_busy: false,
  fog: "#000000",
  ui: { active_tool: "pointer", object_color_id: "red", object_size: 1, decor_id: "tree", recent_picks: [], chat_lines: [] },
  settings: {},
  companion: { person_ready: false, pet_ready: false },
  address: { label: null, loading: false },
  scale_bar: { total_meters: 0, bar_width_px: 0, is_default_zoom: true, label: "" },
  compass: { show: false },
  previews: [],
  object_storage_version: 0,
  report: null,
};

let snapshot_text = "";
let snapshot: UiSnapshot = INITIAL;
const snapshot_listeners = new Set<() => void>();

function set_snapshot(text: string): void {
  if (text === snapshot_text) return;
  snapshot_text = text;
  snapshot = JSON.parse(text) as UiSnapshot;
  if (snapshot.report !== null) void print_report(snapshot.report);
  for (const listener of snapshot_listeners) listener();
}

async function print_report(text: string): Promise<void> {
  console.log(text);
  let copied = false;
  try {
    await navigator.clipboard.writeText(text);
    copied = true;
  } catch {
    // The clipboard can refuse without a user gesture or on an insecure
    // origin; the console print above is the fallback.
  }
  console.log(`[ledger] report ${copied ? "copied to clipboard" : "printed above (clipboard refused)"} — ${text.length} bytes`);
}

function subscribe_snapshot(listener: () => void): () => void {
  snapshot_listeners.add(listener);
  return () => snapshot_listeners.delete(listener);
}

export function kernel_snapshot(): UiSnapshot {
  return snapshot;
}

/** The part of the kernel's snapshot a component shows. */
export function useKernel<T>(select: (s: UiSnapshot) => T): T {
  return select(useSyncExternalStore(subscribe_snapshot, kernel_snapshot));
}

export type KernelAction = { type: string; [key: string]: unknown };

/** Applies one UI action in the kernel, and reads what it asked of the shell
 * at once — a sensor permission has to be asked inside the gesture that led
 * to it. Whether the action read. */
export function kernel_action(action: KernelAction): boolean {
  const applied = CORE_BUILD.call(Cmd.WORLD_ACTION, packet((b) => b.str(JSON.stringify(action))), (result) => result === 1);
  read_world();
  request_frame();
  return applied;
}

// ---- toasts ----

export type ToastEvent = { show: true; id: string; message_key: string; tone: number } | { show: false; id: string };

const toast_listeners = new Set<(e: ToastEvent) => void>();
const toast_backlog: ToastEvent[] = [];

/** Hears the kernel's toasts; the ones raised before anyone listened first. */
export function on_toast(listener: (e: ToastEvent) => void): () => void {
  toast_listeners.add(listener);
  for (const e of toast_backlog.splice(0)) listener(e);
  return () => toast_listeners.delete(listener);
}

function emit_toast(e: ToastEvent): void {
  if (toast_listeners.size === 0) toast_backlog.push(e);
  for (const listener of toast_listeners) listener(e);
}

// ---- the turn ----

type WorldTurn = { draw: boolean; decor: DecorModelRow[]; skins: ModelSkinRow[] };

let last_turn: WorldTurn = { draw: false, decor: [], skins: [] };

function read_world(): WorldTurn {
  last_turn = CORE_BUILD.call(Cmd.WORLD_READ, null, (_, reply) => {
    const [toasts, geo, orientation, log, draw, ui, decor, skins] = reply!.get<[BranchCursor, BranchCursor, BranchCursor, BranchCursor, boolean, string, BranchCursor, BranchCursor]>();
    for (const t of toasts.get<BranchCursor[]>()) {
      const [show, id, message_key, tone] = t.get<[boolean, string, string, number]>();
      emit_toast(show ? { show: true, id, message_key, tone } : { show: false, id });
    }
    for (const c of geo.get<number[]>()) geolocation_command(c);
    for (const c of orientation.get<number[]>()) orientation_command(c);
    for (const line of log.get<string[]>()) console.log(line);
    set_snapshot(ui);
    return {
      draw,
      decor: decor.get<BranchCursor[]>().map((r) => {
        const [group, mesh, skin] = r.get<[number, number, number]>();
        return { group, mesh, skin };
      }),
      skins: skins.get<BranchCursor[]>().map((r) => {
        const [model, skin] = r.get<[number, number]>();
        return { model, skin };
      }),
    };
  });
  return last_turn;
}

type Driver = { element: HTMLElement; last: number };

let driver: Driver | null = null;
let pending = 0;

/** Asks for a frame: the next animation frame runs the kernel's turn. */
export function request_frame(): void {
  if (pending || driver === null) return;
  pending = requestAnimationFrame(frame);
}

type IOSOrientationEvent = DeviceOrientationEvent & { webkitCompassHeading?: number };
type IOSOrientationEventCtor = typeof DeviceOrientationEvent & { requestPermission?: () => Promise<"granted" | "denied"> };

function orientation_supported(): boolean {
  return typeof DeviceOrientationEvent !== "undefined";
}

function orientation_needs_permission(): boolean {
  return orientation_supported() && typeof (DeviceOrientationEvent as IOSOrientationEventCtor).requestPermission === "function";
}

/** The canvas's resolution: the device's pixels, clamped to [1, 2]. */
function dpr(): number {
  return Math.min(2, Math.max(1, globalThis.devicePixelRatio || 1));
}

function frame(now: number): void {
  pending = 0;
  const d = driver;
  if (d === null) return;
  const delta = Math.min(0.1, Math.max(0, (now - d.last) / 1000));
  d.last = now;
  const width = Math.max(1, d.element.clientWidth);
  const height = Math.max(1, d.element.clientHeight);
  const asked = CORE_BUILD.call(
    Cmd.WORLD_UPDATE,
    packet((b) => b.f64(width).f64(height).f64(dpr()).f64(delta).bool("geolocation" in navigator).bool(orientation_supported()).bool(orientation_needs_permission())),
    (result) => result === 1,
  );
  const turn = read_world();
  run_fetches();
  run_loads();
  gpu_frame(dpr(), KERNEL_SHEET, turn.decor, turn.skins, image_of);
  if (asked || turn.draw) request_frame();
}

// ---- fetches (crates/pxl/core/src/module/world/sched/fetch_scheduler.rs) ----

const running = new Map<string, AbortController>();

type TerrainAt = { lanes: [number, number, number, number, number]; blocks: Block[] };

function copy_lane(src: ArrayBufferView, blocks: Block[]): number {
  const block = PAGE_POOL.acquire(src.byteLength);
  blocks.push(block);
  new Uint8Array(block.buf, block.byte_offset, src.byteLength).set(new Uint8Array(src.buffer, src.byteOffset, src.byteLength));
  return block.byte_offset;
}

/** A decoded page copied into the page section, lane by lane: the response
 * is dropped and one memory holds the kernel's data. */
function copy_terrain(src: Terrain): TerrainAt {
  const blocks: Block[] = [];
  const g = src.ground;
  const lanes: TerrainAt["lanes"] = [copy_lane(g.land, blocks), copy_lane(g.water, blocks), copy_lane(g.structure, blocks), copy_lane(g.transport, blocks), copy_lane(src.ele, blocks)];
  return { lanes, blocks };
}

function send_terrain(page: bigint, t: TerrainAt, own: boolean): void {
  const [land, water, structure, transport, ele] = t.lanes;
  CORE_BUILD.send(
    0,
    Cmd.PAGE_TERRAIN,
    packet((b) =>
      b.u64(BigInt.asUintN(64, page)).u32(land).u32(water).u32(structure).u32(transport).u32(ele)
        .branch((bs) => {
          for (const block of t.blocks) bs.branch((e) => e.u32(block.byte_offset).u32(block.bytes));
        })
        .bool(own)
    ),
  );
}

/** The one shared open-ocean page: a 404 from the published bake means
 * "nothing but water here", and every such page aliases one copy that no page
 * owns. A failure is not cached, so a transient error does not poison every
 * ocean page for the session. */
let ocean: Promise<TerrainAt> | null = null;

function ocean_terrain(): Promise<TerrainAt> {
  ocean ??= fetch(`${TILE_BASE_URL}/_ocean`)
    .then(async (res) => {
      if (!res.ok) throw new Error(`[tile/_ocean] http ${res.status}`);
      return copy_terrain(decode_terrain(new Uint8Array(await res.arrayBuffer())));
    })
    .catch((err) => {
      ocean = null;
      throw err;
    });
  return ocean;
}

function release_blocks(blocks: readonly Block[]): void {
  for (const block of blocks) PAGE_POOL.release(block);
}

async function run_demand(key: string, category: string, url: string): Promise<void> {
  const controller = new AbortController();
  running.set(key, controller);
  const signal = controller.signal;
  const page = BigInt(key.slice(key.indexOf(":") + 1));
  const phases: [string, number][] = [];
  let outcome = 0;
  let error: string | null = null;
  try {
    const t0 = performance.now();
    const res = await fetch(url, { signal });
    if (category === "terrain") {
      if (res.status === 404 && PUBLISHED_TILES) {
        const t = await ocean_terrain();
        send_terrain(page, { lanes: t.lanes, blocks: [] }, false);
      } else {
        if (!res.ok) throw new Error(`[tile/${page}] http ${res.status}`);
        const bytes = new Uint8Array(await res.arrayBuffer());
        const t1 = performance.now();
        const decoded = decode_terrain(bytes);
        const t2 = performance.now();
        const t = copy_terrain(decoded);
        phases.push(["net_ms", t1 - t0], ["decode_ms", t2 - t1], ["adopt_ms", performance.now() - t2], ["bytes", bytes.byteLength]);
        if (signal.aborted) release_blocks(t.blocks);
        else send_terrain(page, t, true);
      }
    } else if (category === "poi") {
      if (res.status === 404 && PUBLISHED_TILES) {
        CORE_BUILD.send(0, Cmd.PAGE_POI, packet((b) => b.u64(BigInt.asUintN(64, page)).branch(null)));
      } else {
        if (!res.ok) throw new Error(`[poi/${page}] http ${res.status}`);
        const bytes = new Uint8Array(await res.arrayBuffer());
        const t1 = performance.now();
        const block = PAGE_POOL.acquire(bytes.byteLength);
        new Uint8Array(block.buf, block.byte_offset, bytes.byteLength).set(bytes);
        phases.push(["net_ms", t1 - t0], ["parse_ms", performance.now() - t1], ["bytes", bytes.byteLength]);
        if (signal.aborted) release_blocks([block]);
        else CORE_BUILD.send(0, Cmd.PAGE_POI, packet((b) => b.u64(BigInt.asUintN(64, page)).branch((e) => e.u32(block.byte_offset).u32(bytes.byteLength))));
      }
    } else if (category === "geocode") {
      if (res.status === 404) {
        CORE_BUILD.send(0, Cmd.FETCH_GEOCODE, packet((b) => b.u64(BigInt.asUintN(64, page)).str(null)));
      } else {
        if (!res.ok) throw new Error(`[geocode/${page}] http ${res.status}`);
        const body = await res.text();
        CORE_BUILD.send(0, Cmd.FETCH_GEOCODE, packet((b) => b.u64(BigInt.asUintN(64, page)).str(body)));
      }
    }
  } catch (err) {
    outcome = signal.aborted ? 1 : 2;
    if (!signal.aborted) error = err instanceof Error ? `${err.name}: ${err.message}` : String(err);
  } finally {
    running.delete(key);
    CORE_BUILD.send(
      0,
      Cmd.FETCH_UPDATE,
      packet((b) =>
        b.str(key).u32(outcome).str(error).branch((ps) => {
          for (const [name, value] of phases) ps.branch((p) => p.str(name).f64(value));
        })
      ),
    );
    request_frame();
  }
}

function run_fetches(): void {
  const started: [string, string, string][] = [];
  let aborts: string[] = [];
  CORE_BUILD.call(Cmd.FETCH_READ, null, (_, reply) => {
    const [s, a, r] = reply!.get<[BranchCursor, BranchCursor, BranchCursor]>();
    for (const row of s.get<BranchCursor[]>()) started.push(row.get<[string, string, string]>());
    aborts = a.get<string[]>();
    for (const row of r.get<BranchCursor[]>()) {
      const [at, bytes] = row.get<[number, number]>();
      PAGE_POOL.release(PAGE_POOL.adopt(at, bytes));
    }
  });
  for (const key of aborts) running.get(key)?.abort();
  for (const [key, category, url] of started) void run_demand(key, category, url);
}

// ---- loads (crates/pxl/core/src/render/gltf/load.rs) ----

const images = new Map<number, ImageBitmap>();
let next_image = 1;

/** An image the kernel names by the handle it was given. */
export function image_of(handle: number): ImageBitmap | null {
  return images.get(handle) ?? null;
}

async function load_model(url: string): Promise<void> {
  try {
    const res = await fetch(url);
    if (!res.ok) throw new Error(`gltf: ${url} ${res.status}`);
    const bytes = new Uint8Array(await res.arrayBuffer());
    CORE_BUILD.send(0, Cmd.LOAD_UPDATE, packet((b) => b.str(url).u8_array(bytes)));
  } catch (err) {
    console.error(`[kernel] ${err instanceof Error ? err.message : String(err)}`);
    CORE_BUILD.send(0, Cmd.LOAD_DELETE, packet((b) => b.str(url)));
  }
  request_frame();
}

/** A skin, as the renderer uploads it: the file's own pixels, top row first,
 * no colour conversion and no premultiplied alpha. */
async function load_image(url: string): Promise<void> {
  try {
    const res = await fetch(url);
    if (!res.ok) throw new Error(`image: ${url} ${res.status}`);
    const image = await createImageBitmap(await res.blob(), { colorSpaceConversion: "none", premultiplyAlpha: "none" });
    const handle = next_image++;
    images.set(handle, image);
    CORE_BUILD.send(0, Cmd.LOAD_IMAGE, packet((b) => b.str(url).u32(handle)));
  } catch (err) {
    console.error(`[kernel] ${err instanceof Error ? err.message : String(err)}`);
    CORE_BUILD.send(0, Cmd.LOAD_DELETE, packet((b) => b.str(url)));
  }
  request_frame();
}

function run_loads(): void {
  const requests = CORE_BUILD.call(Cmd.LOAD_READ, null, (_, reply) => reply!.get<[BranchCursor]>()[0].get<BranchCursor[]>().map((r) => r.get<[number, string]>()));
  for (const [kind, url] of requests) void (kind === 0 ? load_model(url) : load_image(url));
}

// ---- sensors (crates/pxl/core/src/module/device/pose) ----

let geolocation_watch: number | null = null;

function geolocation_command(c: number): void {
  if (c === 1) {
    if (geolocation_watch !== null) navigator.geolocation.clearWatch(geolocation_watch);
    geolocation_watch = null;
    return;
  }
  if (geolocation_watch !== null || !("geolocation" in navigator)) return;
  geolocation_watch = navigator.geolocation.watchPosition(
    (pos) => {
      const { latitude, longitude, altitude, accuracy, heading } = pos.coords;
      CORE_BUILD.send(0, Cmd.SENSOR_UPDATE, packet((b) => b.f64(longitude).f64(latitude).f64(altitude ?? NaN).f64(accuracy).f64(heading ?? NaN)));
      request_frame();
    },
    (err) => {
      // The dead watch is cleared, not just forgotten: it keeps calling back,
      // and a later start would race it.
      if (geolocation_watch !== null) navigator.geolocation.clearWatch(geolocation_watch);
      geolocation_watch = null;
      const code = err.code === err.PERMISSION_DENIED ? 1 : err.code === err.POSITION_UNAVAILABLE ? 2 : err.code === err.TIMEOUT ? 3 : 0;
      CORE_BUILD.send(0, Cmd.SENSOR_ERROR, packet((b) => b.u32(code)));
      request_frame();
    },
    { enableHighAccuracy: true, maximumAge: 5000, timeout: 30000 },
  );
}

let orientation_event: string | null = null;

function on_orientation(e: Event): void {
  const o = e as IOSOrientationEvent;
  CORE_BUILD.send(0, Cmd.SENSOR_ORIENTATION, packet((b) => b.f64(o.webkitCompassHeading ?? NaN).f64(o.alpha ?? NaN).f64(o.beta ?? NaN).f64(o.gamma ?? NaN)));
  request_frame();
}

function listen_orientation(name: string): void {
  if (orientation_event !== null) return;
  orientation_event = name;
  globalThis.addEventListener(name, on_orientation, true);
}

function orientation_command(c: number): void {
  if (c === 0) {
    listen_orientation("ondeviceorientationabsolute" in globalThis ? "deviceorientationabsolute" : "deviceorientation");
  } else if (c === 1) {
    (DeviceOrientationEvent as IOSOrientationEventCtor).requestPermission!().then(
      (permission) => {
        const granted = permission === "granted";
        if (granted) listen_orientation("deviceorientation");
        CORE_BUILD.send(0, Cmd.SENSOR_PERMISSION, packet((b) => b.u32(1).bool(granted).bool(false)));
        request_frame();
      },
      (err) => {
        console.error("[kernel] orientation permission error:", err);
        CORE_BUILD.send(0, Cmd.SENSOR_PERMISSION, packet((b) => b.u32(1).bool(false).bool(true)));
        request_frame();
      },
    );
  } else {
    if (orientation_event !== null) globalThis.removeEventListener(orientation_event, on_orientation, true);
    orientation_event = null;
  }
}

// ---- the page's interrupts ----

function send_rect(element: HTMLElement): void {
  const r = element.getBoundingClientRect();
  CORE_BUILD.send(0, Cmd.INPUT_UPDATE, packet((b) => b.f64(r.left).f64(r.top).f64(r.width).f64(r.height)));
}

function send_pointer(phase: number, e: PointerEvent): void {
  const kind = e.pointerType === "touch" ? 1 : e.pointerType === "pen" ? 2 : 0;
  CORE_BUILD.send(0, Cmd.INPUT_POINTER, packet((b) => b.u32(phase).u32(kind).i32(e.pointerId).i32(e.button).f64(e.clientX).f64(e.clientY)));
  request_frame();
}

/** Timers the kernel keeps — a debounce, a retry, a toast's dismissal — are
 * read on its turns, so a still view still gets one this often. */
const HEARTBEAT_MS = 250;

function send_language(language: string | undefined): void {
  if (!language) return;
  kernel_action({ type: "set_language", lang: language.split("-")[0].toLowerCase() });
}

/**
 * The one frame loop over `element`: the kernel's turns, the pointer's events
 * on it, the tab's visibility, the sensors' subscription and the language.
 */
export function KernelDriver({ element }: { element: HTMLElement | null }): null {
  useLayoutEffect(() => {
    if (!element) return;
    driver = { element, last: performance.now() };

    const on_down = (e: PointerEvent) => {
      send_rect(element);
      send_pointer(0, e);
    };
    const on_move = (e: PointerEvent) => send_pointer(1, e);
    const on_up = (e: PointerEvent) => send_pointer(2, e);
    const on_leave = (e: PointerEvent) => send_pointer(3, e);
    const on_wheel = (e: WheelEvent) => {
      if (e.ctrlKey) e.preventDefault();
      CORE_BUILD.send(0, Cmd.INPUT_WHEEL, packet((b) => b.f64(e.deltaY)));
      request_frame();
    };
    const on_context_menu = (e: MouseEvent) => e.preventDefault();
    const on_resize = () => {
      send_rect(element);
      request_frame();
    };
    const on_visibility = () => {
      CORE_BUILD.send(0, Cmd.SENSOR_VISIBILITY, packet((b) => b.bool(document.visibilityState === "visible")));
      request_frame();
    };
    const on_language = (language: string) => send_language(language);

    element.addEventListener("pointerdown", on_down);
    element.addEventListener("pointermove", on_move);
    element.addEventListener("pointerleave", on_leave);
    element.addEventListener("contextmenu", on_context_menu);
    element.addEventListener("wheel", on_wheel, { passive: false });
    globalThis.addEventListener("resize", on_resize);
    globalThis.addEventListener("scroll", on_resize, true);
    globalThis.addEventListener("pointerup", on_up);
    globalThis.addEventListener("pointercancel", on_up);
    document.addEventListener("visibilitychange", on_visibility);
    i18next.on("languageChanged", on_language);
    const observer = new ResizeObserver(on_resize);
    observer.observe(element);

    // Recovers from a denied state the moment the user grants location access
    // from outside the app, since nothing else would ask again.
    let permission: PermissionStatus | null = null;
    const on_permission = () => {
      CORE_BUILD.send(0, Cmd.SENSOR_PERMISSION, packet((b) => b.u32(0).bool(permission?.state === "granted").bool(false)));
      request_frame();
    };
    navigator.permissions?.query({ name: "geolocation" }).then((status) => {
      permission = status;
      status.addEventListener("change", on_permission);
    }).catch(() => {});

    send_rect(element);
    send_language(i18next.language);
    kernel_action({ type: "sensors_subscribe", geolocation: "geolocation" in navigator, orientation: orientation_supported(), needs_permission: orientation_needs_permission() });
    const heartbeat = setInterval(request_frame, HEARTBEAT_MS);
    request_frame();

    return () => {
      clearInterval(heartbeat);
      kernel_action({ type: "sensors_unsubscribe" });
      permission?.removeEventListener("change", on_permission);
      observer.disconnect();
      i18next.off("languageChanged", on_language);
      document.removeEventListener("visibilitychange", on_visibility);
      globalThis.removeEventListener("pointercancel", on_up);
      globalThis.removeEventListener("pointerup", on_up);
      globalThis.removeEventListener("scroll", on_resize, true);
      globalThis.removeEventListener("resize", on_resize);
      element.removeEventListener("wheel", on_wheel);
      element.removeEventListener("contextmenu", on_context_menu);
      element.removeEventListener("pointerleave", on_leave);
      element.removeEventListener("pointermove", on_move);
      element.removeEventListener("pointerdown", on_down);
      if (pending) cancelAnimationFrame(pending);
      pending = 0;
      driver = null;
    };
  }, [element]);
  return null;
}

// ---- boot ----

CORE_BUILD.call(
  Cmd.WORLD_CREATE,
  packet((b) => b.str(APP_ENV).str(BUILD_TIME).str(BUILD_ID).str(BUILD_NUMBER).bool(PUBLISHED_TILES).bool(HANDHELD).u32(PAGE_SECTION_BYTES).f64(CORE_SECTIONS.mesh.bytes)),
  (result) => result,
);
