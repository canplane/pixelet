// pxl-kernel/mem/fixed.test.rs

use super::*;

/// The architecture's map of the memory, which shows these ranges.
const ARCHITECTURE: &str = include_str!("../../../../../../ARCHITECTURE.md");

const NAMES: [&str; COUNT] = ["terrain", "bake", "mounts", "blocks", "gpu", "host", "profiler"];

/// Bytes as the map writes them: whole KiB under a MiB, MiB to two places
/// over one, trailing zeros dropped.
fn bytes(n: u32) -> String {
    if n < 1 << 20 {
        format!("{} KiB", n.div_ceil(1 << 10))
    } else {
        let mib = format!("{:.2}", f64::from(n) / f64::from(1u32 << 20));
        format!("{} MiB", mib.trim_end_matches('0').trim_end_matches('.'))
    }
}

/// A row of the map, in its columns.
fn row(at: &str, size: &str, range: &str, by: &str) -> String {
    format!("{at:<15}{size:<25}{range:<23}{by}").trim_end().to_string()
}

/// `.fixed` and where the heap begins, as the map must show them: each
/// range's bytes, and its place past `D`, where the linker's data ends.
/// The map is held to the ranges here rather than kept by hand, since the
/// numbers move whenever a range does and the prose around them does not.
#[test]
fn the_architecture_s_memory_map_is_the_kernel_s() {
    let mut want = Vec::new();
    let mut at = 0u32;
    for (k, &n) in RANGES.iter().enumerate() {
        let place = if at == 0 { "D".to_string() } else { format!("D + {}", bytes(at)) };
        want.push(row(&place, &bytes(n), &format!(".fixed.{}", NAMES[k]), "the kernel (mem/fixed.rs)"));
        at += n.div_ceil(PAGE_BYTES) * PAGE_BYTES;
    }
    assert_eq!(at, BYTES);
    want.push(row(&format!("D + {}", bytes(BYTES)), "the rest", ".heap", "the allocator"));
    let want = want.join("\n");
    assert!(ARCHITECTURE.contains(&want), "ARCHITECTURE.md's map of .fixed is not the kernel's; it is:\n{want}\n");
}
