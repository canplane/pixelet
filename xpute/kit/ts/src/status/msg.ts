// @xpute/kit/status/msg.ts

/** A message (`kit/spec/msg.json5`): what a person reads of an error, a bug
 * or a log record, bounded text, which its maker cuts. */

import { MSG_CAP } from "./msg.spec.ts";

export { MSG_CAP };

/** `msg` as far as the cap holds it in UTF-8, cut at a character boundary,
 * as xpute-kit msg.rs `cut_msg` cuts it. */
export function cut_msg(msg: string): string {
  // A UTF-16 unit is at most three bytes of UTF-8.
  if (msg.length * 3 <= MSG_CAP) return msg;
  let sz = 0;
  let len = 0;
  for (const ch of msg) {
    const c = ch.codePointAt(0)!;
    sz += c < 0x80 ? 1 : c < 0x800 ? 2 : c < 0x10000 ? 3 : 4;
    if (sz > MSG_CAP) return msg.slice(0, len);
    len += ch.length;
  }
  return msg;
}
