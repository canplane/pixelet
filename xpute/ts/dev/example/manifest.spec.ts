// ts/dev/example/manifest.spec.ts
//
// GENERATED from rust/dev/example/manifest.json

/** The manifest its JSON declares: the sizes a runtime may give, what a
 * reservation is counted in, the most a frame's packet takes, where the
 * front's header and the stream lie (0 without one), which
 * the runtime lays before the first turn, whether it asks for the file
 * device, and the GPU it asks for. */
export const MANIFEST = {
  mem_bytes: [2097152],
  region_bytes: 2097152,
  pkt_bytes: 256,
  head_at: 65536,
  stream_at: 0,
  fs: false,
  gpu: null,
} as const;
