// @pxl/client/service/core/core.ts

/** The core as the UI reaches it: the UI's own interface, mounted for the
 * core before it boots, and a reference to each interface the core serves,
 * opened over the UI's remote end of the link (`ui`) once it has. */

import type { Ref, Remote } from "@xpute/runtime";
import { tell_life } from "@xpute/runtime";
import * as camera from "@/abi/camera.ts";
import * as edit from "@/abi/edit.ts";
import * as input from "@/abi/input.ts";
import * as perf from "@/abi/perf.ts";
import * as scene from "@/abi/scene.ts";
import * as sensor from "@/abi/sensor.ts";
import * as setting from "@/abi/setting.ts";
import * as source from "@/abi/source.ts";
import * as ui_abi from "@/abi/ui.ts";
import { set_snapshot, view_posted } from "@/service/core/snapshot.ts";
import { sensor_start, sensor_stop } from "@/service/core/sensor.ts";

/** The interfaces the core serves, by their bindings. */
const SERVED = { camera, edit, input, perf, scene, sensor, setting, source };

/** A reference to each interface the core serves, which its binding's
 * functions take first. */
export let core: Record<keyof typeof SERVED, Ref>;
export let ui: Remote;

/** Mounts the UI's interface on `remote`, before the core boots. */
export function mount_ui(remote: Remote): void {
  ui = remote;
  ui_abi.mount(remote, {
    sensor_start,
    sensor_stop,
    show: set_snapshot,
    view: (azimuth, m_per_px) => view_posted({ azimuth, m_per_px }),
  });
}

/** Opens each interface the core serves, once it has booted, and tells it
 * the page's lifecycle. */
export async function open_core(remote: Remote): Promise<void> {
  const names = Object.keys(SERVED) as (keyof typeof SERVED)[];
  const refs = await Promise.all(names.map((n) => SERVED[n].open(remote)));
  core = Object.fromEntries(names.map((n, k) => [n, refs[k]])) as typeof core;
  await tell_life(remote);
}
