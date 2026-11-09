// @pxl/client/ui/feature/debug/perf/measure.ts

/**
 * What a build made for measuring runs at boot (`PXL_PERF_DRIVE`,
 * ui/layout.tsx): the standard drive once the world has started and had a
 * moment to load, with its values, the report and the trace, as one JSON text
 * told in numbered pieces that fit a line of Android's log
 * (script/perf/ listens). The values name the build, dev or release, which
 * no comparison mixes. The settings the drive changed are put back.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import * as setting from "@/service/core/setting.ts";
import { say } from "@/service/platform/say.ts";
import * as snapshot from "@/service/core/snapshot.ts";
import { MEASURE_PLACE } from "@/service/platform/sensor-listener.ts";
import { DRIVE_SETTINGS, perfDrive } from "./drive.ts";
import { perfTrace } from "./trace.ts";
import { perfReportRead } from "./view.ts";

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

async function tell(text: string): Promise<void> {
  const n = Math.ceil(text.length / PIECE);
  for (let i = 0; i < n; i++) {
    const piece = text.slice(i * PIECE, (i + 1) * PIECE);
    say(`PXLPERF ${i + 1}/${n} ${piece}`);
    if ((i + 1) % BURST === 0) {
      await new Promise((done) => setTimeout(done, BURST_GAP_MS));
    }
  }
}

const off = (v: f64 | null, at: f64) =>
  v === null || Math.abs(v - at) > PLACE_SLACK_DEG;

export async function measure(): Promise<void> {
  try {
    await new Promise((done) => setTimeout(done, SETTLE_MS));
    const before = DRIVE_SETTINGS.map(
      ([key]) => [key, snapshot.cur().settings[key]] as const,
    );
    const report = await perfDrive();
    const r = await perfReportRead();
    const trace = await perfTrace();
    for (const [key, value] of before) setting.set(key, value);
    const [lat, lon] = [r.view.pivot_lat, r.view.pivot_lon];
    if (off(lat, MEASURE_PLACE.lat) || off(lon, MEASURE_PLACE.lon)) {
      throw new Error(`stands at ${lat}, ${lon}`);
    }
    const f = r.frames;
    await tell(
      JSON.stringify({
        build: __IS_DEV__ ? "dev" : "release",
        longest: Math.max(0, ...r.longs.map((l) => l.durationMs)),
        gpu: f
          ? [f.gpu_busy.p50, f.gpu_busy.p95, f.n, f.gpu_timed]
          : [null, null, 0, 0],
        tick: f ? [f.tick.p50, f.tick.p95] : [null, null],
        report,
        trace,
      }),
    );
  } catch (e) {
    await tell(JSON.stringify({ error: String(e) }));
  }
}
