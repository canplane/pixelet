// script/vector/pxl/shared/spec/space/pvf3/projection.ts
//
// project and unproject over reals across lattice seams and signs, for
// crates/pxl/spec/src/space/pvf3/projection.test.rs.

import { projection, type PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { f64_bits, seeded } from "../../../../../../vector.ts";

export function vectors() {
  const rnd = seeded(0x9ec7);
  const reals = [0, 0.5, -0.5, -0.2, 0.999, -0.999, 1, -1, 1e-12, -1e-12, 8388607.5, -8388608.25, 32767.9, -32768];
  for (let k = 0; k < 40; k++) reals.push(((rnd() / 0x1_0000_0000) - 0.5) * 2 ** ((rnd() % 26) - 2));
  const out = [];
  for (let k = 0; k + 2 < reals.length; k += 1) {
    const [x, y, z] = [reals[k], reals[(k * 7 + 1) % reals.length], reals[(k * 13 + 2) % reals.length]];
    const p: PVF3 = projection.project({ i: 0n, f: 0n }, x, y, z);
    const u = projection.unproject([0, 0, 0], p.i, p.f);
    out.push({ x: f64_bits(x), y: f64_bits(y), z: f64_bits(z), i: p.i.toString(), f: p.f.toString(), u: u.map(f64_bits) });
  }
  return out;
}
