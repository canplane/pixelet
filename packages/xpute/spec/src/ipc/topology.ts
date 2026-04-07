// @xpute/spec/ipc/topology.ts

import type { NodeSelector } from "@xpute/spec/rpc/node.ts";
import type { AdvertByKind, Dial } from "@xpute/spec/net/xport/mod.ts";

// ---- Workers ----

const WORKER_ID_W = 8;
export const WORKER_ID_CAP = 1 << WORKER_ID_W; // 256
export const WORKER_ID_SHAMT = 32 - WORKER_ID_W;

export const enum WorkerID {
  MAIN = 1, // Main thread (I/O bound)
  CORE = 2, // CPU worker

  GUEST = WORKER_ID_CAP - 1, // Guest linker (Virtual worker)
}

// ---- Devices ----

export const enum DevicePort {
  ENGINE = 1,

  ENGINE_TS = 2,
  ENGINE_SPARE = 3,
}

// export type WorkerID = u8;
// export type DevicePort = u16;

// ---- System Manifest ----

export type ModuleLoader<T> = () => Promise<{ default: T }>;
export type NamespaceLoader = () => Promise<Record<string, any>>;

export const enum DeviceType {
  JS = "JS",
  BINDGEN = "BINDGEN",
}

// Loadable entry point (Module specifier / WASM URL)
export type DeviceSpec =
  | JsDeviceSpec
  | BindgenDeviceSpec;

export type JsDeviceSpec = {
  type: DeviceType.JS;
  name: string;

  load: NamespaceLoader;
};

export type BindgenModule = {
  default: (input?: {
    module_or_path?: string | URL | Response | WebAssembly.Module | BufferSource;
    memory?: WebAssembly.Memory;
  }) => Promise<any>;
  [key: string]: any;
};

export type BindgenRuntime = {
  load(js_url: string | URL): Promise<BindgenModule>;
  init(mod: BindgenModule, wasm_url: string | URL, memory?: WebAssembly.Memory): Promise<any>;
  create_memory?(opts?: { initial?: number; maximum?: number }): WebAssembly.Memory;
};

export type BindgenDeviceSpec = {
  type: DeviceType.BINDGEN;
  name: string;

  js_url: string | URL;
  wasm_url: string | URL;
  memory?: {
    initial?: number;
    maximum?: number;
  };
};

// NOTE: Worker.GUEST is virtual; do not list it in runtime topology.
export type DeviceTopology = readonly [DevicePort, DeviceSpec][];
export type Topology = readonly [WorkerID, DeviceTopology][];

// NOTE: Prefer `satisfies XputeManifest` for concrete manifests (avoid `as const` noise).
export type XputeManifest = {
  worker: ModuleLoader<any>;
  wasm: {
    [DeviceType.BINDGEN]: ModuleLoader<BindgenRuntime>;
  };

  guest: ModuleLoader<any>;
  devices: Topology;

  net: {
    /** * [roles]
     * 이 노드가 네트워크 내에서 수행하는 논리적 역할들의 비트 마스크.
     * MULTICAST/ANYCAST 시 패킷 필터링의 기준이 됨.
     */
    role: NodeSelector;
    /** * [seeds]
     * 시스템의 위상(Topology) 정보를 담고 있는 '씨앗' 주소 리스트입니다.
     * 단일 주소라도 배열로 관리하여 미래의 Multi-CDN 확장성을 확보합니다.
     */
    seeds: Dial[];
    /**
     * Optional transport advert hints exposed during HELLO.
     * - transport-specific interpretation (kind-gated by bootstrap policy)
     * - keep manifest-level and runtime-agnostic
     */
    advert?: AdvertByKind;
  };
};
