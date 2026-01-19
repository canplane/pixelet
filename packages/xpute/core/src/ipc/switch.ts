// @xpute/core/ipc/switch.ts

import { HAS_BITS, SET_BITS, type u32 } from "@xpute/spec/lib/kit/word.ts";
import { Errno } from "@xpute/spec/sys/errno.ts";
import { XputeError } from "@xpute/spec/sys/error.ts";

import { type Command, type DataFrame, EMPTY_ARGS, FrameFlag, FrameKind, type Vector, type WorkerLink } from "@xpute/spec/ipc/frame.ts";
import { DevicePort, WORKER_ID_CAP, WORKER_ID_SHAMT, WorkerID } from "@xpute/spec/ipc/topology.ts";
import { Node } from "@xpute/spec/rpc/node.ts";

export type SwitchInfo = {
  worker_id: WorkerID;
  port: DevicePort; // 라우팅엔 안 씀, 오직 하이재킹 우선순위 비교용
};

export class Switch {
  // Switch tables
  private vec_table = new Map<Vector, SwitchInfo>();
  private cmd_table = new Map<Command, SwitchInfo>();

  // Links: Worker ID -> Link (Physical connection)
  private links: (WorkerLink | undefined)[] = new Array(WORKER_ID_CAP);

  private sealed = false;

  seal() {
    this.sealed = true;

    // Snapshot export: Worker ID -> Switch table
    const vec_snapshots: [Vector, DevicePort][][] = Array.from({ length: WORKER_ID_CAP }, () => []);
    const cmd_snapshots: [Command, DevicePort][][] = Array.from({ length: WORKER_ID_CAP }, () => []);
    this.vec_table.forEach(({ worker_id, port }, vec) => vec_snapshots[worker_id].push([vec, port]));
    this.cmd_table.forEach(({ worker_id, port }, cmd) => cmd_snapshots[worker_id].push([cmd, port]));

    for (let worker_id = 1; worker_id < WorkerID.GUEST; worker_id++) {
      this.links[worker_id]?.send({
        kind: FrameKind.LINK_INIT_ACK,
        vec_table: vec_snapshots[worker_id],
        cmd_table: cmd_snapshots[worker_id],
      });
    }

    console.debug(`switch: routes sealed`);
  }

  link(worker_id: WorkerID, link: WorkerLink) {
    if (this.sealed) return;

    if (!(0 < worker_id && worker_id <= WorkerID.GUEST)) {
      throw new XputeError(Errno.EINVAL, `switch: invalid worker id: ${worker_id}: must be 1-${WorkerID.GUEST}`);
    }
    this.links[worker_id] = link;
  }

  /** Register vector */
  register_vec(vec: Vector, worker_id: WorkerID, port: DevicePort) {
    if (this.sealed) return;

    const existing: SwitchInfo | undefined = this.vec_table.get(vec);

    if (existing) {
      // Higher port value wins (policy): port encodes takeover priority.
      if (port < existing.port) return;
      // takeover
      console.warn(`switch: vector ${vec} hijacked (${existing.worker_id} -> ${worker_id})`);
    }
    this.vec_table.set(vec, { worker_id, port });
  }

  /** Register command */
  register_cmd(cmd: Command, worker_id: WorkerID, port: DevicePort) {
    if (this.sealed) return;

    const existing: SwitchInfo | undefined = this.cmd_table.get(cmd);

    if (existing) {
      // Higher port value wins (policy): port encodes takeover priority.
      if (port < existing.port) return;
      // takeover
      console.warn(`switch: command '${cmd}' hijacked (${existing.worker_id} -> ${worker_id})`);
    }
    this.cmd_table.set(cmd, { worker_id, port });
  }

  forward(src_worker_id: WorkerID, frame: DataFrame): void {
    const flags: u32 = frame.flags;
    let dst_worker_id: WorkerID;

    switch (frame.kind) {
      case FrameKind.SIGNAL: {
        if (HAS_BITS(flags, FrameFlag.RES)) {
          dst_worker_id = frame.tx_id >>> WORKER_ID_SHAMT; // local <- local, local <- 255
          if (!(frame.sel === Node.LOOPBACK || src_worker_id === WorkerID.GUEST)) {
            dst_worker_id = WorkerID.GUEST; // local -> 255
          }

          const link: WorkerLink | undefined = this.links[dst_worker_id];
          if (!link) return console.warn(`switch: orphan transaction: 0x${frame.tx_id.toString(16)}`);
          link.send(frame); // TODO: 링크가 터지면? 워커 Died라든가.
        } else {
          try {
            dst_worker_id = WorkerID.GUEST; // local -> 255
            if (frame.sel === Node.LOOPBACK || src_worker_id === WorkerID.GUEST) {
              // local <- local, local <- 255
              const route: SwitchInfo | undefined = this.vec_table.get(frame.vec);
              if (!route) throw new XputeError(Errno.ENOENT);
              dst_worker_id = route.worker_id;
            }

            const link: WorkerLink | undefined = this.links[dst_worker_id];
            if (!link) throw new XputeError(Errno.ENETUNREACH); // Worker Died. TODO: Route Invalidation
            link.send(frame);
          } catch (err) {
            const errno: Errno = err instanceof XputeError ? err.errno : Errno.EIO;
            this.fault_request(src_worker_id, frame, errno);
          }
        }
        return;
      }
      case FrameKind.POST: {
        if (HAS_BITS(flags, FrameFlag.RES)) {
          dst_worker_id = frame.tx_id >>> WORKER_ID_SHAMT; // local -> 255, local <- 255

          const link: WorkerLink | undefined = this.links[dst_worker_id];
          if (!link) return console.warn(`switch: orphan transaction: 0x${frame.tx_id.toString(16)}`);
          link.send(frame);
        } else {
          try {
            dst_worker_id = WorkerID.GUEST; // local -> 255
            if (src_worker_id === WorkerID.GUEST) {
              // local <- 255
              const route: SwitchInfo | undefined = this.cmd_table.get(frame.cmd);
              if (!route) throw new XputeError(Errno.ENOENT);
              dst_worker_id = route.worker_id;
            }

            const link: WorkerLink | undefined = this.links[dst_worker_id];
            if (!link) throw new XputeError(Errno.ENETUNREACH); // Worker Died. TODO: Route Invalidation
            link.send(frame);
          } catch (err) {
            const errno: Errno = err instanceof XputeError ? err.errno : Errno.EIO;
            this.fault_request(src_worker_id, frame, errno);
          }
        }
        return;
      }
    }
  }

  /** Response to failed request */
  private fault_request<T extends DataFrame>(src_worker_id: WorkerID, frame: T, errno: Errno): void {
    if (!HAS_BITS(frame.flags, FrameFlag.ACKREQ)) return;

    const flags: u32 = frame.flags;

    const nack_frame: T = {
      ...frame,
      flags: SET_BITS(flags, FrameFlag.RES),
      errno,
      args: EMPTY_ARGS,
      body: null,
    };

    const link: WorkerLink | undefined = this.links[src_worker_id];
    if (!link) return console.warn(`switch: orphan transaction: 0x${frame.tx_id.toString(16)}`);
    link.send(nack_frame);
  }
}
