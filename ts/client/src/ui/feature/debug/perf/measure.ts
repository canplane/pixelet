// @pxl/client/ui/feature/debug/perf/measure.ts

/**
 * What a build made for measuring runs as its core starts (`PXL_PERF_DRIVE`,
 * ui/layout.tsx): its progress, a line a second from then on, and the
 * standard drive once the world has started and had a moment to load, with
 * its values, the report and the trace, as one JSON text told in numbered
 * pieces that fit a line of Android's log (script/perf/ listens). The
 * values name the build, dev or release, which no comparison mixes. The
 * settings the drive changed are put back.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import { read } from "@/service/core/perf.ts";
import {
  bootState,
  watchBoot,
  BOOT_STARTED,
  BOOT_TERMINATED,
} from "@/service/sys/boot.ts";
import * as setting from "@/service/core/setting.ts";
import { tellDrive } from "@/service/platform/drive.ts";
import * as visibility from "@/service/platform/visibility.ts";
import * as overlay from "@/service/core/overlay.ts";
import { MEASURE_PLACE } from "@/service/platform/sensor-listener.ts";
import { DRIVE_SETTINGS, perfDrive } from "./drive.ts";
import { perfReportText } from "./report.ts";
import { perfTrace } from "./trace.ts";

/** What a load is given to settle before the drive. */
const SETTLE_MS = 35_000;
/** How far the pivot may lie from the place, in degrees: about ten meters. */
const PLACE_SLACK_DEG = 1e-4;
/** The text is told this many characters a line, within a line of Android's
 * log, even as multi-byte text. */
const PIECE = 1000;
/** A trace is thousands of lines: this many are told at a time, a pause
 * apart, so that a log's buffer drains between them. */
const BURST = 32;
const BURST_GAP_MS = 16;
/** How far apart the run's progress is logged. */
const PROGRESS_MS = 1000;

async function tell(text: string): Promise<void> {
  const n = Math.ceil(text.length / PIECE);
  for (let i = 0; i < n; i++) {
    const piece = text.slice(i * PIECE, (i + 1) * PIECE);
    tellDrive(`PXLPERF ${i + 1}/${n} ${piece}`);
    if ((i + 1) % BURST === 0) {
      await new Promise((done) => setTimeout(done, BURST_GAP_MS));
    }
  }
}

const strays = (v: f64 | null, at: f64) =>
  v === null || Math.abs(v - at) > PLACE_SLACK_DEG;

/** Says the run's progress, a line a second until the answer is called,
 * whether or not the core answers: when it was logged, whether the page is
 * shown, the boot's state (service/sys/boot.ts), how long the read of `NOW`'s
 * draw that is out has gone unanswered (`unanswered_ms`, 0 with none out), and
 * that draw as the last answer gave it, each as its name and its value, or the
 * read's error. One read is out at a time. A second with no line is one the
 * UI's own thread did not run in. */
function progress(): () => void {
  let draw = "";
  let asked = -1;
  const ask = async () => {
    asked = performance.now();
    try {
      const d = await read("draw");
      draw = Object.entries(d)
        .map(([k, v]) => `${k} ${v}`)
        .join(" ");
    } catch (e) {
      draw = `err ${String(e)}`;
    }
    asked = -1;
  };
  const tick = setInterval(() => {
    const t = performance.now();
    const unanswered = asked < 0 ? 0 : t - asked;
    tellDrive(
      `PXLRUN t ${t} shown ${Number(visibility.shown())} boot ${bootState()} unanswered_ms ${unanswered} ${draw}`,
    );
    if (asked < 0) void ask();
  }, PROGRESS_MS);
  return () => clearInterval(tick);
}

/** The boot's state turns `BOOT_STARTED`: the world has been made. */
function started(): Promise<void> {
  return new Promise((done) => {
    if (bootState() === BOOT_STARTED) return done();
    const stop = watchBoot(() => {
      if (bootState() !== BOOT_STARTED) return;
      stop();
      done();
    });
  });
}

/** A run measures a page or an app shown from the core's start
 * (`BOOT_STARTING`), to its result, on whatever runs it. It ends
 * at once with the error where the page is hidden, which draws nothing while
 * its core waits for it to be shown again, or where the core stops, whose
 * reason the host says. */
export async function measure(): Promise<void> {
  const stopProgress = progress();
  const unwatch: (() => void)[] = [];
  const ended = new Promise<never>((_, fail) => {
    const hidden = () =>
      visibility.shown() || fail(new Error("the page is hidden"));
    const stopped = () => {
      const b = bootState();
      if (b === BOOT_TERMINATED) {
        fail(new Error(`the core stopped (boot ${b})`));
      }
    };
    hidden();
    stopped();
    unwatch.push(visibility.watch(hidden), watchBoot(stopped));
  });
  try {
    await tell(await Promise.race([run(), ended]));
  } catch (e) {
    await tell(JSON.stringify({ error: String(e) }));
  } finally {
    unwatch.forEach((w) => w());
    stopProgress();
  }
}

/** The drive's values, its report and its trace, as the text told, the
 * values and the report from the drive's one read. The drive waits for the
 * world to be made and the load to settle. */
async function run(): Promise<string> {
  await started();
  await new Promise((done) => setTimeout(done, SETTLE_MS));
  const before = DRIVE_SETTINGS.map(
    ([key]) => [key, overlay.currentSnapshot().settings[key]] as const,
  );
  const perfReport = await perfDrive();
  const trace = await perfTrace();
  for (const [key, value] of before) setting.set(key, value);
  const [lat, lon] = [perfReport.view.pivot_lat, perfReport.view.pivot_lon];
  if (strays(lat, MEASURE_PLACE.lat) || strays(lon, MEASURE_PLACE.lon)) {
    throw new Error(`stands at ${lat}, ${lon}`);
  }
  const frames = perfReport.frames;
  return JSON.stringify({
    build: __DEV__ ? "dev" : "release",
    longest: Math.max(0, ...perfReport.longs.map((l) => l.durationMs)),
    gpu: frames
      ? [frames.gpuBusy.p50, frames.gpuBusy.p95, frames.n, frames.gpuTimed]
      : [null, null, 0, 0],
    tick: frames ? [frames.tick.p50, frames.tick.p95] : [null, null],
    report: perfReportText(perfReport),
    trace,
  });
}
