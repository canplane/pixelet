// @xpute/example/web/main.ts

/**
 * The page: the core started in its worker, its turns let run, and SUM asked
 * across the port, the core calling ASK as it adds.
 */

import { QUIET, start } from "@xpute/runtime";
import { Command, encode_sum } from "../asker.spec.ts";
import { ANSWER, manifest } from "./core.ts";

const out = document.querySelector("#out")!;
const say = (line: string) => (out.textContent += `${line}\n`);

const link = await start(
  manifest,
  { ...QUIET, stop: () => say("stopped") },
  () =>
    new Worker(new URL("./core.worker.ts", import.meta.url), {
      type: "module",
    }),
);
link.resume();
const sum = await link.request(Command.SUM, (room) => encode_sum(room, 2, 3));
say(`2 + 3 + ${ANSWER} = ${sum}`);
