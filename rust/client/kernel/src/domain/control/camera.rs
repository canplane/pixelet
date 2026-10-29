// pxl-kernel/domain/control/camera.rs

//! The camera and everything that moves it.

pub mod controller;
pub mod first_person;
pub mod gesture;
pub mod input;
pub mod orbit;
pub mod state;

#[cfg(test)]
#[path = "camera/fixture.test.rs"]
pub(crate) mod fixture;
