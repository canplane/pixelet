// @xpute/core/rpc/peer/nat.ts

import { type NodeSelector, VALUE_MASK } from "@xpute/spec/rpc/node.ts";
import type { PID } from "@xpute/spec/net/crypto.ts";

export class NAT {
  private vid_cnt: NodeSelector = 0n;

  /** Translation table */
  readonly vt = new Map<NodeSelector, PID>();
  readonly pt = new Map<PID, NodeSelector>();

  constructor() {}

  private issue_vid(): NodeSelector {
    let cnt = this.vid_cnt;
    do {
      cnt = (cnt + 1n) & VALUE_MASK; // wrap
      // ID-mode value 예약: 0 (Loopback), VALUE_MASK (Reserved)
    } while (this.vt.has(cnt) || cnt === 0n || cnt === VALUE_MASK);
    return (this.vid_cnt = cnt);
  }

  register(pid: PID): NodeSelector {
    let vid = this.pt.get(pid);
    if (vid !== undefined) return vid;
    vid = this.issue_vid();

    this.vt.set(vid, pid);
    this.pt.set(pid, vid);

    return vid;
  }

  unregister(pid: PID): NodeSelector | undefined {
    const vid: NodeSelector | undefined = this.pt.get(pid);
    if (vid !== undefined) {
      this.vt.delete(vid);
      this.pt.delete(pid);
    }
    return vid;
  }
}
