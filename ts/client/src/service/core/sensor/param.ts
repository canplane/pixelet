// @pxl/client/service/core/sensor/param.ts

/** The numbers of the device's sensors: how long the compass waits out a
 * silence. */

/** Orientation has no Permissions API, so a revoked permission is never
 * reported. A gap this long while tracking is taken as a loss instead. This
 * holds only where the sensor talks while still (`silenceIsLoss`). */
export const WATCHDOG_TIMEOUT_MS = 8000;
