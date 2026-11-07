// pxl-kernel/domain/control/camera.rs

//! The camera and the controllers that move it; what asks them to is
//! above it (domain/control/input.rs, navigation).

pub mod controller;
pub mod first_person;
pub mod gesture;
pub mod orbit;
pub mod state;

#[cfg(test)]
#[path = "camera/fixture.test.rs"]
pub(crate) mod fixture;
