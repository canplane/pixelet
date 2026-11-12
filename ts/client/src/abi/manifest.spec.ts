// ts/client/src/abi/manifest.spec.ts
//
// GENERATED from spec/client/core/sys/manifest.json

/** The manifest its JSON declares: the core's memory, whole before its
 * first entry and never grown, and the GPU the core asks for. */
export const MANIFEST = {
  mem_bytes: 536870912,
  gpu: {
    power: "high-performance",
    features: {
      required: ["depth32float-stencil8"],
      optional: ["timestamp-query"],
    },
  },
} as const;
