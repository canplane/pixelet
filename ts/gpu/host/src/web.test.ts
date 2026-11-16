// @pxl/gpu/host/src/web.test.ts

import { afterEach, test, vi } from "vitest";
import assert from "node:assert/strict";

import { ERRNO_EINPROGRESS } from "@xpute/kit/status/errno.spec.ts";
import { DEVICE_READY, GET_DEVICE, HANDLE } from "./abi/gpu.ts";
import { Lift, Lower } from "@xpute/link/cabi.ts";
import type { LinkObject, Side, Tag } from "@xpute/link";
import { type GpuReport, WebGpu } from "./web.ts";

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

/** Asks the guest's device ready on a page whose `navigator` is `nav`, and
 * answers what the worker posted the page once the ask is answered. */
async function ready(nav: object): Promise<GpuReport[]> {
  vi.stubGlobal("navigator", nav);
  vi.stubGlobal("isSecureContext", true);
  const posted: GpuReport[] = [];
  vi.stubGlobal("postMessage", (m: GpuReport) => posted.push(m));
  const named = new Map<number, LinkObject>();
  const added: LinkObject[] = [];
  const answered = Promise.withResolvers<Tag>();
  const link = {
    mount_named: (h: number, o: LinkObject) => named.set(h, o),
    add: (o: LinkObject) => added.push(o),
    task_return: (t: Tag) => answered.resolve(t),
    on_cancel: () => {},
    mem: new ArrayBuffer(0),
  };
  new WebGpu(
    link as unknown as Side,
    { power: "high-performance", required_feats: [], optional_feats: [] },
    { frame_latency: 2, upload_chunk_sz: 1 << 16 },
  ).mount();
  const reply = named.get(HANDLE)!.call(GET_DEVICE, 0, null);
  assert.ok(typeof reply !== "number" && reply.data !== null);
  const out = new Uint8Array(4);
  reply.data[1](new Lower(out, 4));
  const device = added[new Lift(out).u32(0) - 1]!;
  assert.equal(device.call(DEVICE_READY, 7, null), ERRNO_EINPROGRESS);
  assert.equal(await answered.promise, 7);
  return posted;
}

test("the worker posts the page what its ask found of a GPU", async () => {
  assert.deepEqual(await ready({}), [{ kind: "gpu", state: "unsupported" }]);
  assert.deepEqual(await ready({ gpu: { requestAdapter: async () => null } }), [
    { kind: "gpu", state: "missing" },
  ]);
});
