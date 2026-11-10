// ts/dev/triangle/machine.spec.ts
//
// GENERATED from rust/dev/triangle/machine.json

/** The machine its JSON declares: the sizes a runtime may give, what a
 * reservation is counted in, the rings' sizes, where the front's header,
 * the rings, their slots and the stream lie (0 without one), which
 * the runtime lays before the first turn, whether it asks for the file
 * device, and the GPU it asks for. */
export const MACHINE = {
  mem_bytes: [4194304],
  region_bytes: 4194304,
  sq_cap: 8,
  cq_cap: 16,
  slot_bytes: 256,
  head_at: 65536,
  sq_at: 69632,
  cq_at: 73728,
  sq_slots_at: 77824,
  cq_slots_at: 79872,
  stream_at: 83968,
  fs: false,
  gpu: { power: "low-power", features: { required: [], optional: [] } },
} as const;
