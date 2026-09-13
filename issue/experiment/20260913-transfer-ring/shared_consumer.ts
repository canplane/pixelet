// issue/experiment/20260913-transfer-ring/shared_consumer.ts

self.onmessage = (e) => {
  const { sab, st, n, size, touch } = e.data;
  const state = new Int32Array(st);
  const dst = new Uint8Array(size);
  let _sum = 0;
  while (Atomics.load(state, n + 2) === 0) {
    let any = false;
    for (let i = 0; i < n; i++) {
      if (Atomics.load(state, i) === 1) {
        const v = new Uint8Array(sab, i * size, size);
        if (touch === "full") dst.set(v);
        else _sum += v[79];
        Atomics.store(state, i, 2);
        Atomics.notify(state, n + 1);
        any = true;
      }
    }
    if (!any) Atomics.wait(state, n, Atomics.load(state, n), 1);
  }
};
