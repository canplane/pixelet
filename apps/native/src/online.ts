// @pxl/native/online.ts

/** No signal is read here, so the device is taken as online and a failed read is the server's. */
export function onlineWatch(_report: (online: boolean) => void): void {}
