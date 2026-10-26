// xpute-kit/lib.rs

//! The generic kit, `@xpute/kit`'s counterpart: no domain knowledge.
//! `xpute-guest` depends on it, never the reverse. Without its `std`
//! feature it is `no_std` and holds `math::rng` alone.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
pub mod abi;
#[cfg(feature = "std")]
pub mod alloc;
#[cfg(feature = "std")]
pub mod codec;
#[cfg(feature = "std")]
pub mod collection;
#[cfg(feature = "std")]
pub mod golden;
pub mod math;
#[cfg(feature = "std")]
pub mod status;
#[cfg(feature = "std")]
pub mod wire;
