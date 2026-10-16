// @pxl/web/feature/debug/eye.ts

/**
 * A development page's pinned eye, so every load casts the same rays and two
 * builds read at one view: `?eye=lon,lat,dist,polar,azimuth` puts the camera's
 * focus at lon, lat (degrees) on the ground and the camera `dist` units from
 * it, `polar` from straight down and `azimuth` from north (radians), each
 * left as it is where the URL stops before it.
 */

import { Command, encode_input_go_to, encode_input_view } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command, kernel_room } from "@/kernel/shell.ts";

export function pinEye(): void {
  const eye = new URLSearchParams(location.search).get("eye");
  if (eye === null) return;
  const [lon, lat, dist, polar, azimuth] = eye.split(",").map((v) => (v === "" ? null : Number(v)));
  if (lon == null || lat == null) return;
  kernel_command(Command.INPUT_GO_TO, encode_input_go_to(kernel_room, lon, lat));
  kernel_command(Command.INPUT_VIEW, encode_input_view(kernel_room, dist ?? null, polar ?? null, azimuth ?? null));
}
