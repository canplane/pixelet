// ts/client/src/abi/machine.spec.ts
//
// GENERATED from spec/client/core/sys/machine.json

/** The machine its JSON declares: the sizes a runtime may give, what a
 * reservation is counted in, the rings' sizes, where the front's header,
 * the rings, their slots and the stream lie (0 without one), which
 * the runtime lays before the first turn, whether it asks for the file
 * device, and the GPU it asks for. */
export const MACHINE = {
  mem_bytes: [1073741824, 536870912],
  region_bytes: 67108864,
  sq_cap: 512,
  cq_cap: 1024,
  slot_bytes: 4096,
  head_at: 1048576,
  sq_at: 1052672,
  cq_at: 1064960,
  sq_slots_at: 1085440,
  cq_slots_at: 3182592,
  stream_at: 7376896,
  fs: true,
  gpu: {
    power: "high-performance",
    features: {
      required: ["depth32float-stencil8"],
      optional: ["timestamp-query"],
    },
  },
} as const;
