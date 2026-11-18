// @xpute/runtime-web/page.ts

/**
 * The UI's end of the guest's worker, as @xpute/expo is the native one of its
 * runtime: the link's end (@xpute/link `End`) over the page's half of the
 * UI's crossing, a port to the worker (`open_ui`), on which the UI calls the
 * guest and implements its imports by the generated bindings as any end does;
 * it hears the guest's stop and its trap (`stopped_on`, `trap_on`), and
 * starts the guest (`start`). A page has one worker, as a process has one
 * runtime, and one port to it, which moves an object beside the calls
 * (`give`). The worker stands in for the UI on the guest's link (remote.ts)
 * and posts what reaches it (message.ts): a method of an object the UI
 * implements, answered back by its tag, at once or later; an open of one
 * under the handle the guest's end issued, by the opener of the prefix that
 * matched; the guest's cancel of a method and its drop of an object; and the
 * answer to a call of the UI's. What the worker posts besides is a message by
 * its kind, the runtime's own and a host's or the program's alike
 * (`msg_on`), heard as it comes. Nothing is kept for a late hearer: the
 * worker dies with its page, so no UI comes late to it, where the native
 * runtime outlives its UI and keeps the guest's stop and its trap.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { i32 } from "@xpute/kit/abi/word.ts";
import {
  ERRNO_EINPROGRESS,
  ERRNO_EMSGSIZE,
} from "@xpute/kit/status/errno.spec.ts";
import {
  type Answered,
  CANCEL,
  DROP,
  End,
  type Handle,
  type Method,
  type Packet,
  pkt_of,
  ROOT,
  type Tag,
} from "@xpute/link/link.ts";
import { Lift, Lower } from "@xpute/link/cabi.ts";
import { sender } from "@xpute/link/wait.ts";
import type { UiStop } from "@xpute/host/runtime.ts";
import { tell_life } from "./life.ts";
import type {
  Instantiate,
  Stopped,
  ToPage,
  ToWorker,
  Trap,
} from "./message.ts";

/** A message the worker posts the page. */
interface Message {
  kind: string;
}

/** The worker, the UI's port to it, and the port it takes with the
 * instantiation. */
let worker: Worker;
let ui_port: MessagePort;
let port: MessagePort;

/** Who hears what the worker posts. */
const hearers = new Set<(m: Message) => void>();

const post = (m: ToWorker) => ui_port.postMessage(m);

/** The UI's end, over the port: a request posted, answered later by its
 * tag, since one thread cannot wait on the other, and an answer posted. */
const ui = new (class extends End {
  protected _out(
    tag: Tag,
    h: Handle,
    m: Method,
    res: boolean,
    result: i32,
    pkt: Packet,
  ): Answered {
    if (res) {
      post({ kind: "answer", tag, result, pkt: pkt_of(pkt) });
      return { result: 0, data: null };
    }
    post({ kind: "request", tag, h, m, pkt: pkt_of(pkt) });
    return { result: ERRNO_EINPROGRESS, data: null };
  }
})();

/** No packet. */
const NONE = new Uint8Array(0);

/** The reader of each packet the worker posts, and the writer of the answer
 * the UI's end gives at once, which holds no value: every method the UI
 * implements is an `async func` (worker.ts `enter`), so a value written there
 * is past its room, EMSGSIZE. */
const args = new Lift(NONE);
const ret = new Lower(NONE, 0);

/** Hands the UI's end the frame the worker posted, its packet `pkt`, and
 * answers what the end answers (`End.input`). */
function input(
  tag: Tag,
  h: Handle,
  m: Method,
  res: boolean,
  result: i32,
  pkt: U8Array | null,
): i32 {
  const p = pkt ?? NONE;
  args.reset(p.buffer, p.byteOffset, p.length);
  ret.reset(NONE.buffer, 0, 0);
  return ui.input(tag, h, m, res, result, args, ret);
}

/** What the worker posts on the UI's port, by its kind, handed to the UI's
 * end: a method's result at once goes back as a later answer, and the
 * guest's cancel and drop are the link's own. */
const HEAR: {
  [K in ToPage["kind"]]: (m: Extract<ToPage, { kind: K }>) => void;
} = {
  request: (m) => {
    const result = input(m.tag, m.h, m.m, false, 0, m.pkt);
    if (result === ERRNO_EINPROGRESS) return;
    ui.task_return(m.tag, ret.finish() < 0 ? ERRNO_EMSGSIZE : result);
  },
  answer: (m) => void input(m.tag, ROOT, 0, true, m.result, m.pkt),
  open: (m) => ui.open_at(m.k, m.h, m.uri),
  cancel: (m) =>
    void input(0, ROOT, CANCEL, false, 0, pkt_of([4, (w) => w.u32(0, m.tag)])),
  drop: (m) => void input(0, m.h, DROP, false, 0, null),
};

/** Opens the guest's module worker at `url`, and the UI's end over one port
 * of a channel to it; the other goes to the worker as the guest starts
 * (`start`). */
export function open_ui(url: string | URL): End {
  worker = new Worker(url, { type: "module" });
  worker.onmessage = ({ data: m }: MessageEvent<Message>) => {
    for (const hear of hearers) hear(m);
  };
  const { port1, port2 } = new MessageChannel();
  ui_port = port1;
  port = port2;
  ui_port.onmessage = ({ data: m }: MessageEvent<ToPage>) =>
    (HEAR[m.kind] as (m: ToPage) => void)(m);
  return ui;
}

/** Gives `thing` beside the UI's calls under `h`, a handle the worker's end
 * issued, until the UI drops `h`: on the same port as the calls, so that it
 * lands before what the UI sends after it. */
export function give(h: Handle, thing: Transferable): void {
  ui_port.postMessage({ kind: "give", h, thing } satisfies ToWorker, [thing]);
}

/** Adds `hear` to what is told each message of `kind` the worker posts from
 * now on. */
export function msg_on<M extends Message>(
  kind: M["kind"],
  hear: (m: M) => void,
): { remove(): void } {
  const f = (m: Message) => {
    if (m.kind === kind) hear(m as M);
  };
  hearers.add(f);
  return { remove: () => hearers.delete(f) };
}

/** Adds `hear` to what is told the guest's stop (`Stopped`). */
export function stopped_on(hear: (stop: UiStop) => void): { remove(): void } {
  return msg_on<Stopped>("stopped", (m) => hear(m.stop));
}

/** Adds `hear` to what is told that the guest trapped, with what stopped it
 * (`Trap`). */
export function trap_on(hear: (err: string) => void): { remove(): void } {
  return msg_on<Trap>("trap", (m) => hear(m.err));
}

/** Starts the guest with what the UI implements, once it is mounted on the
 * UI's end, as @xpute/expo `start` does natively: the worker is sent the
 * guest's `module`, fetched with `init`, the UI's port, the named handles and
 * prefixes of what the UI implements, and `more`, the program's own fields,
 * which its worker reads with them (`Instantiate`). The guest is told the
 * page's lifecycle from here on (`tell_life`), as the native module tells its
 * runtime the app's. */
export function start(
  module: string,
  init: RequestInit | undefined,
  more: object = {},
): void {
  const ask: Instantiate = {
    module,
    init,
    port,
    named: ui.named(),
    prefixes: ui.prefixes(),
  };
  worker.postMessage({ ...more, ...ask }, [port]);
  tell_life(sender(ui));
}
