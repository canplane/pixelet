// xpute-guest/lib.rs

//! The guest's end of xpute: its end of the link (xpute-link) to its host,
//! the clocks it reads, its log records (`log`) and the report of its
//! failures (`stop`). Its memory and its allocator are the program's.

/// The bindings of xpute's packages on the guest's side, a module to a
/// package, generated from their WIT (xpute-link-gen).
pub mod abi;
// What `link!` expands to names the link's own items through this crate.
#[doc(hidden)]
pub use xpute_link;
pub mod clock;
pub mod link;
pub mod log;
pub mod stop;
pub mod wait;

/// Defines the guest's three exports (link/wit/types.wit), over frames of
/// `cap` bytes, with `boot` mounting what the guest implements and opening
/// what it calls, and `enter` and `end` starting and ending each entry. From
/// its init on, the guest's failures are its own (stop.rs `hook`).
#[macro_export]
macro_rules! link {
    ($cap:expr, $boot:expr, $enter:expr, $end:expr) => {
        #[cfg(target_arch = "wasm32")]
        #[unsafe(no_mangle)]
        pub extern "C" fn link_init() -> u64 {
            $crate::stop::hook();
            $crate::link::init($cap, $enter, $end)
        }
        #[cfg(target_arch = "wasm32")]
        #[unsafe(no_mangle)]
        pub extern "C" fn link_boot() {
            $crate::link::boot($boot)
        }
        #[cfg(target_arch = "wasm32")]
        #[unsafe(no_mangle)]
        pub extern "C" fn link_in() -> i32 {
            $crate::link::recv()
        }
        // A native guest's exports catch its panic and answer a trap
        // (`TRAP`, or `TRAP_AT` for init's layout).
        #[cfg(not(target_arch = "wasm32"))]
        #[unsafe(no_mangle)]
        pub extern "C" fn link_init() -> u64 {
            $crate::link::caught($crate::xpute_link::TRAP_AT, || {
                $crate::stop::hook();
                $crate::link::init($cap, $enter, $end)
            })
        }
        #[cfg(not(target_arch = "wasm32"))]
        #[unsafe(no_mangle)]
        pub extern "C" fn link_boot() -> i32 {
            $crate::link::caught($crate::xpute_link::TRAP, || {
                $crate::link::boot($boot);
                0
            })
        }
        #[cfg(not(target_arch = "wasm32"))]
        #[unsafe(no_mangle)]
        pub extern "C" fn link_in() -> i32 {
            $crate::link::caught($crate::xpute_link::TRAP, $crate::link::recv)
        }
    };
}
