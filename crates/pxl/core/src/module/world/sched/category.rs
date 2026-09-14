// pxl_core/module/world/sched/category.rs

//! Static per-category classification — set once from profiling, read by the
//! dev overlay and the findings analysis, never fed back into scheduling. This
//! is the "arithmetic intensity / bound-class" row of
//! issue/20260909-cooperative-scheduler-and-task-ledger.md: it says which
//! optimization lever a hotspot in this category needs, not how fast it
//! currently is.
//!
//! - network   → prefetch + priority; a faster kernel does nothing
//! - memory    → touch less data / overlap on a worker; WASM+SIMD barely helps
//! - compute   → WASM + SIMD pays here
//! - gpu-fill  → fewer post-fx passes / lower dpr / less overdraw

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundClass {
    Network,
    Memory,
    Compute,
    GpuFill,
}

impl BoundClass {
    pub fn as_str(self) -> &'static str {
        match self {
            BoundClass::Network => "network",
            BoundClass::Memory => "memory",
            BoundClass::Compute => "compute",
            BoundClass::GpuFill => "gpu-fill",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CategoryInfo {
    pub bound: BoundClass,
    /// one line on the lever this bound implies for this category.
    pub note: &'static str,
}

const CATEGORIES: &[(&str, CategoryInfo)] = &[
    (
        "terrain",
        CategoryInfo {
            bound: BoundClass::Network,
            note: "Measured 2026-09-09: 4.2MB/page, mean net 322.5ms vs decode 0.6ms — decode_terrain is an XTP cursor read and is free. The demand is network-bound end to end.",
        },
    ),
    (
        "poi",
        CategoryInfo {
            bound: BoundClass::Network,
            note: "Measured 2026-09-09: mean net 269.3ms vs JSON.parse 2.2ms on ~114KB pages (295KB worst). Latency-bound; lever is priority order, not parallelism.",
        },
    ),
    (
        "geocode",
        CategoryInfo {
            bound: BoundClass::Network,
            note: "single-slot latest-wins; one small page lookup, pure RTT. Already debounced upstream, so the lever left is queue priority.",
        },
    ),
    (
        "search",
        CategoryInfo {
            bound: BoundClass::Network,
            note: "the one feature with no static bake — always a live API round trip, and the user is watching. Runs ahead of every streaming category on purpose.",
        },
    ),
];

pub fn category_info(category: &str) -> Option<CategoryInfo> {
    CATEGORIES.iter().find(|(name, _)| *name == category).map(|(_, info)| *info)
}
