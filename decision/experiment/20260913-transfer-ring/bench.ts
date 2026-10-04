// decision/experiment/20260913-transfer-ring/bench.ts

const url = (f) => new URL(f, import.meta.url).href;
const ms = 1500;
async function transfer(n, size, touch) {
  const a = new Worker(url("transfer_producer.ts"), { type: "module" }), b = new Worker(url("transfer_consumer.ts"), { type: "module" });
  const { port1, port2 } = new MessageChannel();
  b.postMessage({ port: port2, touch, size }, [port2]);
  const r = await new Promise((res) => {
    a.onmessage = (e) => res(e.data);
    a.postMessage({ port: port1, n, size, ms, touch }, [port1]);
  });
  a.terminate();
  b.terminate();
  return r;
}
async function shared(n, size, touch) {
  const a = new Worker(url("shared_producer.ts"), { type: "module" }), b = new Worker(url("shared_consumer.ts"), { type: "module" });
  const sab = new SharedArrayBuffer(n * size), st = new SharedArrayBuffer(4 * (n + 3));
  b.postMessage({ sab, st, n, size, touch });
  const r = await new Promise((res) => {
    a.onmessage = (e) => res(e.data);
    a.postMessage({ sab, st, n, size, ms, touch });
  });
  a.terminate();
  b.terminate();
  return r;
}
const f = (r) => `${Math.round(r.per_sec).toString().padStart(7)}/s ${r.mb_s.toFixed(0).padStart(6)} MB/s rtt ${r.rtt_ms.toFixed(3)} ms`;
for (const touch of ["header", "full"]) {
  for (const size of [65536, 1 << 20]) {
    console.log(`\n-- touch=${touch}  buffer=${size >> 10} KB`);
    for (const n of [1, 2, 4, 8, 16]) console.log(`N=${String(n).padStart(2)}  transfer ${f(await transfer(n, size, touch))}  |  SAB ${f(await shared(n, size, touch))}`);
  }
}
