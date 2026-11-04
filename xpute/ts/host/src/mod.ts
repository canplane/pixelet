// @xpute/host

/**
 * What a program takes of the host: a guest opened over a memory of its own
 * (`open_module`, `reserve_mem`), mounted in one call (`mount`) with all the
 * program decides — its toc, its hostcalls' answers, its ports, its policy —
 * and the host that gives (`Host`). The files under `src/` are xpute's own.
 */

export {
  type Config,
  type Guest,
  type Host,
  mount,
  type Stream,
} from "./host.ts";
export type { OnEvent } from "./ipc/doorbell.ts";
export { type Binding, Namespace } from "./ipc/ns.ts";
export { FileError, type Fs, type SysHost } from "./ipc/sys.ts";
export { Toc } from "./ipc/toc.ts";
export { reserve_mem } from "./mem/reserve.ts";
export { open_module } from "./module.ts";
export { FixedQuota } from "./sched/fixed_quota.ts";
export { type FrameGrant, FramePacing } from "./sched/frame_pacing.ts";
export type { Grant, SchedulingPolicy } from "./sched/policy.ts";
