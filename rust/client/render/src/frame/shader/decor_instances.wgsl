// pxl-render/decor_instances.wgsl

// The decor's instances, `drawn::COLUMNS` lanes and the index list a slot each (gpu/generator.rs
// `instances`): what `decor_compact_cs` writes, and the slots `decor_args_cs` lets a region draw.

@group(0) @binding(15) var<storage, read_write> instances: array<u32>;

/// The slots the instances hold.
fn instance_cap() -> u32 {
  return arrayLength(&instances) / (DRAWN_COLUMNS + 1u);
}
