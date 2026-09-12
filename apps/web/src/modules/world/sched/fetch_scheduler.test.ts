// @pxl/web/modules/world/sched/fetch_scheduler.test.ts

import { assertEquals } from "@std/assert";

import type { Demand } from "./demand.ts";
import { ledger_snapshot } from "./ledger.ts";
import { error_reason, fetch_pending_count, fetch_running, fetch_scheduler_reset, fetch_sync, MAX_INFLIGHT } from "./fetch_scheduler.ts";

// Scheduler state is module-global (see fetch_scheduler.ts's own doc), so
// each test resets it first.

type Handle = {
  demand: Demand;
  ctl: { started: boolean; aborted: boolean };
  resolve: () => void;
};

/** A Demand whose `run` blocks until `resolve()` is called (works whether or
 * not it has started yet) and records whether it started / was aborted. */
function blocking_demand(key: string, category: string, priority: number): Handle {
  const ctl = { started: false, aborted: false };
  let open!: () => void;
  const gate = new Promise<void>((res) => (open = res));

  const demand: Demand = {
    key,
    category,
    priority,
    run: (signal) => {
      ctl.started = true;
      return new Promise<void>((resolve, reject) => {
        if (signal.aborted) {
          ctl.aborted = true;
          return reject(new DOMException("aborted", "AbortError"));
        }
        signal.addEventListener("abort", () => {
          ctl.aborted = true;
          reject(new DOMException("aborted", "AbortError"));
        });
        gate.then(resolve);
      });
    },
  };
  return { demand, ctl, resolve: () => open() };
}

const flush = () => new Promise((r) => setTimeout(r, 0));

Deno.test("fetch_scheduler - runs at most MAX_INFLIGHT at once, admits the rest as slots free", async () => {
  fetch_scheduler_reset();
  const cat = "quota";
  const h = Array.from({ length: MAX_INFLIGHT + 3 }, (_, i) => blocking_demand(`${cat}:${i}`, cat, i));

  fetch_sync(cat, h.map((x) => x.demand));
  await flush();

  assertEquals(h.filter((x) => x.ctl.started).length, MAX_INFLIGHT);
  assertEquals(h.slice(MAX_INFLIGHT).some((x) => x.ctl.started), false);
  assertEquals(fetch_pending_count(cat), MAX_INFLIGHT + 3);

  h[0].resolve();
  await flush();
  assertEquals(h[MAX_INFLIGHT].ctl.started, true);

  for (const x of h) x.resolve();
  await flush();
  assertEquals(fetch_pending_count(cat), 0);
});

Deno.test("fetch_scheduler - lowest priority number runs first", async () => {
  fetch_scheduler_reset();
  const cat = "priority";
  const far = blocking_demand(`${cat}:far`, cat, 10);
  const near = blocking_demand(`${cat}:near`, cat, 1);
  const mid = blocking_demand(`${cat}:mid`, cat, 5);

  // saturate the quota so nothing in `cat` runs until a slot frees
  const fill = Array.from({ length: MAX_INFLIGHT }, (_, i) => blocking_demand(`fill:${i}`, "fill", -1));
  fetch_sync("fill", fill.map((x) => x.demand));
  await flush();

  fetch_sync(cat, [far.demand, near.demand, mid.demand]);
  await flush();
  assertEquals([near.ctl.started, mid.ctl.started, far.ctl.started], [false, false, false]);

  fill[0].resolve();
  await flush();
  assertEquals([near.ctl.started, mid.ctl.started, far.ctl.started], [true, false, false]);

  fill[1].resolve();
  await flush();
  assertEquals([near.ctl.started, mid.ctl.started, far.ctl.started], [true, true, false]);

  fill[2].resolve();
  await flush();
  assertEquals(far.ctl.started, true);

  for (const x of [...fill, far, near, mid]) x.resolve();
  await flush();
});

Deno.test("fetch_scheduler - a later category submitting in the same tick still wins on priority", async () => {
  // The bug the coalesced pump fixes: whoever called sync() first used to
  // fill every slot outright, so MapScene's useTileFetcher-before-
  // usePoiFetcher order beat priority and a POI page sat behind lower-
  // ranked terrain pages. Both submissions here land in one tick.
  fetch_scheduler_reset();
  const low = Array.from({ length: MAX_INFLIGHT }, (_, i) => blocking_demand(`low:${i}`, "low", 5));
  const high = blocking_demand("high:0", "high", 0);

  fetch_sync("low", low.map((x) => x.demand));
  fetch_sync("high", [high.demand]);
  await flush();

  assertEquals(high.ctl.started, true);
  assertEquals(low.filter((x) => x.ctl.started).length, MAX_INFLIGHT - 1);

  for (const x of [...low, high]) x.resolve();
  await flush();
});

Deno.test("fetch_scheduler - sync() aborts an in-flight demand no longer wanted", async () => {
  fetch_scheduler_reset();
  const cat = "yield-run";
  const keep = blocking_demand(`${cat}:keep`, cat, 0);
  const drop = blocking_demand(`${cat}:drop`, cat, 1);

  fetch_sync(cat, [keep.demand, drop.demand]);
  await flush();
  assertEquals([keep.ctl.started, drop.ctl.started], [true, true]);

  fetch_sync(cat, [keep.demand]); // drop no longer wanted
  await flush();
  assertEquals(drop.ctl.aborted, true);
  assertEquals(keep.ctl.aborted, false);

  keep.resolve();
  await flush();
  assertEquals(fetch_pending_count(cat), 0);
});

Deno.test("fetch_scheduler - fetch_running answers only for keys in flight", async () => {
  fetch_scheduler_reset();
  const cat = "running-query";
  const a = blocking_demand(`${cat}:a`, cat, 0);
  assertEquals(fetch_running(a.demand.key), false);
  fetch_sync(cat, [a.demand]);
  assertEquals(fetch_running(a.demand.key), false); // queued, not yet pumped
  await flush();
  assertEquals(fetch_running(a.demand.key), true);
  a.resolve();
  await flush();
  assertEquals(fetch_running(a.demand.key), false);
});

Deno.test("fetch_scheduler - sync() drops a queued demand no longer wanted", async () => {
  fetch_scheduler_reset();
  const cat = "yield-queue";
  const fill = Array.from({ length: MAX_INFLIGHT }, (_, i) => blocking_demand(`fill:${i}`, "fill", -1));
  fetch_sync("fill", fill.map((x) => x.demand));
  await flush();

  const q = blocking_demand(`${cat}:q`, cat, 0);
  fetch_sync(cat, [q.demand]);
  await flush();
  assertEquals(q.ctl.started, false);
  assertEquals(fetch_pending_count(cat), 1);

  fetch_sync(cat, []);
  assertEquals(fetch_pending_count(cat), 0);

  for (const x of fill) x.resolve();
  await flush();
  assertEquals(q.ctl.started, false); // freeing slots must not resurrect it
});

Deno.test("fetch_scheduler - re-submitting a running key does not start a second run", async () => {
  fetch_scheduler_reset();
  const cat = "dedup";
  const first = blocking_demand(`${cat}:x`, cat, 0);
  fetch_sync(cat, [first.demand]);
  await flush();
  assertEquals(first.ctl.started, true);

  const second = blocking_demand(`${cat}:x`, cat, 0); // same key
  fetch_sync(cat, [second.demand]);
  await flush();
  assertEquals(second.ctl.started, false);
  assertEquals(fetch_pending_count(cat), 1);

  first.resolve();
  await flush();
  assertEquals(fetch_pending_count(cat), 0);
});

Deno.test("fetch_scheduler - ledger records outcome for completed and aborted demands", async () => {
  fetch_scheduler_reset();
  const cat = "ledger";
  const ok = blocking_demand(`${cat}:ok`, cat, 0);
  const killed = blocking_demand(`${cat}:killed`, cat, 1);

  fetch_sync(cat, [ok.demand, killed.demand]);
  await flush();

  ok.resolve();
  fetch_sync(cat, [ok.demand]); // drops `killed`
  await flush();

  const mine = ledger_snapshot().filter((r) => r.category === cat);
  assertEquals(mine.find((r) => r.key === `${cat}:ok`)?.outcome, "ok");
  assertEquals(mine.find((r) => r.key === `${cat}:killed`)?.outcome, "aborted");
  for (const r of mine) assertEquals(typeof r.start_ms, "number");
});

Deno.test("error_reason - an http failure is grouped by its status, anything else by its kind", () => {
  assertEquals(error_reason(new Error("[geocode/5788:1842:0] http 503")), "http 503");
  assertEquals(error_reason(new Error("[tile/1:2:0] http 429")), "http 429");
  assertEquals(error_reason(new TypeError("Failed to fetch")), "TypeError: Failed to fetch");
  assertEquals(error_reason("boom"), "string: boom");
});
