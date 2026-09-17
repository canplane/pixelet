// @pxl/web/wasm/kernel.ts

/**
 * The central scheduler's driver of the kernel (crates/pxl/kernel behind
 * crates/pxl/wasm), and the devices it runs for it: nothing decided here.
 *
 * The turns are the strobe's (@xpute/core/sched/strobe.ts): a frame rings
 * the doorbell with a quota after submitting what the frame carries
 * (WORLD_UPDATE: the viewport, the gap, the frame interval as measured, the
 * I/O credits). In that turn the kernel applies what waits on the submission
 * ring, runs its tick, draws its canvases through the host's WebGPU
 * (wasm/gpu.ts), and leaves on the completion ring what it asks of the host —
 * the sensors' watches, console lines, the UI's snapshot when it
 * changed, the fetches to start and cancel, the loads — which the host reads
 * before the ring returns; the return is when the kernel wants its next turn.
 * What arrives later — a response, a sensor fix, a pointer — is submitted
 * and asks for the next frame.
 *
 * A canvas is the kernel's once handed over (`open_surface`): the page lays it
 * out, and nothing here draws in it.
 *
 * A fetch is HTTP and nothing more: the kernel names the URL, reads the body
 * the host holds in its own turn, and decides what the status and the bytes
 * mean.
 *
 * The UI is a display and an interrupt source: it renders `useKernel`'s
 * snapshot and applies `kernel_action`s.
 */

import { useLayoutEffect, useSyncExternalStore } from "react";

import type { u32 } from "@xpute/core/kit/word.ts";
import type { BranchCursor } from "@xpute/core/io/marshal/tree/mod.ts";
import { Strobe } from "@xpute/core/sched/strobe.ts";
import { Cmd } from "@pxl/shared/spec/cmd.ts";
import type { Message } from "@pxl/shared/spec/message.ts";
import { FetchFail } from "@pxl/shared/spec/fetch.ts";

import { APP_ENV, BUILD_ID, BUILD_NUMBER, BUILD_TIME, PUBLISHED_TILES } from "@/lib/env/config.ts";
import { HANDHELD } from "@/shared/lib/browser/handheld.ts";
import { CORE_BUILD, packet } from "@/wasm/core_build.ts";
import { CORE_KERNEL } from "@/wasm/core_mem.ts";
import { acquire_device, bind_surface, register_image, unbind_surface } from "@/wasm/gpu.ts";
import { boot_block, hold_body, quantum_policy, text_at } from "@/wasm/module.ts";

// ---- the snapshot the UI renders (crates/pxl/kernel/src/shell/shell.rs) ----

export type PoseStatus = "idle" | "loading" | "ready" | "error";
export type UiSnapshot = {
  /** `center` is the map centre as [lon, lat]. */
  map: { status: "idle" | "loading" | "success" | "error"; is_off_center: boolean; center: [number, number] | null };
  /** `error` is what went wrong, as @pxl/shared/spec/message's number. */
  location: { status: PoseStatus; has_location: boolean; error: Message | null };
  companion: { person_ready: boolean; pet_ready: boolean };
  orientation: { status: PoseStatus; heading: number | null };
  view: { azimuth: number };
  is_follow_mode: boolean;
  camera_busy: boolean;
  fog: string;
  settings: Record<string, boolean | string | number>;
  scale_bar: { total_meters: number; bar_width_px: number; is_default_zoom: boolean };
  compass: { show: boolean };
  object_storage_version: number;
};

const INITIAL: UiSnapshot = {
  map: { status: "idle", is_off_center: false, center: null },
  location: { status: "idle", has_location: false, error: null },
  companion: { person_ready: false, pet_ready: false },
  orientation: { status: "idle", heading: null },
  view: { azimuth: 0 },
  is_follow_mode: false,
  camera_busy: false,
  fog: "#000000",
  settings: {},
  scale_bar: { total_meters: 0, bar_width_px: 0, is_default_zoom: true },
  compass: { show: false },
  object_storage_version: 0,
};

let snapshot_text = "";
let snapshot: UiSnapshot = INITIAL;
const snapshot_listeners = new Set<() => void>();

function set_snapshot(text: string): void {
  if (text === snapshot_text) return;
  snapshot_text = text;
  snapshot = JSON.parse(text) as UiSnapshot;
  for (const listener of snapshot_listeners) listener();
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

/** Applies one UI action in the kernel, rung at once with no quota: what it
 * asks of the host is heard inside the call, including the sensors' close sent
 * after the strobe has stopped. */
export function kernel_action(action: KernelAction): void {
  CORE_BUILD.send(0, Cmd.WORLD_ACTION, packet((b) => b.str(JSON.stringify(action))));
  CORE_BUILD.ring(0);
  request_frame();
}

/** The tracking button's press: crates/pxl/kernel/src/shell/shell.rs's Tracking. */
export const TRACKING_RECENTER = 0, TRACKING_FOLLOW = 1, TRACKING_UNFOLLOW = 2;

/** The tracking button. Following needs the compass, so where iOS gates it,
 * the prompt runs here, inside the click, and the kernel hears the answer
 * before the press — the way a page's handler awaits `requestPermission` and
 * the browser only reports the outcome. */
export async function kernel_tracking(tracking: u32): Promise<void> {
  if (tracking === TRACKING_FOLLOW && !(await orientation_permit())) {
    console.error("[orientation] could not start the sensor.");
    return;
  }
  CORE_BUILD.send(0, Cmd.INPUT_TRACKING, packet((b) => b.u32(tracking)));
  strobe.input();
  request_frame();
}

/** The sensor button: the compass's prompt, where iOS gates it, is raised
 * synchronously in the click — Safari consumes the gesture's activation, so
 * one reached after an await can be refused — and the rest follows. */
export function kernel_enable_sensors(): void {
  void orientation_permit();
  kernel_action({ type: "enable_sensors" });
}

// ---- what the kernel asks: the completion ring's signals ----

/** crates/pxl/wasm/src/lib.rs's sensor numbers. */
const GEOLOCATION = 0;
const ORIENTATION = 1;

function on_signal(_tag: u32, cmd: Cmd, f: BranchCursor | null): void {
  switch (cmd) {
    case Cmd.WORLD_SNAPSHOT: {
      // The JSON is far past a ring frame, so the kernel names the block it
      // holds it in. It is the kernel's and stays valid until it makes the
      // next one, which is a turn away.
      const [at, len] = f!.get<[number, number]>();
      set_snapshot(text_at(CORE_KERNEL.memory, at >>> 0, len >>> 0));
      return;
    }
    case Cmd.WORLD_UPDATE:
      return;
    case Cmd.SENSOR_CREATE:
      if (f!.at(0).get<number>() === GEOLOCATION) geolocation_start();
      else orientation_listen();
      return;
    case Cmd.SENSOR_DELETE:
      if (f!.at(0).get<number>() === GEOLOCATION) geolocation_stop();
      else orientation_remove();
      return;
    case Cmd.FETCH_CREATE: {
      const [key, url] = f!.get<[bigint, string]>();
      void run_fetch(key, url);
      return;
    }
    case Cmd.FETCH_DELETE:
      running.get(f!.at(0).get<bigint>())?.abort();
      return;
    case Cmd.LOAD_CREATE: {
      const [kind, url] = f!.get<[number, string]>();
      void (kind === 0 ? load_model(url) : load_image(url));
      return;
    }
  }
  console.warn(`[kernel] signal 0x${cmd.toString(16)} with no one to hear it`);
}

CORE_BUILD.signal = on_signal;

// ---- the turn ----

let driver: HTMLElement | null = null;

/** Asks for a frame: the next animation frame strobes the kernel. */
export function request_frame(): void {
  strobe.request();
}

type IOSOrientationEvent = DeviceOrientationEvent & { webkitCompassHeading?: number };
type IOSOrientationEventCtor = typeof DeviceOrientationEvent & { requestPermission?: () => Promise<"granted" | "denied"> };

function orientation_supported(): boolean {
  return typeof DeviceOrientationEvent !== "undefined";
}

function orientation_needs_permission(): boolean {
  return orientation_supported() && typeof (DeviceOrientationEvent as IOSOrientationEventCtor).requestPermission === "function";
}

function dpr(): number {
  return globalThis.devicePixelRatio || 1;
}

/** Fetches the kernel may have in flight at once: a browser opens about six
 * connections to an origin, and matching that keeps the kernel's queue — not
 * the socket pool — as what orders the work. */
const NET_CREDITS = 6;

/** The central scheduler's turns for the kernel, sized by the quantum the
 * kernel declared at boot; each frame carries the viewport it runs in. */
const strobe = new Strobe(CORE_BUILD, quantum_policy(boot_block(CORE_KERNEL)), (grant, delta_s) => {
  const element = driver;
  if (element === null) return;
  const width = Math.max(1, element.clientWidth);
  const height = Math.max(1, element.clientHeight);
  CORE_BUILD.send(
    0,
    Cmd.WORLD_UPDATE,
    packet((b) => b.f64(width).f64(height).f64(dpr()).f64(delta_s).f64(grant.frame_ms).bool(grant.interactive).u32(NET_CREDITS)),
  );
}, (run) => requestAnimationFrame(run));

// ---- surfaces: the canvases the kernel draws (crates/pxl/render/src/gpu/surface.rs) ----

/** What a surface shows: surface.rs's ROLE_WORLD and ROLE_CASE. */
export const SURFACE_WORLD = 0;
export const SURFACE_CASE = 1;

export type OpenSurface = { handle: u32; close: () => void };

/** Hands `canvas` to the kernel to draw `role` into, and follows its layout
 * box in device pixels. The handle is 0 when the kernel has no surface left,
 * and then nothing is drawn in it.
 *
 * The box is the layout's, not the bounding rectangle: a rectangle includes
 * transforms, so a canvas mounted inside a drawer that is sliding or scaling in
 * would report a size it only passes through, and no resize follows a
 * transform. Where the observer reports device pixels they are exact; where it
 * does not (Safari), the CSS box times the pixel ratio stands in, and a zoom —
 * which changes the ratio and not the box — is caught by the window's resize. */
export function open_surface(canvas: HTMLCanvasElement, role: u32): OpenSurface {
  const handle = CORE_BUILD.call(Cmd.SURFACE_CREATE, packet((b) => b.u32(role)), (result) => result >>> 0);
  if (handle === 0) return { handle, close: () => {} };
  bind_surface(handle, canvas);
  const send = (width: number, height: number) => {
    CORE_BUILD.send(0, Cmd.SURFACE_UPDATE, packet((b) => b.u32(handle).u32(width).u32(height).f64(dpr())));
    request_frame();
  };
  const send_box = () => send(Math.round(canvas.clientWidth * dpr()), Math.round(canvas.clientHeight * dpr()));
  const observer = new ResizeObserver((entries) => {
    const entry = entries[entries.length - 1];
    const exact = entry.devicePixelContentBoxSize?.[0];
    if (exact) send(exact.inlineSize, exact.blockSize);
    else send_box();
  });
  try {
    observer.observe(canvas, { box: "device-pixel-content-box" });
  } catch {
    observer.observe(canvas);
  }
  globalThis.addEventListener("resize", send_box);
  return {
    handle,
    close: () => {
      globalThis.removeEventListener("resize", send_box);
      observer.disconnect();
      CORE_BUILD.call(Cmd.SURFACE_DELETE, packet((b) => b.u32(handle)), (result) => result);
      unbind_surface(handle);
    },
  };
}

// ---- fetches: HTTP for the kernel (crates/pxl/kernel/src/io/fetch.rs) ----

const running = new Map<bigint, AbortController>();

/** Runs one fetch the kernel started: the response's bytes written into the
 * room the kernel gives them, and the status reported. */
async function run_fetch(key: bigint, url: string): Promise<void> {
  const controller = new AbortController();
  running.set(key, controller);
  const signal = controller.signal;
  let outcome = 0;
  let status = 0;
  let held = 0;
  let bytes = 0;
  let fail: FetchFail = FetchFail.None;
  let net_ms = 0;
  try {
    const t0 = performance.now();
    const res = await fetch(url, { signal });
    const body = new Uint8Array(await res.arrayBuffer());
    net_ms = performance.now() - t0;
    status = res.status;
    if (body.byteLength > 0) {
      held = hold_body(body);
      bytes = body.byteLength;
    }
  } catch (err) {
    outcome = signal.aborted ? 1 : 2;
    // An abort is the scheduler's own doing and carries no reason. Everything
    // else is the transport's: a browser raises TypeError for offline, DNS,
    // CORS and a refused connection alike, and its words for them are this
    // side's to log — what the kernel needs is which of these happened.
    if (!signal.aborted) {
      fail = err instanceof Error && err.name === "TypeError" ? FetchFail.Network : FetchFail.Shell;
      console.warn(`[fetch] ${key}: ${err instanceof Error ? `${err.name}: ${err.message}` : String(err)}`);
    }
  } finally {
    running.delete(key);
    CORE_BUILD.send(0, Cmd.FETCH_UPDATE, packet((b) => b.u64(key).u32(outcome).u32(status).u32(held).u32(bytes).u32(fail).f64(net_ms)));
    request_frame();
  }
}

// ---- loads (crates/pxl/kernel/src/io/load.rs) ----

let next_image = 1;

async function load_model(url: string): Promise<void> {
  try {
    const res = await fetch(url);
    if (!res.ok) throw new Error(`gltf: ${url} ${res.status}`);
    const bytes = new Uint8Array(await res.arrayBuffer());
    // A model is far past a ring frame: the host holds it, and the kernel
    // reads it in its turn.
    CORE_BUILD.send(0, Cmd.LOAD_UPDATE, packet((b) => b.str(url).u32(hold_body(bytes)).u32(bytes.byteLength)));
  } catch (err) {
    console.error(`[kernel] ${err instanceof Error ? err.message : String(err)}`);
    CORE_BUILD.send(0, Cmd.LOAD_DELETE, packet((b) => b.str(url)));
  }
  request_frame();
}

/** An image, as the kernel's renderer uploads it: the file's own pixels, top
 * row first, no colour conversion and no premultiplied alpha. */
async function load_image(url: string): Promise<void> {
  try {
    const res = await fetch(url);
    if (!res.ok) throw new Error(`image: ${url} ${res.status}`);
    const image = await createImageBitmap(await res.blob(), { colorSpaceConversion: "none", premultiplyAlpha: "none" });
    const handle = next_image++;
    register_image(handle, image);
    CORE_BUILD.send(0, Cmd.LOAD_IMAGE, packet((b) => b.str(url).u32(handle)));
  } catch (err) {
    console.error(`[kernel] ${err instanceof Error ? err.message : String(err)}`);
    CORE_BUILD.send(0, Cmd.LOAD_DELETE, packet((b) => b.str(url)));
  }
  request_frame();
}

// ---- sensors (crates/pxl/kernel/src/module/device/pose) ----

let geolocation_watch: number | null = null;

function geolocation_stop(): void {
  if (geolocation_watch !== null) navigator.geolocation.clearWatch(geolocation_watch);
  geolocation_watch = null;
}

function geolocation_start(): void {
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
      geolocation_stop();
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

function orientation_listen(): void {
  listen_orientation("ondeviceorientationabsolute" in globalThis ? "deviceorientationabsolute" : "deviceorientation");
}

/** crates/pxl/kernel/src/control/sensor/store.rs's PermissionAnswer. */
const PERMISSION_ASKING = 0, PERMISSION_GRANTED = 1, PERMISSION_DENIED = 2, PERMISSION_FAILED = 3;

function send_permission(sensor: number, answer: number): void {
  CORE_BUILD.send(0, Cmd.SENSOR_PERMISSION, packet((b) => b.u32(sensor).u32(answer)));
  request_frame();
}

/**
 * Whether the compass may be listened to, prompting where iOS gates it. The
 * prompt is called before anything is awaited, so it stays inside the gesture
 * that called this; a listener already running needs no prompt.
 */
function orientation_permit(): Promise<boolean> {
  if (!orientation_needs_permission() || orientation_event !== null) return Promise.resolve(true);
  const prompt = (DeviceOrientationEvent as IOSOrientationEventCtor).requestPermission!();
  send_permission(ORIENTATION, PERMISSION_ASKING);
  return prompt.then(
    (permission) => {
      const granted = permission === "granted";
      if (granted) listen_orientation("deviceorientation");
      send_permission(ORIENTATION, granted ? PERMISSION_GRANTED : PERMISSION_DENIED);
      return granted;
    },
    (err) => {
      console.error("[kernel] orientation permission error:", err);
      send_permission(ORIENTATION, PERMISSION_FAILED);
      return false;
    },
  );
}

function orientation_remove(): void {
  if (orientation_event !== null) globalThis.removeEventListener(orientation_event, on_orientation, true);
  orientation_event = null;
}

// ---- the page's interrupts ----

function send_rect(element: HTMLElement): void {
  const r = element.getBoundingClientRect();
  CORE_BUILD.send(0, Cmd.INPUT_UPDATE, packet((b) => b.f64(r.left).f64(r.top).f64(r.width).f64(r.height)));
}

/** What the pointer holds: a tool, the placeable cube's or decor's catalog
 * number, and the object size. The kernel keeps no pick of its own; each
 * pointer event carries this. */
type Holding = { tool: u32; value: u32; size: number };

let holding: Holding = { tool: 0, value: 0, size: 0 };
/** The last hovering pointer, while it is over the canvas. */
let last_hover: { kind: number; id: number; button: number; x: number; y: number } | null = null;

function send_pointer_at(phase: number, kind: number, id: number, button: number, x: number, y: number): void {
  CORE_BUILD.send(0, Cmd.INPUT_POINTER, packet((b) => b.u32(phase).u32(kind).i32(id).i32(button).f64(x).f64(y).u32(holding.tool).u32(holding.value).f64(holding.size)));
  strobe.input();
  request_frame();
}

function send_pointer(phase: number, e: PointerEvent): void {
  const kind = e.pointerType === "touch" ? 1 : e.pointerType === "pen" ? 2 : 0;
  last_hover = phase === 3 || kind === 1 ? null : { kind, id: e.pointerId, button: e.button, x: e.clientX, y: e.clientY };
  send_pointer_at(phase, kind, e.pointerId, e.button, e.clientX, e.clientY);
}

/**
 * Sets what the pointer holds. A hover under a still pointer would otherwise
 * show the old pick until the pointer moves, so a move at the last position
 * is sent, as a browser synthesizes a mouse move when what is under a still
 * cursor changes.
 */
export function kernel_hold(tool: u32, value: u32, size: number): void {
  holding = { tool, value, size };
  if (last_hover) send_pointer_at(1, last_hover.kind, last_hover.id, last_hover.button, last_hover.x, last_hover.y);
}

/**
 * The one frame loop over `element`: the kernel's turns, the pointer's events
 * on it, the tab's visibility and the sensors' open and close.
 */
export function KernelDriver({ element }: { element: HTMLElement | null }): null {
  useLayoutEffect(() => {
    if (!element) return;
    driver = element;

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
      strobe.input();
      request_frame();
    };
    const on_context_menu = (e: MouseEvent) => e.preventDefault();
    const on_resize = () => {
      send_rect(element);
      request_frame();
    };
    // Rung at once with no quota: a hidden page gets no animation frame, and
    // hiding is the last moment a phone's tab is sure to get for the pending
    // save.
    const send_visibility = (visible: boolean) => {
      CORE_BUILD.send(0, Cmd.SENSOR_VISIBILITY, packet((b) => b.bool(visible)));
      CORE_BUILD.ring(0);
    };
    const on_visibility = () => {
      send_visibility(document.visibilityState === "visible");
      request_frame();
    };
    const on_page_hide = () => send_visibility(false);

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
    globalThis.addEventListener("pagehide", on_page_hide);
    const observer = new ResizeObserver(on_resize);
    observer.observe(element);

    // Recovers from a denied state the moment the user grants location access
    // from outside the app, since nothing else would ask again.
    let permission: PermissionStatus | null = null;
    const on_permission = () => {
      send_permission(GEOLOCATION, permission?.state === "granted" ? PERMISSION_GRANTED : PERMISSION_DENIED);
    };
    navigator.permissions?.query({ name: "geolocation" }).then((status) => {
      permission = status;
      status.addEventListener("change", on_permission);
    }).catch(() => {});

    send_rect(element);
    kernel_action({ type: "sensors_open" });
    strobe.start();

    return () => {
      strobe.stop();
      kernel_action({ type: "sensors_close" });
      permission?.removeEventListener("change", on_permission);
      observer.disconnect();
      globalThis.removeEventListener("pagehide", on_page_hide);
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
      driver = null;
    };
  }, [element]);
  return null;
}

// ---- boot ----

/** Asks for a device and tells the kernel when one answers, and again after
 * one is lost. */
function gpu_boot(): void {
  const lost = () => {
    CORE_BUILD.send(0, Cmd.GPU_DELETE);
    gpu_boot();
    request_frame();
  };
  void acquire_device(lost).then((info) => {
    if ("reason" in info) {
      console.error(`[gpu] ${info.reason}; nothing can be drawn`);
      return;
    }
    CORE_BUILD.send(0, Cmd.GPU_CREATE, packet((b) => b.u32(info.align).bool(info.timestamps)));
    request_frame();
  }, (err) => console.error("[gpu] the device request failed; nothing can be drawn:", err));
}

gpu_boot();

CORE_BUILD.call(
  Cmd.WORLD_CREATE,
  packet((b) =>
    b.str(APP_ENV).str(BUILD_TIME).str(BUILD_ID).str(BUILD_NUMBER).bool(PUBLISHED_TILES).bool(HANDHELD)
      .bool("geolocation" in navigator).bool(orientation_supported()).bool(orientation_needs_permission())
  ),
  (result) => result,
);
