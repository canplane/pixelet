// @pxl/web/modules/world/sched/alloc_probe.ts

/**
 * Dev probe: who allocates ArrayBuffers, and how much.
 *
 * A Safari heap timeline read the page's JavaScript memory climbing at a
 * quarter of a gigabyte a second and collecting back to a live heap that
 * held 30 MB of objects — the growth was ArrayBuffer garbage, and a
 * snapshot, taken after a collection, cannot say who made it. This wraps
 * the constructors that make one — `ArrayBuffer`, every typed array given
 * a length, `slice` — and adds the bytes to the call site's row. What it
 * cannot see is what the browser allocates under a call of its own (a
 * canvas read back for an upload, a fetch body); those appear as the
 * difference between this report and the timeline.
 *
 * Installed by the ledger overlay in dev, read with `__pxl_alloc_report()`.
 * The wrappers cost a stack capture per allocation over the threshold,
 * which is why small ones are counted without a site.
 */

type Row = { bytes: number; count: number };

const SITE_BYTES = 64 * 1024;
const FRAMES = 4;
const rows = new Map<string, Row>();
let small_bytes = 0;
let small_count = 0;
let installed = false;

function site(): string {
  const stack = (new Error().stack ?? "").split("\n").filter((l) => l.includes("/") && !l.includes("alloc_probe.ts")).slice(0, FRAMES);
  return stack.map((l) => l.trim().replace(/^at /, "").replace(/\?[tv]=[\w]+/, "")).join(" < ");
}

function record(bytes: number): void {
  if (bytes < SITE_BYTES) {
    small_bytes += bytes;
    small_count++;
    return;
  }
  const key = site();
  const row = rows.get(key);
  if (row) {
    row.bytes += bytes;
    row.count++;
  } else rows.set(key, { bytes, count: 1 });
}

type TypedCtor = { new (...args: unknown[]): ArrayBufferView; BYTES_PER_ELEMENT: number };

/** A proxy with a construct trap keeps `instanceof`, statics and the
 * prototype chain as they were; only construction is observed. */
function wrap<T extends object>(ctor: T, bytes_of: (args: unknown[]) => number): T {
  return new Proxy(ctor, {
    construct(target, args, new_target) {
      const n = bytes_of(args);
      if (n > 0) record(n);
      return Reflect.construct(target as new (...a: unknown[]) => object, args, new_target);
    },
  });
}

export function alloc_probe_install(): void {
  if (installed) return;
  installed = true;
  const g = globalThis as Record<string, unknown>;

  const OrigAB = g.ArrayBuffer as typeof ArrayBuffer;
  g.ArrayBuffer = wrap(OrigAB, (args) => (typeof args[0] === "number" ? args[0] : 0));

  for (const name of ["Uint8Array", "Uint8ClampedArray", "Int8Array", "Uint16Array", "Int16Array", "Uint32Array", "Int32Array", "Float32Array", "Float64Array", "BigInt64Array", "BigUint64Array"]) {
    const Orig = g[name] as TypedCtor;
    g[name] = wrap(Orig, (args) => {
      const first = args[0];
      // A length, or an array-like to copy: both make a new buffer. A view
      // over an existing buffer does not.
      if (typeof first === "number") return first * Orig.BYTES_PER_ELEMENT;
      // A buffer (shared or not) makes a view, not a copy; Safari has no
      // SharedArrayBuffer global without cross-origin isolation, so the
      // shape is checked rather than the constructor.
      if (first && typeof first === "object" && !(first instanceof OrigAB) && !("byteLength" in first && !("length" in first))) {
        const n = (first as { length?: number }).length;
        if (typeof n === "number") return n * Orig.BYTES_PER_ELEMENT;
      }
      return 0;
    });
  }

  const slice = OrigAB.prototype.slice;
  OrigAB.prototype.slice = function (this: ArrayBuffer, begin?: number, end?: number) {
    const out = slice.call(this, begin, end);
    record(out.byteLength);
    return out;
  };
}

export function alloc_probe_report(): string {
  const sorted = [...rows.entries()].sort((a, b) => b[1].bytes - a[1].bytes);
  const total = sorted.reduce((s, [, r]) => s + r.bytes, 0) + small_bytes;
  const lines = [`=== alloc probe: ${(total / 1048576).toFixed(0)} MB in ${sorted.length} sites + ${(small_bytes / 1048576).toFixed(1)} MB in ${small_count} small (<64KB) ===`];
  for (const [key, r] of sorted.slice(0, 25)) lines.push(`${(r.bytes / 1048576).toFixed(1).padStart(8)} MB  x${String(r.count).padStart(6)}  ${key}`);
  const text = lines.join("\n");
  console.log(text);
  return text;
}

export function alloc_probe_reset(): void {
  rows.clear();
  small_bytes = 0;
  small_count = 0;
}
