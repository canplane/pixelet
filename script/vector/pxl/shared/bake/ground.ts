// script/vector/pxl/shared/bake/ground.ts
//
// shown_ground over every mix of painted and unpainted channels, with and
// without structures, for crates/pxl/spec/src/bake/ground.test.rs.

import { GROUND_CH_KEYS, type GroundChannelKey, LAND_BASE_TYPE, shown_ground } from "@pxl/shared/bake/ground.ts";

export function vectors() {
  const out = [];
  for (let mask = 0; mask < 16; mask++) {
    for (const show of [false, true]) {
      const read = (ch: GroundChannelKey) => {
        const i = GROUND_CH_KEYS.indexOf(ch);
        return (mask >> i) & 1 ? 10 + i : 0;
      };
      const s = shown_ground(read, show);
      out.push({ mask, show, channel: s.channel, type_id: s.type_id });
    }
  }
  return { LAND_BASE_TYPE, shown: out };
}
