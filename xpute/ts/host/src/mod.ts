// @xpute/host

/**
 * What a program takes of the host: a guest booted as its machine declares,
 * by this build's platform (`boot`), or mounted where the program opened it
 * (`mount`), with all the program decides — what backs its files, what hears
 * its commands, its policy — and the host that gives (`Host`). The files
 * under `src/` are xpute's own.
 */

export {
  type Abort,
  boot,
  type Config,
  type Gpu,
  type GpuAsk,
  type GpuGrant,
  type Guest,
  type Host,
  type Machine,
  type ModuleSource,
  mount,
} from "./host.ts";
export type { Encode, OnEvent } from "./ipc/doorbell.ts";
export { type Binding, Namespace } from "./ipc/ns.ts";
export { FileError, type Fs, FsServer } from "./ipc/fs.ts";
export { type Front, Head } from "./ipc/head.ts";
export { reserve_mem } from "./mem/reserve.ts";
export { open_module } from "./module.ts";
export { FixedQuota } from "./sched/fixed_quota.ts";
export { type FrameGrant, FramePacing } from "./sched/frame_pacing.ts";
export type { Grant, SchedulingPolicy } from "./sched/policy.ts";
