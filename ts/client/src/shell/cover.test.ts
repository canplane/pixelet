// @pxl/client/shell/cover.test.ts

import { expect, test } from "vitest";

import { Boot, Gpu, Source } from "@/abi/state.ts";
import { coverOf, type Standing } from "./cover.ts";

const running: Standing = {
  boot: Boot.Started,
  gpu: Gpu.Present,
  source: Source.Declared,
  online: true,
  givenUp: false,
  readFailing: false,
  stopCode: null,
  made: 1,
};
const starting: Standing = {
  ...running,
  boot: Boot.Starting,
  source: Source.Asking,
  gpu: Gpu.Asking,
  made: 0,
};
const loading: Standing = { ...running, boot: Boot.Loading, made: 0.5 };

test("coverOf - before it ran: failed where it will not, progress while it waits", () => {
  const of = (s: Partial<Standing>) => coverOf({ ...starting, ...s });
  for (const [s, key] of [
    [{ boot: Boot.Failed }, "failed.boot"],
    [{ gpu: Gpu.Missing }, "failed.gpu"],
    [{ source: Source.Refused }, "failed.source"],
  ] as const) {
    expect(of(s)).toEqual({ kind: "failed", of: key });
  }
  for (const [s, key] of [
    [{}, "progress.gpu"],
    [{ gpu: Gpu.Waiting }, "progress.gpu"],
    [{ online: false }, "progress.offline"],
    [{ source: Source.Failing }, "progress.server"],
    [{ gpu: Gpu.Present }, "progress.source"],
  ] as const) {
    expect(of(s)).toEqual({ kind: "progress", of: key });
  }
});

test("coverOf - loading, the world under it: what it waits on, then a panic if broken", () => {
  const of = (s: Partial<Standing>) => coverOf({ ...loading, ...s });
  for (const [s, key] of [
    [{}, "progress.shaders"],
    [{ made: 1 }, "toast.world_loading"],
    [{ online: false }, "progress.offline"],
    [{ readFailing: true }, "progress.server"],
    [{ gpu: Gpu.Waiting }, "progress.gpu"],
  ] as const) {
    expect(of(s)).toEqual({ kind: "progress", of: key });
  }
  expect(of({ gpu: Gpu.Lost })).toMatchObject({ kind: "panic" });
  expect(of({ givenUp: true })).toMatchObject({ kind: "panic" });
});

test("coverOf - once it shows, a panic or nothing", () => {
  const of = (s: Partial<Standing>) => coverOf({ ...running, ...s });
  expect(of({})).toBeNull();
  expect(of({ online: false })).toBeNull();
  expect(of({ gpu: Gpu.Waiting })).toBeNull();
  expect(of({ readFailing: true })).toBeNull();
  expect(of({ gpu: Gpu.Lost })).toMatchObject({
    kind: "panic",
    code: "GPU_LOST",
  });
  expect(of({ givenUp: true })).toMatchObject({
    kind: "panic",
    code: "WORLD_UNAVAILABLE",
  });
  expect(of({ boot: Boot.Stopped, stopCode: "X" })).toEqual({
    kind: "panic",
    of: "panic.kernel",
    code: "X",
  });
});
