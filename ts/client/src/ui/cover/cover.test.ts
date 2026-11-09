// @pxl/client/ui/cover/cover.test.ts

import { expect, test } from "vitest";

import { Message } from "@/abi/cmd.spec.ts";

import {
  BOOT_FAILED,
  BOOT_LOADING,
  BOOT_OPENING,
  BOOT_STARTED,
  BOOT_STARTING,
  BOOT_TERMINATED,
  GPU_ASKING,
  GPU_LOST,
  GPU_INSECURE,
  GPU_MISSING,
  GPU_UNSUPPORTED,
  GPU_PRESENT,
  GPU_WAITING,
  SOURCE_ASKING,
  SOURCE_DECLARED,
  SOURCE_FAILING,
  SOURCE_REFUSED,
} from "@/abi/state.ts";
import { coverOf, type Standing } from "./cover.ts";

const running: Standing = {
  boot: BOOT_STARTED,
  gpu: GPU_PRESENT,
  source: SOURCE_DECLARED,
  online: true,
  stall: null,
  stopCode: null,
  progress: 1,
  area: 1,
  placed: true,
};
const starting: Standing = {
  ...running,
  boot: BOOT_STARTING,
  source: SOURCE_ASKING,
  gpu: GPU_ASKING,
  progress: 0,
};
const loading: Standing = { ...running, boot: BOOT_LOADING, progress: 0.5 };

test("coverOf - before it ran: failed where it will not, progress while it waits", () => {
  const of = (s: Partial<Standing>) => coverOf({ ...starting, ...s });
  for (const [s, key] of [
    [{ boot: BOOT_FAILED }, "failed.boot"],
    [{ gpu: GPU_MISSING }, "failed.gpu"],
    [{ gpu: GPU_UNSUPPORTED }, "failed.webgpu"],
    [{ gpu: GPU_INSECURE }, "failed.insecure"],
    [{ source: SOURCE_REFUSED }, "failed.source"],
  ] as const) {
    expect(of(s)).toEqual({ kind: "failed", of: key });
  }
  for (const [s, key] of [
    [{}, "progress.gpu"],
    [{ gpu: GPU_WAITING }, "progress.gpu"],
    [{ online: false }, "progress.offline"],
    [{ source: SOURCE_FAILING }, "progress.server"],
    // The source is asked for beside the GPU and has no step of its own.
    [{ gpu: GPU_PRESENT }, "progress.shaders"],
  ] as const) {
    expect(of(s)).toEqual({
      kind: "progress",
      of: key,
      share: expect.any(Number),
    });
  }
});

test("coverOf - opening the core: its own step, before any GPU is asked for", () => {
  expect(coverOf({ ...starting, boot: BOOT_OPENING })).toEqual({
    kind: "progress",
    of: "progress.app",
    share: expect.any(Number),
  });
});

test("coverOf - loading, the world under it: what it waits on, then a panic if broken", () => {
  const of = (s: Partial<Standing>) => coverOf({ ...loading, ...s });
  for (const [s, key] of [
    [{}, "progress.shaders"],
    [{ progress: 1 }, "progress.area"],
    [{ progress: 1, placed: false }, "progress.location"],
    [{ placed: false }, "progress.shaders"],
    [{ online: false }, "progress.offline"],
    [{ stall: Message.WorldReadFailing }, "progress.server"],
    [{ gpu: GPU_WAITING }, "progress.gpu"],
  ] as const) {
    expect(of(s)).toEqual({
      kind: "progress",
      of: key,
      share: expect.any(Number),
    });
  }
  expect(of({ gpu: GPU_LOST })).toMatchObject({ kind: "panic" });
  expect(of({ stall: Message.WorldUnavailable })).toMatchObject({
    kind: "panic",
  });
});

test("coverOf - once it shows, a panic or nothing", () => {
  const of = (s: Partial<Standing>) => coverOf({ ...running, ...s });
  expect(of({})).toBeNull();
  expect(of({ online: false })).toBeNull();
  expect(of({ gpu: GPU_WAITING })).toBeNull();
  expect(of({ stall: Message.WorldReadFailing })).toBeNull();
  expect(of({ gpu: GPU_LOST })).toMatchObject({
    kind: "panic",
    code: "GPU_LOST",
  });
  expect(of({ stall: Message.WorldUnavailable })).toMatchObject({
    kind: "panic",
    code: "WORLD_UNAVAILABLE",
  });
  expect(of({ boot: BOOT_TERMINATED, stopCode: "X" })).toEqual({
    kind: "panic",
    of: "panic.app",
    code: "X",
  });
});

test("coverOf - the bar's share: half what the renderer makes, half what shows", () => {
  const share = (progress: number, area: number) =>
    coverOf({ ...loading, progress, area, placed: true });
  expect(share(0, 0)).toMatchObject({ share: 0 });
  expect(share(1, 0)).toMatchObject({ share: 0.5 });
  expect(share(1, 0.5)).toMatchObject({ share: 0.75 });
  expect(share(1, 1)).toMatchObject({ share: 1 });
});
