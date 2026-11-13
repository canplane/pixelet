// @pxl/client/abi/types.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

export const Sensor = {
  Geolocation: 0,
  Orientation: 1,
} as const;
export type Sensor = (typeof Sensor)[keyof typeof Sensor];

/** The name of the `Sensor` member numbered `n`, or undefined for a number
 * no member has. */
export function sensor_name(n: number): keyof typeof Sensor | undefined {
  for (const k in Sensor)
    if (Sensor[k as keyof typeof Sensor] === n) return k as keyof typeof Sensor;
  return undefined;
}

/** The `Sensor` member `n` is, or null for a number no member has. */
export function sensor_of(n: number): Sensor | null {
  return sensor_name(n) === undefined ? null : (n as Sensor);
}
