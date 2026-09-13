// issue/experiment/20260913-transfer-ring/shared_producer.ts

// The same window over a SharedArrayBuffer: N slots, a state per slot, Atomics
// to hand over. Polls with a 1 ms wait, so it keeps a core busy — an upper
// bound on handover speed, not a fair implementation.
self.onmessage = (e) => {
  const { sab, st, n, size, ms, touch } = e.data;
  const state = new Int32Array(st);
  const src = new Uint8Array(size);
  for (let i = 0; i < size; i++) src[i] = i;
  let back = 0, lat = 0;
  const t0 = performance.now();
  const stamps = new Float64Array(n);
  while (performance.now() - t0 < ms) {
    let any = false;
    for (let i = 0; i < n; i++) {
      if (Atomics.load(state, i) === 2) {
        lat += performance.now() - stamps[i];
        back++;
        Atomics.store(state, i, 0);
      }
      if (Atomics.load(state, i) === 0) {
        const v = new Uint8Array(sab, i * size, size);
        if (touch === "full") v.set(src);
        else v.subarray(16, 80).set(src.subarray(0, 64));
        stamps[i] = performance.now();
        Atomics.store(state, i, 1);
        Atomics.notify(state, n);
        any = true;
      }
    }
    if (!any) Atomics.wait(state, n + 1, 0, 1);
  }
  Atomics.store(state, n + 2, 1);
  Atomics.notify(state, n);
  const sec = (performance.now() - t0) / 1000;
  self.postMessage({ per_sec: back / sec, mb_s: back * size / sec / 1048576, rtt_ms: lat / back });
};
