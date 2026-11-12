// @pxl/client/abi/types.ts
//
// GENERATED from spec/client/core/wit (xpute-gen spec/wit.rs)

export const Sensor = {
  Geolocation: 0,
  Orientation: 1,
} as const;
export type Sensor = (typeof Sensor)[keyof typeof Sensor];
