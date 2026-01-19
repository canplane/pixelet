// @xpute/core/ipc/worker/device_loader.ts

import type { Device, DeviceUplink } from "@xpute/spec/ipc/link.ts";
import type { DeviceSpec } from "@xpute/spec/ipc/topology.ts";

type DeviceCtor = new (tx: DeviceUplink) => Device;

/**
 * [Dynamic Asset Loader]
 * 역할(Role)에 맞는 구현체(Class or Wasm)를 동적으로 로딩합니다.
 */
async function load_ctor(device_spec: DeviceSpec): Promise<DeviceCtor> {
  switch (device_spec.type) {
    // 1. TypeScript/JavaScript Class Loading
    case "JS": {
      const module = await device_spec.load();
      return module.default as DeviceCtor;
    }
    // 2. WebAssembly Module Loading
    case "WASM": {
      // TODO: Instantiate WASM module and return interface
      throw new Error("WASM Loading not yet implemented");
    }
    default: {
      throw new Error(`[Loader] Unsupported Device Spec: ${device_spec}`);
    }
  }
}

export async function load_device(device_spec: DeviceSpec, tx: DeviceUplink): Promise<Device> {
  const ctor: DeviceCtor = await load_ctor(device_spec);
  return new ctor(tx);
}
