// ts/client/src/abi/cmd/message.spec.ts
//
// GENERATED from spec/client/core/door/ui.json

// A message the core raises and the UI shows.
//
// The core decides which message a state calls for and carries the number.
// It never carries the words: a string that exists to be read has no reason
// to sit in the core's memory, and the UI has the locale files already
// (ts/client/src/ui/shared/locale) — the same split `strerror` is on the other side of.
//
// The number is the name here, and a band is what a message is about.

// ---- 0x02 where the device says it is ----

export const GeolocationUnsupported = 0x0203;
export const GeolocationPermissionDenied = 0x0204;
export const GeolocationPositionUnavailable = 0x0205;
export const GeolocationTimeout = 0x0206;
export const GeolocationGenericError = 0x0207;

// ---- 0x04 why the server does not answer the world's reads ----

/** Reads reach the server and fail; they are tried again. */
export const WorldReadFailing = 0x0403;
/** Reads failed past every retry; the world is not coming. */
export const WorldUnavailable = 0x0404;
