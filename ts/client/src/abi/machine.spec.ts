// ts/client/src/abi/machine.spec.ts
//
// GENERATED from rust/client/kernel/machine.json

/** The machine its JSON declares: the sizes a host may give, what a
 * reservation is counted in, the rings' sizes, where the front's header,
 * the rings, their slots and the GPU stream lie (0 without the socket),
 * which the host lays before the first turn, and the GPU socket where it
 * is taken. */
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
  gpu_at: 7376896,
  gpu: {
    bytes: 4194304,
    power: "high-performance",
    features: {
      required: ["depth32float-stencil8"],
      optional: ["timestamp-query"],
    },
  },
} as const;
