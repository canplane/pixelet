// @pxl/client/runtime/kernel.ts

/** The kernel as the client reaches it once booted (abi/boot.ts): the shell
 * port's commands bound to what sends them, and what the shell asks of the
 * runtime besides its frames. Beside the runtime (a device), `runtime` is it
 * too, for what reads the kernel's memory; a page's shell has none, its runtime
 * in the kernel's worker. Set once, before anything reaches them. */

import type { Encode, Runtime, Sender } from "@xpute/runtime";
import type { NodeValue, NodeView } from "@xpute/kit/wire/xtp/mod.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { shell_exports, type ShellExports } from "@/abi/cmd.spec.ts";

/** What the shell asks of the runtime besides the kernel's commands. */
export interface Control {
  /** Turns run while the world's view is open (`Runtime.resume`, `pause`). */
  resume(): void;
  pause(): void;
  /** A turn asked for, as something the kernel did not hear of changed. */
  wake(): void;
  /** An input now: the turns after it are interactive (frame pacing). */
  input(): void;
  /** Whether the platform shows the program (`Runtime.visible`). */
  visible(visible: boolean): void;
  /** Whether a command's packet fits a ring's slot. */
  fits(encode: Encode): boolean;
  /** A development console's command, its arguments its packet's fields. */
  cmd(cmd: u32, ...args: NodeValue<NodeView>[]): void;
  /** A page's canvas lent to the GPU under the kernel's handle, and let go. */
  bind(handle: u32, canvas: HTMLCanvasElement): void;
  unbind(handle: u32): void;
  /** A profiler's reading (port/perf.ts `read`), made where the kernel's
   * memory is. */
  perf(what: string, args: unknown[]): Promise<unknown>;
}

export let kernel: ShellExports;
export let control: Control;
export let runtime: Runtime;

export function set_kernel(sender: Sender, c: Control): void {
  kernel = shell_exports(sender);
  control = c;
}

/** The runtime beside what reads the kernel's memory. */
export function set_runtime(r: Runtime): void {
  runtime = r;
}
