// @xpute/spec/rpc/debug.ts

import { HAS_BITS } from "@xpute/spec/lib/kit/word.ts";

import { FrameFlag, type TxOptions } from "@xpute/spec/ipc/frame.ts";
import { FANOUT, IS_LOOPBACK, IS_ROLE_SEL, IS_UNIVERSAL, type NodeSelector, role_indices, VALUE } from "@xpute/spec/rpc/node.ts";

export const sel_repr = (sel: NodeSelector, flags: FrameFlag, opts: TxOptions) => {
  if (IS_LOOPBACK(sel)) return { target: "loopback" };

  const roles = role_indices(sel);
  const ack = { ack: HAS_BITS(flags, FrameFlag.ACKREQ) ? "on" : "off" };
  let cast;

  if (opts.fanout === undefined || opts.fanout === FANOUT.ONE) {
    cast = { cast: IS_ROLE_SEL(sel) ? "anycast" : "unicast" };

    if (IS_ROLE_SEL(sel)) {
      if (IS_UNIVERSAL(sel)) {
        return { target: "universal", ...cast, ...ack };
      }
      return { roles, ...cast, ...ack };
    }

    return { id: VALUE(sel), ...cast, ...ack };
  }

  if (!IS_ROLE_SEL(sel)) {
    return { nonsense: "k-cast/multicast/broadcast cannot apply to id-mode selector" };
  }

  if (opts.fanout !== FANOUT.ALL) {
    cast = { cast: `k-cast (${opts.fanout === FANOUT.AUTO ? "auto" : opts.fanout})` };

    if (IS_UNIVERSAL(sel)) {
      return { target: "universal", ...cast, ...ack };
    }
    return { roles, ...cast, ...ack };
  }

  cast = { cast: IS_UNIVERSAL(sel) ? "broadcast" : "multicast" };

  if (IS_UNIVERSAL(sel)) {
    return { ...cast, ...ack };
  }
  return { roles, ...cast, ...ack };
};
