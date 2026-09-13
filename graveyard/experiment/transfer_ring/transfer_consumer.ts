// graveyard/experiment/transfer_ring/transfer_consumer.ts

self.onmessage = (e) => {
  const { port, touch } = e.data;
  let _sum = 0;
  const dst = new Uint8Array(new ArrayBuffer(e.data.size));
  port.onmessage = (m) => {
    const buf = m.data;
    if (touch === "full") dst.set(new Uint8Array(buf));
    else _sum += new Uint8Array(buf, 16, 64)[63];
    port.postMessage(buf, [buf]);
  };
};
