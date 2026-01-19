// @pxl/engine/lib.ts

import { Errno } from "@xpute/spec/sys/errno.ts";
import { XputeError } from "@xpute/spec/sys/error.ts";

import type { Device, DeviceEnvironment, DeviceUplink } from "@xpute/spec/ipc/link.ts";
import { type Command, type DataFrame, FrameKind, type Vector } from "@xpute/spec/ipc/frame.ts";
import { VectorPort } from "@xpute/spec/control/signal.ts";
import { CommandPort } from "@xpute/spec/control/post.ts";

import * as page_module from "@pxl/engine/modules/page/mod.ts";

export default class Engine implements Device {
  private env: DeviceEnvironment;

  constructor(tx: DeviceUplink) {
    // connect ports
    this.env = {
      signal: new VectorPort(tx),
      post: new CommandPort(tx),
    };

    // load modules
    this.load_modules();
  }

  private load_modules() {
    page_module.load(this.env);
  }

  capable_vecs(): Vector[] {
    return this.env.signal.vecs();
  }

  capable_cmds(): Command[] {
    return this.env.post.cmds();
  }

  receive(frame: DataFrame): Promise<DataFrame> {
    switch (frame.kind) {
      case FrameKind.SIGNAL:
        return this.env.signal.receive(frame);
      case FrameKind.POST:
        return this.env.post.receive(frame);
      default: {
        throw new XputeError(Errno.EBADMSG, `device: unknown frame: ${JSON.stringify(frame)}`);
      }
    }
  }
}
