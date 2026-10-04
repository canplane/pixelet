// decision/experiment/20260913-transfer-ring/transfer_producer.ts

// Holds N buffers and may only send while it holds one; the consumer returns
// each after reading. The free list is the window and the credit.
self.onmessage = (e) => {
  const { port, n, size, ms, touch } = e.data;
  const free = [];
  for (let i = 0; i < n; i++) free.push(new ArrayBuffer(size));
  const src = new Uint8Array(size);
  for (let i = 0; i < size; i++) src[i] = i;
  let back = 0, lat = 0, seq = 0, done = false;

  const pump = () => {
    while (!done && free.length) {
      const buf = free.pop(), h = new Float64Array(buf, 0, 2);
      if (touch === "full") new Uint8Array(buf).set(src);
      else new Uint8Array(buf, 16, 64).set(src.subarray(0, 64));
      h[0] = seq++;
      h[1] = performance.now();
      port.postMessage(buf, [buf]);
    }
  };
  port.onmessage = (m) => {
    const buf = m.data, h = new Float64Array(buf, 0, 2);
    lat += performance.now() - h[1];
    back++;
    free.push(buf);
    pump();
  };
  const t0 = performance.now();
  pump();
  setTimeout(() => {
    done = true;
    const sec = (performance.now() - t0) / 1000;
    self.postMessage({ per_sec: back / sec, mb_s: back * size / sec / 1048576, rtt_ms: lat / back });
  }, ms);
};
