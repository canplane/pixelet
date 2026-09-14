// script/vector/xpute/core/kit/json.ts
//
// What JSON.stringify writes for flat string maps and what JSON.parse
// reads back — key order, escapes, undefined values — for
// crates/xpute/core/src/kit/json.test.rs. Text is hex UTF-8: a vector line
// holds no tab or newline.

import { bytes_to_hex } from "@xpute/core/kit/encoding.ts";

const hex = (s: string) => bytes_to_hex(new TextEncoder().encode(s));

export function vectors() {
  const maps: [string, string | undefined][][] = [
    [],
    [["name", "가게"], ["amenity", "cafe"]],
    [["b", "1"], ["10", "x"], ["a", "2"], ["2", "y"], ["01", "z"], ["4294967295", "w"], ["4294967294", "v"]],
    [["gone", undefined], ["kept", ""]],
    [["q", 'say "hi"\\'], ["c", "\b\f\n\r\t  /"]],
    [["emoji", "\u{1F600}"], ["dup", "first"], ["x", "1"], ["dup", "last"]],
  ];
  const texts = [
    "{}",
    ' { "a" : "1" , "b":"2" } ',
    '{"z":"1","3":"x","a":"2","1":"y"}',
    '{"esc":"\\"\\\\\\/\\b\\f\\n\\r\\t\\u00e9\\ud83d\\ude00"}',
    '{"k":"first","k":"last"}',
    '{"n":1}',
    '{"a":"1",}',
    '{"a":"1"} x',
    "[]",
  ];
  return {
    stringify: maps.map((m) => ({
      keys: m.map(([k]) => hex(k)),
      values: m.map(([, v]) => (v === undefined ? "undefined" : hex(v))),
      json: hex(JSON.stringify(Object.fromEntries(m))),
    })),
    parse: texts.map((t) => {
      try {
        const o = JSON.parse(t);
        if (typeof o !== "object" || o === null || Array.isArray(o) || Object.values(o).some((v) => typeof v !== "string")) return { text: hex(t), read: "refused" };
        return { text: hex(t), read: Object.entries(o).map(([k, v]) => `${hex(k)}=${hex(v as string)}`).join(",") || "empty" };
      } catch {
        return { text: hex(t), read: "refused" };
      }
    }),
  };
}
