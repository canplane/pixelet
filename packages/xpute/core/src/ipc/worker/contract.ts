// @xpute/core/ipc/worker/contract.ts

import type { Async } from "@xpute/spec/lib/kit/type.ts";

import { WorkerID } from "@xpute/spec/ipc/topology.ts";
import type { WorkerLink } from "@xpute/spec/ipc/frame.ts";
import type { DeviceUplink, NetUplink } from "@xpute/spec/ipc/link.ts";
import { CommandPort } from "@xpute/spec/control/post.ts";

import { NetGateway } from "@xpute/core/net/gateway.ts";

export type WorkerDriver = {
  spawn(worker_id: WorkerID): Promise<WorkerLink>;
};

/**
 * 게스트가 호스트에게 제공하는 하드웨어 추상화 계층(HAL)
 * 호스트는 이 장치들을 통해 외부(Network, App)와 소통
 */
export type GuestDevice = {
  post: CommandPort;
  net_gateway: NetGateway;

  start?: () => Async<void>;
};

/** 게스트 측에 위치한 xpute 모듈의 mod.ts가 export default로 내보내야 하는 부팅 함수 시그니처 */
export type GuestBootstrapper = (tx: DeviceUplink, net_uplink: NetUplink) => Async<GuestDevice>;
