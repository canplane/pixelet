// ts/client/src/abi/manifest.spec.ts
//
// GENERATED from spec/client/core/sys/manifest.json

/** The manifest its JSON declares: the sizes a runtime may give, what a
 * reservation is counted in, the most a frame's packet takes, where the
 * front's header and the stream lie (0 without one), which
 * the runtime lays before the first turn, whether it asks for the file
 * device, and the GPU it asks for. */
export const MANIFEST = {
  mem_bytes: [1073741824, 536870912],
  region_bytes: 67108864,
  pkt_bytes: 4096,
  head_at: 1048576,
  stream_at: 1052672,
  fs: true,
  gpu: {
    power: "high-performance",
    features: {
      required: ["depth32float-stencil8"],
      optional: ["timestamp-query"],
    },
  },
} as const;
