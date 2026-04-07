// @xpute/core/lib.ts

import { type DeviceTopology, WorkerID, type XputeManifest } from "@xpute/spec/ipc/topology.ts";
import { type Frame, FrameKind, type WorkerLink } from "@xpute/spec/ipc/frame.ts";
import type { WorkerDriver } from "./ipc/worker/contract.ts";

import { Switch } from "./ipc/switch.ts";
import { init_worker } from "./ipc/worker.ts";
import { init_guest } from "./ipc/guest.ts";
import { create_virtual_link } from "./ipc/worker/virtual.ts";

export default class Xpute {
  private worker_driver!: WorkerDriver;

  private switch = new Switch();

  constructor(private readonly manifest: XputeManifest) {}

  /** Start the Xpute host runtime. */
  async start() {
    const { worker, wasm, guest, devices, net } = this.manifest;

    this.worker_driver = new (await worker()).default();

    console.debug(`xpute: host starting...`);

    // mount workers
    await Promise.all(devices.map(([worker_id, device_specs]) => this.mount_worker(worker_id, device_specs, wasm)));

    // mount guest
    await this.mount_guest(guest, net);

    // seal
    this.switch.seal();
    console.debug(`xpute: online`);
  }

  /** Mount workers: 1-254 */
  private async mount_worker(worker_id: WorkerID, device_specs: DeviceTopology, wasm: XputeManifest["wasm"]) {
    let downlink: WorkerLink;
    if (worker_id === WorkerID.MAIN) {
      const { downlink: _downlink, arg } = create_virtual_link(worker_id);
      downlink = _downlink;
      await init_worker(arg, device_specs, wasm);
    } else {
      downlink = await this.worker_driver.spawn(worker_id);
    }

    this.link(worker_id, downlink);
  }

  /** Mount guest: Virtual worker 255 */
  private async mount_guest(guest_spec: XputeManifest["guest"], net_spec: XputeManifest["net"]) {
    const worker_id: WorkerID = WorkerID.GUEST;

    const { downlink, arg } = create_virtual_link(worker_id);
    await init_guest(arg, guest_spec, net_spec);

    this.link(worker_id, downlink);
  }

  /** Create bidirectional links */
  private link(worker_id: WorkerID, downlink: WorkerLink): void {
    this.switch.link(worker_id, downlink);

    downlink.on((frame: Frame) => {
      switch (frame.kind) {
        case FrameKind.SIGNAL:
        case FrameKind.POST: {
          this.switch.forward(worker_id, frame);
          return;
        }

        case FrameKind.LINK_INIT_SYN: {
          frame.capable_vecs.forEach((vec) => this.switch.register_vec(vec, worker_id, frame.port));
          frame.capable_cmds.forEach((cmd) => this.switch.register_cmd(cmd, worker_id, frame.port));
          return;
        }
      }
      console.warn(`xpute: unknown frame:`, frame);
    });

    console.debug(`xpute: worker mounted:`, worker_id);
  }
}
