// @pxl/native/platform/host.ts

/**
 * The kernel's host on a device, as it stands before the kernel is built for
 * one: it starts nothing and hears every command, so the shell can be run
 * and looked at over an empty world. The Rust kernel, linked into the app,
 * is what takes its place — the same five calls the web's host answers
 * (apps/web/src/platform/web/host.ts).
 */

import type { U8Array } from "@xpute/core/abi/array.ts";
import type { u32 } from "@xpute/core/abi/word.ts";
import type { GroundChannelKey } from "@/catalog/cell/cube_type.spec.ts";
import type { ActiveTool, Command, Tracking } from "@/kernel/abi/cmd.spec.ts";
import type { UnsupportedReason } from "@/app/store/unsupported-store.ts";

export function host_boot(): Promise<UnsupportedReason | null> {
  return Promise.resolve(null);
}

export function host_command(cmd: Command, _pkt?: U8Array): void {
  console.log(`[host] command ${cmd}`);
}

export function host_tracking(tracking: Tracking): Promise<void> {
  console.log(`[host] tracking ${tracking}`);
  return Promise.resolve();
}

export function host_enable_sensors(): void {
  console.log("[host] sensors");
}

export function host_hold(_tool: ActiveTool, _channel: GroundChannelKey | null, _value: u32, _tier: u32): void {}
