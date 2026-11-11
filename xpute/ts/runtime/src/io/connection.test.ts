// @xpute/runtime/io/connection.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import type { U8Array } from "@xpute/kit/abi/array.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import { Scheduler } from "../sched/scheduler.ts";
import { NO_WAKE } from "../sched/wake.ts";
import { Connection, type Heard } from "./connection.ts";
import * as frame from "./frame.ts";

/** A frame's room in the test core's memory, which holds two past the first
 * room, as offset 0 names nothing: the one the core receives into, then the
 * one it sends from, then bytes of its own. */
const FRAME = 64;
const RECEIVE_AT = FRAME;
const SEND_AT = 2 * FRAME;
const OWN_AT = 3 * FRAME;

/** What a test core does in a turn, through its host as a core does. */
interface Core {
  mem: ArrayBuffer;
  /** Every frame that waits for the core, received. */
  take(): frame.Frame[];
  /** Sends `f`, naming `named` bytes of the core's own. */
  send(f: Omit<frame.Frame, "pkt">, named?: number): void;
}

const DEAF: Heard = { on_turn() {}, on_wake() {} };

function bell(run: (c: Core) => number, heard: Heard = DEAF): Connection {
  const mem = new ArrayBuffer(OWN_AT + FRAME);
  const c: Core = {
    mem,
    take() {
      const got: frame.Frame[] = [];
      while (connection.host.receive(RECEIVE_AT, FRAME) > 0) {
        const f = frame.frame_at(mem, RECEIVE_AT);
        got.push({ ...f, pkt: f.pkt && f.pkt.slice() });
      }
      return got;
    },
    send(f, named = 0) {
      frame.lay_frame(mem, SEND_AT, { ...f, pkt: null });
      connection.host.send(SEND_AT, OWN_AT, named);
    },
  };
  const connection: Connection = new Connection(
    mem,
    FRAME - frame.WORD_BYTES,
    () => run(c),
    heard,
  );
  return connection;
}

const quiet = <T>(f: () => T): T => {
  const err = console.error;
  console.error = () => {};
  try {
    return f();
  } finally {
    console.error = err;
  }
};

test("connection - a turn run from inside a listener is refused and leaves nothing sent", () => {
  const received: number[] = [];
  // This core sends one import on every turn.
  const connection = bell((c) => {
    received.push(c.take().length);
    c.send({ tag: 0, cmd: 2, flags: 0, result: 0 });
    return -1;
  });
  let refused = 0;
  connection.listen(() => {
    assert.throws(() => connection.run(0), InvariantError);
    refused++;
  });
  connection.run(0);
  connection.run(0);
  assert.deepStrictEqual(refused, 2);
  assert.deepStrictEqual(received, [0, 0], "the refused run reaches no turn");
});

test("connection - a core that traps throws at no one: it is told once, and runs no more", () =>
  quiet(() => {
    let runs = 0;
    let terminated = 0;
    const connection = bell(() => {
      runs++;
      throw new Error("the core trapped");
    });
    connection.onterminated = () => terminated++;
    assert.deepStrictEqual(connection.run(0), -1);
    assert.deepStrictEqual(connection.run(0), -1);
    assert.deepStrictEqual(runs, 1);
    assert.deepStrictEqual(terminated, 1);
  }));

test("connection - a frame the runtime cannot take stops the core, and it is told once", () =>
  quiet(() => {
    let runs = 0;
    let terminated = 0;
    const connection = bell((c) => {
      runs++;
      c.send({ tag: 0, cmd: 2, flags: 0, result: 0 });
      return 0;
    });
    connection.listen(() => {
      throw new InvariantError(Errno.EBADF);
    });
    connection.onterminated = () => terminated++;
    assert.deepStrictEqual(connection.run(0), -1);
    assert.deepStrictEqual(connection.run(0), -1);
    assert.deepStrictEqual(runs, 1, "a stopped core is run no more");
    assert.deepStrictEqual(terminated, 1);
  }));

test("connection - a request answered in the turn that traps takes its answer", async () => {
  // This core answers the request, then traps in the same turn.
  const connection = bell((c) => {
    for (const f of c.take()) {
      c.send({ tag: f.tag, cmd: f.cmd, flags: frame.RES, result: 7 });
    }
    throw new Error("the core trapped");
  });
  const answer = connection.request(1);
  quiet(() => connection.run(0));
  assert.deepStrictEqual(
    (await answer)[0],
    7,
    "not the stop's ENOTRECOVERABLE",
  );
});

test("connection - a turn run is heard before its wake", () => {
  // This core's turn asks for the next in 20 ms.
  const heard: string[] = [];
  const connection = bell(() => 20, {
    on_turn: () => heard.push("turn"),
    on_wake: (wake_ms) => heard.push(`wake ${wake_ms}`),
  });
  connection.run(0);
  assert.deepStrictEqual(heard, ["turn", "wake 20"]);
});

test("connection - what a listener completes asks the next turn, and a turn that sent nothing asks none", () => {
  let turns = 0;
  // This core has no wake of its own, and its first turn sends a request.
  const connection = bell(
    (c) => {
      c.take();
      if (++turns === 1)
        c.send({ tag: 3, cmd: 2, flags: frame.ACKREQ, result: 0 });
      return NO_WAKE;
    },
    {
      on_turn: () => scheduler.on_turn(),
      on_wake: (wake_ms) => scheduler.on_wake(wake_ms),
    },
  );
  connection.listen(() => 0);
  const moments: ((now: number) => void)[] = [];
  const scheduler = new Scheduler(
    connection,
    {
      next: (consider) => moments.push(consider),
      admit: () => true,
      grant: () => ({ deadline: Infinity }),
      observe: () => {},
    },
    () => {},
  );
  scheduler.resume();
  moments.shift()!(0);
  assert.deepStrictEqual(turns, 1);
  moments.shift()!(0);
  assert.deepStrictEqual(turns, 2, "the completion is the core's to take");
  assert.deepStrictEqual(
    moments.length,
    0,
    "a turn that sent nothing asks none",
  );
});

test("connection - a request is answered by the reply its turn sends, and waits for no turn", async () => {
  let turns = 0;
  // This core replies to each frame with ACKREQ as it applies it.
  const connection = bell((c) => {
    turns++;
    for (const f of c.take()) {
      if (f.flags & frame.ACKREQ) {
        c.send({
          tag: f.tag,
          cmd: f.cmd,
          flags: frame.RES,
          result: 40 + f.tag,
        });
      }
    }
    return -1;
  });
  const answer = connection.request(1);
  assert.deepStrictEqual(turns, 0, "requesting runs nothing");
  connection.run(0);
  assert.deepStrictEqual((await answer)[0], 41);
});

test("connection - a request still waiting as the core stops is answered ENOTRECOVERABLE", async () => {
  const connection = bell(() => {
    throw new Error("the core trapped");
  });
  const answer = connection.request(1);
  quiet(() => connection.run(0));
  assert.deepStrictEqual(await answer, [Errno.ENOTRECOVERABLE, null]);
  assert.deepStrictEqual(await connection.request(1), [
    Errno.ENOTRECOVERABLE,
    null,
  ]);
});

test("connection - the memory a frame names is taken as it was sent", () => {
  // This core names three bytes of its own, then writes over them in the
  // same turn.
  const connection = bell((c) => {
    const own = new Uint8Array(c.mem, OWN_AT, 3);
    own.set([7, 8, 9]);
    c.send({ tag: 0, cmd: 2, flags: 0, result: 0 }, 3);
    own.fill(0);
    return -1;
  });
  let named: U8Array | null = null;
  connection.listen((_tag, _cmd, _f, _pkt, n) => {
    named = n && n.slice();
  });
  connection.run(0);
  assert.deepStrictEqual(named, new Uint8Array([7, 8, 9]));
});

test("connection - a reply hands over the memory it names, as it was sent", async () => {
  // This core replies naming three bytes of its own, then writes over them in
  // the same turn.
  const connection = bell((c) => {
    const own = new Uint8Array(c.mem, OWN_AT, 3);
    for (const f of c.take()) {
      own.set([7, 8, 9]);
      c.send({ tag: f.tag, cmd: f.cmd, flags: frame.RES, result: 0 }, 3);
      own.fill(0);
    }
    return -1;
  });
  const answer = connection.request(1);
  connection.run(0);
  assert.deepStrictEqual(await answer, [0, new Uint8Array([7, 8, 9])]);
});
