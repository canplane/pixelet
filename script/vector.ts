// script/vector.ts
//
// What every generator under script/vector/ writes a number through: a
// double as the hex of its bits, so NaN, -0 and the last bit survive JSON;
// and the one seeded stream every generator draws from, so an input set is
// the same on every run (xorshift32).

/** A double's bits in hex. NaN is written as the one canonical quiet NaN:
 * ECMAScript leaves a NaN's payload to the engine, and V8's differs from run
 * to run (Math.log(-1) has come out both signaling and quiet), so a payload
 * is not something a vector can hold the other side to. */
export function f64_bits(x: number): string {
  if (Number.isNaN(x)) return "0x7ff8000000000000";
  const v = new DataView(new ArrayBuffer(8));
  v.setFloat64(0, x, true);
  return "0x" + v.getBigUint64(0, true).toString(16).padStart(16, "0");
}

export function seeded(seed: number): () => number {
  let s = seed | 0 || 1;
  return () => {
    s ^= s << 13;
    s ^= s >>> 17;
    s ^= s << 5;
    return s >>> 0;
  };
}
