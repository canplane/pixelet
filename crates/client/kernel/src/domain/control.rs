// pxl-kernel/domain/control.rs

//! What the user does to the world and the view: the camera, the pointer's
//! picks and edits, navigation, and the device's sensors.

pub mod camera;
/// The commands and the snapshot the kernel and the shell agree on,
/// generated from cmd.json. What any guest and host share is
/// `xpute_guest::abi`'s.
#[path = "control/cmd.spec.rs"]
pub mod cmd;
pub mod interaction;
pub mod navigation;
pub mod param;
pub mod sensor;
