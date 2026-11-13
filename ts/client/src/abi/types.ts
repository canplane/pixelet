// @pxl/client/abi/types.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import { uri } from "@xpute/link/link.ts";

/** The interface's qualified name: its package and its name. */
export const NAME = uri("pxl:core", "types");

export const Sensor = {
  Geolocation: 0,
  Orientation: 1,
} as const;
export type Sensor = (typeof Sensor)[keyof typeof Sensor];

/** The name of the `Sensor` member numbered `n`, or undefined for a number
 * no member has. */
export function sensor_name(n: number): keyof typeof Sensor | undefined {
  return (Object.keys(Sensor) as (keyof typeof Sensor)[]).find(
    (k) => Sensor[k] === n,
  );
}

/** The `Sensor` member `n` is, or null for a number no member has. */
export function sensor_of(n: number): Sensor | null {
  return sensor_name(n) === undefined ? null : (n as Sensor);
}
