// rust/client/core/src/domain/cmd.spec.rs
//
// GENERATED from spec/client/core/door/ui.json

//! The ui protocol's enums a module under its own code reads,
//! which its code (rust/client/core/src/door/cmd.spec.rs) names from here.

#![allow(
    non_camel_case_types,
    clippy::upper_case_acronyms,
    dead_code,
    reason = "generated from the spec"
)]

/// What the pointer is, as the browser's `pointerType` says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum PointerType {
    Mouse = 0,
    Touch = 1,
    Pen = 2,
}

impl PointerType {
    pub const ALL: [PointerType; 3] = [PointerType::Mouse, PointerType::Touch, PointerType::Pen];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<PointerType> {
        PointerType::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Tracking {
    /// The camera goes back to the device's place.
    Recenter = 0,
    /// Follow mode on, where the compass is listened to.
    Follow = 1,
    /// Follow mode off, north up again.
    Unfollow = 2,
}

impl Tracking {
    pub const ALL: [Tracking; 3] = [Tracking::Recenter, Tracking::Follow, Tracking::Unfollow];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<Tracking> {
        Tracking::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// The browser's PositionError codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum GeolocationErrorCode {
    Other = 0,
    PermissionDenied = 1,
    PositionUnavailable = 2,
    Timeout = 3,
}

impl GeolocationErrorCode {
    pub const ALL: [GeolocationErrorCode; 4] = [
        GeolocationErrorCode::Other,
        GeolocationErrorCode::PermissionDenied,
        GeolocationErrorCode::PositionUnavailable,
        GeolocationErrorCode::Timeout,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<GeolocationErrorCode> {
        GeolocationErrorCode::ALL
            .into_iter()
            .find(|m| *m as u32 == number)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum PermissionAnswer {
    /// The UI's prompt is up.
    Asking = 0,
    Granted = 1,
    Denied = 2,
    /// The prompt itself threw.
    Failed = 3,
}

impl PermissionAnswer {
    pub const ALL: [PermissionAnswer; 4] = [
        PermissionAnswer::Asking,
        PermissionAnswer::Granted,
        PermissionAnswer::Denied,
        PermissionAnswer::Failed,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<PermissionAnswer> {
        PermissionAnswer::ALL
            .into_iter()
            .find(|m| *m as u32 == number)
    }
}

/// Where a sensor's reading is: none asked for, asked for, arriving, failed, or none to be had, the device having no such sensor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum SensorStatus {
    #[default]
    Idle = 0,
    Loading = 1,
    Ready = 2,
    Error = 3,
    Unavailable = 4,
}

impl SensorStatus {
    pub const ALL: [SensorStatus; 5] = [
        SensorStatus::Idle,
        SensorStatus::Loading,
        SensorStatus::Ready,
        SensorStatus::Error,
        SensorStatus::Unavailable,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<SensorStatus> {
        SensorStatus::ALL.into_iter().find(|m| *m as u32 == number)
    }
}

/// A message the core raises and the UI shows.
///
/// The core decides which message a state calls for and carries the number.
/// It never carries the words: a string that exists to be read has no reason
/// to sit in the core's memory, and the UI has the locale files already
/// (ts/client/src/ui/shared/locale) — the same split `strerror` is on the other side of.
///
/// The number is the name here, and a band is what a message is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Message {
    // ---- 0x02 where the device says it is ----
    GeolocationUnsupported = 0x0203,
    GeolocationPermissionDenied = 0x0204,
    GeolocationPositionUnavailable = 0x0205,
    GeolocationTimeout = 0x0206,
    GeolocationGenericError = 0x0207,

    // ---- 0x04 why the server does not answer the world's reads ----
    /// Reads reach the server and fail; they are tried again.
    WorldReadFailing = 0x0403,
    /// Reads failed past every retry; the world is not coming.
    WorldUnavailable = 0x0404,
}

impl Message {
    pub const ALL: [Message; 7] = [
        Message::GeolocationUnsupported,
        Message::GeolocationPermissionDenied,
        Message::GeolocationPositionUnavailable,
        Message::GeolocationTimeout,
        Message::GeolocationGenericError,
        Message::WorldReadFailing,
        Message::WorldUnavailable,
    ];

    /// The member a number names, or none for a number nobody gave out.
    pub fn of(number: u32) -> Option<Message> {
        Message::ALL.into_iter().find(|m| *m as u32 == number)
    }
}
