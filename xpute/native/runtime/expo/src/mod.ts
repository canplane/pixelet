// @xpute/native-runtime

/**
 * This is xpute's native module, which an Expo app autolinks (ios/, android/).
 * It starts the runtime linked into the app beside the guest, on the guest's
 * own thread (ios/CXpute/xpute.h `xpute_start`), and carries the UI's link to
 * it and back (remote.ts). The module tells the runtime of the app's lifecycle
 * itself (Swift, Kotlin), not through JavaScript.
 */

export { bytes, open_remote, start } from "./remote.ts";
export { log, stopped_on, trap_on } from "./xpute.ts";
