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
  ENGINE_SPARE = 2,
}

// export type WorkerID = u8;
// export type DevicePort = u16;

// ---- System Manifest ----

/** * [Loader]
 * 모듈을 비동기적으로 가져오는 함수 타입.
 * WASM 또는 JS 바이너리를 동적으로 런타임에 주입하기 위해 사용.
 */
export type Loader<T> = () => Promise<T>;

export type WASMArtifact = WebAssembly.Module | WebAssembly.Instance | Uint8Array; // (wasm bytes)

// Loadable entry point (Module specifier / WASM URL)
export type DeviceSpec = { type: "JS"; load: Loader<any> } | { type: "WASM"; load: Loader<WASMArtifact> };

// NOTE: Worker.GUEST is virtual; do not list it in runtime topology.
export type DeviceTopology = readonly [DevicePort, DeviceSpec][];
export type Topology = readonly [WorkerID, DeviceTopology][];

// NOTE: Prefer `satisfies XputeManifest` for concrete manifests (avoid `as const` noise).
export type XputeManifest = {
  worker: Loader<any>;
  guest: Loader<any>;
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
