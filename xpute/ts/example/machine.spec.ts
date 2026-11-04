// ts/example/machine.spec.ts
//
// GENERATED from rust/example/machine.json

/** The machine its JSON declares: the sizes a host may give, what a
 * reservation is counted in, the rings' sizes, where the front's header,
 * the rings, their slots and the GPU stream lie (0 without the socket),
 * which the host lays before the first turn, and the GPU socket where it
 * is taken. */
export const MACHINE = {
  mem_bytes: [2097152],
  region_bytes: 2097152,
  sq_cap: 8,
  cq_cap: 16,
  slot_bytes: 256,
  head_at: 65536,
  sq_at: 69632,
  cq_at: 73728,
  sq_slots_at: 77824,
  cq_slots_at: 79872,
  gpu_at: 0,
  gpu: null,
} as const;
