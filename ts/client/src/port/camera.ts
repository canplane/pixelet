// @pxl/client/port/camera.ts

/** The camera's commands: the tracking button, the compass, a search
 * result. */

import type { f64 } from "@xpute/kit/abi/word.ts";
import {
  Command,
  encode_camera_goto,
  encode_camera_tracking,
  Tracking,
} from "@/abi/cmd.spec.ts";
import { HOST } from "@/host/mount.ts";
import * as port from "@/abi/port.spec.ts";
import { bracket } from "@/bracket.tsx";
import { send_input } from "./input.ts";

/** iOS Safari's compass prompt runs here, inside the press. */
export async function tracking(tracking: Tracking): Promise<void> {
  if (tracking === Tracking.Follow && !(await bracket.sensor.orient.permit())) {
    console.error("[orientation] could not start the sensor.");
    return;
  }
  send_input(
    Command.CAMERA_TRACKING,
    encode_camera_tracking(HOST.doorbell, tracking),
  );
}

/** The compass's press: north up. */
export function north_up(): void {
  HOST.call(port.SHELL, Command.CAMERA_NORTH_UP);
}

/** A search result: the pivot moved there, the view as it is. */
export function goto(lon: f64, lat: f64): void {
  HOST.call(
    port.SHELL,
    Command.CAMERA_GOTO,
    encode_camera_goto(HOST.doorbell, lon, lat, null, null, null),
  );
}
