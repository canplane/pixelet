// pxl_shared/spec/message.rs
// What the kernel has to say, as a number; the words are the shell's.

/// A message the kernel raises and the shell shows.
///
/// The kernel decides which message a state calls for and carries the number.
/// It never carries the words: a string that exists to be read has no reason
/// to sit in the kernel's memory, and the shell has the locale files already
/// (bootstrap/locale) — the same split `strerror` is on the other side of.
///
/// The number is the name here. A band is what a message is about, and a
/// message keeps its number for as long as anything can be holding one: an
/// old shell reads a number a new kernel sends, so a number that has meant
/// something must never come to mean another thing. Retire one by leaving
/// the gap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Message {
    None = 0,

    // ---- 0x01 the world's own state ----
    MapLoading = 0x0101,
    MapLoadError = 0x0102,
    CompanionLoading = 0x0103,
    WorldReady = 0x0104,

    // ---- 0x02 where the device says it is ----
    LocationChecking = 0x0201,
    LocationWeakSignal = 0x0202,
    GeolocationUnsupported = 0x0203,
    GeolocationPermissionDenied = 0x0204,
    GeolocationPositionUnavailable = 0x0205,
    GeolocationTimeout = 0x0206,
    GeolocationGenericError = 0x0207,

    // ---- 0x03 which way the device is facing ----
    OrientationUnsupported = 0x0301,
    OrientationPermissionError = 0x0302,
    OrientationPermissionDenied = 0x0303,
    OrientationSignalLost = 0x0304,
}
