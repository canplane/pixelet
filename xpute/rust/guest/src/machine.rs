// xpute-guest/machine.rs

//! The machine a program declares (its machine.json, written as `MACHINE` by
//! xpute-tool): the memory it is given, its stack, its rings and whether it
//! takes the GPU socket. The front of the memory follows from it, the same on
//! every platform and on both ends:
//!
//! ```text
//! 0 ─ stack ─ header page ─ SQ ─ CQ ─ SQ slots ─ CQ slots ─ GPU stream ─ GLOBAL_BASE
//! ```
//!
//! The host lays it before the first turn: the header (ipc/head.rs) and each
//! ring's own header. Past `global_base` a module's linker places its data,
//! and past where the data ends the program's fixed ranges and then the heap.
//! The first turn holds what the host laid to the machine and opens the rings
//! (`boot`), before the program's entry.

use core::panic::Location;

use xpute_kit::status::errno::Errno;

use crate::global::Global;
use crate::ipc::door::Door;
use crate::ipc::head::{self, Head};
use crate::ipc::ring::{ring_bytes, slots_bytes, Ring};
use crate::mem::section::{page_up, PAGE_BYTES};
use crate::wasm::LinkConfig;

/// The GPU socket: a stream the guest writes its GPU calls into and the host
/// runs once it is published (xpute-gpu), which says its own head
/// (ipc/stream.rs).
#[derive(Clone, Copy)]
pub struct Gpu {
    pub bytes: u32,
    /// WebGPU's power preference.
    pub power: &'static str,
    /// WebGPU's feature names the device must have, and those it takes where
    /// the adapter has them.
    pub required: &'static [&'static str],
    pub optional: &'static [&'static str],
}

#[derive(Clone, Copy)]
pub struct Machine {
    /// The sizes a host may give, any one of them; the module is linked
    /// against the smallest, and the heap is checked against it.
    pub mem_bytes: &'static [u32],
    /// What a reservation is counted in.
    pub region_bytes: u32,
    /// The heap the smallest size must leave past the front, the data and
    /// the fixed ranges.
    pub min_heap_bytes: u32,
    /// A module's stack, first in the memory.
    pub stack_bytes: u32,
    /// A native guest's thread's stack, outside the memory.
    pub thread_stack_bytes: u32,
    pub slot_bytes: u32,
    pub sq_cap: u32,
    pub cq_cap: u32,
    /// What one turn posts besides its replies (`Door::new`).
    pub turn_posts: u32,
    /// Whether the guest asks for the file device (abi/fs.json).
    pub fs: bool,
    pub gpu: Option<Gpu>,
}

impl Machine {
    /// Where the front's header is: the page past the stack.
    pub const fn head_at(&self) -> u32 {
        page_up(self.stack_bytes)
    }

    pub const fn sq_at(&self) -> u32 {
        self.head_at() + PAGE_BYTES
    }

    pub const fn cq_at(&self) -> u32 {
        self.sq_at() + page_up(ring_bytes(self.sq_cap))
    }

    pub const fn sq_slots_at(&self) -> u32 {
        self.cq_at() + page_up(ring_bytes(self.cq_cap))
    }

    pub const fn cq_slots_at(&self) -> u32 {
        self.sq_slots_at() + slots_bytes(self.sq_cap, self.slot_bytes)
    }

    /// Where the GPU stream is, past the slots; 0 without the socket.
    pub const fn gpu_at(&self) -> u32 {
        match self.gpu {
            Some(_) => self.cq_slots_at() + slots_bytes(self.cq_cap, self.slot_bytes),
            None => 0,
        }
    }

    /// What the front's header says for this machine.
    pub const fn head(&self) -> Head {
        Head {
            sq_cap: self.sq_cap,
            cq_cap: self.cq_cap,
            slot_bytes: self.slot_bytes,
            sq_at: self.sq_at(),
            cq_at: self.cq_at(),
            gpu_at: self.gpu_at(),
        }
    }

    /// The front's end, where a module's linker may begin its data
    /// (`--global-base`) and a native guest's own ranges begin.
    pub const fn global_base(&self) -> u32 {
        let end = self.cq_slots_at() + slots_bytes(self.cq_cap, self.slot_bytes);
        page_up(match self.gpu {
            Some(g) => end + g.bytes,
            None => end,
        })
    }

    pub const fn min_mem_bytes(&self) -> u32 {
        let mut least = u32::MAX;
        let mut i = 0;
        while i < self.mem_bytes.len() {
            if self.mem_bytes[i] < least {
                least = self.mem_bytes[i];
            }
            i += 1;
        }
        least
    }

    /// Where the program's fixed ranges begin: the page past the data.
    pub const fn fixed_at(&self, data_end: u32) -> u32 {
        page_up(data_end)
    }

    /// Where the heap begins: past the fixed ranges.
    pub const fn heap_at(&self, data_end: u32, fixed_bytes: u32) -> u32 {
        self.fixed_at(data_end) + page_up(fixed_bytes)
    }

    /// A module's link numbers.
    pub const fn link(&self) -> LinkConfig {
        LinkConfig {
            stack_bytes: self.stack_bytes,
            global_base: Some(self.global_base()),
            init_bytes: Some(self.min_mem_bytes()),
        }
    }

    /// Fails to compile, used in a const, where the declaration cannot run:
    /// a ring's capacity not a power of two, a turn's posts past what the
    /// completion ring holds besides a full submission ring's replies, a
    /// size not a whole number of regions, or the smallest size short of the
    /// front, `fixed_bytes` and the heap. The linker's data is counted from
    /// the front's end (`global_base`), as a native guest's is.
    pub const fn check(&self, fixed_bytes: u32) {
        assert!(self.sq_cap.is_power_of_two() && self.cq_cap.is_power_of_two());
        assert!(self.sq_cap + self.turn_posts < self.cq_cap);
        assert!(self.slot_bytes.is_power_of_two() && self.slot_bytes.is_multiple_of(8));
        assert!(!self.mem_bytes.is_empty());
        let mut i = 0;
        while i < self.mem_bytes.len() {
            assert!(self.mem_bytes[i].is_multiple_of(self.region_bytes));
            i += 1;
        }
        assert!(
            self.heap_at(self.global_base(), fixed_bytes) as u64 + self.min_heap_bytes as u64
                <= self.min_mem_bytes() as u64
        );
    }
}

static DOOR: Global<Option<Door>> = Global::new();

/// The front the host laid held to `m`, once, on the first turn: its header
/// and each ring's, then the rings opened where they lie. Where a word is not
/// the machine's, the guest stops with ENOEXEC, the word's place and what it
/// holds, its report posted alone on a completion ring laid where the machine
/// puts it, the one place both ends know without the host's words.
pub fn boot(m: &Machine) {
    let mem = crate::mem::core();
    let (sq_slots, cq_slots) = (m.sq_slots_at(), m.cq_slots_at());
    // SAFETY: the front's header page, which the host wrote before the turn.
    let differs = unsafe { head::differs(mem, m.head_at() as usize, &m.head()) }
        .or_else(|| Ring::header_differs(mem, m.sq_at() as usize, m.sq_cap, sq_slots, m.slot_bytes))
        .or_else(|| {
            Ring::header_differs(mem, m.cq_at() as usize, m.cq_cap, cq_slots, m.slot_bytes)
        });
    // SAFETY: the front's ranges, the rings' alone, in memory that never moves.
    let (sq, cq) = unsafe {
        match differs {
            None => (
                Ring::new(mem, m.sq_at() as usize),
                Ring::new(mem, m.cq_at() as usize),
            ),
            Some(_) => (
                Ring::init(mem, m.sq_at() as usize, m.sq_cap, sq_slots, m.slot_bytes),
                Ring::init(mem, m.cq_at() as usize, m.cq_cap, cq_slots, m.slot_bytes),
            ),
        }
    };
    DOOR.set(Some(Door::new(sq, cq, m.turn_posts)));
    if let Some((at, found)) = differs {
        door().abort(Errno::ENOEXEC, Location::caller(), at.into(), found.into());
        xpute_kit::bug!(ENOEXEC, at, found);
    }
}

/// The door `boot` laid; asking before it is a bug.
pub fn door() -> &'static Door {
    use xpute_kit::status::bug::OrBug;
    DOOR.get(|| None)
        .as_ref()
        .or_bug(xpute_kit::status::errno::Errno::ENOTRECOVERABLE)
}

/// Whether `boot` has run.
pub fn booted() -> bool {
    DOOR.get(|| None).is_some()
}

/// A guest of `machine` (its `MACHINE`), its fixed ranges `fixed_bytes` long,
/// whose turns `entry` runs (`entry::Entry`) once the first has opened the
/// front the host laid (`boot`), in the form its target takes. As a
/// WebAssembly module it is `wasm_abi!` over the machine: the heap past the
/// fixed ranges and the stack it was linked with. Anywhere else it is
/// xpute-host's `native_guest!`, so a program's crate depends on xpute-host
/// for every target but wasm32.
///
/// Given `gpu`, the numbers its native GPU host runs by (xpute-gpu
/// `Numbers`), the machine takes the GPU socket, and off WebAssembly it is
/// xpute-gpu's `native_guest!` instead: the GPU's host a device beside the
/// guest, polled before each turn and rung as the guest publishes its stream
/// (`BootInfo::publish`), and its admission handed the entry
/// (`BootInfo::admit`). The program's crate
/// then depends on xpute-gpu with its `host` feature for every target but
/// wasm32, where a page's host draws and admits every frame.
///
/// ```ignore
/// xpute_guest::guest!(MACHINE, FIXED_BYTES, door::run);
/// xpute_guest::guest!(MACHINE, FIXED_BYTES, door::run, gpu: NUMBERS);
/// ```
#[macro_export]
macro_rules! guest {
    (@wasm $machine:expr, $fixed_bytes:expr, $entry:expr) => {
        #[cfg(target_arch = "wasm32")]
        mod xpute_guest_machine {
            #[allow(unused_imports)]
            use super::*;

            pub(super) fn xpute_turn(quota_ms: f64, boot: fn() -> $crate::entry::BootInfo) -> f64 {
                if !$crate::machine::booted() {
                    $crate::machine::boot(&$machine);
                }
                ($entry)(quota_ms, boot)
            }
        }

        #[cfg(target_arch = "wasm32")]
        $crate::wasm_abi!(
            $crate::wasm::Config {
                heap_at: |data_end| $machine.heap_at(data_end, $fixed_bytes),
                stack_bytes: $machine.stack_bytes,
            },
            xpute_guest_machine::xpute_turn
        );
    };
    ($machine:expr, $fixed_bytes:expr, $entry:expr, gpu: $gpu:expr $(,)?) => {
        const _: () = $machine.check($fixed_bytes);
        const _: $crate::entry::Entry = $entry;
        const _: () = assert!(
            $machine.gpu.is_some(),
            "a guest given the GPU's numbers takes the GPU socket"
        );
        const _: () = {
            let _ = $gpu;
        };

        $crate::guest!(@wasm $machine, $fixed_bytes, $entry);

        #[cfg(not(target_arch = "wasm32"))]
        ::xpute_gpu::native_guest!($machine, $fixed_bytes, $entry, $gpu);
    };
    ($machine:expr, $fixed_bytes:expr, $entry:expr $(,)?) => {
        const _: () = $machine.check($fixed_bytes);
        const _: $crate::entry::Entry = $entry;

        $crate::guest!(@wasm $machine, $fixed_bytes, $entry);

        #[cfg(not(target_arch = "wasm32"))]
        ::xpute_host::native_guest!($machine, $fixed_bytes, $entry);
    };
}
