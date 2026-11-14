// xpute-kit/lib.rs

//! This is the generic code under both of xpute's ends, and the Rust
//! counterpart of `@xpute/kit`. The kit knows nothing of any program's domain.
//! `xpute-guest` depends on the kit, and the kit never depends on the guest.
//! Without its `std` feature the crate is `no_std` and holds only `math::rng`.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
pub mod abi;
#[cfg(feature = "std")]
pub mod alloc;
#[cfg(feature = "std")]
pub mod codec;
#[cfg(feature = "std")]
pub mod collection;
pub mod math;
#[cfg(feature = "std")]
pub mod status;
#[cfg(feature = "std")]
pub mod wire;
