// @xpute/runtime/abi/gpu/command.spec.ts
//
// GENERATED from spec/abi/gpu.json

// Every command, by the side that sends it.

// ---- gpu, export ----

/** The GPU the machine asked for answered, the first time or again after a loss, whose
 * sign is in the stream's device block. */
export const GPU_READY = 0x0201;
/** Answered with the surface's handle, 0 where the core opens none. */
export const GPU_SURFACE_OPEN = 0x0202;
/** The surface's layout box in device pixels, transforms aside, and the device pixel ratio;
 * the buffer it is drawn into is the core's. */
export const GPU_SURFACE_RESIZE = 0x0203;
/** The surface is going: what it drew and held on the device is let go. */
export const GPU_SURFACE_CLOSE = 0x0204;
