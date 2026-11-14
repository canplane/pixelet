// pxl-render/frame/shader/decor_instance.wgsl

// The decor's instances (gpu/generator.rs `instances`): `drawing::COLUMN_CNT` lanes and the index
// list, each a word a slot. `decor_compact_cs` writes them, and `decor_args_cs` lets a region
// draw no more slots than they hold.

@group(0) @binding(15) var<storage, read_write> instances: array<u32>;

fn instance_cap() -> u32 {
  return arrayLength(&instances) / (DRAWING_COLUMN_CNT + 1u);
}
