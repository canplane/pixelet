// xpute-core/machine.rs

//! A program declares its machine: the memory it is given, its stack, its
//! rings, and whether it has a stream. The program writes it in its
//! machine.json, from which xpute-gen generates `MACHINE`. The front of the
//! memory, the region both ends share, follows from the machine and is laid
//! out the same on every platform and on both ends:
//!
//! ```text
//! 0 ─ stack ─ header page ─ SQ ─ CQ ─ SQ slots ─ CQ slots ─ stream ─ GLOBAL_BASE
//! ```
//!
//! The runtime writes the front before the first turn: the header
//! (ipc/head.rs) and each ring's own header. Past `global_base` a module's
//! linker places its data; past the end of the data come the program's fixed
//! ranges, and then the heap. Before the program's entry runs, the first turn
//! checks what the runtime wrote against the machine and opens the rings
//! (`boot`).

use core::panic::Location;

use xpute_kit::status::errno::Errno;

use crate::global::Global;
use crate::ipc::door::Door;
use crate::ipc::head::{self, Head};
use crate::ipc::ring::{ring_bytes, slots_bytes, Ring};
use crate::mem::section::{page_up, PAGE_BYTES};
use crate::wasm::LinkConfig;

/// A stream past the rings, which the core writes records into and something
/// beside it runs once they are published (ipc/stream.rs). What the records
/// mean is the plugin's.
#[derive(Clone, Copy)]
pub struct Stream {
    pub bytes: u32,
}

#[derive(Clone, Copy)]
pub struct Machine {
    /// The memory sizes a runtime may give, any one of them. The module is
    /// linked against the smallest, and the heap is checked against it.
    pub mem_bytes: &'static [u32],
    /// The unit a reservation is counted in.
    pub region_bytes: u32,
    /// The heap the smallest memory size must leave past the front, the data
    /// and the fixed ranges.
    pub min_heap_bytes: u32,
    /// The stack of a module core, placed first in the memory.
    pub stack_bytes: u32,
    /// The stack of a native core's thread, which lies outside the memory.
    pub thread_stack_bytes: u32,
    pub slot_bytes: u32,
    pub sq_cap: u32,
    pub cq_cap: u32,
    /// How many frames one turn may post besides its replies (`Door::new`).
    pub turn_posts: u32,
    /// This says whether the core asks for the file device (abi/fs.json).
    pub fs: bool,
    pub stream: Option<Stream>,
}

impl Machine {
    /// The front's header lies in the page past the stack.
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

    /// The stream lies past the slots, and this is 0 without one.
    pub const fn stream_at(&self) -> u32 {
        match self.stream {
            Some(_) => self.cq_slots_at() + slots_bytes(self.cq_cap, self.slot_bytes),
            None => 0,
        }
    }

    /// This is the header the runtime writes in the front for this machine.
    pub const fn head(&self) -> Head {
        Head {
            sq_cap: self.sq_cap,
            cq_cap: self.cq_cap,
            slot_bytes: self.slot_bytes,
            sq_at: self.sq_at(),
            cq_at: self.cq_at(),
            stream_at: self.stream_at(),
        }
    }

    /// The end of the front: a module's linker may begin its data here
    /// (`--global-base`), and a native core's own ranges begin here.
    pub const fn global_base(&self) -> u32 {
        let end = self.cq_slots_at() + slots_bytes(self.cq_cap, self.slot_bytes);
        page_up(match self.stream {
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

    pub const fn fixed_at(&self, data_end: u32) -> u32 {
        page_up(data_end)
    }

    pub const fn heap_at(&self, data_end: u32, fixed_bytes: u32) -> u32 {
        self.fixed_at(data_end) + page_up(fixed_bytes)
    }

    /// These are the numbers a module core of this machine is linked with.
    pub const fn link(&self) -> LinkConfig {
        LinkConfig {
            stack_bytes: self.stack_bytes,
            global_base: Some(self.global_base()),
            init_bytes: Some(self.min_mem_bytes()),
        }
    }

    /// Called in a const, this fails the build for a machine that cannot run: a
    /// ring's capacity that is not a power of two; more posts in a turn than
    /// the completion ring holds besides a full submission ring's replies; a
    /// memory size that is not a whole number of regions; or a smallest memory
    /// size too small for the front, `fixed_bytes` and the heap. The linker's
    /// data is counted from the end of the front (`global_base`), as a native
    /// core's ranges are.
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

/// Checks the front the runtime wrote against `m`, once, on the first turn: the
/// front's header and each ring's header. It then opens the rings where they
/// lie. Where a word of the front differs from the machine's, the core stops
/// with ENOEXEC, the word's place and what it holds. That report is posted
/// alone on a completion ring laid afresh where the machine puts it, the one
/// place both ends know without trusting the runtime's words.
pub fn boot(m: &Machine) {
    let mem = crate::mem::core();
    let (sq_slots, cq_slots) = (m.sq_slots_at(), m.cq_slots_at());
    // SAFETY: the front's header page, which the runtime wrote before the turn.
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

/// This is the door `boot` opened. Asking for it before `boot` is a bug.
pub fn door() -> &'static Door {
    use xpute_kit::status::bug::OrBug;
    DOOR.get(|| None)
        .as_ref()
        .or_bug(xpute_kit::status::errno::Errno::ENOTRECOVERABLE)
}

pub fn booted() -> bool {
    DOOR.get(|| None).is_some()
}

/// Defines a core of `machine` (its `MACHINE`) in the form its target takes.
/// Its fixed ranges are `fixed_bytes` long, and `entry` runs its turns
/// (`entry::Entry`) once the first turn has opened the front the runtime
/// wrote (`boot`). As a WebAssembly module the core is `wasm_abi!` over the
/// machine, with the heap past the fixed ranges and the stack it was linked
/// with. On any other target it is xpute-runtime's `native_core!`, so a
/// program's crate depends on xpute-runtime for every target but wasm32.
///
/// ```ignore
/// xpute_core::core!(MACHINE, FIXED_BYTES, door::run);
/// ```
#[macro_export]
macro_rules! core {
    (@wasm $machine:expr, $fixed_bytes:expr, $entry:expr) => {
        #[cfg(target_arch = "wasm32")]
        mod xpute_core_machine {
            #[allow(unused_imports)]
            use super::*;

            pub(super) fn xpute_turn(deadline_ms: f64, boot: fn() -> $crate::entry::BootInfo) -> f64 {
                if !$crate::machine::booted() {
                    $crate::machine::boot(&$machine);
                }
                ($entry)(deadline_ms, boot)
            }
        }

        #[cfg(target_arch = "wasm32")]
        $crate::wasm_abi!(
            $crate::wasm::Config {
                heap_at: |data_end| $machine.heap_at(data_end, $fixed_bytes),
                stack_bytes: $machine.stack_bytes,
            },
            xpute_core_machine::xpute_turn
        );
    };
    ($machine:expr, $fixed_bytes:expr, $entry:expr $(,)?) => {
        const _: () = $machine.check($fixed_bytes);
        const _: $crate::entry::Entry = $entry;

        $crate::core!(@wasm $machine, $fixed_bytes, $entry);

        #[cfg(not(target_arch = "wasm32"))]
        ::xpute_runtime::native_core!($machine, $fixed_bytes, $entry);
    };
}
