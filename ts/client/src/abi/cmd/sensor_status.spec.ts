// @pxl/client/abi/cmd/sensor_status.spec.ts
//
// GENERATED from spec/client/kernel/domain/control/cmd.json

// Where a sensor's reading is: none asked for, asked for, arriving, failed, or none to be had, the device having no such sensor.

export const Idle = 0;
export const Loading = 1;
export const Ready = 2;
export const Error = 3;
export const Unavailable = 4;
