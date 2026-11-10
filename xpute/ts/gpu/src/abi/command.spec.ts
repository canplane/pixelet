// @xpute/gpu/abi/command.spec.ts
//
// GENERATED from spec/abi/gpu.json

// Every command, by the side that sends it.

// ---- gpu, export ----

/** The GPU the machine asked for has answered, for the first time or again after the device
 * was lost. The STATUS word of the stream's device block tells the core the device's state. */
export const GPU_READY = 0x0001;
/** Asks the core to open a surface. The core answers with the surface's handle, or with 0
 * when it opens none. */
export const GPU_SURFACE_OPEN = 0x0002;
/** The surface's layout box in device pixels, ignoring transforms, and the device pixel
 * ratio. The size of the buffer the surface is drawn into is the core's to choose. */
export const GPU_SURFACE_RESIZE = 0x0003;
/** The surface is going away, and the core lets go of what it drew and held on the device
 * for it. */
export const GPU_SURFACE_CLOSE = 0x0004;
