// @xpute/core/ipc/worker/device_loader.ts

import type { Device, DeviceUplink } from "@xpute/spec/ipc/link.ts";
import type { BindgenDeviceSpec, BindgenRuntime, DeviceSpec, JsDeviceSpec, XputeManifest } from "@xpute/spec/ipc/topology.ts";
import { DevicePort, DeviceType } from "@xpute/spec/ipc/topology.ts";

type DeviceCtor = new (tx: DeviceUplink) => Device;

const runtimes_cache = new Map<DeviceType, Promise<any>>();

const actions = {
  [DeviceType.JS]: async (port: DevicePort, spec: JsDeviceSpec, tx: DeviceUplink): Promise<Device> => {
    const mod = await spec.load();

    const ctor = mod[spec.name] as DeviceCtor;
    if (typeof ctor !== "function") throw new Error(`${port}: not a constructor: ${spec.name}`);
    return new ctor(tx);
  },

  [DeviceType.BINDGEN]: async (port: DevicePort, spec: BindgenDeviceSpec, tx: DeviceUplink, runtime: BindgenRuntime): Promise<Device> => {
    const mod = await runtime.load(spec.js_url);
    const memory = runtime.create_memory?.(spec.memory);
    await runtime.init(mod, spec.wasm_url, memory);

    const ctor = mod[spec.name];
    if (typeof ctor !== "function") throw new Error(`${port}: not a constructor: ${spec.name}`);
    return new ctor(tx);
  },
};

export async function load_device(port: DevicePort, spec: DeviceSpec, tx: DeviceUplink, loaders: XputeManifest["wasm"]): Promise<Device> {
  if (spec.type === DeviceType.JS) {
    return actions[DeviceType.JS](port, spec, tx);
  }

  let runtime_promise = runtimes_cache.get(spec.type);
  if (!runtime_promise) {
    runtime_promise = loaders[spec.type]().then((mod) => mod.default);
    runtimes_cache.set(spec.type, runtime_promise);
  }
  const runtime = await runtime_promise;
  const device: Device = await actions[spec.type](port, spec, tx, runtime);

  return device;
}
