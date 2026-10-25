// xpute-gpu/lib.rs

//! The calls a guest makes to a GPU it cannot reach, as WebGPU names them:
//! the ops of a stream's records and the enumerations their words carry
//! (spec/gpu/), and whether its host admits a frame now (`admit`). Beside
//! `kit` and the guest's end rather than in them, since it is one kind of
//! call, not the arrangement: a program that draws nothing links none of it.

#[path = "api.spec.rs"]
pub mod spec;

#[path = "enum.spec.rs"]
pub mod enums;

pub mod admit;
pub mod encode;

#[cfg(feature = "host")]
pub mod host;

#[cfg(test)]
#[path = "programs.test.rs"]
mod programs;
