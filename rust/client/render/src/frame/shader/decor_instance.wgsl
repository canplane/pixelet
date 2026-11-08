// pxl-render/decor_instance.wgsl

// The decor's instances (gpu/generator.rs `instances`): `drawn::COLUMNS` lanes and the index
// list, each a word a slot. `decor_compact_cs` writes them, and `decor_args_cs` lets a region
// draw no more slots than they hold.

@group(0) @binding(15) var<storage, read_write> instances: array<u32>;

/// How many slots the instances hold.
fn instance_cap() -> u32 {
  return arrayLength(&instances) / (DRAWN_COLUMNS + 1u);
}
