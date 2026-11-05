// Scratch only, not committed: what the page's harness does, run on the app and logged.
import { Dimensions } from "react-native";
import { perfReportText } from "@pxl/client/shell/debug/perf/report.ts";
import { perfReportRead } from "@pxl/client/shell/debug/perf/view.ts";
import { SETTING } from "@pxl/client/shell/setting/schema.ts";
import * as perf from "@pxl/client/port/perf.ts";
import * as setting from "@pxl/client/port/setting.ts";
import * as input from "@pxl/client/port/input.ts";
import { PointerPhase, PointerType } from "@pxl/client/abi/cmd.spec.ts";

const wait = (ms: number) => new Promise<void>((done) => setTimeout(done, ms));
const say = (name: string, text: string) => {
  for (const line of text.split("\n")) console.log(`PROBE ${name}|${line}`);
};
const report = () => perfReportText(perfReportRead());
const { width, height } = Dimensions.get("window");
const [x, y] = [width / 2, height / 2];
const p = (phase: PointerPhase, at: [number, number]) =>
  input.pointer(phase, PointerType.Mouse, 1, 2, at[0], at[1]);
const circle = async (secs: number, turn: number, radius: number, sweep: boolean) => {
  const on = (a: number): [number, number] => [x + radius * Math.cos(a), y + radius * Math.sin(a)];
  const start = -Math.PI / 2;
  p(PointerPhase.Down, on(start));
  const t0 = performance.now();
  let n = 0;
  const step = setInterval(() => {
    p(PointerPhase.Move, on(start + ((performance.now() - t0) / 1000 / turn) * 2 * Math.PI));
    if (sweep) input.wheel(Math.floor(n / 60) % 2 ? 100 : -100);
    n++;
  }, 16);
  await wait(secs * 1000);
  clearInterval(step);
  p(PointerPhase.Up, on(start));
};

export async function probe(): Promise<void> {
  const t0 = Date.now();
  let world = 0;
  for (let i = 0; i < 120; i++) {
    await wait(1000);
    try {
      const n = (perfReportRead() as { frames?: { n?: number } }).frames?.n ?? 0;
      if (n === 0) {
        setting.set(SETTING.perf.key, 1);
        await wait(300);
        perf.start();
        continue;
      }
      const r = report();
      const pivot = (r.match(/view +pivot ([^ ]+)/) ?? [])[1];
      if ((perfReportRead() as { frames?: { gpu_read?: number } }).frames?.gpu_read && pivot && pivot !== "-") {
        world = i;
        break;
      }
    } catch (e) {
      console.log("PROBE wait|" + String(e));
    }
  }
  say("boot", `window ${width}x${height} world loop ${world}`);
  say("world", `shown ${(Date.now() - t0) / 1000} s`);
  say("first", report());
  setting.set(SETTING.ray_counts.key, 0);
  setting.set(SETTING.shadows.key, 0);
  setting.set(SETTING.res.key, SETTING.res.options.fixed);
  await wait(5000);
  perf.start();
  await circle(25, 6, 100, false);
  say("off", report());
  await wait(3000);
  say("shot", "off");
  await wait(3000);
  setting.set(SETTING.shadows.key, 1);
  setting.set(SETTING.sky_time.key, SETTING.sky_time.options.noon);
  await wait(6000);
  perf.start();
  await circle(25, 6, 100, false);
  say("on", report());
  setting.set(SETTING.ray_counts.key, 1);
  await wait(1000);
  perf.start();
  await circle(8, 6, 100, false);
  say("counts", report());
  setting.set(SETTING.ray_counts.key, 0);
  setting.set(SETTING.sky_time.key, SETTING.sky_time.options.dawn);
  for (let n = 0; n < 60; n++) {
    input.wheel(100);
    await wait(16);
  }
  p(PointerPhase.Down, [x, y]);
  for (let n = 1; n <= 60; n++) {
    p(PointerPhase.Move, [x, y - (2000 * n) / 60]);
    await wait(16);
  }
  p(PointerPhase.Up, [x, y - 2000]);
  await wait(2000);
  perf.start();
  await circle(25, 4, 250, true);
  say("drive", report());
  say("done", "done");
}
