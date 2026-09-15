// xpute_core/sys/memory.rs

//! A linear memory cut into sections: the mechanism under a kernel's memory
//! map.
//!
//! The central scheduler reserves the memory and gives it to the kernel; the
//! kernel lays out everything inside it: a section is a range of whole
//! regions, each the range one pool cuts, none borrowing from another. What
//! the sections are for, and how many regions each takes, is the kernel's
//! policy, not this file's. The reservation itself (`reserve_memory`) is the
//! host's.

/// A WebAssembly page.
pub const WASM_PAGE_BYTES: u32 = 1 << 16;

/// Words a section takes in a boot block: its offset, bytes and region size.
pub const SECTION_WORDS: u32 = 3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Section {
    pub offset: u32,
    pub bytes: u32,
    /// The region size the section's pool cuts its range in.
    pub region_bytes: u32,
}

/// Sections over a memory of `bytes`, in address order, into `out`, which
/// holds one more than `regions`. The first starts past `front_bytes` — what
/// the module keeps for itself — and ends at region `regions[0]`, cut in
/// `first_region_bytes`; each next takes `regions[i]` regions; the last takes
/// every whole region left, at least one. Panics where the TypeScript throws.
pub fn lay_out(bytes: u32, front_bytes: u32, region_bytes: u32, first_region_bytes: u32, regions: &[u32], out: &mut [Section]) {
    let total = bytes / region_bytes;
    let asked: u32 = regions.iter().sum();
    assert!(!regions.is_empty() && out.len() == regions.len() + 1, "memory: a section in `out` for each of `regions` and one more");
    let first_end = regions[0] * region_bytes;
    assert!(
        front_bytes < first_end && front_bytes.is_multiple_of(first_region_bytes) && (first_end - front_bytes).is_multiple_of(first_region_bytes),
        "memory: a first section past the front is not whole regions"
    );
    assert!(total > asked, "memory: too few regions for the sections");
    out[0] = Section {
        offset: front_bytes,
        bytes: first_end - front_bytes,
        region_bytes: first_region_bytes,
    };
    let mut end = first_end;
    for (i, &n) in regions.iter().enumerate().skip(1) {
        out[i] = Section {
            offset: end,
            bytes: n * region_bytes,
            region_bytes,
        };
        end += n * region_bytes;
    }
    out[regions.len()] = Section {
        offset: end,
        bytes: total * region_bytes - end,
        region_bytes,
    };
}

/// Writes a section's words at `at`.
pub fn write_section(words: &mut [u32], at: usize, s: Section) {
    words[at..at + SECTION_WORDS as usize].copy_from_slice(&[s.offset, s.bytes, s.region_bytes]);
}

/// The section whose words are at `at`.
pub fn read_section(words: &[u32], at: usize) -> Section {
    Section {
        offset: words[at],
        bytes: words[at + 1],
        region_bytes: words[at + 2],
    }
}

#[cfg(test)]
#[path = "memory.test.rs"]
mod test;
