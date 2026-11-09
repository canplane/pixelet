// @xpute/runtime

/**
 * An app takes this from the runtime. From one manifest it starts its core
 * where the platform runs it (`start`, with `serve` inside a page's worker),
 * or boots the core beside it (`boot`). It also takes the UI's link to the
 * core, the types of what the app decides (its files, its policy, its GPU),
 * and the devices' commands it sends with. The runtime's other parts are
 * imported by path.
 */

export { serve } from "./remote.ts";
export { start } from "./start";
export {
  type Abort,
  boot,
  type Canvas,
  type Config,
  type Gpu,
  type GpuAsk,
  type GpuGrant,
  type GpuState,
  type Link,
  mem,
  type Machine,
  type Manifest,
  type ModuleSource,
  type Protocol,
  read_abort,
  QUIET,
  type Ui,
} from "./runtime.ts";
export type { Encode, OnImport } from "./ipc/doorbell.ts";
export { type Binding, Namespace } from "./ipc/ns.ts";
export { FileError, type Fs } from "./ipc/fs.ts";
export { Command as FsCommand, type FsProtocol } from "./abi/fs.spec.ts";
export { Command as GpuCommand, type GpuProtocol } from "./abi/gpu.spec.ts";
export { Timeslice } from "./sched/timeslice.ts";
export { type FrameGrant, FramePacing } from "./sched/frame_pacing.ts";
export { next_frame } from "./platform";
export type { Grant, SchedulingPolicy } from "./sched/policy.ts";
