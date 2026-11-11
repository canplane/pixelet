// xpute-core/manifest.rs

//! A program declares its manifest: the memory it is given, its stack, the
//! most a frame's packet takes, and whether it has a stream. The program writes it in its
//! manifest.json, from which xpute-gen generates `MANIFEST`. The front of the
//! memory, the region both ends share, follows from the manifest and is laid
//! out the same on every platform and on both ends:
//!
//! ```text
//! 0 ─ stack ─ header page ─ stream ─ GLOBAL_BASE
//! ```
//!
//! The runtime writes the front's header (io/head.rs) before the first turn.
//! Past `global_base` a module's linker places its data; past the end of the
//! data come the program's fixed ranges, and then the heap. Before the
//! program's entry runs, the first turn checks the header against the manifest
//! and opens the door, its two frames on the heap (io/door.rs `boot`).

use crate::io::door::ABORT_PKT_BYTES;
use crate::io::frame::WORD_BYTES;
use crate::io::head::Head;
use crate::mem::section::{page_up, PAGE_BYTES};
use crate::wasm::LinkConfig;

/// A stream past the front's header, which the core writes records into and something
/// beside it runs once they are published (io/stream.rs). What the records
/// mean is the plugin's.
#[derive(Clone, Copy)]
pub struct Stream {
    pub bytes: u32,
}

#[derive(Clone, Copy)]
pub struct Manifest {
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
    /// The most bytes a frame's packet takes, either way.
    pub pkt_bytes: u32,
    /// This says whether the core asks for the file device (abi/fs.json).
    pub fs: bool,
    pub stream: Option<Stream>,
}

impl Manifest {
    /// The front's header lies in the page past the stack.
    pub const fn head_at(&self) -> u32 {
        page_up(self.stack_bytes)
    }

    /// A frame's room: its words and the most its packet takes.
    pub const fn frame_bytes(&self) -> u32 {
        WORD_BYTES + self.pkt_bytes
    }

    /// The stream lies on the page past the header's, and this is 0 without
    /// one.
    pub const fn stream_at(&self) -> u32 {
        match self.stream {
            Some(_) => self.head_at() + PAGE_BYTES,
            None => 0,
        }
    }

    /// This is the header the runtime writes in the front for this manifest.
    pub const fn head(&self) -> Head {
        Head {
            pkt_bytes: self.pkt_bytes,
            stream_at: self.stream_at(),
        }
    }

    /// The end of the front: a module's linker may begin its data here
    /// (`--global-base`), and a native core's own ranges begin here.
    pub const fn global_base(&self) -> u32 {
        let end = self.head_at() + PAGE_BYTES;
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

    /// These are the numbers a module core of this manifest is linked with.
    pub const fn link(&self) -> LinkConfig {
        LinkConfig {
            stack_bytes: self.stack_bytes,
            global_base: Some(self.global_base()),
            init_bytes: Some(self.min_mem_bytes()),
        }
    }

    /// Called in a const, this fails the build for a manifest that cannot run: a
    /// packet's room that is not a whole number of XTP words, or that cannot
    /// hold a stop report with its file path left out (`ABORT_PKT_BYTES`); a
    /// memory size that is not a whole number of regions; or a smallest memory
    /// size too small for the front, `fixed_bytes` and the heap. The linker's
    /// data is counted from the end of the front (`global_base`), as a native
    /// core's ranges are.
    pub const fn check(&self, fixed_bytes: u32) {
        assert!(self.pkt_bytes >= ABORT_PKT_BYTES && self.pkt_bytes.is_multiple_of(8));
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
